use crate::archive::warc::WarcReader;
use crate::capture::browser::BrowserFinder;
use crate::capture::cdp::CdpClient;
use crate::database::Database;
use anyhow::{Context, Result};
use base64::Engine;
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

pub struct SingleFileExporter;

impl SingleFileExporter {
    /// Export a captured page as a fully self-contained offline Single-File HTML
    pub fn export_single_file_html(db: &Database, capture_id: &str, output_path: &Path) -> Result<()> {
        let (capture, _resources, rendered_text) = db.get_capture_details(capture_id)?;

        // 1. Read main HTML body from WARC
        let mut html_body = String::new();
        if !capture.warc_file.is_empty() {
            let warc_path = db.base_dir.join(&capture.warc_file);
            if warc_path.exists() {
                if let Ok(record) = WarcReader::read_record_at(&warc_path, capture.warc_offset as u64, capture.warc_length as u64) {
                    if let Ok((_status, _headers, body)) = WarcReader::parse_http_response(&record.content) {
                        html_body = String::from_utf8_lossy(&body).to_string();
                    }
                }
            }
        }

        if html_body.is_empty() {
            // Fallback to basic HTML wrapping rendered text
            let title = capture.title.as_deref().unwrap_or("Offline WebVault Snapshot");
            let target_url = capture.url.as_deref().unwrap_or("");
            html_body = format!(
                r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>{}</title><style>body{{font-family:sans-serif;line-height:1.6;padding:2rem;max-width:800px;margin:auto;background:#fff;color:#111;}}</style></head><body><h1>{}</h1><p><small>Source: <a href="{}">{}</a></small></p><hr/><pre style="white-space:pre-wrap;">{}</pre></body></html>"#,
                title, title, target_url, target_url, rendered_text
            );
        }

        let mut res_by_url: HashMap<String, (String, Vec<u8>)> = HashMap::new();
        let conn = db.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT url, mime_type, warc_file, warc_offset, warc_length FROM resources WHERE capture_id = ?1"
        )?;
        let rows = stmt.query_map(rusqlite::params![capture_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?.unwrap_or_else(|| "application/octet-stream".to_string()),
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<i64>>(4)?.unwrap_or(0),
            ))
        })?;

        for r in rows.flatten() {
            let (url, mime, warc_file, warc_offset, warc_length) = r;
            if let Some(wf) = warc_file {
                let wp = db.base_dir.join(&wf);
                if wp.exists() {
                    if let Ok(rec) = WarcReader::read_record_at(&wp, warc_offset as u64, warc_length as u64) {
                        if let Ok((_, _, body)) = WarcReader::parse_http_response(&rec.content) {
                            res_by_url.insert(url, (mime, body));
                        }
                    }
                }
            }
        }

        let find_resource = |url_ref: &str| -> Option<(String, Vec<u8>)> {
            let mut candidates = vec![url_ref.to_string()];
            if url_ref.starts_with("//") {
                candidates.push(format!("https:{}", url_ref));
                candidates.push(format!("http:{}", url_ref));
            }
            let clean_no_query = url_ref.split('?').next().unwrap_or(url_ref);
            if clean_no_query != url_ref {
                candidates.push(clean_no_query.to_string());
            }
            let clean_no_at = clean_no_query.split('@').next().unwrap_or(clean_no_query);
            if clean_no_at != clean_no_query {
                candidates.push(clean_no_at.to_string());
            }

            for c in &candidates {
                if let Some(r) = res_by_url.get(c) {
                    return Some(r.clone());
                }
                for (key, val) in &res_by_url {
                    if key == c || key.ends_with(c) || c.ends_with(key) || key.contains(clean_no_at) {
                        return Some(val.clone());
                    }
                }
            }
            None
        };

        let processed_html = Self::inline_html_resources(&html_body, find_resource);

        // Inject no-referrer to keep any remaining external elements working without 403
        let safe_html = if let Ok(ref_re) = Regex::new(r#"(?i)<meta\b[^>]*\bname=["']referrer["'][^>]*>"#) {
            if ref_re.is_match(&processed_html) {
                ref_re.replace_all(&processed_html, r#"<meta name="referrer" content="no-referrer">"#).to_string()
            } else if processed_html.contains("<head>") {
                processed_html.replacen("<head>", "<head><meta name=\"referrer\" content=\"no-referrer\">", 1)
            } else {
                processed_html
            }
        } else {
            processed_html
        };

        let banner = format!(
            "<!-- WebVault Archive | Capture: {} | URL: {} | Time: {} | Exported: {} -->\n",
            capture_id,
            capture.url.as_deref().unwrap_or("unknown"),
            capture.captured_at,
            chrono::Utc::now().to_rfc3339()
        );

        let final_output = format!("{}{}", banner, safe_html);

        if let Some(parent) = output_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(output_path, final_output.as_bytes())
            .with_context(|| format!("Failed to write single-file HTML to {:?}", output_path))?;

        Ok(())
    }

