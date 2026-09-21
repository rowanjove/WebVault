pub mod models;
pub mod schema;

use anyhow::{Context, Result};
use models::*;
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
    warc_lock: Arc<Mutex<()>>,
    pub base_dir: PathBuf,
}

impl Database {
    pub fn init<P: AsRef<Path>>(base_dir: P) -> Result<Self> {
        let base_dir = base_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&base_dir)
            .with_context(|| format!("Failed to create base dir: {:?}", base_dir))?;

        let db_path = base_dir.join("app.db");
        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open SQLite database at {:?}", db_path))?;

        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(schema::CREATE_TABLES_SQL)
            .context("Failed to execute initial schema migrations")?;

        let _ = conn.execute("ALTER TABLE resources ADD COLUMN warc_length INTEGER DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE crawl_queue ADD COLUMN error_message TEXT", []);
        run_migrations(&conn)?;
        fail_interrupted_jobs(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            warc_lock: Arc::new(Mutex::new(())),
            base_dir,
        })
    }

    pub fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| anyhow::anyhow!("Database lock poisoned: {}", e))
    }

    pub fn lock_warc(&self) -> Result<std::sync::MutexGuard<'_, ()>> {
        self.warc_lock
            .lock()
            .map_err(|e| anyhow::anyhow!("WARC lock poisoned: {}", e))
    }

    pub fn reopen(&self) -> Result<()> {
        let mut conn = self.lock_conn()?;
        *conn = Connection::open(self.base_dir.join("app.db"))
            .context("Failed to reopen SQLite database")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        run_migrations(&conn)?;
        Ok(())
    }

    pub fn close_for_replace(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        drop(conn);
        let mut conn = self.lock_conn()?;
        *conn = Connection::open_in_memory()?;
        Ok(())
    }

    // Sites
    pub fn list_sites(&self) -> Result<Vec<Site>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT 
                s.id, s.name, s.root_url, s.normalized_host, s.created_at, s.updated_at, s.enabled,
                (SELECT COUNT(*) FROM pages p WHERE p.site_id = s.id) as page_count,
                (SELECT COUNT(*) FROM captures c JOIN pages p ON c.page_id = p.id WHERE p.site_id = s.id) as capture_count,
                (SELECT COALESCE(SUM(c.warc_length), 0) FROM captures c JOIN pages p ON c.page_id = p.id WHERE p.site_id = s.id) as storage_bytes,
                (SELECT MAX(c.captured_at) FROM captures c JOIN pages p ON c.page_id = p.id WHERE p.site_id = s.id) as last_capture_at
            FROM sites s
            ORDER BY s.updated_at DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            let last_capture_at: Option<i64> = row.get(10)?;
            Ok(Site {
                id: row.get(0)?,
                name: row.get(1)?,
                root_url: row.get(2)?,
                normalized_host: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                enabled: row.get::<_, i32>(6)? == 1,
                page_count: Some(row.get(7)?),
                capture_count: Some(row.get(8)?),
                storage_bytes: Some(row.get(9)?),
                last_capture_at,
            })
        })?;

        let mut sites = Vec::new();
        for r in rows {
            sites.push(r?);
        }
        Ok(sites)
    }

    pub fn create_site(&self, name: &str, root_url: &str) -> Result<Site> {
        let conn = self.lock_conn()?;
        let id = format!("site_{}", uuid::Uuid::new_v4().simple());
        let parsed_url = url::Url::parse(root_url).context("Invalid root URL")?;
        let normalized_host = parsed_url.host_str().unwrap_or("unknown").to_lowercase();
        let now = chrono::Utc::now().timestamp_millis();

        conn.execute(
            r#"
            INSERT INTO sites (id, name, root_url, normalized_host, created_at, updated_at, enabled)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)
            "#,
            params![id, name, root_url, normalized_host, now, now],
        )?;

        // Also insert default profile
        let profile_id = format!("prof_{}", uuid::Uuid::new_v4().simple());
        conn.execute(
            r#"
            INSERT INTO crawl_profiles (id, site_id, scope_type, max_depth, max_pages, max_size_mb, created_at, updated_at)
            VALUES (?1, ?2, 'host', 2, 500, 2048, ?3, ?4)
            "#,
            params![profile_id, id, now, now],
        )?;

        Ok(Site {
            id,
            name: name.to_string(),
            root_url: root_url.to_string(),
            normalized_host,
            created_at: now,
            updated_at: now,
            enabled: true,
            page_count: Some(0),
            capture_count: Some(0),
            storage_bytes: Some(0),
            last_capture_at: None,
        })
    }

    pub fn delete_site(&self, id: &str) -> Result<()> {
        if id.is_empty() || id.contains("..") || id.contains('/') || id.contains('\\') {
            anyhow::bail!("Invalid site id");
        }

        let screenshot_rels: Vec<String> = {
            let conn = self.lock_conn()?;
            let mut stmt = conn.prepare(
                r#"
                SELECT c.screenshot_path
                FROM captures c
                JOIN pages p ON c.page_id = p.id
                WHERE p.site_id = ?1 AND c.screenshot_path IS NOT NULL
                "#,
            )?;
            let rows = stmt.query_map(params![id], |row| row.get::<_, Option<String>>(0))?;
            rows.filter_map(|r| r.ok().flatten()).collect()
        };

        {
            let conn = self.lock_conn()?;
            let _ = conn.execute(
                "DELETE FROM fts_pages WHERE page_id IN (SELECT id FROM pages WHERE site_id = ?1)",
                params![id],
            );
            conn.execute("DELETE FROM sites WHERE id = ?1", params![id])?;
        }

        let _ = std::fs::remove_dir_all(self.base_dir.join("archives").join(id));
        let _ = std::fs::remove_dir_all(self.base_dir.join("browser_profiles").join(id));

        for rel in screenshot_rels {
            let path = self.base_dir.join(&rel);
            if path.starts_with(&self.base_dir) {
                let _ = std::fs::remove_file(path);
            }
        }
        if let Ok(entries) = std::fs::read_dir(self.base_dir.join("screenshots")) {
            let prefix = format!("ss_{}_", id);
            for entry in entries.flatten() {
                if entry.file_name().to_string_lossy().starts_with(&prefix) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        Ok(())
    }

    pub fn get_site_profile(&self, site_id: &str) -> Result<CrawlProfile> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, site_id, scope_type, include_rules, exclude_rules, max_depth, max_pages,
                   max_size_mb, max_duration_minutes, concurrency, autoscroll, autoplay, autofetch,
                   popup_policy, iframe_policy, ad_policy, media_policy
            FROM crawl_profiles WHERE site_id = ?1 LIMIT 1
            "#,
        )?;

        let profile = stmt.query_row(params![site_id], |row| {
            let inc_json: String = row.get(3).unwrap_or_else(|_| "[]".to_string());
            let exc_json: String = row.get(4).unwrap_or_else(|_| "[]".to_string());
            let include_rules = serde_json::from_str(&inc_json).unwrap_or_default();
            let exclude_rules = serde_json::from_str(&exc_json).unwrap_or_default();

            Ok(CrawlProfile {
                id: row.get(0)?,
                site_id: row.get(1)?,
                scope_type: row.get(2)?,
                include_rules,
                exclude_rules,
                max_depth: row.get(5)?,
                max_pages: row.get(6)?,
                max_size_mb: row.get(7)?,
                max_duration_minutes: row.get(8)?,
                concurrency: row.get(9)?,
                autoscroll: row.get::<_, i32>(10)? == 1,
                autoplay: row.get::<_, i32>(11)? == 1,
                autofetch: row.get::<_, i32>(12)? == 1,
                popup_policy: row.get(13)?,
                iframe_policy: row.get(14)?,
                ad_policy: row.get(15)?,
                media_policy: row.get(16)?,
            })
        });

        match profile {
            Ok(p) => Ok(p),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                let mut def = CrawlProfile::default();
                def.site_id = site_id.to_string();
                Ok(def)
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn update_site_profile(&self, profile: &CrawlProfile) -> Result<()> {
        let conn = self.lock_conn()?;
        let now = chrono::Utc::now().timestamp_millis();
        let inc_json = serde_json::to_string(&profile.include_rules)?;
        let exc_json = serde_json::to_string(&profile.exclude_rules)?;

        conn.execute(
            r#"
            INSERT INTO crawl_profiles (
                id, site_id, scope_type, include_rules, exclude_rules, max_depth, max_pages,
                max_size_mb, max_duration_minutes, concurrency, autoscroll, autoplay, autofetch,
                popup_policy, iframe_policy, ad_policy, media_policy, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
            ON CONFLICT(id) DO UPDATE SET
                scope_type = excluded.scope_type,
                include_rules = excluded.include_rules,
                exclude_rules = excluded.exclude_rules,
                max_depth = excluded.max_depth,
                max_pages = excluded.max_pages,
                max_size_mb = excluded.max_size_mb,
                max_duration_minutes = excluded.max_duration_minutes,
                concurrency = excluded.concurrency,
                autoscroll = excluded.autoscroll,
                autoplay = excluded.autoplay,
                autofetch = excluded.autofetch,
                popup_policy = excluded.popup_policy,
                iframe_policy = excluded.iframe_policy,
                ad_policy = excluded.ad_policy,
                media_policy = excluded.media_policy,
                updated_at = excluded.updated_at
            "#,
            params![
                profile.id, profile.site_id, profile.scope_type, inc_json, exc_json,
                profile.max_depth, profile.max_pages, profile.max_size_mb, profile.max_duration_minutes,
                profile.concurrency, profile.autoscroll as i32, profile.autoplay as i32, profile.autofetch as i32,
                profile.popup_policy, profile.iframe_policy, profile.ad_policy, profile.media_policy,
                now, now
            ],
        )?;
        Ok(())
    }

    // Pages & Captures
    pub fn list_pages(&self, site_id: &str) -> Result<Vec<PageItem>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, site_id, url, normalized_url, title, first_seen, last_seen, capture_count
            FROM pages WHERE site_id = ?1 ORDER BY last_seen DESC
            "#,
        )?;

        let rows = stmt.query_map(params![site_id], |row| {
            Ok(PageItem {
                id: row.get(0)?,
                site_id: row.get(1)?,
                url: row.get(2)?,
                normalized_url: row.get(3)?,
                title: row.get(4)?,
                first_seen: row.get(5)?,
                last_seen: row.get(6)?,
                capture_count: row.get(7)?,
            })
        })?;

        let mut pages = Vec::new();
        for r in rows {
            pages.push(r?);
        }
        Ok(pages)
    }

    pub fn list_captures(&self, page_id: &str) -> Result<Vec<CaptureItem>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT c.id, c.page_id, c.job_id, c.captured_at, c.status_code, c.mime_type,
                   c.warc_file, c.warc_offset, c.warc_length, c.screenshot_path,
                   c.text_hash, c.dom_hash, c.visual_hash, c.resource_count,
                   c.missing_resource_count, c.capture_score, p.url, p.title
            FROM captures c
            JOIN pages p ON c.page_id = p.id
            WHERE c.page_id = ?1
            ORDER BY c.captured_at DESC
            "#,
        )?;

        let rows = stmt.query_map(params![page_id], |row| {
            Ok(CaptureItem {
                id: row.get(0)?,
                page_id: row.get(1)?,
                job_id: row.get(2)?,
                captured_at: row.get(3)?,
                status_code: row.get(4)?,
                mime_type: row.get(5)?,
                warc_file: row.get(6)?,
                warc_offset: row.get(7)?,
                warc_length: row.get(8)?,
                screenshot_path: row.get(9)?,
                text_hash: row.get(10)?,
                dom_hash: row.get(11)?,
                visual_hash: row.get(12)?,
                resource_count: row.get(13)?,
                missing_resource_count: row.get(14)?,
                capture_score: row.get(15)?,
                url: row.get(16)?,
                title: row.get(17)?,
            })
        })?;

        let mut captures = Vec::new();
        for r in rows {
            captures.push(r?);
        }
        Ok(captures)
    }

    pub fn get_capture_details(&self, capture_id: &str) -> Result<(CaptureItem, Vec<ResourceItem>, String)> {
        let conn = self.lock_conn()?;
        let mut cap_stmt = conn.prepare(
            r#"
            SELECT c.id, c.page_id, c.job_id, c.captured_at, c.status_code, c.mime_type,
                   c.warc_file, c.warc_offset, c.warc_length, c.screenshot_path,
                   c.text_hash, c.dom_hash, c.visual_hash, c.resource_count,
                   c.missing_resource_count, c.capture_score, p.url, p.title
            FROM captures c
            JOIN pages p ON c.page_id = p.id
            WHERE c.id = ?1
            "#,
        )?;

        let capture = cap_stmt.query_row(params![capture_id], |row| {
            Ok(CaptureItem {
                id: row.get(0)?,
                page_id: row.get(1)?,
                job_id: row.get(2)?,
                captured_at: row.get(3)?,
                status_code: row.get(4)?,
                mime_type: row.get(5)?,
                warc_file: row.get(6)?,
                warc_offset: row.get(7)?,
                warc_length: row.get(8)?,
                screenshot_path: row.get(9)?,
                text_hash: row.get(10)?,
                dom_hash: row.get(11)?,
                visual_hash: row.get(12)?,
                resource_count: row.get(13)?,
                missing_resource_count: row.get(14)?,
                capture_score: row.get(15)?,
                url: row.get(16)?,
                title: row.get(17)?,
            })
        })?;

        let mut res_stmt = conn.prepare(
            r#"
            SELECT id, capture_id, url, normalized_url, mime_type, status_code, size, sha256, resource_type, warc_file, warc_offset, warc_length
            FROM resources WHERE capture_id = ?1 ORDER BY size DESC
            "#,
        )?;

        let res_rows = res_stmt.query_map(params![capture_id], |row| {
            Ok(ResourceItem {
                id: row.get(0)?,
                capture_id: row.get(1)?,
                url: row.get(2)?,
                normalized_url: row.get(3)?,
                mime_type: row.get(4)?,
                status_code: row.get(5)?,
                size: row.get(6)?,
                sha256: row.get(7)?,
                resource_type: row.get(8)?,
                warc_file: row.get(9)?,
                warc_offset: row.get(10)?,
                warc_length: row.get(11)?,
            })
        })?;

        let mut resources = Vec::new();
        for r in res_rows {
            resources.push(r?);
        }

        // Get clean text from FTS table if available
        let mut fts_stmt = conn.prepare("SELECT clean_text FROM fts_pages WHERE capture_id = ?1 LIMIT 1")?;
        let rendered_text: String = fts_stmt
            .query_row(params![capture_id], |row| row.get(0))
            .unwrap_or_default();

        Ok((capture, resources, rendered_text))
    }

    pub fn get_system_stats(&self, browser_detected: bool, browser_path: &str) -> Result<SystemStats> {
        let conn = self.lock_conn()?;
        let site_count: i64 = conn.query_row("SELECT COUNT(*) FROM sites", [], |r| r.get(0)).unwrap_or(0);
        let page_count: i64 = conn.query_row("SELECT COUNT(*) FROM pages", [], |r| r.get(0)).unwrap_or(0);
        let capture_count: i64 = conn.query_row("SELECT COUNT(*) FROM captures", [], |r| r.get(0)).unwrap_or(0);
        let resource_count: i64 = conn.query_row("SELECT COUNT(*) FROM resources", [], |r| r.get(0)).unwrap_or(0);
        let storage_bytes: i64 = conn.query_row("SELECT COALESCE(SUM(warc_length), 0) FROM captures", [], |r| r.get(0)).unwrap_or(0);
        let active_jobs: i64 = conn.query_row("SELECT COUNT(*) FROM crawl_jobs WHERE status IN ('running', 'queued')", [], |r| r.get(0)).unwrap_or(0);

        Ok(SystemStats {
            site_count,
            page_count,
            capture_count,
            resource_count,
            storage_bytes,
            active_jobs,
            storage_path: self.base_dir.to_string_lossy().to_string(),
            browser_path: browser_path.to_string(),
            browser_detected,
        })
    }

    pub fn save_site_credential(
        &self,
        site_id: &str,
        name: &str,
        cookies_json: &str,
        storage_json: &str,
    ) -> Result<SiteCredential> {
        let conn = self.lock_conn()?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();

        let sealed_cookies = crate::protect::seal_text(cookies_json)?;
        let sealed_storage = crate::protect::seal_text(storage_json)?;
        conn.execute(
            r#"
            INSERT INTO site_credentials (id, site_id, name, cookies_json, storage_json, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(site_id, name) DO UPDATE SET
                cookies_json = ?4,
                storage_json = ?5,
                updated_at = ?6
            "#,
            params![id, site_id, name, sealed_cookies, sealed_storage, now],
        )?;

        let cred = conn.query_row(
            "SELECT id, site_id, name, cookies_json, storage_json, updated_at FROM site_credentials WHERE site_id = ?1 AND name = ?2",
            params![site_id, name],
            |row| {
                Ok(SiteCredential {
                    id: row.get(0)?,
                    site_id: row.get(1)?,
                    name: row.get(2)?,
                    cookies_json: row.get(3)?,
                    storage_json: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            },
        )?;
        let cred = decode_credential(cred)?;

        Ok(cred)
    }

    pub fn get_site_credentials(&self, site_id: &str) -> Result<Vec<SiteCredential>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, site_id, name, cookies_json, storage_json, updated_at FROM site_credentials WHERE site_id = ?1 ORDER BY updated_at DESC"
        )?;
        let rows = stmt.query_map(params![site_id], |row| {
            Ok(SiteCredential {
                id: row.get(0)?,
                site_id: row.get(1)?,
                name: row.get(2)?,
                cookies_json: row.get(3)?,
                storage_json: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(decode_credential(r?)?);
        }
        Ok(list)
    }

    pub fn delete_site_credential(&self, credential_id: &str) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM site_credentials WHERE id = ?1", params![credential_id])?;
        Ok(())
    }

    pub fn import_batch_urls(&self, site_id: &str, urls: &[String]) -> Result<usize> {
        let conn = self.lock_conn()?;
        let now = chrono::Utc::now().timestamp_millis();
        let mut count = 0;

        for raw_url in urls {
            let clean = raw_url.trim();
            if clean.is_empty() || (!clean.starts_with("http://") && !clean.starts_with("https://")) {
                continue;
            }
            let norm = clean.trim_end_matches('/').to_string();
            let page_id = uuid::Uuid::new_v4().to_string();

            let res = conn.execute(
                r#"
                INSERT INTO pages (id, site_id, url, normalized_url, title, first_seen, last_seen, capture_count)
                VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?5, 0)
                ON CONFLICT(site_id, normalized_url) DO NOTHING
                "#,
                params![page_id, site_id, clean, norm, now],
            );

            if let Ok(inserted) = res {
                if inserted > 0 {
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    pub fn create_tag(&self, name: &str, color: &str) -> Result<TagItem> {
        let conn = self.lock_conn()?;
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO tags (id, name, color) VALUES (?1, ?2, ?3) ON CONFLICT(name) DO UPDATE SET color = ?3",
            params![id, name, color],
        )?;
        let tag = conn.query_row(
            "SELECT id, name, color FROM tags WHERE name = ?1",
            params![name],
            |r| Ok(TagItem { id: r.get(0)?, name: r.get(1)?, color: r.get(2)? }),
        )?;
        Ok(tag)
    }

    pub fn list_tags(&self) -> Result<Vec<TagItem>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("SELECT id, name, color FROM tags ORDER BY name ASC")?;
        let rows = stmt.query_map([], |r| {
            Ok(TagItem { id: r.get(0)?, name: r.get(1)?, color: r.get(2)? })
        })?;
        let mut res = Vec::new();
        for item in rows { res.push(item?); }
        Ok(res)
    }

    pub fn delete_tag(&self, tag_id: &str) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM tags WHERE id = ?1", params![tag_id])?;
        Ok(())
    }

    pub fn add_page_tag(&self, page_id: &str, tag_id: &str) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT OR IGNORE INTO page_tags (page_id, tag_id) VALUES (?1, ?2)",
            params![page_id, tag_id],
        )?;
        Ok(())
    }

    pub fn remove_page_tag(&self, page_id: &str, tag_id: &str) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "DELETE FROM page_tags WHERE page_id = ?1 AND tag_id = ?2",
            params![page_id, tag_id],
        )?;
        Ok(())
    }

    pub fn get_page_tags(&self, page_id: &str) -> Result<Vec<TagItem>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT t.id, t.name, t.color FROM tags t JOIN page_tags pt ON t.id = pt.tag_id WHERE pt.page_id = ?1 ORDER BY t.name ASC"
        )?;
        let rows = stmt.query_map(params![page_id], |r| {
            Ok(TagItem { id: r.get(0)?, name: r.get(1)?, color: r.get(2)? })
        })?;
        let mut res = Vec::new();
        for item in rows { res.push(item?); }
        Ok(res)
    }

    pub fn create_bookmark(&self, capture_id: &str, note: &str) -> Result<BookmarkItem> {
        let conn = self.lock_conn()?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO bookmarks (id, capture_id, note, created_at) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(capture_id) DO UPDATE SET note = ?3",
            params![id, capture_id, note, now],
        )?;
        let bm = conn.query_row(
            r#"
            SELECT b.id, b.capture_id, b.note, b.created_at, p.url, p.title
            FROM bookmarks b
            JOIN captures c ON b.capture_id = c.id
            JOIN pages p ON c.page_id = p.id
            WHERE b.capture_id = ?1
            "#,
            params![capture_id],
            |r| Ok(BookmarkItem {
                id: r.get(0)?,
                capture_id: r.get(1)?,
                note: r.get(2)?,
                created_at: r.get(3)?,
                url: r.get(4)?,
                title: r.get(5)?,
            }),
        )?;
        Ok(bm)
    }

    pub fn list_bookmarks(&self) -> Result<Vec<BookmarkItem>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT b.id, b.capture_id, b.note, b.created_at, p.url, p.title
            FROM bookmarks b
            JOIN captures c ON b.capture_id = c.id
            JOIN pages p ON c.page_id = p.id
            ORDER BY b.created_at DESC
            "#
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(BookmarkItem {
                id: r.get(0)?,
                capture_id: r.get(1)?,
                note: r.get(2)?,
                created_at: r.get(3)?,
                url: r.get(4)?,
                title: r.get(5)?,
            })
        })?;
        let mut res = Vec::new();
        for item in rows { res.push(item?); }
        Ok(res)
    }

    pub fn delete_bookmark(&self, bookmark_id: &str) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM bookmarks WHERE id = ?1 OR capture_id = ?1", params![bookmark_id])?;
        Ok(())
    }

    pub fn save_user_script(
        &self,
        site_id: Option<&str>,
        name: &str,
        script_content: &str,
        enabled: bool,
    ) -> Result<UserScriptItem> {
        let conn = self.lock_conn()?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO user_scripts (id, site_id, name, script_content, enabled, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, site_id, name, script_content, if enabled { 1 } else { 0 }, now],
        )?;
        Ok(UserScriptItem {
            id,
            site_id: site_id.map(|s| s.to_string()),
            name: name.to_string(),
            script_content: script_content.to_string(),
            enabled,
            created_at: now,
        })
    }

    pub fn list_user_scripts(&self, site_id: Option<&str>) -> Result<Vec<UserScriptItem>> {
        let conn = self.lock_conn()?;
        let mapper = |r: &rusqlite::Row| {
            Ok(UserScriptItem {
                id: r.get(0)?,
                site_id: r.get(1)?,
                name: r.get(2)?,
                script_content: r.get(3)?,
                enabled: r.get::<_, i32>(4)? != 0,
                created_at: r.get(5)?,
            })
        };
        let mut res = Vec::new();
        if let Some(sid) = site_id {
            let mut stmt = conn.prepare(
                "SELECT id, site_id, name, script_content, enabled, created_at FROM user_scripts WHERE site_id = ?1 OR site_id IS NULL ORDER BY created_at DESC"
            )?;
            for item in stmt.query_map(params![sid], mapper)? {
                res.push(item?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, site_id, name, script_content, enabled, created_at FROM user_scripts ORDER BY created_at DESC"
            )?;
            for item in stmt.query_map([], mapper)? {
                res.push(item?);
            }
        }
        Ok(res)
    }

    pub fn delete_user_script(&self, script_id: &str) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM user_scripts WHERE id = ?1", params![script_id])?;
        Ok(())
    }

    pub fn add_rss_feed(&self, site_id: &str, feed_url: &str, title: Option<&str>) -> Result<RssFeedItem> {
        let conn = self.lock_conn()?;
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO rss_feeds (id, site_id, feed_url, title, last_synced) VALUES (?1, ?2, ?3, ?4, NULL) ON CONFLICT(site_id, feed_url) DO UPDATE SET title = coalesce(?4, title)",
            params![id, site_id, feed_url, title],
        )?;
        let feed = conn.query_row(
            "SELECT id, site_id, feed_url, title, last_synced FROM rss_feeds WHERE site_id = ?1 AND feed_url = ?2",
            params![site_id, feed_url],
            |r| Ok(RssFeedItem {
                id: r.get(0)?,
                site_id: r.get(1)?,
                feed_url: r.get(2)?,
                title: r.get(3)?,
                last_synced: r.get(4)?,
            }),
        )?;
        Ok(feed)
    }

    pub fn list_rss_feeds(&self, site_id: &str) -> Result<Vec<RssFeedItem>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, site_id, feed_url, title, last_synced FROM rss_feeds WHERE site_id = ?1 ORDER BY feed_url ASC"
        )?;
        let rows = stmt.query_map(params![site_id], |r| {
            Ok(RssFeedItem {
                id: r.get(0)?,
                site_id: r.get(1)?,
                feed_url: r.get(2)?,
                title: r.get(3)?,
                last_synced: r.get(4)?,
            })
        })?;
        let mut res = Vec::new();
        for item in rows { res.push(item?); }
        Ok(res)
    }

    pub fn delete_rss_feed(&self, feed_id: &str) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM rss_feeds WHERE id = ?1", params![feed_id])?;
        Ok(())
    }

    pub async fn sync_rss_feed(&self, feed_id: &str) -> Result<usize> {
        let (site_id, feed_url) = {
            let conn = self.lock_conn()?;
            conn.query_row(
                "SELECT site_id, feed_url FROM rss_feeds WHERE id = ?1",
                params![feed_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )?
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;
        let xml_text = client.get(&feed_url).send().await?.text().await?;

        let item_link_re = regex::Regex::new(r#"(?s)<item\b.*?<link>(.*?)</link>"#)?;
        let item_link_attr_re = regex::Regex::new(r#"<link\b[^>]*href=["']([^"']+)["']"#)?;

        let mut discovered_urls = Vec::new();
        for cap in item_link_re.captures_iter(&xml_text) {
            let u = cap[1].trim();
            if u.starts_with("http://") || u.starts_with("https://") {
                discovered_urls.push(u.to_string());
            }
        }
        for cap in item_link_attr_re.captures_iter(&xml_text) {
            let u = cap[1].trim();
            if u.starts_with("http://") || u.starts_with("https://") {
                discovered_urls.push(u.to_string());
            }
        }

        let count = self.import_batch_urls(&site_id, &discovered_urls)?;
        let now = chrono::Utc::now().timestamp_millis();
        {
            let conn = self.lock_conn()?;
            let _ = conn.execute(
                "UPDATE rss_feeds SET last_synced = ?1 WHERE id = ?2",
                params![now, feed_id],
            );
        }

        Ok(count)
    }
}

fn decode_credential(mut cred: SiteCredential) -> Result<SiteCredential> {
    cred.cookies_json = crate::protect::open_text(&cred.cookies_json)?;
    cred.storage_json = crate::protect::open_text(&cred.storage_json)?;
    Ok(cred)
}

fn fail_interrupted_jobs(conn: &Connection) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE crawl_jobs SET status = 'failed', finished_at = ?1 WHERE status IN ('running', 'queued')",
        params![now],
    )?;
    Ok(())
}

fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at INTEGER NOT NULL
        );
        "#,
    )?;

    let version: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    if version < 1 {
        conn.execute_batch(
            r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS fts_pages_mig USING fts5(
                page_id UNINDEXED,
                capture_id UNINDEXED,
                url,
                title,
                clean_text,
                meta_desc,
                tokenize = 'unicode61'
            );
            INSERT INTO fts_pages_mig (page_id, capture_id, url, title, clean_text, meta_desc)
                SELECT page_id, capture_id, url, title, clean_text, meta_desc FROM fts_pages;
            DROP TABLE fts_pages;
            ALTER TABLE fts_pages_mig RENAME TO fts_pages;
            "#,
        )?;
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (1, ?1)",
            params![now],
        )?;
    }

    let version: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    if version < 2 {
        reindex_fts_cjk(conn)?;
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (2, ?1)",
            params![now],
        )?;
    }

    Ok(())
}

