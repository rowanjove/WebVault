use anyhow::{Context, Result};
use chrono::Utc;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum WarcRecordType {
    WarcInfo,
    Request,
    Response,
    Metadata,
    Revisit,
    Resource,
}

impl WarcRecordType {
    pub fn as_str(&self) -> &'static str {
        match self {
            WarcRecordType::WarcInfo => "warcinfo",
            WarcRecordType::Request => "request",
            WarcRecordType::Response => "response",
            WarcRecordType::Metadata => "metadata",
            WarcRecordType::Revisit => "revisit",
            WarcRecordType::Resource => "resource",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "warcinfo" => Some(WarcRecordType::WarcInfo),
            "request" => Some(WarcRecordType::Request),
            "response" => Some(WarcRecordType::Response),
            "metadata" => Some(WarcRecordType::Metadata),
            "revisit" => Some(WarcRecordType::Revisit),
            "resource" => Some(WarcRecordType::Resource),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WarcRecord {
    pub record_type: WarcRecordType,
    pub record_id: String,
    pub date: String,
    pub target_uri: Option<String>,
    pub content_type: String,
    pub payload_digest: Option<String>,
    pub concurrent_to: Option<String>,
    pub extra_headers: HashMap<String, String>,
    pub content: Vec<u8>,
}

impl WarcRecord {
    pub fn new(record_type: WarcRecordType, content_type: &str, content: Vec<u8>) -> Self {
        let digest = format!("sha256:{}", hex::encode(Sha256::digest(&content)));
        Self {
            record_type,
            record_id: format!("<urn:uuid:{}>", Uuid::new_v4()),
            date: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            target_uri: None,
            content_type: content_type.to_string(),
            payload_digest: Some(digest),
            concurrent_to: None,
            extra_headers: HashMap::new(),
            content,
        }
    }

    pub fn with_target_uri(mut self, uri: &str) -> Self {
        self.target_uri = Some(uri.to_string());
        self
    }

    pub fn create_response_record(
        uri: &str,
        http_status: u16,
        status_text: &str,
        headers: &HashMap<String, String>,
        body: &[u8],
    ) -> Self {
        let mut http_msg = Vec::new();
        // HTTP Status line
        http_msg.extend_from_slice(format!("HTTP/1.1 {} {}\r\n", http_status, status_text).as_bytes());
        for (k, v) in headers {
            http_msg.extend_from_slice(format!("{}: {}\r\n", k, v).as_bytes());
        }
        http_msg.extend_from_slice(b"\r\n");
        http_msg.extend_from_slice(body);

        let mut rec = Self::new(WarcRecordType::Response, "application/http; msgtype=response", http_msg);
        rec.target_uri = Some(uri.to_string());
        rec
    }

    pub fn create_request_record(
        uri: &str,
        method: &str,
        headers: &HashMap<String, String>,
        body: &[u8],
    ) -> Self {
        let mut http_msg = Vec::new();
        let parsed = url::Url::parse(uri).ok();
        let path = parsed.as_ref().map(|u| u.path()).unwrap_or("/");
        let query = parsed.as_ref().and_then(|u| u.query()).map(|q| format!("?{}", q)).unwrap_or_default();
        
        http_msg.extend_from_slice(format!("{} {}{} HTTP/1.1\r\n", method, path, query).as_bytes());
        for (k, v) in headers {
            http_msg.extend_from_slice(format!("{}: {}\r\n", k, v).as_bytes());
        }
        http_msg.extend_from_slice(b"\r\n");
        http_msg.extend_from_slice(body);

        let mut rec = Self::new(WarcRecordType::Request, "application/http; msgtype=request", http_msg);
        rec.target_uri = Some(uri.to_string());
        rec
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut header = String::new();
        header.push_str("WARC/1.1\r\n");
        header.push_str(&format!("WARC-Type: {}\r\n", self.record_type.as_str()));
        header.push_str(&format!("WARC-Record-ID: {}\r\n", self.record_id));
        header.push_str(&format!("WARC-Date: {}\r\n", self.date));
        if let Some(ref uri) = self.target_uri {
            header.push_str(&format!("WARC-Target-URI: {}\r\n", uri));
        }
        if let Some(ref digest) = self.payload_digest {
            header.push_str(&format!("WARC-Payload-Digest: {}\r\n", digest));
        }
        if let Some(ref conc) = self.concurrent_to {
            header.push_str(&format!("WARC-Concurrent-To: {}\r\n", conc));
        }
        for (k, v) in &self.extra_headers {
            header.push_str(&format!("{}: {}\r\n", k, v));
        }
        header.push_str(&format!("Content-Type: {}\r\n", self.content_type));
        header.push_str(&format!("Content-Length: {}\r\n", self.content.len()));
        header.push_str("\r\n");

        let mut output = header.into_bytes();
        output.extend_from_slice(&self.content);
        output.extend_from_slice(b"\r\n\r\n");
        output
    }
}

pub struct WarcWriter {
    file_path: PathBuf,
    max_file_size: u64,
}

impl WarcWriter {
    pub const DEFAULT_MAX_FILE_SIZE: u64 = 2 * 1024 * 1024 * 1024; // 2GB

    pub fn new<P: AsRef<Path>>(file_path: P) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            max_file_size: Self::DEFAULT_MAX_FILE_SIZE,
        }
    }

