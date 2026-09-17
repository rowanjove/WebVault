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
                let out_path = dest_dir.join("imported.warc.gz");
                let mut out_file = File::create(&out_path)?;
                std::io::copy(&mut file, &mut out_file)?;
                extracted_warc = Some(out_path);
            }
        }

        let warc_path = extracted_warc.context("No WARC file found in WACZ archive")?;
        Ok(UnpackedWacz {
            warc_path,
            title,
            description,
            cdx_content,
            pages_jsonl,
        })
    }
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
}