fn space_cjk(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(
            c,
            '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}' | '\u{20000}'..='\u{2A6DF}'
        ) {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
            out.push(c);
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn reindex_fts_cjk(conn: &Connection) -> Result<()> {
    let rows: Vec<(String, String, String, String, String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT page_id, capture_id, url, title, clean_text, meta_desc FROM fts_pages",
        )?;
        let mapped = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3).unwrap_or_default(),
                r.get::<_, String>(4).unwrap_or_default(),
                r.get::<_, String>(5).unwrap_or_default(),
            ))
        })?;
        mapped.filter_map(|r| r.ok()).collect()
    };

    conn.execute_batch(
        r#"
        DROP TABLE IF EXISTS fts_pages;
        CREATE VIRTUAL TABLE fts_pages USING fts5(
            page_id UNINDEXED,
            capture_id UNINDEXED,
            url,
            title,
            clean_text,
            meta_desc,
            tokenize = 'unicode61'
        );
        "#,
    )?;

    for (page_id, capture_id, url, title, clean_text, meta_desc) in rows {
        conn.execute(
            r#"
            INSERT INTO fts_pages (page_id, capture_id, url, title, clean_text, meta_desc)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                page_id,
                capture_id,
                url,
                space_cjk(&title),
                space_cjk(&clean_text),
                space_cjk(&meta_desc)
            ],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_b_database_operations() {
        let temp_dir = std::env::temp_dir().join(format!("webvault_test_db_{}", uuid::Uuid::new_v4().simple()));
        let db = Database::init(&temp_dir).expect("Database initialization failed");

        // 1. Create site and page
        let site = db.create_site("Test Site", "https://example.com").expect("Failed to create site");
        let page_count = db.import_batch_urls(&site.id, &[
            "https://example.com/page1".to_string(),
            "https://example.com/page2".to_string(),
        ]).expect("Failed to import batch URLs");
        assert_eq!(page_count, 2);

        let pages = db.list_pages(&site.id).expect("Failed to list pages");
        assert_eq!(pages.len(), 2);
        let p1 = &pages[0];

        // 2. Tags
        let tag1 = db.create_tag("Tech", "#6366f1").expect("Failed to create tag");
        let tag2 = db.create_tag("Archive", "#10b981").expect("Failed to create tag");
        let all_tags = db.list_tags().expect("Failed to list tags");
        assert_eq!(all_tags.len(), 2);

        db.add_page_tag(&p1.id, &tag1.id).expect("Failed to tag page");
        db.add_page_tag(&p1.id, &tag2.id).expect("Failed to tag page");
        let page_tags = db.get_page_tags(&p1.id).expect("Failed to get page tags");
        assert_eq!(page_tags.len(), 2);

        db.remove_page_tag(&p1.id, &tag1.id).expect("Failed to untag page");
        let page_tags_after = db.get_page_tags(&p1.id).expect("Failed to get page tags");
        assert_eq!(page_tags_after.len(), 1);

        let cred = db
            .save_site_credential(&site.id, "default", r#"[{"name":"sid","value":"abc"}]"#, "{}")
            .expect("save credential");
        assert_eq!(cred.cookies_json, r#"[{"name":"sid","value":"abc"}]"#);
        let stored: String = {
            let conn = db.lock_conn().unwrap();
            conn.query_row(
                "SELECT cookies_json FROM site_credentials WHERE id = ?1",
                params![cred.id],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert!(stored.starts_with("wv1:"), "credentials must be sealed at rest");
        let loaded = db.get_site_credentials(&site.id).unwrap();
        assert_eq!(loaded[0].cookies_json, r#"[{"name":"sid","value":"abc"}]"#);

        let script = db.save_user_script(
            Some(&site.id),
            "Remove Banner",
            "document.querySelector('.banner')?.remove();",
            true,
        ).expect("Failed to save user script");
        assert_eq!(script.name, "Remove Banner");

        let scripts = db.list_user_scripts(Some(&site.id)).expect("Failed to list scripts");
        assert_eq!(scripts.len(), 1);
        assert_eq!(scripts[0].enabled, true);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_monitor_rule_deserialization_and_crawl_queue_schema() {
        let temp_dir = std::env::temp_dir().join(format!("test_mq_{}", uuid::Uuid::new_v4().simple()));
        let db = Database::init(&temp_dir).expect("Failed to init db");

        // Test crawl_queue error_message column
        let site = db.create_site("Test", "http://a.com").unwrap();
        let conn = db.lock_conn().expect("Lock conn");
        conn.execute(
            r#"
            INSERT INTO crawl_jobs (id, site_id, status, started_at)
            VALUES ('j1', ?1, 'running', 1000)
            "#,
            [&site.id],
        ).unwrap();
        conn.execute(
            r#"
            INSERT INTO crawl_queue (id, job_id, url, normalized_url, depth, status, discovered_at)
            VALUES ('q1', 'j1', 'http://a.com', 'http://a.com', 0, 'processing', 1000)
            "#,
            [],
        ).unwrap();
        let rows_affected = conn.execute(
            "UPDATE crawl_queue SET status = 'failed', error_message = ?1 WHERE job_id = ?2 AND normalized_url = ?3",
            rusqlite::params!["connection timed out", "j1", "http://a.com"],
        ).unwrap();
        assert_eq!(rows_affected, 1);

        // Test MonitorRule deserialization without id (from frontend)
        let json_data = r#"{
            "site_id": "s1",
            "url": "https://example.com/check",
            "schedule": "1h",
            "strategy": "content_hash"
        }"#;
        let rule: MonitorRule = serde_json::from_str(json_data).expect("Must deserialize MonitorRule without id");
        assert!(rule.id.is_empty());
        assert_eq!(rule.enabled, true);
        assert_eq!(rule.url, "https://example.com/check");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_delete_site_removes_archive_and_profile_dirs() {
        let temp_dir = std::env::temp_dir().join(format!("webvault_del_{}", uuid::Uuid::new_v4().simple()));
        let db = Database::init(&temp_dir).expect("init");
        let site = db.create_site("Delete Me", "https://example.com").unwrap();

        let archive_dir = temp_dir.join("archives").join(&site.id);
        let profile_dir = temp_dir.join("browser_profiles").join(&site.id);
        let ss_dir = temp_dir.join("screenshots");
        std::fs::create_dir_all(&archive_dir).unwrap();
        std::fs::create_dir_all(&profile_dir).unwrap();
        std::fs::create_dir_all(&ss_dir).unwrap();
        std::fs::write(archive_dir.join("data.warc.gz"), b"warc").unwrap();
        std::fs::write(profile_dir.join("cookie"), b"secret").unwrap();
        std::fs::write(ss_dir.join(format!("ss_{}_1.jpg", site.id)), b"jpg").unwrap();

        db.delete_site(&site.id).unwrap();

        assert!(db.list_sites().unwrap().is_empty());
        assert!(!archive_dir.exists());
        assert!(!profile_dir.exists());
        assert!(!ss_dir.join(format!("ss_{}_1.jpg", site.id)).exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_completed_update_does_not_overwrite_cancelled() {
        let temp_dir = std::env::temp_dir().join(format!("webvault_job_{}", uuid::Uuid::new_v4().simple()));
        let db = Database::init(&temp_dir).unwrap();
        let site = db.create_site("Jobs", "https://example.com").unwrap();
        let conn = db.lock_conn().unwrap();
        conn.execute(
            "INSERT INTO crawl_jobs (id, site_id, status, started_at) VALUES ('j1', ?1, 'running', 1)",
            [&site.id],
        ).unwrap();
        conn.execute(
            "UPDATE crawl_jobs SET status = 'cancelled', finished_at = 2 WHERE id = 'j1' AND status = 'running'",
            [],
        ).unwrap();
        let overwritten = conn.execute(
            "UPDATE crawl_jobs SET status = 'completed', finished_at = 3 WHERE id = 'j1' AND status = 'running'",
            [],
        ).unwrap();
        assert_eq!(overwritten, 0);
        let status: String = conn.query_row("SELECT status FROM crawl_jobs WHERE id = 'j1'", [], |r| r.get(0)).unwrap();
        assert_eq!(status, "cancelled");
        drop(conn);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_concurrent_warc_appends_remain_readable() {
        use crate::archive::warc::{WarcReader, WarcRecord, WarcWriter};
        let temp_dir = std::env::temp_dir().join(format!("webvault_cwarc_{}", uuid::Uuid::new_v4().simple()));
        let db = Database::init(&temp_dir).unwrap();
        let warc_path = temp_dir.join("data.warc.gz");
        let writer = std::sync::Arc::new(WarcWriter::new(&warc_path));
        let mut handles = Vec::new();
        for i in 0..8 {
            let db = db.clone();
            let writer = writer.clone();
            handles.push(std::thread::spawn(move || {
                let mut headers = std::collections::HashMap::new();
                headers.insert("Content-Type".to_string(), "text/plain".to_string());
                let body = format!("payload-{}", i);
                let rec = WarcRecord::create_response_record(
                    &format!("https://example.com/{}", i),
                    200,
                    "OK",
                    &headers,
                    body.as_bytes(),
                );
                let _g = db.lock_warc().unwrap();
                writer.append_record(&rec).unwrap()
            }));
        }
        let mut offsets = Vec::new();
        for h in handles {
            offsets.push(h.join().unwrap());
        }
        for (offset, length) in offsets {
            let rec = WarcReader::read_record_at(&warc_path, offset, length).expect("record readable after concurrent append");
            let (_status, _headers, body) = WarcReader::parse_http_response(&rec.content).unwrap();
            assert!(std::str::from_utf8(&body).unwrap().starts_with("payload-"));
        }
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

