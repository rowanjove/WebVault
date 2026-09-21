use crate::archive::dedup::Deduplicator;
use crate::archive::service::CapturePersistenceService;
use crate::archive::warc::WarcWriter;
use crate::capture::browser::{BrowserFinder, BrowserLaunchOptions};
use crate::capture::cdp::{CaptureResult, CdpClient};
use crate::database::models::{ChangeEventItem, MonitorRule};
use crate::database::Database;
use anyhow::Result;
use std::path::PathBuf;

pub struct MonitorEngine {
    db: Database,
    browser_path: Option<PathBuf>,
}

impl MonitorEngine {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            browser_path: None,
        }
    }

    pub fn with_browser(mut self, browser_path: Option<PathBuf>) -> Self {
        self.browser_path = browser_path;
        self
    }

    pub fn list_rules(&self) -> Result<Vec<MonitorRule>> {
        let conn = self.db.lock_conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, site_id, url, schedule, strategy, selector, keyword, enabled, last_checked, next_check, last_status
            FROM monitor_rules ORDER BY next_check ASC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(MonitorRule {
                id: row.get(0)?,
                site_id: row.get(1)?,
                url: row.get(2)?,
                schedule: row.get(3)?,
                strategy: row.get(4)?,
                selector: row.get(5)?,
                keyword: row.get(6)?,
                enabled: row.get::<_, i32>(7)? == 1,
                last_checked: row.get(8)?,
                next_check: row.get(9)?,
                last_status: row.get(10)?,
            })
        })?;

        let mut rules = Vec::new();
        for r in rows {
            rules.push(r?);
        }
        Ok(rules)
    }

    pub fn create_rule(&self, rule: &MonitorRule) -> Result<MonitorRule> {
        let conn = self.db.lock_conn()?;
        let id = format!("rule_{}", uuid::Uuid::new_v4().simple());
        let now = chrono::Utc::now().timestamp_millis();
        let interval_ms = Self::parse_schedule_ms(&rule.schedule);
        let next_check = now + interval_ms;

        conn.execute(
            r#"
            INSERT INTO monitor_rules (id, site_id, url, schedule, strategy, selector, keyword, enabled, last_checked, next_check, last_status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'Initialized')
            "#,
            rusqlite::params![
                id, rule.site_id, rule.url, rule.schedule, rule.strategy,
                rule.selector, rule.keyword, rule.enabled as i32, now, next_check
            ],
        )?;

        let mut created = rule.clone();
        created.id = id;
        created.last_checked = Some(now);
        created.next_check = Some(next_check);
        created.last_status = Some("Initialized".to_string());
        Ok(created)
    }

    pub fn toggle_rule(&self, id: &str, enabled: bool) -> Result<()> {
        let conn = self.db.lock_conn()?;
        conn.execute(
            "UPDATE monitor_rules SET enabled = ?1 WHERE id = ?2",
            rusqlite::params![enabled as i32, id],
        )?;
        Ok(())
    }

    pub fn delete_rule(&self, id: &str) -> Result<()> {
        let conn = self.db.lock_conn()?;
        conn.execute("DELETE FROM monitor_rules WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    pub fn list_change_events(&self) -> Result<Vec<ChangeEventItem>> {
        let conn = self.db.lock_conn()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT c.id, c.page_id, p.site_id, p.url, p.title, c.old_capture_id, c.new_capture_id,
                   coalesce(c1.captured_at, c.created_at) as old_time,
                   coalesce(c2.captured_at, c.created_at) as new_time,
                   c.text_changed, c.dom_changed, c.visual_changed, c.resource_changed,
                   c.change_score, c.created_at
            FROM change_events c
            JOIN pages p ON c.page_id = p.id
            LEFT JOIN captures c1 ON c.old_capture_id = c1.id
            LEFT JOIN captures c2 ON c.new_capture_id = c2.id
            ORDER BY c.created_at DESC
            LIMIT 100
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ChangeEventItem {
                id: row.get(0)?,
                page_id: row.get(1)?,
                site_id: row.get(2)?,
                url: row.get(3)?,
                title: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                old_capture_id: row.get(5)?,
                new_capture_id: row.get(6)?,
                old_time: row.get(7)?,
                new_time: row.get(8)?,
                text_changed: row.get::<_, i32>(9)? == 1,
                dom_changed: row.get::<_, i32>(10)? == 1,
                visual_changed: row.get::<_, i32>(11)? == 1,
                resource_changed: row.get::<_, i32>(12)? == 1,
                change_score: row.get(13)?,
                created_at: row.get(14)?,
            })
        })?;

        let mut events = Vec::new();
        for r in rows {
            events.push(r?);
        }
        Ok(events)
    }

    pub async fn check_pending_rules(&self) -> Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        let due_rules: Vec<MonitorRule> = {
            let conn = self.db.lock_conn()?;
            let mut stmt = conn.prepare(
                r#"
                SELECT id, site_id, url, schedule, strategy, selector, keyword, enabled, last_checked, next_check, last_status
                FROM monitor_rules WHERE enabled = 1 AND next_check <= ?1
                "#,
            )?;
            let rows = stmt.query_map(rusqlite::params![now], |row| {
                Ok(MonitorRule {
                    id: row.get(0)?,
                    site_id: row.get(1)?,
                    url: row.get(2)?,
                    schedule: row.get(3)?,
                    strategy: row.get(4)?,
                    selector: row.get(5)?,
                    keyword: row.get(6)?,
                    enabled: row.get::<_, i32>(7)? == 1,
                    last_checked: row.get(8)?,
                    next_check: row.get(9)?,
                    last_status: row.get(10)?,
                })
            })?;
            let mut res = Vec::new();
            for r in rows {
                res.push(r?);
            }
            res
        };

        if due_rules.is_empty() {
            return Ok(());
        }

        for rule in due_rules {
            let interval = Self::parse_schedule_ms(&rule.schedule);
            let next = now + interval;
            let status = match self.check_one_rule(&rule, now).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("Monitor check failed for {}: {}", rule.url, e);
                    format!("Check failed: {}", e)
                }
            };

            if let Ok(conn) = self.db.lock_conn() {
                let _ = conn.execute(
                    "UPDATE monitor_rules SET last_checked = ?1, next_check = ?2, last_status = ?3 WHERE id = ?4",
                    rusqlite::params![now, next, status, rule.id],
                );
            }
        }

        Ok(())
    }

    async fn check_one_rule(&self, rule: &MonitorRule, now: i64) -> Result<String> {
        if self.browser_path.is_some() {
            self.check_rule_cdp(rule, now).await
        } else {
            self.check_rule_http(rule, now).await
        }
    }

    async fn check_rule_cdp(&self, rule: &MonitorRule, now: i64) -> Result<String> {
        let browser_path = self
            .browser_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No browser"))?;
        let capture = self.capture_via_cdp(browser_path, rule).await?;
        let hash = Deduplicator::compute_text_hash(&capture.clean_text);
        self.apply_hash_result(rule, now, &hash, InspectedPage::Cdp(capture))
    }

    async fn capture_via_cdp(
        &self,
        browser_path: &std::path::Path,
        rule: &MonitorRule,
    ) -> Result<CaptureResult> {
        let temp_dir = self.db.base_dir.join("temp");
        let site_profile_dir = self.db.base_dir.join("browser_profiles").join(&rule.site_id);
        let launch_options = BrowserLaunchOptions {
            headless: true,
            user_data_dir: Some(site_profile_dir),
            window_width: 1920,
            window_height: 1080,
        };
        let port = BrowserFinder::find_available_port();
        let browser =
            BrowserFinder::launch_with_options(browser_path, port, &temp_dir, &launch_options).await?;
        let client = CdpClient::new(browser.port);
        let creds = self.db.get_site_credentials(&rule.site_id).ok();
        let (cookies, storage) = if let Some(ref list) = creds {
            if let Some(first) = list.first() {
                (Some(first.cookies_json.as_str()), Some(first.storage_json.as_str()))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };
        let result = client
            .capture_page_with_credentials(&rule.url, true, 20, cookies, storage, None)
            .await;
        drop(browser);
        result
    }

    async fn check_rule_http(&self, rule: &MonitorRule, now: i64) -> Result<String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        let resp = client.get(&rule.url).send().await?;
        if !resp.status().is_success() {
            return Ok(format!("HTTP {}", resp.status()));
        }
        let text = resp.text().await.unwrap_or_default();
        let hash = hash_monitor_content(&text);
        self.apply_hash_result(rule, now, &hash, InspectedPage::Html(text))
    }

    fn apply_hash_result(
        &self,
        rule: &MonitorRule,
        now: i64,
        hash: &str,
        page: InspectedPage,
    ) -> Result<String> {
        let prev_hash: Option<String> = {
            let conn = self.db.lock_conn()?;
            conn.query_row(
                r#"
                SELECT c.text_hash FROM captures c
                JOIN pages p ON c.page_id = p.id
                WHERE p.site_id = ?1 AND p.url = ?2
                ORDER BY c.captured_at DESC LIMIT 1
                "#,
                rusqlite::params![rule.site_id, rule.url],
                |row| row.get(0),
            )
            .ok()
        };

        match prev_hash {
            None => {
                self.persist_inspected(rule, now, hash, page)?;
                Ok("Baseline recorded".to_string())
            }
            Some(prev) if prev != hash => {
                let page_info: Option<(String, Option<String>)> = {
                    let conn = self.db.lock_conn()?;
                    conn.query_row(
                        r#"
                        SELECT p.id, (
                            SELECT c.id FROM captures c
                            WHERE c.page_id = p.id
                            ORDER BY c.captured_at DESC LIMIT 1
                        ) as latest_cap_id
                        FROM pages p WHERE p.site_id = ?1 AND p.url = ?2 LIMIT 1
                        "#,
                        rusqlite::params![rule.site_id, rule.url],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .ok()
                };
                let new_cap_id = self.persist_inspected(rule, now, hash, page)?;
                if let Some((page_id, old_cap_id)) = page_info {
                    let mut change_score = 30.0;
                    let mut text_changed = true;
                    if let Some(ref old_id) = old_cap_id {
                        if let Ok(diff_res) = crate::diff::DiffEngine::compare(&self.db, old_id, &new_cap_id) {
                            change_score = diff_res.change_score;
                            text_changed = diff_res.text_diff.added_lines > 0
                                || diff_res.text_diff.removed_lines > 0;
                        }
                    }
                    if let Ok(conn) = self.db.lock_conn() {
                        let ev_id = format!("ev_{}", uuid::Uuid::new_v4().simple());
                        let _ = conn.execute(
                            r#"
                            INSERT INTO change_events (
                                id, page_id, old_capture_id, new_capture_id,
                                text_changed, dom_changed, visual_changed, resource_changed,
                                change_score, created_at
                            ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 0, 0, ?6, ?7)
                            "#,
                            rusqlite::params![
                                ev_id, page_id, old_cap_id, new_cap_id,
                                text_changed as i32, change_score, now
                            ],
                        );
                    }
                    Ok(format!("Change Detected ({:.0}%)", change_score))
                } else {
                    Ok("Change Detected".to_string())
                }
            }
            Some(_) => Ok("Checked - No Change".to_string()),
        }
    }

    fn persist_inspected(
        &self,
        rule: &MonitorRule,
        now: i64,
        hash: &str,
        page: InspectedPage,
    ) -> Result<String> {
        match page {
            InspectedPage::Html(html) => {
                persist_monitor_snapshot(&self.db, &rule.site_id, &rule.url, &html, hash, now)
            }
            InspectedPage::Cdp(capture) => {
                let warc_full = self.db.base_dir.join(format!("archives/{}/data.warc.gz", rule.site_id));
                let writer = WarcWriter::new(&warc_full);
                let persisted = CapturePersistenceService::persist_capture(
                    &self.db,
                    &rule.site_id,
                    "monitor",
                    &capture,
                    &writer,
                )?;
                Ok(persisted.capture_id)
            }
        }
    }

    fn parse_schedule_ms(schedule: &str) -> i64 {
        match schedule {
            "5m" => 5 * 60 * 1000,
            "15m" => 15 * 60 * 1000,
            "30m" => 30 * 60 * 1000,
            "1h" => 60 * 60 * 1000,
            "6h" => 6 * 60 * 60 * 1000,
            "12h" => 12 * 60 * 60 * 1000,
            "24h" | "1d" => 24 * 60 * 60 * 1000,
            _ => 60 * 60 * 1000, // default 1h
        }
    }
}

