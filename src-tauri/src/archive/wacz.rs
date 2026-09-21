use anyhow::{Context, Result};
use chrono::Utc;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde_json::json;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

#[derive(Debug, Clone)]
pub struct UnpackedWacz {
    pub warc_path: PathBuf,
    pub extra_warcs: Vec<PathBuf>,
    pub title: String,
    pub description: String,
    pub cdx_content: Option<String>,
    pub pages_jsonl: Option<String>,
}

pub struct WaczPackage;

impl WaczPackage {
    /// Export a site or capture collection to a .wacz file following WACZ 1.1.1 spec
    pub fn create_wacz<P: AsRef<Path>>(
        warc_gz_path: P,
        output_wacz_path: P,
        title: &str,
        description: &str,
        cdx_content: &str,
        pages_jsonl: Option<&str>,
    ) -> Result<()> {
        let warc_gz_path = warc_gz_path.as_ref();
        let output_wacz_path = output_wacz_path.as_ref();

        if let Some(parent) = output_wacz_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let out_file = File::create(output_wacz_path)
            .with_context(|| format!("Failed to create WACZ at {:?}", output_wacz_path))?;
        let mut zip = ZipWriter::new(out_file);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        let now = Utc::now().to_rfc3339();
        let mut resources = vec![
            json!({
                "name": "data",
                "path": "archive/data.warc.gz",
                "mediatype": "application/warc"
            }),
            json!({
                "name": "indexes",
                "path": "indexes/index.cdx.gz",
                "mediatype": "application/x-gzip"
            }),
        ];

        if pages_jsonl.is_some() {
            resources.push(json!({
                "name": "pages",
                "path": "pages/pages.jsonl",
                "mediatype": "application/x-ndjson"
            }));
        }

        let datapackage = json!({
            "profile": "data-package",
            "wacz_version": "1.1.1",
            "software": "WebVault 1.0.0",
            "title": title,
            "description": description,
            "created": now,
            "resources": resources
        });
        zip.start_file("datapackage.json", options)?;
        zip.write_all(serde_json::to_string_pretty(&datapackage)?.as_bytes())?;

        // 1. Write gzipped CDX/CDXJ index
        zip.start_file("indexes/index.cdx.gz", options)?;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(cdx_content.as_bytes())?;
        let compressed_cdx = encoder.finish()?;
        zip.write_all(&compressed_cdx)?;

        // 2. Write pages.jsonl if present
        if let Some(pages) = pages_jsonl {
            zip.start_file("pages/pages.jsonl", options)?;
            zip.write_all(pages.as_bytes())?;
        }

        // 3. Write raw WARC archive
        zip.start_file("archive/data.warc.gz", SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored))?;
        let mut warc_file = File::open(warc_gz_path)
            .with_context(|| format!("Failed to open WARC {:?}", warc_gz_path))?;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let n = warc_file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            zip.write_all(&buffer[..n])?;
        }

        if let Some(dir) = warc_gz_path.parent() {
            let primary = warc_gz_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if name == primary {
                        continue;
                    }
                    if name.ends_with(".warc.gz") || name.ends_with(".warc") {
                        zip.start_file(
                            format!("archive/{}", name),
                            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
                        )?;
                        let mut extra = File::open(entry.path())?;
                        loop {
                            let n = extra.read(&mut buffer)?;
                            if n == 0 {
                                break;
                            }
                            zip.write_all(&buffer[..n])?;
                        }
                    }
                }
            }
        }

        zip.finish()?;
        Ok(())
    }

    /// Unpack a .wacz archive and extract the warc.gz and metadata
    pub fn unpack_wacz<P1: AsRef<Path>, P2: AsRef<Path>>(
        wacz_path: P1,
        dest_dir: P2,
    ) -> Result<UnpackedWacz> {
        let wacz_path = wacz_path.as_ref();
        let dest_dir = dest_dir.as_ref();
        std::fs::create_dir_all(dest_dir)?;

        let file = File::open(wacz_path)?;
        let mut archive = ZipArchive::new(file)?;

        let mut extracted_warc = None;
        let mut extra_warcs = Vec::new();
        let mut title = "Imported WACZ".to_string();
        let mut description = String::new();
        let mut cdx_content = None;
        let mut pages_jsonl = None;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();

            if name == "datapackage.json" {
                let mut content = String::new();
                file.read_to_string(&mut content)?;
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(t) = v["title"].as_str() {
                        title = t.to_string();
                    }
                    if let Some(d) = v["description"].as_str() {
                        description = d.to_string();
                    }
                }
            } else if name == "pages/pages.jsonl" || name.ends_with("/pages.jsonl") || name == "pages.jsonl" {
                let mut content = String::new();
                if file.read_to_string(&mut content).is_ok() {
                    pages_jsonl = Some(content);
                }
            } else if name == "indexes/index.cdx.gz" || name.ends_with("/index.cdx.gz") {
                let mut gz_bytes = Vec::new();
                if file.read_to_end(&mut gz_bytes).is_ok() {
                    let mut decoder = GzDecoder::new(&gz_bytes[..]);
                    let mut text = String::new();
                    if decoder.read_to_string(&mut text).is_ok() {
                        cdx_content = Some(text);
                    }
                }
            } else if name == "indexes/index.cdx" || name.ends_with("/index.cdx") || name.ends_with(".cdx") || name.ends_with(".cdxj") {
                let mut content = String::new();
                if file.read_to_string(&mut content).is_ok() {
                    cdx_content = Some(content);
                }
            } else if name.ends_with(".warc.gz") || name.ends_with(".warc") {
                let fname = std::path::Path::new(&name)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "imported.warc.gz".to_string());
                let out_path = dest_dir.join(&fname);
                let mut out_file = File::create(&out_path)?;
                std::io::copy(&mut file, &mut out_file)?;
                if extracted_warc.is_none() || fname == "data.warc.gz" || fname == "imported.warc.gz" {
                    if let Some(prev) = extracted_warc.take() {
                        if prev != out_path {
                            extra_warcs.push(prev);
                        }
                    }
                    extracted_warc = Some(out_path);
                } else {
                    extra_warcs.push(out_path);
                }
            }
        }

        let warc_path = extracted_warc.context("No WARC file found in WACZ archive")?;
        Ok(UnpackedWacz {
            warc_path,
            extra_warcs,
            title,
            description,
            cdx_content,
            pages_jsonl,
        })
    }

    pub fn import_into_db(
        db: &crate::database::Database,
        wacz_path: &Path,
    ) -> Result<crate::database::models::Site> {
        let temp_dest = db.base_dir.join("temp").join(format!("imp_{}", uuid::Uuid::new_v4().simple()));
        let unpacked = Self::unpack_wacz(wacz_path, &temp_dest)?;
        let normalizer = crate::crawler::normalizer::UrlNormalizer::new();
        let mut initial_root_url = String::new();
        let mut titles = std::collections::HashMap::<String, String>::new();

        if let Some(ref pj) = unpacked.pages_jsonl {
            for line in pj.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                    let url = v["url"].as_str().unwrap_or_default();
                    let title = v["title"].as_str().unwrap_or_default();
                    if url.is_empty() {
                        continue;
                    }
                    if initial_root_url.is_empty() {
                        initial_root_url = url.to_string();
                    }
                    if !title.is_empty() {
                        let norm = normalizer.normalize(url).unwrap_or_else(|_| url.to_string());
                        titles.insert(norm, title.to_string());
                    }
                }
            }
        }

        if initial_root_url.is_empty() {
            initial_root_url = format!("https://imported.local/{}", uuid::Uuid::new_v4().simple());
        }

        let site = db.create_site(&unpacked.title, &initial_root_url)?;
        let site_warc_rel = format!("archives/{}/data.warc.gz", site.id);
        let site_warc_full = db.base_dir.join(&site_warc_rel);
        if let Some(p) = site_warc_full.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::copy(&unpacked.warc_path, &site_warc_full)?;
        for extra in &unpacked.extra_warcs {
            if let Some(name) = extra.file_name() {
                let dest = site_warc_full.parent().unwrap_or(db.base_dir.as_path()).join(name);
                if dest != site_warc_full {
                    let _ = std::fs::copy(extra, dest);
                }
            }
        }
        index_imported_warc(db, &site.id, &site_warc_rel, &site_warc_full, &titles)?;
        for extra in &unpacked.extra_warcs {
            if extra.exists() {
                if let Some(name) = extra.file_name() {
                    let dest = site_warc_full.parent().unwrap_or(db.base_dir.as_path()).join(name);
                    if dest != site_warc_full {
                        let rel = format!("archives/{}/{}", site.id, name.to_string_lossy());
                        let _ = index_imported_warc(db, &site.id, &rel, &dest, &titles);
                    }
                }
            }
        }
        let _ = std::fs::remove_dir_all(&temp_dest);
        Ok(site)
    }
}

