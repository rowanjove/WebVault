use crate::archive::warc::{WarcRecord, WarcWriter};
use crate::database::models::WaybackCaptureItem;
use crate::database::Database;
use crate::search::SearchEngine;
use anyhow::Result;
use chrono::NaiveDateTime;
use reqwest::Client;
use std::collections::HashMap;

pub struct WaybackProvider {
    client: Client,
}

impl Default for WaybackProvider {
    fn default() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .user_agent("WebVault/1.0.0 (Local Web Time Machine; +https://github.com/webvault/webvault)")
                .build()
                .unwrap_or_default(),
        }
    }
}

impl WaybackProvider {
    pub fn new() -> Self {
        Self::default()
    }

    /// Query the Internet Archive CDX API for historical captures of a URL
    pub async fn query_cdx(&self, target_url: &str) -> Result<Vec<WaybackCaptureItem>> {
        let encoded: String = url::form_urlencoded::byte_serialize(target_url.as_bytes()).collect();
        let cdx_url = format!(
            "https://web.archive.org/cdx/search/cdx?url={}&output=json&fl=original,timestamp,statuscode,mimetype,digest,length&collapse=timestamp:8&limit=100",
            encoded
        );

        let resp = self.client.get(&cdx_url).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("Wayback CDX query returned status: {}", resp.status());
        }

        let matrix: Vec<Vec<String>> = resp.json().await?;
        if matrix.is_empty() {
            return Ok(Vec::new());
        }

        // First row is header: ["original","timestamp","statuscode","mimetype","digest","length"]
        let mut results = Vec::new();
        for row in matrix.into_iter().skip(1) {
            if row.len() >= 6 {
                let url = row[0].clone();
                let timestamp = row[1].clone();
                let status_code = row[2].clone();
                let mime_type = row[3].clone();
                let digest = row[4].clone();
                let length = row[5].clone();
                let id = format!("wb_{}_{}", timestamp, &digest[..digest.len().min(8)]);

                results.push(WaybackCaptureItem {
                    id,
                    url,
                    timestamp,
                    status_code,
                    mime_type,
                    digest,
                    length,
                    imported: false,
                });
            }
        }

        // Sort descending by timestamp
        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(results)
    }

    /// Download a historical capture from Wayback and import it directly into WebVault
    pub async fn import_capture(
        &self,
        db: &Database,
        site_id: &str,
        item: &WaybackCaptureItem,
    ) -> Result<()> {
        // Use `id_` modifier to get the raw unmodified original HTTP body from Wayback Machine
        let raw_url = format!("https://web.archive.org/web/{}id_/{}", item.timestamp, item.url);
        let resp = self.client.get(&raw_url).send().await?;
        let status = resp.status().as_u16();

        let mut headers = HashMap::new();
        for (k, v) in resp.headers() {
            if let Ok(val) = v.to_str() {
                headers.insert(k.as_str().to_lowercase(), val.to_string());
            }
        }

        let body = resp.bytes().await?.to_vec();

        // Convert Wayback timestamp (YYYYMMDDhhmmss) to epoch ms
        let epoch_ms = if let Ok(dt) = NaiveDateTime::parse_from_str(&item.timestamp, "%Y%m%d%H%M%S") {
            dt.and_utc().timestamp_millis()
        } else {
            chrono::Utc::now().timestamp_millis()
        };

        // Write to local WARC
        let warc_rel_path = format!("archives/{}/data.warc.gz", site_id);
        let warc_full_path = db.base_dir.join(&warc_rel_path);
        let writer = WarcWriter::new(&warc_full_path);

        let record = WarcRecord::create_response_record(&item.url, status, "OK", &headers, &body);
        let (offset, length) = {
            let _warc_guard = db.lock_warc()?;
            writer.append_record(&record)?
        };

        // Ensure page entry exists
        let normalizer = crate::crawler::normalizer::UrlNormalizer::new();
        let norm_url = normalizer.normalize(&item.url).unwrap_or_else(|_| item.url.clone());

        let page_id = format!("page_{}", uuid::Uuid::new_v4().simple());
        let cap_id = format!("cap_{}", uuid::Uuid::new_v4().simple());

        let conn = db.lock_conn()?;
        conn.execute(
            r#"
            INSERT INTO pages (id, site_id, url, normalized_url, title, first_seen, last_seen, capture_count)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)
            ON CONFLICT(site_id, normalized_url) DO UPDATE SET
                capture_count = capture_count + 1,
                last_seen = MAX(last_seen, excluded.last_seen)
            "#,
            rusqlite::params![page_id, site_id, item.url, norm_url, format!("Wayback Snapshot {}", item.timestamp), epoch_ms, epoch_ms],
        )?;

        // Query the actual page_id in case of conflict
        let actual_page_id: String = conn.query_row(
            "SELECT id FROM pages WHERE site_id = ?1 AND normalized_url = ?2",
            rusqlite::params![site_id, norm_url],
            |row| row.get(0),
        )?;

        // Insert Capture
        conn.execute(
            r#"
            INSERT INTO captures (
                id, page_id, job_id, captured_at, status_code, mime_type,
                warc_file, warc_offset, warc_length, capture_score
            ) VALUES (?1, ?2, 'wayback_import', ?3, ?4, ?5, ?6, ?7, ?8, 100.0)
            "#,
            rusqlite::params![
                cap_id, actual_page_id, epoch_ms, status as i32, item.mime_type,
                warc_rel_path, offset as i64, length as i64
            ],
        )?;

        // Record Wayback Capture entry
        conn.execute(
            r#"
            INSERT INTO wayback_captures (id, url, timestamp, status_code, mime_type, digest, length, imported)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)
            ON CONFLICT(id) DO UPDATE SET imported = 1
            "#,
            rusqlite::params![item.id, item.url, item.timestamp, item.status_code, item.mime_type, item.digest, item.length],
        )?;

        drop(conn);

        let html = String::from_utf8_lossy(&body);
        let clean_text = crate::capture::behavior::TextExtractor::extract_clean_text(&html);
        let title = crate::capture::behavior::TextExtractor::extract_title(&html)
            .unwrap_or_else(|| format!("Wayback Snapshot {}", item.timestamp));
        let _ = SearchEngine::index_page(
            db,
            &actual_page_id,
            &cap_id,
            &item.url,
            &title,
            &clean_text,
            "",
        );

        Ok(())
    }
}