enum InspectedPage {
    Html(String),
    Cdp(CaptureResult),
}

pub fn hash_monitor_content(html: &str) -> String {
    let clean = crate::capture::behavior::TextExtractor::extract_clean_text(html);
    crate::archive::dedup::Deduplicator::compute_text_hash(&clean)
}

fn persist_monitor_snapshot(
    db: &Database,
    site_id: &str,
    url: &str,
    html: &str,
    hash: &str,
    now: i64,
) -> Result<String> {
    let warc_rel_path = format!("archives/{}/data.warc.gz", site_id);
    let warc_full_path = db.base_dir.join(&warc_rel_path);
    let writer = crate::archive::warc::WarcWriter::new(&warc_full_path);

    let mut headers = std::collections::HashMap::new();
    headers.insert("Content-Type".to_string(), "text/html; charset=utf-8".to_string());
    let rec = crate::archive::warc::WarcRecord::create_response_record(url, 200, "OK", &headers, html.as_bytes());
    let (offset, length) = {
        let _warc_guard = db.lock_warc()?;
        writer.append_record(&rec)?
    };

    let new_cap_id = format!("cap_{}", uuid::Uuid::new_v4().simple());
    let clean_text = crate::capture::behavior::TextExtractor::extract_clean_text(html);
    let title = crate::capture::behavior::TextExtractor::extract_title(html).unwrap_or_else(|| url.to_string());
    let normalizer = crate::crawler::normalizer::UrlNormalizer::new();
    let norm_url = normalizer.normalize(url).unwrap_or_else(|_| url.to_string());
    let page_id = format!("page_{}", uuid::Uuid::new_v4().simple());

    let actual_page_id = {
        let conn = db.lock_conn()?;
        conn.execute(
            r#"
            INSERT INTO pages (id, site_id, url, normalized_url, title, first_seen, last_seen, capture_count)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)
            ON CONFLICT(site_id, normalized_url) DO UPDATE SET
                capture_count = capture_count + 1,
                title = excluded.title,
                last_seen = excluded.last_seen
            "#,
            rusqlite::params![page_id, site_id, url, norm_url, title, now, now],
        )?;
        let real_id: String = conn
            .query_row(
                "SELECT id FROM pages WHERE site_id = ?1 AND normalized_url = ?2",
                rusqlite::params![site_id, norm_url],
                |r| r.get(0),
            )
            .unwrap_or(page_id);
        conn.execute(
            r#"
            INSERT INTO captures (
                id, page_id, job_id, captured_at, status_code, mime_type,
                warc_file, warc_offset, warc_length, text_hash,
                resource_count, missing_resource_count, capture_score
            ) VALUES (?1, ?2, 'monitor', ?3, 200, 'text/html', ?4, ?5, ?6, ?7, 1, 0, 100.0)
            "#,
            rusqlite::params![
                new_cap_id, real_id, now, warc_rel_path,
                offset as i64, length as i64, hash
            ],
        )?;
        real_id
    };

    let _ = crate::search::SearchEngine::index_page(db, &actual_page_id, &new_cap_id, url, &title, &clean_text, "");
    Ok(new_cap_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_hash_matches_cdp_clean_text_hash() {
        let html = "<html><head><title>T</title></head><body><h1>Hello World</h1><script>alert(1)</script></body></html>";
        let clean = crate::capture::behavior::TextExtractor::extract_clean_text(html);
        let expected = crate::archive::dedup::Deduplicator::compute_text_hash(&clean);
        assert_eq!(hash_monitor_content(html), expected);
        assert_ne!(hash_monitor_content(html), crate::archive::dedup::Deduplicator::compute_text_hash(html));
    }

    #[test]
    fn test_monitor_baseline_persists_first_snapshot() {
        let temp = std::env::temp_dir().join(format!("webvault_mon_{}", uuid::Uuid::new_v4().simple()));
        let db = Database::init(&temp).unwrap();
        let site = db.create_site("Mon", "https://example.com").unwrap();
        let html = "<html><body><p>baseline body</p></body></html>";
        let hash = hash_monitor_content(html);
        let cap_id = persist_monitor_snapshot(&db, &site.id, "https://example.com/watch", html, &hash, 1).unwrap();
        assert!(!cap_id.is_empty());
        let pages = db.list_pages(&site.id).unwrap();
        assert_eq!(pages.len(), 1);
        let caps = db.list_captures(&pages[0].id).unwrap();
        assert_eq!(caps.len(), 1);
        assert_eq!(caps[0].text_hash.as_deref(), Some(hash.as_str()));
        let _ = std::fs::remove_dir_all(&temp);
    }
}
