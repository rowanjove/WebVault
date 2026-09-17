use crate::archive::dedup::Deduplicator;
use crate::archive::warc::{WarcRecord, WarcWriter};
use crate::capture::cdp::CaptureResult;
use crate::crawler::normalizer::UrlNormalizer;
use crate::database::Database;
use crate::search::SearchEngine;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct PersistedCapture {
    pub page_id: String,
    pub capture_id: String,
    pub warc_path: PathBuf,
    pub bytes_written: u64,
}

pub struct CapturePersistenceService;

impl CapturePersistenceService {
    pub fn persist_capture(
        db: &Database,
        site_id: &str,
        job_id: &str,
        capture: &CaptureResult,
        writer: &WarcWriter,
    ) -> Result<PersistedCapture> {
        let now = chrono::Utc::now().timestamp_millis();
        let normalizer = UrlNormalizer::new();
        let norm_url = normalizer.normalize(&capture.url).unwrap_or_else(|_| capture.url.clone());

        let screenshot_rel_path = if let Some(ref ss_bytes) = capture.screenshot_bytes {
            let ss_name = format!("ss_{}_{}.jpg", site_id, now);
            let ss_dir = db.base_dir.join("screenshots");
            let _ = std::fs::create_dir_all(&ss_dir);
            let _ = std::fs::write(ss_dir.join(&ss_name), ss_bytes);
            Some(format!("screenshots/{}", ss_name))
        } else {
            None
        };

        let mut main_headers = HashMap::new();
        main_headers.insert("Content-Type".to_string(), "text/html; charset=utf-8".to_string());
        let main_rec = WarcRecord::create_response_record(
            &capture.url,
            capture.status_code,
            "OK",
            &main_headers,
            capture.html.as_bytes(),
        );

        let (actual_warc_path, main_offset, main_length) = writer.append_record_rolling(&main_rec)?;
        let warc_rel_path = actual_warc_path
            .strip_prefix(&db.base_dir)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| actual_warc_path.to_string_lossy().replace('\\', "/"));

        let page_id = format!("page_{}", uuid::Uuid::new_v4().simple());
        let cap_id = format!("cap_{}", uuid::Uuid::new_v4().simple());
        let text_hash = Deduplicator::compute_text_hash(&capture.clean_text);

        let mut total_bytes = main_length;

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
                rusqlite::params![page_id, site_id, capture.url, norm_url, capture.title, now, now],
            )?;

            let real_id: String = conn.query_row(
                "SELECT id FROM pages WHERE site_id = ?1 AND normalized_url = ?2",
                rusqlite::params![site_id, norm_url],
                |r| r.get(0),
            ).unwrap_or(page_id);

            conn.execute(
                r#"
                INSERT INTO captures (
                    id, page_id, job_id, captured_at, status_code, mime_type,
                    warc_file, warc_offset, warc_length, screenshot_path, text_hash,
                    resource_count, missing_resource_count, capture_score
                ) VALUES (?1, ?2, ?3, ?4, ?5, 'text/html', ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                "#,
                rusqlite::params![
                    cap_id, real_id, job_id, now, capture.status_code as i32,
                    warc_rel_path, main_offset as i64, main_length as i64,
                    screenshot_rel_path, text_hash,
                    capture.resource_count as i64, capture.missing_resource_count as i64,
                    capture.capture_score
                ],
            )?;

            for (_req, resp) in &capture.network_records {
                if resp.url != capture.url && !resp.body.is_empty() {
                    let sub_rec = WarcRecord::create_response_record(
                        &resp.url,
                        resp.status,
                        &resp.status_text,
                        &resp.headers,
                        &resp.body,
                    );
                    if let Ok((sub_warc_path, res_offset, res_length)) = writer.append_record_rolling(&sub_rec) {
                        total_bytes += res_length;
                        let res_id = format!("res_{}", uuid::Uuid::new_v4().simple());
                        let norm_res_url = normalizer.normalize(&resp.url).unwrap_or_else(|_| resp.url.clone());
                        let sha = Deduplicator::compute_sha256(&resp.body);
                        let sub_warc_rel = sub_warc_path
                            .strip_prefix(&db.base_dir)
                            .map(|p| p.to_string_lossy().replace('\\', "/"))
                            .unwrap_or_else(|_| sub_warc_path.to_string_lossy().replace('\\', "/"));

                        let _ = conn.execute(
                            r#"
                            INSERT INTO resources (
                                id, capture_id, url, normalized_url, mime_type, status_code, size, sha256,
                                warc_file, warc_offset, warc_length, resource_type
                            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                            "#,
                            rusqlite::params![
                                res_id, cap_id, resp.url, norm_res_url, resp.mime_type,
                                resp.status as i32, resp.size as i64, sha,
                                sub_warc_rel, res_offset as i64, res_length as i64, "asset"
                            ],
                        );
                    }
                }
            }

            real_id
        };

        if let Err(e) = SearchEngine::index_page(
            db,
            &actual_page_id,
            &cap_id,
            &capture.url,
            &capture.title,
            &capture.clean_text,
            "",
        ) {
            tracing::warn!("Failed indexing capture {} in FTS5: {}", cap_id, e);
        }

        Ok(PersistedCapture {
            page_id: actual_page_id,
            capture_id: cap_id,
            warc_path: actual_warc_path,
            bytes_written: total_bytes,
        })
    }
}