    pub fn inline_html_resources(
        html_body: &str,
        find_resource: impl Fn(&str) -> Option<(String, Vec<u8>)>,
    ) -> String {
        // Inline Images (src="...")
        let img_regex = match Regex::new(r#"(?i)<img\b([^>]*)\bsrc=["']([^"']+)["']([^>]*)>"#) {
            Ok(r) => r,
            Err(_) => return html_body.to_string(),
        };
        let processed_images = img_regex.replace_all(html_body, |caps: &regex::Captures| {
            let before_attrs = &caps[1];
            let src = &caps[2];
            let after_attrs = &caps[3];

            if src.starts_with("data:") {
                return caps[0].to_string();
            }

            if let Some((mime, body)) = find_resource(src) {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&body);
                format!(r#"<img{}src="data:{};base64,{}"{}>"#, before_attrs, mime, b64, after_attrs)
            } else {
                caps[0].to_string()
            }
        }).to_string();

        // Inline CSS stylesheets (<link rel="stylesheet" href="...">)
        let link_css_regex = match Regex::new(r#"(?i)<link\b[^>]*\brel=["']stylesheet["'][^>]*\bhref=["']([^"']+)["'][^>]*>"#) {
            Ok(r) => r,
            Err(_) => return processed_images,
        };
        let processed_css = link_css_regex.replace_all(&processed_images, |caps: &regex::Captures| {
            let href = &caps[1];
            if let Some((_, body)) = find_resource(href) {
                let css_str = String::from_utf8_lossy(&body);
                format!("<style>/* inlined from {} */\n{}</style>", href, css_str)
            } else {
                caps[0].to_string()
            }
        }).to_string();

        // Restore canvas snapshots if present in the page
        if processed_css.contains("data-webvault-canvas-snapshot") {
            let canvas_restore_script = r#"<script>(() => {
                window.addEventListener('DOMContentLoaded', () => {
                    document.querySelectorAll('canvas[data-webvault-canvas-snapshot]').forEach(c => {
                        const s = c.getAttribute('data-webvault-canvas-snapshot');
                        if (s) {
                            const img = new Image();
                            img.onload = () => {
                                const ctx = c.getContext('2d');
                                if (ctx) ctx.drawImage(img, 0, 0, c.width, c.height);
                            };
                            img.src = s;
                        }
                    });
                });
            })();</script>"#;
            if processed_css.contains("<head>") {
                processed_css.replacen("<head>", &format!("<head>{}", canvas_restore_script), 1)
            } else {
                format!("{}{}", canvas_restore_script, processed_css)
            }
        } else {
            processed_css
        }
    }

    /// Export a captured page as high-resolution PDF via headless browser CDP Page.printToPDF
    pub async fn export_pdf(
        db: &Database,
        capture_id: &str,
        replay_port: u16,
        replay_token: &str,
        output_path: &Path,
    ) -> Result<()> {
        let browser_path = BrowserFinder::find_browser()
            .context("No supported Chromium browser found (Chrome/Edge) for PDF export")?;

        let port = BrowserFinder::find_available_port();
        let temp_dir = db.base_dir.join("temp");
        let browser_proc = BrowserFinder::launch(&browser_path, port, &temp_dir).await?;

        let replay_url = format!("http://127.0.0.1:{}/replay/{}?t={}", replay_port, capture_id, replay_token);
        let cdp = CdpClient::new(browser_proc.port);

        let pdf_bytes = cdp.print_to_pdf(&replay_url).await?;

        if let Some(parent) = output_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(output_path, &pdf_bytes)
            .with_context(|| format!("Failed to write PDF to {:?}", output_path))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_singlefile_inlining() {
        let input_html = r#"<html><head><link rel="stylesheet" href="/style.css"></head><body><img src="/logo.png" alt="logo"/></body></html>"#;
        let inlined = SingleFileExporter::inline_html_resources(input_html, |url| {
            if url == "/style.css" {
                Some(("text/css".to_string(), b"body { color: red; }".to_vec()))
            } else if url == "/logo.png" {
                Some(("image/png".to_string(), b"mock_png_bytes".to_vec()))
            } else {
                None
            }
        });

        assert!(inlined.contains("<style>/* inlined from /style.css */\nbody { color: red; }</style>"));
        assert!(inlined.contains(r#"src="data:image/png;base64,"#));
    }

    #[test]
    fn test_canvas_snapshot_inlining_and_restore() {
        let input_html = r#"<html><head></head><body><canvas data-webvault-canvas-snapshot="data:image/png;base64,mockdata"></canvas></body></html>"#;
        let inlined = SingleFileExporter::inline_html_resources(input_html, |_| None);

        assert!(inlined.contains("data-webvault-canvas-snapshot"));
        assert!(inlined.contains("getContext('2d')"));
        assert!(inlined.contains("drawImage"));
    }
}

