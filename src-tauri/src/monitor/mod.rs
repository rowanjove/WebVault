use crate::database::models::{ChangeEventItem, MonitorRule};
use crate::database::Database;
use anyhow::Result;

pub struct MonitorEngine {
    db: Database,
}

impl MonitorEngine {
    pub fn new(db: Database) -> Self {
        Self { db }
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
            SELECT c.id, c.page_id, p.url, p.title, c.old_capture_id, c.new_capture_id,
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
                url: row.get(2)?,
                title: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                old_capture_id: row.get(4)?,
                new_capture_id: row.get(5)?,
                old_time: row.get(6)?,
                new_time: row.get(7)?,
                text_changed: row.get::<_, i32>(8)? == 1,
                dom_changed: row.get::<_, i32>(9)? == 1,
                visual_changed: row.get::<_, i32>(10)? == 1,
                resource_changed: row.get::<_, i32>(11)? == 1,
                change_score: row.get(12)?,
                created_at: row.get(13)?,
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

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        for rule in due_rules {
            let interval = Self::parse_schedule_ms(&rule.schedule);
            let next = now + interval;
            let mut status = "Checked - No Change".to_string();

            if let Ok(resp) = client.get(&rule.url).send().await {
                if resp.status().is_success() {
                    let text = resp.text().await.unwrap_or_default();
                    let hash = crate::archive::dedup::Deduplicator::compute_text_hash(&text);

                    let prev_hash: Option<String> = {
                        let conn = self.db.lock_conn()?;
                        conn.query_row(
                            r#"
                            SELECT c.text_hash FROM captures c
                            JOIN pages p ON c.page_id = p.id
                            WHERE p.url = ?1
                            ORDER BY c.captured_at DESC LIMIT 1
                            "#,
                            rusqlite::params![rule.url],
                            |row| row.get(0),
                        ).ok()
                    };

                    if let Some(prev) = prev_hash {
                        if prev != hash {
                            status = "Change Detected".to_string();
                            
                            // Find page and latest old capture
                            let page_info: Option<(String, Option<String>)> = {
                                let conn = self.db.lock_conn()?;
                                conn.query_row(
                                    r#"
                                    SELECT p.id, (
                                        SELECT c.id FROM captures c
                                        WHERE c.page_id = p.id
                                        ORDER BY c.captured_at DESC LIMIT 1
                                    ) as latest_cap_id
                                    FROM pages p WHERE p.url = ?1 LIMIT 1
                                    "#,
                                    rusqlite::params![rule.url],
                                    |row| Ok((row.get(0)?, row.get(1)?)),
                                ).ok()
                            };

                            if let Some((page_id, old_cap_id)) = page_info {
                                // Save new snapshot to WARC
                                let warc_rel_path = format!("archives/{}/data.warc.gz", rule.site_id);
                                let warc_full_path = self.db.base_dir.join(&warc_rel_path);
                                let writer = crate::archive::warc::WarcWriter::new(&warc_full_path);

                                let mut headers = std::collections::HashMap::new();
                                headers.insert("Content-Type".to_string(), "text/html; charset=utf-8".to_string());
                                let rec = crate::archive::warc::WarcRecord::create_response_record(
                                    &rule.url,
                                    200,
                                    "OK",
                                    &headers,
                                    text.as_bytes(),
                                );
                                let (offset, length) = writer.append_record(&rec).unwrap_or((0, 0));

                                // Insert new capture record
                                let new_cap_id = format!("cap_{}", uuid::Uuid::new_v4().simple());
                                let clean_text = crate::capture::behavior::TextExtractor::extract_clean_text(&text);
                                let title = crate::capture::behavior::TextExtractor::extract_title(&text)
                                    .unwrap_or_else(|| rule.url.clone());

                                if let Ok(conn) = self.db.lock_conn() {
                                    let _ = conn.execute(
                                        r#"
                                        INSERT INTO captures (
                                            id, page_id, job_id, captured_at, status_code, mime_type,
                                            warc_file, warc_offset, warc_length, text_hash,
                                            resource_count, missing_resource_count, capture_score
                                        ) VALUES (?1, ?2, 'monitor', ?3, 200, 'text/html', ?4, ?5, ?6, ?7, 1, 0, 100.0)
                                        "#,
                                        rusqlite::params![
                                            new_cap_id, page_id, now, warc_rel_path,
                                            offset as i64, length as i64, hash
                                        ],
                                    );
                                }

                                // Update search index
                                let _ = crate::search::SearchEngine::index_page(
                                    &self.db,
                                    &page_id,
                                    &new_cap_id,
                                    &rule.url,
                                    &title,
                                    &clean_text,
                                    "",
                                );

                                // Compare with previous capture if available
                                let mut change_score = 30.0;
                                let mut text_changed = true;
                                if let Some(ref old_id) = old_cap_id {
                                    if let Ok(diff_res) = crate::diff::DiffEngine::compare(&self.db, old_id, &new_cap_id) {
                                        change_score = diff_res.change_score;
                                        text_changed = diff_res.text_diff.added_lines > 0 || diff_res.text_diff.removed_lines > 0;
                                    }
                                }

                                // Record change event
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

                                status = format!("Change Detected ({:.0}%)", change_score);
                            }
                        }
                    }
                } else {
                    status = format!("HTTP {}", resp.status());
                }
            }

            if let Ok(conn) = self.db.lock_conn() {
                let _ = conn.execute(
                    "UPDATE monitor_rules SET last_checked = ?1, next_check = ?2, last_status = ?3 WHERE id = ?4",
                    rusqlite::params![now, next, status, rule.id],
                );
            }
        }

        Ok(())
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