    pub fn with_max_file_size(mut self, max_size: u64) -> Self {
        self.max_file_size = max_size;
        self
    }

    /// Finds the active file path to write to, considering max_file_size rollover.
    pub fn get_active_file_path(&self) -> PathBuf {
        let path = &self.file_path;
        if !path.exists() {
            return path.clone();
        }

        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return path.clone(),
        };

        if metadata.len() < self.max_file_size {
            return path.clone();
        }

        // File reached or exceeded max size, check rolled volumes: stem-0001.ext, stem-0002.ext...
        let parent = path.parent().unwrap_or_else(|| Path::new(""));
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("data.warc.gz");

        let (stem, ext) = if filename.ends_with(".warc.gz") {
            (&filename[..filename.len() - 8], "warc.gz")
        } else if filename.ends_with(".warc") {
            (&filename[..filename.len() - 5], "warc")
        } else if let Some(dot_idx) = filename.rfind('.') {
            (&filename[..dot_idx], &filename[dot_idx + 1..])
        } else {
            (filename, "warc.gz")
        };

        let mut idx = 1;
        loop {
            let candidate_name = format!("{}-{:04}.{}", stem, idx, ext);
            let candidate_path = parent.join(candidate_name);
            if !candidate_path.exists() {
                return candidate_path;
            }
            if let Ok(meta) = std::fs::metadata(&candidate_path) {
                if meta.len() < self.max_file_size {
                    return candidate_path;
                }
            }
            idx += 1;
        }
    }

    /// Appends record with rollover support, returning (active_file_path, offset, length)
    pub fn append_record_rolling(&self, record: &WarcRecord) -> Result<(PathBuf, u64, u64)> {
        let active_path = self.get_active_file_path();

        if let Some(parent) = active_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let serialized = record.serialize();
        let is_gz = active_path.to_string_lossy().ends_with(".gz");

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&active_path)
            .with_context(|| format!("Failed to open WARC file {:?}", active_path))?;

        let offset = file.seek(std::io::SeekFrom::End(0))?;

        if is_gz {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&serialized)?;
            let compressed = encoder.finish()?;
            file.write_all(&compressed)?;
            let length = compressed.len() as u64;
            Ok((active_path, offset, length))
        } else {
            file.write_all(&serialized)?;
            let length = serialized.len() as u64;
            Ok((active_path, offset, length))
        }
    }

    pub fn append_record(&self, record: &WarcRecord) -> Result<(u64, u64)> {
        let (_, offset, length) = self.append_record_rolling(record)?;
        Ok((offset, length))
    }
}

pub struct WarcReader;

impl WarcReader {
    pub fn read_record_at<P: AsRef<Path>>(file_path: P, offset: u64, length: u64) -> Result<WarcRecord> {
        let mut file = std::fs::File::open(file_path.as_ref())
            .with_context(|| format!("Failed to open WARC {:?}", file_path.as_ref()))?;

        let file_len = file.metadata()?.len();
        if offset >= file_len {
            anyhow::bail!("Offset {} exceeds WARC file length {}", offset, file_len);
        }

        file.seek(std::io::SeekFrom::Start(offset))?;
        let is_gz = file_path.as_ref().to_string_lossy().ends_with(".gz");

        let take_len = if length > 0 && offset + length <= file_len {
            length
        } else {
            file_len - offset
        };

        let raw_data = if is_gz {
            let mut decoder = GzDecoder::new(file.take(take_len));
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            decompressed
        } else {
            let mut buf = vec![0u8; take_len as usize];
            file.read_exact(&mut buf)?;
            buf
        };

        Self::parse_record(&raw_data)
    }