fn index_imported_warc(
    db: &crate::database::Database,
    site_id: &str,
    warc_rel: &str,
    warc_full: &Path,
    titles: &std::collections::HashMap<String, String>,
) -> Result<()> {
    use crate::archive::warc::{WarcReader, WarcRecordType};
    let normalizer = crate::crawler::normalizer::UrlNormalizer::new();
    let now = chrono::Utc::now().timestamp_millis();
    let mut last_html_cap: Option<String> = None;

    WarcReader::for_each_record(warc_full, |offset, length, rec| {
        if rec.record_type != WarcRecordType::Response {
            return Ok(());
        }
        let url = match rec.target_uri {
            Some(u) if !u.is_empty() => u,
            _ => return Ok(()),
        };
        let (status, headers, body) = match WarcReader::parse_http_response(&rec.content) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let mime = headers
            .get("content-type")
            .cloned()
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let norm_url = normalizer.normalize(&url).unwrap_or_else(|_| url.clone());
        let is_html = mime.contains("text/html") || mime.contains("application/xhtml");

        if is_html {
            let title = titles
                .get(&norm_url)
                .cloned()
                .or_else(|| {
                    crate::capture::behavior::TextExtractor::extract_title(&String::from_utf8_lossy(&body))
                })
                .unwrap_or_else(|| url.clone());
            let clean = crate::capture::behavior::TextExtractor::extract_clean_text(&String::from_utf8_lossy(&body));
            let page_id = format!("page_{}", uuid::Uuid::new_v4().simple());
            let cap_id = format!("cap_{}", uuid::Uuid::new_v4().simple());
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
                let actual_page_id: String = conn
                    .query_row(
                        "SELECT id FROM pages WHERE site_id = ?1 AND normalized_url = ?2",
                        rusqlite::params![site_id, norm_url],
                        |r| r.get(0),
                    )
                    .unwrap_or_else(|_| page_id.clone());
                conn.execute(
                    r#"
                    INSERT INTO captures (
                        id, page_id, job_id, captured_at, status_code, mime_type,
                        warc_file, warc_offset, warc_length, capture_score
                    ) VALUES (?1, ?2, 'wacz_import', ?3, ?4, ?5, ?6, ?7, ?8, 100.0)
                    "#,
                    rusqlite::params![
                        cap_id,
                        actual_page_id,
                        now,
                        status as i32,
                        mime,
                        warc_rel,
                        offset as i64,
                        length as i64
                    ],
                )?;
                actual_page_id
            };
            let _ = crate::search::SearchEngine::index_page(
                db,
                &actual_page_id,
                &cap_id,
                &url,
                &title,
                &clean,
                "",
            );
            last_html_cap = Some(cap_id);
        } else if let Some(ref cap_id) = last_html_cap {
            let conn = db.lock_conn()?;
            let res_id = format!("res_{}", uuid::Uuid::new_v4().simple());
            let _ = conn.execute(
                r#"
                INSERT INTO resources (
                    id, capture_id, url, normalized_url, mime_type, status_code, size,
                    warc_file, warc_offset, warc_length, resource_type
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'asset')
                "#,
                rusqlite::params![
                    res_id,
                    cap_id,
                    url,
                    norm_url,
                    mime,
                    status as i32,
                    body.len() as i64,
                    warc_rel,
                    offset as i64,
                    length as i64
                ],
            );
        }
        Ok(())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wacz_roundtrip_with_indexes_and_pages() {
        let temp_dir = std::env::temp_dir().join(format!("test_wacz_{}", uuid::Uuid::new_v4().simple()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let warc_path = temp_dir.join("sample.warc.gz");
        std::fs::write(&warc_path, b"FAKE_WARC_DATA_BYTES").unwrap();

        let wacz_path = temp_dir.join("test.wacz");
        let pages_jsonl = "{\"id\":\"p1\",\"url\":\"https://example.com/\",\"title\":\"Home\",\"ts\":\"2026-01-01T00:00:00Z\"}\n";
        let cdx_content = "https://example.com/ 20260101000000 {\"url\": \"https://example.com/\", \"mime\": \"text/html\"}\n";

        WaczPackage::create_wacz(
            &warc_path,
            &wacz_path,
            "Sample Title",
            "Sample Description",
            cdx_content,
            Some(pages_jsonl),
        ).expect("Must create WACZ");

        let dest_dir = temp_dir.join("unpacked");
        let unpacked = WaczPackage::unpack_wacz(&wacz_path, &dest_dir).expect("Must unpack WACZ");

        assert_eq!(unpacked.title, "Sample Title");
        assert!(unpacked.warc_path.exists());
        assert!(unpacked.pages_jsonl.unwrap().contains("https://example.com/"));
        assert!(unpacked.cdx_content.unwrap().contains("text/html"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_wacz_import_indexes_warc_offsets() {
        use crate::archive::warc::{WarcRecord, WarcWriter};
        use crate::database::Database;
        use std::collections::HashMap;

        let temp = std::env::temp_dir().join(format!("wacz_imp_{}", uuid::Uuid::new_v4().simple()));
        let src = temp.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let warc_path = src.join("data.warc.gz");
        let writer = WarcWriter::new(&warc_path);
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "text/html; charset=utf-8".to_string());
        let html1 = WarcRecord::create_response_record(
            "https://example.com/",
            200,
            "OK",
            &headers,
            b"<html><title>Home</title><body>one</body></html>",
        );
        let html2 = WarcRecord::create_response_record(
            "https://example.com/about",
            200,
            "OK",
            &headers,
            b"<html><title>About</title><body>two</body></html>",
        );
        let css = {
            let mut h = HashMap::new();
            h.insert("Content-Type".to_string(), "text/css".to_string());
            WarcRecord::create_response_record("https://example.com/app.css", 200, "OK", &h, b"body{color:red}")
        };
        let (p1, o1, l1) = writer.append_record_rolling(&html1).unwrap();
        let _ = (p1, o1, l1);
        writer.append_record_rolling(&css).unwrap();
        let (_p2, o2, l2) = writer.append_record_rolling(&html2).unwrap();
        assert!(l2 > 0);
        assert!(o2 > 0);

        let wacz_path = src.join("pack.wacz");
        WaczPackage::create_wacz(&warc_path, &wacz_path, "Imported", "", "", None).unwrap();

        let dest = temp.join("dest");
        let db = Database::init(&dest).unwrap();
        let site = WaczPackage::import_into_db(&db, &wacz_path).unwrap();
        let conn = db.lock_conn().unwrap();
        let cap_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM captures c JOIN pages p ON c.page_id = p.id WHERE p.site_id = ?1",
                rusqlite::params![site.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cap_count, 2);
        let zero_len: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM captures c JOIN pages p ON c.page_id = p.id WHERE p.site_id = ?1 AND c.warc_length = 0",
                rusqlite::params![site.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(zero_len, 0);
        let res_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM resources", [], |r| r.get(0))
            .unwrap();
        assert!(res_count >= 1);
        let _ = std::fs::remove_dir_all(&temp);
    }
}