    fn find_crlf_crlf(data: &[u8]) -> Option<usize> {
        data.windows(4).position(|w| w == b"\r\n\r\n")
    }

    pub fn parse_record(data: &[u8]) -> Result<WarcRecord> {
        let header_end = Self::find_crlf_crlf(data).context("WARC record header end not found")?;
        let header_text = String::from_utf8_lossy(&data[..header_end]);
        let content_start = header_end + 4;

        let mut lines = header_text.lines();
        let first_line = lines.next().context("Empty WARC record")?;
        if !first_line.starts_with("WARC/1.1") && !first_line.starts_with("WARC/1.0") {
            anyhow::bail!("Invalid WARC version line: {}", first_line);
        }

        let mut record_type = WarcRecordType::Response;
        let mut record_id = String::new();
        let mut date = String::new();
        let mut target_uri = None;
        let mut content_type = "application/octet-stream".to_string();
        let mut payload_digest = None;
        let mut content_length = 0;
        let mut extra_headers = HashMap::new();

        for line in lines {
            if let Some((k, v)) = line.split_once(':') {
                let k = k.trim();
                let v = v.trim();
                match k.to_lowercase().as_str() {
                    "warc-type" => {
                        if let Some(t) = WarcRecordType::from_str(v) {
                            record_type = t;
                        }
                    }
                    "warc-record-id" => record_id = v.to_string(),
                    "warc-date" => date = v.to_string(),
                    "warc-target-uri" => target_uri = Some(v.to_string()),
                    "content-type" => content_type = v.to_string(),
                    "warc-payload-digest" => payload_digest = Some(v.to_string()),
                    "content-length" => content_length = v.parse::<usize>().unwrap_or(0),
                    _ => {
                        extra_headers.insert(k.to_string(), v.to_string());
                    }
                }
            }
        }

        let end_idx = (content_start + content_length).min(data.len());
        let content = data[content_start..end_idx].to_vec();

        Ok(WarcRecord {
            record_type,
            record_id,
            date,
            target_uri,
            content_type,
            payload_digest,
            concurrent_to: None,
            extra_headers,
            content,
        })
    }

    /// Extract HTTP payload (headers and body) from a WARC response record
    pub fn parse_http_response(content: &[u8]) -> Result<(u16, HashMap<String, String>, Vec<u8>)> {
        let header_end = Self::find_crlf_crlf(content).context("HTTP response header end not found")?;
        let header_text = String::from_utf8_lossy(&content[..header_end]);
        let body_start = header_end + 4;

        let mut lines = header_text.lines();
        let status_line = lines.next().context("Missing HTTP status line")?;
        let mut parts = status_line.split_whitespace();
        let _http_ver = parts.next();
        let status_code: u16 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(200);

        let mut headers = HashMap::new();
        for line in lines {
            if let Some((k, v)) = line.split_once(':') {
                headers.insert(k.trim().to_lowercase(), v.trim().to_string());
            }
        }

        let body = content[body_start..].to_vec();
        Ok((status_code, headers, body))
    }

    /// Walk a (possibly gzip-member-concatenated) WARC and yield (offset, length, record).
    pub fn for_each_record<P, F>(file_path: P, mut visit: F) -> Result<()>
    where
        P: AsRef<Path>,
        F: FnMut(u64, u64, WarcRecord) -> Result<()>,
    {
        let path = file_path.as_ref();
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to open WARC {:?}", path))?;
        if data.is_empty() {
            return Ok(());
        }

        if path.to_string_lossy().ends_with(".gz") {
            let mut offset = 0u64;
            while (offset as usize) < data.len() {
                let input = &data[offset as usize..];
                let mut decoder = flate2::bufread::GzDecoder::new(input);
                let mut decompressed = Vec::new();
                if decoder.read_to_end(&mut decompressed).is_err() {
                    break;
                }
                let unread = decoder.into_inner();
                let consumed = (input.len() - unread.len()) as u64;
                if consumed == 0 {
                    break;
                }
                if let Ok(rec) = Self::parse_record(&decompressed) {
                    visit(offset, consumed, rec)?;
                }
                offset += consumed;
            }
        } else {
            let mut offset = 0usize;
            while offset < data.len() {
                while offset < data.len() && matches!(data[offset], b'\r' | b'\n') {
                    offset += 1;
                }
                if offset >= data.len() {
                    break;
                }
                match Self::parse_record(&data[offset..]) {
                    Ok(rec) => {
                        let serialized = rec.serialize();
                        let length = serialized.len() as u64;
                        visit(offset as u64, length, rec)?;
                        offset += length as usize;
                    }
                    Err(_) => break,
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_warc_serialization_and_parse() {
        let body = b"<html><body>Hello WebVault!</body></html>".to_vec();
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "text/html; charset=utf-8".to_string());

        let record = WarcRecord::create_response_record(
            "https://example.com/test",
            200,
            "OK",
            &headers,
            &body,
        );

        let serialized = record.serialize();
        let parsed = WarcReader::parse_record(&serialized).expect("Failed to parse serialized record");

        assert_eq!(parsed.record_type, WarcRecordType::Response);
        assert_eq!(parsed.target_uri.as_deref(), Some("https://example.com/test"));

        let (status, resp_headers, parsed_body) = WarcReader::parse_http_response(&parsed.content).expect("Failed to parse http response");
        assert_eq!(status, 200);
        assert_eq!(resp_headers.get("content-type").map(|s| s.as_str()), Some("text/html; charset=utf-8"));
        assert_eq!(parsed_body, body);
    }

    #[test]
    fn test_warc_file_roundtrip() {
        let temp_dir = std::env::temp_dir().join(format!("webvault_test_{}", uuid::Uuid::new_v4().simple()));
        let warc_path = temp_dir.join("test.warc.gz");
        let writer = WarcWriter::new(&warc_path);

        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "text/plain".to_string());
        let body = b"Offline Archive Content in Gzip WARC".to_vec();

        let rec = WarcRecord::create_response_record("https://example.com/file", 200, "OK", &headers, &body);
        let (offset, length) = writer.append_record(&rec).expect("Failed to append WARC record");

        let read_rec = WarcReader::read_record_at(&warc_path, offset, length).expect("Failed to read record at offset");
        let (status, resp_headers, read_body) = WarcReader::parse_http_response(&read_rec.content).expect("Failed to parse HTTP");

        assert_eq!(status, 200);
        assert_eq!(resp_headers.get("content-type").map(|s| s.as_str()), Some("text/plain"));
        assert_eq!(read_body, body);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_warc_file_rollover() {
        let temp_dir = std::env::temp_dir().join(format!("webvault_rollover_test_{}", uuid::Uuid::new_v4().simple()));
        let base_warc = temp_dir.join("data.warc.gz");
        let writer = WarcWriter::new(&base_warc).with_max_file_size(300);

        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "text/plain".to_string());
        let rec = WarcRecord::create_response_record("https://example.com/p1", 200, "OK", &headers, b"Record 1 payload data long enough");

        let (path1, _off1, _len1) = writer.append_record_rolling(&rec).unwrap();
        assert_eq!(path1, base_warc);

        let rec2 = WarcRecord::create_response_record("https://example.com/p2", 200, "OK", &headers, b"Record 2 payload data long enough");
        let (path2, _off2, _len2) = writer.append_record_rolling(&rec2).unwrap();
        assert!(path2.to_string_lossy().contains("data-0001.warc.gz"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_warc_binary_payload_exact_match() {
        // Construct binary payload containing invalid UTF-8 sequences (like raw JPEG/PNG bytes)
        let binary_payload: Vec<u8> = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x80, 0xFE, 0xC0, 0xAF];
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "image/jpeg".to_string());
        headers.insert("X-Custom-Desc".to_string(), "测试中文与二进制分界".to_string());

        let rec = WarcRecord::create_response_record("https://example.com/image.jpg", 200, "OK", &headers, &binary_payload);
        let serialized = rec.serialize();

        let parsed = WarcReader::parse_record(&serialized).expect("Parse WARC record with binary payload");
        let (status, resp_headers, body) = WarcReader::parse_http_response(&parsed.content).expect("Parse HTTP response with binary payload");

        assert_eq!(status, 200);
        assert_eq!(resp_headers.get("content-type").map(|s| s.as_str()), Some("image/jpeg"));
        assert_eq!(body, binary_payload, "Binary payload must match byte-for-byte without UTF-8 corruption");
    }
}
