use crate::archive::singlefile::SingleFileExporter;
use crate::archive::warc::WarcReader;
use crate::database::Database;
use crate::search::SearchEngine;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct ReplayState {
    pub db: Database,
    pub port: u16,
}

pub struct ReplayServer {
    pub port: u16,
}

impl ReplayServer {
    pub async fn start(db: Database) -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();

        let state = ReplayState { db, port };

        let app = Router::new()
            .route("/replay/{capture_id}", get(replay_main_page))
            .route("/replay/{capture_id}/{*url}", get(replay_sub_resource))
            .route("/screenshot/{capture_id}", get(serve_screenshot))
            // REST API endpoints
            .route("/api/v1/health", get(api_health))
            .route("/api/v1/stats", get(api_stats))
            .route("/api/v1/sites", get(api_sites))
            .route("/api/v1/pages", get(api_pages))
            .route("/api/v1/captures", get(api_captures))
            .route("/api/v1/captures/{id}", get(api_capture_detail))
            .route("/api/v1/search", get(api_search))
            .route("/api/v1/export", post(api_export))
            .fallback(get(fallback_replay_resource))
            .layer(CorsLayer::permissive())
            .with_state(state);

        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("Replay & API server error: {}", e);
            }
        });

        Ok(Self { port })
    }
}

use regex::Regex;

async fn replay_main_page(
    State(state): State<ReplayState>,
    Path(capture_id): Path<String>,
) -> Response {
    let conn = match state.db.lock_conn() {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database locked").into_response(),
    };

    let cap_info: rusqlite::Result<(String, i64, i64, String, String)> = conn.query_row(
        "SELECT c.warc_file, c.warc_offset, c.warc_length, c.mime_type, p.url 
         FROM captures c JOIN pages p ON c.page_id = p.id WHERE c.id = ?1",
        rusqlite::params![capture_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    );

    let (warc_file, warc_offset, warc_length, default_mime, target_url) = match cap_info {
        Ok(info) => info,
        Err(_) => return (StatusCode::NOT_FOUND, "Capture not found").into_response(),
    };

    let warc_path = state.db.base_dir.join(&warc_file);
    let record = match WarcReader::read_record_at(&warc_path, warc_offset as u64, warc_length as u64) {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed reading WARC: {}", e)).into_response(),
    };

    let (status, headers, body) = match WarcReader::parse_http_response(&record.content) {
        Ok(parsed) => parsed,
        Err(_) => (200, HashMap::new(), record.content),
    };

    let mime = headers
        .get("content-type")
        .cloned()
        .unwrap_or(default_mime);

    let final_body = if mime.contains("text/html") {
        let text = String::from_utf8_lossy(&body);
        rewrite_html_for_replay(&capture_id, &target_url, &text).into_bytes()
    } else {
        body
    };

    let mut response_headers = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(&mime) {
        response_headers.insert(header::CONTENT_TYPE, v);
    }
    // Security Sandbox headers: isolate from parent Tauri window, allow same-origin for local SPA state
    response_headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("sandbox allow-scripts allow-same-origin allow-forms allow-popups;"),
    );
    // Anti-hotlinking defense: suppress Referer header so CDNs with anti-scraping won't 403 block images
    response_headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );

    let status_code = StatusCode::from_u16(status).unwrap_or(StatusCode::OK);
    (status_code, response_headers, final_body).into_response()
}

/// Rewrite HTML content for accurate archive replay
fn rewrite_html_for_replay(capture_id: &str, target_url: &str, html: &str) -> String {
    let mut rewritten = html.to_string();

    // 1. Enforce no-referrer meta policy to bypass CDN hotlink protection
    if let Ok(ref_re) = Regex::new(r#"(?i)<meta\b[^>]*\bname=["']referrer["'][^>]*>"#) {
        if ref_re.is_match(&rewritten) {
            rewritten = ref_re.replace_all(&rewritten, r#"<meta name="referrer" content="no-referrer">"#).to_string();
        }
    }

    // 2. Inject Sandbox client shim, base tag and Fetch/XHR proxy
    let replay_prefix = format!("/replay/{}/", capture_id);
    let shim_script = format!(
        r#"<meta name="referrer" content="no-referrer"><base href="{0}"><script>(()=>{{const p='{0}';try{{window.localStorage.getItem('_test');}}catch(e){{const m={{}};const s={{getItem:k=>k in m?m[k]:null,setItem:(k,v)=>{{m[k]=String(v);}},removeItem:k=>{{delete m[k];}},clear:()=>{{for(let k in m)delete m[k];}},key:i=>Object.keys(m)[i]||null,get length(){{return Object.keys(m).length;}}}};try{{Object.defineProperty(window,'localStorage',{{value:s,configurable:true}});}}catch(er){{}}try{{Object.defineProperty(window,'sessionStorage',{{value:s,configurable:true}});}}catch(er){{}}}}const of=window.fetch;window.fetch=function(u,i){{if(typeof u==='string'){{if(u.startsWith('//'))u=p+'https:'+u;else if(u.startsWith('http://')||u.startsWith('https://'))u=p+u;else if(u.startsWith('/')&&!u.startsWith('/replay/'))u=p+u.slice(1);}}return of.call(this,u,i);}};const ox=XMLHttpRequest.prototype.open;XMLHttpRequest.prototype.open=function(m,u,...r){{if(typeof u==='string'){{if(u.startsWith('//'))u=p+'https:'+u;else if(u.startsWith('http://')||u.startsWith('https://'))u=p+u;else if(u.startsWith('/')&&!u.startsWith('/replay/'))u=p+u.slice(1);}}return ox.call(this,m,u,...r);}};window.addEventListener('DOMContentLoaded',()=>{{document.querySelectorAll('canvas[data-webvault-canvas-snapshot]').forEach(c=>{{const s=c.getAttribute('data-webvault-canvas-snapshot');if(s){{const img=new Image();img.onload=()=>{{const ctx=c.getContext('2d');if(ctx)ctx.drawImage(img,0,0,c.width,c.height);}};img.src=s;}}}});}});}})();</script>"#,
        replay_prefix
    );

    let head_re = Regex::new(r#"(?i)<head\b[^>]*>"#).unwrap();
    if let Some(mat) = head_re.find(&rewritten) {
        let insert_pos = mat.end();
        rewritten.insert_str(insert_pos, &shim_script);
    } else {
        rewritten = format!("{}{}", shim_script, rewritten);
    }

    // 3. Rewrite asset attributes (src, href, data-src, data-original, data-bg) to route through replay proxy
    let parsed_base = url::Url::parse(target_url).ok();

    let rewrite_single_url = |val: &str| -> String {
        let trimmed = val.trim();
        if trimmed.is_empty() || trimmed.starts_with("data:") || trimmed.starts_with("javascript:") || trimmed.starts_with('#') {
            return trimmed.to_string();
        }
        if trimmed.starts_with(&replay_prefix) {
            return trimmed.to_string();
        }
        if let Some(ref base) = parsed_base {
            if let Ok(joined) = base.join(trimmed) {
                let s = joined.to_string();
                if s.starts_with("http://") || s.starts_with("https://") {
                    return format!("{}{}", replay_prefix, s);
                }
            }
        }
        if trimmed.starts_with("//") {
            return format!("{}https:{}", replay_prefix, trimmed);
        }
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            return format!("{}{}", replay_prefix, trimmed);
        }
        trimmed.to_string()
    };

    // Rewrite src, data-src, data-original, data-lazy-src on media/script elements
    if let Ok(attr_re) = Regex::new(r#"(?i)\b(src|data-src|data-original|data-lazy-src|data-bg)=["']([^"']+)["']"#) {
        rewritten = attr_re.replace_all(&rewritten, |caps: &regex::Captures| {
            let attr = &caps[1];
            let raw_url = &caps[2];
            let new_url = rewrite_single_url(raw_url);
            format!(r#"{}="{}""#, attr, new_url)
        }).to_string();
    }

    // Rewrite srcset on img and source elements for responsive layouts
    if let Ok(srcset_re) = Regex::new(r#"(?i)\bsrcset=["']([^"']+)["']"#) {
        rewritten = srcset_re.replace_all(&rewritten, |caps: &regex::Captures| {
            let content = &caps[1];
            let items: Vec<String> = content.split(',')
                .map(|item| {
                    let parts: Vec<&str> = item.trim().split_whitespace().collect();
                    if parts.is_empty() {
                        item.to_string()
                    } else {
                        let new_u = rewrite_single_url(parts[0]);
                        if parts.len() > 1 {
                            format!("{} {}", new_u, parts[1..].join(" "))
                        } else {
                            new_u
                        }
                    }
                })
                .collect();
            format!(r#"srcset="{}""#, items.join(", "))
        }).to_string();
    }

    // Rewrite link stylesheet/icon hrefs
    if let Ok(link_re) = Regex::new(r#"(?i)<link\b([^>]*)\bhref=["']([^"']+)["']([^>]*)>"#) {
        rewritten = link_re.replace_all(&rewritten, |caps: &regex::Captures| {
            let before = &caps[1];
            let raw_href = &caps[2];
            let after = &caps[3];
            let new_href = rewrite_single_url(raw_href);
            format!(r#"<link{}href="{}"{}>"#, before, new_href, after)
        }).to_string();
    }

    // Rewrite CSS url(...) background-images and web fonts in <style> or inline styles
    if let Ok(css_url_re) = Regex::new(r#"(?i)url\(\s*['"]?((?:https?://|//|/|\./|\.\./)[^'")\s]+)['"]?\s*\)"#) {
        rewritten = css_url_re.replace_all(&rewritten, |caps: &regex::Captures| {
            let raw_url = &caps[1];
            let new_url = rewrite_single_url(raw_url);
            format!("url('{}')", new_url)
        }).to_string();
    }

    rewritten
}

/// Normalizes resource URL from request path, handling URL percent-decoding and protocol restoring
fn normalize_requested_resource_url(resource_url: &str) -> String {
    let mut clean = resource_url.trim_start_matches('/').to_string();

    // Decode percent-encoded URLs if browser encoded colons/slashes (%3A, %2F)
    if clean.contains('%') {
        let bytes = clean.as_bytes();
        let mut decoded = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' && i + 2 < bytes.len() {
                if let Ok(val) = u8::from_str_radix(std::str::from_utf8(&bytes[i+1..i+3]).unwrap_or(""), 16) {
                    decoded.push(val);
                    i += 3;
                    continue;
                }
            }
            decoded.push(bytes[i]);
            i += 1;
        }
        clean = String::from_utf8_lossy(&decoded).to_string();
    }

    if clean.starts_with("https:/") && !clean.starts_with("https://") {
        format!("https://{}", &clean[7..])
    } else if clean.starts_with("http:/") && !clean.starts_with("http://") {
        format!("http://{}", &clean[6..])
    } else {
        clean
    }
}

async fn replay_sub_resource(
    State(state): State<ReplayState>,
    Path((capture_id, raw_resource_url)): Path<(String, String)>,
) -> Response {
    let target_url = normalize_requested_resource_url(&raw_resource_url);
    if target_url.trim().is_empty() || target_url == "/" {
        return replay_main_page(State(state), Path(capture_id)).await;
    }

    // Multi-tier candidate search:
    // 1. Exact match
    // 2. Stripped query params
    // 3. Stripped '@' image resizing suffixes (e.g. Bilibili @672w_378h.avif)
    // 4. Path suffix match
    let url_no_query = target_url.split('?').next().unwrap_or(&target_url).to_string();
    let url_no_at = url_no_query.split('@').next().unwrap_or(&url_no_query).to_string();
    let at_pattern = format!("{}%", url_no_at);
    let path_suffix = if let Ok(u) = url::Url::parse(&target_url) {
        u.path().to_string()
    } else {
        format!("%{}", target_url.trim_start_matches('/'))
    };
    let path_suffix_query = format!("%{}", path_suffix.trim_start_matches('/'));

    let res_info: rusqlite::Result<(String, i64, Option<i64>, String, i32)> = {
        let conn = match state.db.lock_conn() {
            Ok(c) => c,
            Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
        };

        conn.query_row(
            r#"
            SELECT r.warc_file, r.warc_offset, r.warc_length, r.mime_type, r.status_code
            FROM resources r
            WHERE r.capture_id = ?1 AND (
                r.url = ?2 OR r.normalized_url = ?2
                OR r.url = ?3 OR r.normalized_url = ?3
                OR r.url LIKE ?4 OR r.normalized_url LIKE ?4
                OR r.url LIKE ?5 OR r.normalized_url LIKE ?5
            )
            ORDER BY r.size DESC
            LIMIT 1
            "#,
            rusqlite::params![capture_id, target_url, url_no_query, at_pattern, path_suffix_query],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
    };

    if let Ok((warc_file, warc_offset, warc_length, mut mime, _status_code)) = res_info {
        let lower_url = target_url.to_lowercase();
        if lower_url.ends_with(".m3u8") {
            mime = "application/vnd.apple.mpegurl".to_string();
        } else if lower_url.ends_with(".ts") {
            mime = "video/mp2t".to_string();
        } else if lower_url.ends_with(".mpd") {
            mime = "application/dash+xml".to_string();
        } else if lower_url.ends_with(".m4s") {
            mime = "video/iso.segment".to_string();
        } else if lower_url.ends_with(".css") {
            mime = "text/css; charset=utf-8".to_string();
        } else if lower_url.ends_with(".avif") {
            mime = "image/avif".to_string();
        } else if lower_url.ends_with(".webp") {
            mime = "image/webp".to_string();
        } else if lower_url.ends_with(".svg") {
            mime = "image/svg+xml".to_string();
        } else if lower_url.ends_with(".woff2") {
            mime = "font/woff2".to_string();
        } else if lower_url.ends_with(".woff") {
            mime = "font/woff".to_string();
        } else if lower_url.ends_with(".ttf") {
            mime = "font/ttf".to_string();
        } else if lower_url.ends_with(".js") || lower_url.ends_with(".mjs") {
            mime = "application/javascript; charset=utf-8".to_string();
        } else if lower_url.ends_with(".json") {
            mime = "application/json; charset=utf-8".to_string();
        }

        let warc_path = state.db.base_dir.join(&warc_file);
        let record_len = warc_length.unwrap_or(0) as u64;
        if let Ok(record) = WarcReader::read_record_at(&warc_path, warc_offset as u64, record_len) {
            if let Ok((status, headers, body)) = WarcReader::parse_http_response(&record.content) {
                let mut response_headers = HeaderMap::new();
                if let Ok(v) = HeaderValue::from_str(&mime) {
                    response_headers.insert(header::CONTENT_TYPE, v);
                }
                for (k, v) in headers {
                    let k_lower = k.to_lowercase();
                    // Strip encoding and length headers because CDP getResponseBody provides already-decompressed body!
                    if k_lower == "content-encoding"
                        || k_lower == "content-length"
                        || k_lower == "transfer-encoding"
                        || k_lower == "content-security-policy"
                    {
                        continue;
                    }
                    if let Ok(name) = HeaderName::from_bytes(k.as_bytes()) {
                        if let Ok(val) = HeaderValue::from_str(&v) {
                            response_headers.insert(name, val);
                        }
                    }
                }
                response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
                response_headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
                response_headers.insert(HeaderName::from_static("x-webvault-source"), HeaderValue::from_static("archive"));
                let st = StatusCode::from_u16(status).unwrap_or(StatusCode::OK);
                return (st, response_headers, body).into_response();
            }
        }
    }

    // Transparent online proxy fallback if resource is an absolute URL not yet in WARC
    if target_url.starts_with("http://") || target_url.starts_with("https://") {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build();
        if let Ok(cli) = client {
            // First attempt: no referrer to bypass hotlinking protection
            let fetch_res = cli.get(&target_url)
                .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
                .header("Referer", "")
                .send()
                .await;

            // If 403 blocked, retry with target origin as Referer
            let resp = match fetch_res {
                Ok(r) if r.status() == reqwest::StatusCode::FORBIDDEN => {
                    let origin_ref = url::Url::parse(&target_url).ok()
                        .map(|u| format!("{}://{}/", u.scheme(), u.host_str().unwrap_or_default()))
                        .unwrap_or_default();
                    cli.get(&target_url)
                        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
                        .header("Referer", origin_ref)
                        .send()
                        .await
                        .ok()
                }
                Ok(r) => Some(r),
                Err(_) => None,
            };

            if let Some(resp) = resp {
                let status = resp.status();
                let mut response_headers = HeaderMap::new();
                if let Some(ct) = resp.headers().get(header::CONTENT_TYPE) {
                    response_headers.insert(header::CONTENT_TYPE, ct.clone());
                }
                response_headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
                response_headers.insert(HeaderName::from_static("x-webvault-source"), HeaderValue::from_static("live-fallback"));
                if let Ok(bytes) = resp.bytes().await {
                    return (StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK), response_headers, bytes).into_response();
                }
            }
        }
    }

    (StatusCode::NOT_FOUND, "Archived resource not found").into_response()
}

async fn serve_screenshot(
    State(state): State<ReplayState>,
    Path(capture_id): Path<String>,
) -> Response {
    let conn = match state.db.lock_conn() {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    };

    let ss_path: rusqlite::Result<Option<String>> = conn.query_row(
        "SELECT screenshot_path FROM captures WHERE id = ?1",
        rusqlite::params![capture_id],
        |row| row.get(0),
    );

    if let Ok(Some(path_str)) = ss_path {
        let full_path = state.db.base_dir.join(&path_str);
        if let Ok(bytes) = std::fs::read(&full_path) {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
            return (StatusCode::OK, headers, bytes).into_response();
        }
    }

    (StatusCode::NOT_FOUND, "Screenshot not found").into_response()
}

async fn fallback_replay_resource(
    State(state): State<ReplayState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> Response {
    if let Some(referer) = headers.get(header::REFERER).and_then(|r| r.to_str().ok()) {
        if let Some(pos) = referer.find("/replay/") {
            let rest = &referer[pos + 8..];
            let capture_id = rest.split('/').next().unwrap_or_default();
            if !capture_id.is_empty() {
                let resource_path = uri.path().trim_start_matches('/');
                return replay_sub_resource(
                    State(state),
                    Path((capture_id.to_string(), resource_path.to_string())),
                ).await;
            }
        }
    }
    (StatusCode::NOT_FOUND, "Resource not found in archive").into_response()
}

// REST API handlers

async fn api_health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "service": "WebVault Local Web Time Machine",
        "version": "0.1.0"
    }))
}

async fn api_stats(State(state): State<ReplayState>) -> Response {
    let conn = match state.db.lock_conn() {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database locked").into_response(),
    };

    let site_count: i64 = conn.query_row("SELECT COUNT(*) FROM sites", [], |r| r.get(0)).unwrap_or(0);
    let page_count: i64 = conn.query_row("SELECT COUNT(*) FROM pages", [], |r| r.get(0)).unwrap_or(0);
    let capture_count: i64 = conn.query_row("SELECT COUNT(*) FROM captures", [], |r| r.get(0)).unwrap_or(0);
    let resource_count: i64 = conn.query_row("SELECT COUNT(*) FROM resources", [], |r| r.get(0)).unwrap_or(0);
    let storage_bytes: i64 = conn.query_row("SELECT COALESCE(SUM(warc_length), 0) FROM captures", [], |r| r.get(0)).unwrap_or(0);

    Json(json!({
        "site_count": site_count,
        "page_count": page_count,
        "capture_count": capture_count,
        "resource_count": resource_count,
        "storage_bytes": storage_bytes,
        "storage_path": state.db.base_dir.to_string_lossy()
    })).into_response()
}

async fn api_sites(State(state): State<ReplayState>) -> Response {
    match state.db.list_sites() {
        Ok(sites) => Json(json!(sites)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn api_pages(
    State(state): State<ReplayState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let site_id = params.get("site_id");
    match site_id {
        Some(sid) => match state.db.list_pages(sid) {
            Ok(pages) => Json(json!(pages)).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        None => (StatusCode::BAD_REQUEST, "Missing site_id query parameter").into_response(),
    }
}

async fn api_captures(
    State(state): State<ReplayState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let page_id = params.get("page_id");
    match page_id {
        Some(pid) => match state.db.list_captures(pid) {
            Ok(captures) => Json(json!(captures)).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        None => (StatusCode::BAD_REQUEST, "Missing page_id query parameter").into_response(),
    }
}

async fn api_capture_detail(
    State(state): State<ReplayState>,
    Path(capture_id): Path<String>,
) -> Response {
    match state.db.get_capture_details(&capture_id) {
        Ok((capture, resources, rendered_text)) => Json(json!({
            "capture": capture,
            "resources": resources,
            "rendered_text": rendered_text
        })).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    }
}

async fn api_search(
    State(state): State<ReplayState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let query = params.get("q").cloned().unwrap_or_default();
    if query.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "Missing search query parameter 'q'").into_response();
    }
    match SearchEngine::search(&state.db, &query, None) {
        Ok(results) => Json(json!(results)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct ExportPayload {
    pub capture_id: String,
    pub format: String, // "singlefile" or "pdf"
    pub output_path: String,
}

async fn api_export(
    State(state): State<ReplayState>,
    Json(payload): Json<ExportPayload>,
) -> Response {
    let clean_path_str = payload.output_path.trim();
    if clean_path_str.is_empty() || clean_path_str.contains("..") {
        return (StatusCode::BAD_REQUEST, "Invalid output path: path traversal detected").into_response();
    }

    let path = PathBuf::from(clean_path_str);
    // Disallow writing directly to sensitive system locations
    let path_str_lower = clean_path_str.to_lowercase();
    if path_str_lower.starts_with("c:\\windows") || path_str_lower.starts_with("/etc") || path_str_lower.starts_with("/bin") {
        return (StatusCode::FORBIDDEN, "Target path points to protected system directory").into_response();
    }

    let result = match payload.format.as_str() {
        "singlefile" => SingleFileExporter::export_single_file_html(&state.db, &payload.capture_id, &path),
        "pdf" => SingleFileExporter::export_pdf(&state.db, &payload.capture_id, state.port, &path).await,
        _ => return (StatusCode::BAD_REQUEST, "Unsupported format: use 'singlefile' or 'pdf'").into_response(),
    };

    match result {
        Ok(_) => Json(json!({ "success": true, "output_path": payload.output_path })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_replay_server_startup_and_routes() {
        let temp_dir = std::env::temp_dir().join(format!("test_replay_{}", uuid::Uuid::new_v4().simple()));
        let db = Database::init(&temp_dir).expect("Failed to init temp db");
        let server = ReplayServer::start(db).await.expect("Failed to start ReplayServer");
        assert!(server.port > 0);

        let client = reqwest::Client::new();
        let resp = client.get(format!("http://127.0.0.1:{}/api/v1/health", server.port))
            .send()
            .await
            .expect("Failed to call health");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_rewrite_html_and_replay_features() {
        let sample_html = r#"
        <html>
        <head>
            <meta name="referrer" content="no-referrer-when-downgrade">
            <link rel="stylesheet" href="//s1.hdslb.com/bfs/static/style.css">
            <style>
                .banner { background-image: url("//cdn.zhihu.com/bg.png"); }
                @font-face { src: url("/fonts/custom.woff2"); }
            </style>
        </head>
        <body>
            <img src="//i0.hdslb.com/bfs/archive/pic.jpg" data-src="//i0.hdslb.com/bfs/archive/pic.jpg">
            <img src="avatar.jpg" srcset="avatar.jpg 1x, //cdn.com/avatar@2x.jpg 2x">
            <img src="/bfs/local.png">
            <div style="background: url('/bg.jpg')"></div>
        </body>
        </html>
        "#;
        let cap_id = "cap_test_123";
        let target_url = "https://www.bilibili.com/video/";
        let rewritten = rewrite_html_for_replay(cap_id, target_url, sample_html);
        println!("REWRITTEN RESULT:\n{}", rewritten);

        assert!(rewritten.contains(r#"<meta name="referrer" content="no-referrer">"#));
        assert!(rewritten.contains(r#"href="/replay/cap_test_123/https://s1.hdslb.com/bfs/static/style.css""#));
        assert!(rewritten.contains(r#"src="/replay/cap_test_123/https://i0.hdslb.com/bfs/archive/pic.jpg""#));
        assert!(rewritten.contains(r#"src="/replay/cap_test_123/https://www.bilibili.com/bfs/local.png""#));
        assert!(rewritten.contains(r#"src="/replay/cap_test_123/https://www.bilibili.com/video/avatar.jpg""#));
        assert!(rewritten.contains(r#"url('/replay/cap_test_123/https://cdn.zhihu.com/bg.png')"#));
        assert!(rewritten.contains(r#"url('/replay/cap_test_123/https://www.bilibili.com/fonts/custom.woff2')"#));
        assert!(rewritten.contains(r#"url('/replay/cap_test_123/https://www.bilibili.com/bg.jpg')"#));
        assert!(rewritten.contains(r#"srcset="/replay/cap_test_123/https://www.bilibili.com/video/avatar.jpg 1x, /replay/cap_test_123/https://cdn.com/avatar@2x.jpg 2x""#));

        assert_eq!(
            normalize_requested_resource_url("https:/s1.hdslb.com/test.css"),
            "https://s1.hdslb.com/test.css"
        );
        assert_eq!(
            normalize_requested_resource_url("/https://s1.hdslb.com/test.css"),
            "https://s1.hdslb.com/test.css"
        );
        assert_eq!(
            normalize_requested_resource_url("https%3A%2F%2Fs1.hdslb.com%2Ftest.css"),
            "https://s1.hdslb.com/test.css"
        );
    }

    #[tokio::test]
    async fn test_replay_against_actual_user_data() {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        let db_dir = std::path::PathBuf::from(appdata).join("com.webvault.app").join("WebVault");
        if !db_dir.join("app.db").exists() {
            println!("No existing user data, skipping test");
            return;
        }

        let db = Database::init(&db_dir).expect("Failed to open user db");
        let server = ReplayServer::start(db.clone()).await.expect("Failed to start server");
        let client = reqwest::Client::new();

        let cap_id: String = {
            let conn = db.lock_conn().unwrap();
            conn.query_row("SELECT id FROM captures ORDER BY captured_at DESC LIMIT 1", [], |r| r.get(0)).unwrap()
        };
        println!("Testing with latest cap_id: {}", cap_id);

        // 1. Check main page
        let page_resp = client.get(format!("http://127.0.0.1:{}/replay/{}", server.port, cap_id))
            .send()
            .await
            .expect("Failed to request replay main page");
        assert_eq!(page_resp.status(), reqwest::StatusCode::OK);
        let html = page_resp.text().await.unwrap();

        // Print all link tags in the returned HTML!
        for line in html.lines() {
            if line.contains("<link") {
                println!("REPLAY LINK TAG: {}", line.trim());
            }
        }

        // 2. Check CSS sub-resource (Archived)
        let css_url = format!(
            "http://127.0.0.1:{}/replay/{}/https://s1.hdslb.com/bfs/static/shanks/laputa-home/assets/index-c5e37698.css",
            server.port, cap_id
        );
        let css_resp = client.get(&css_url).send().await.expect("Failed to request CSS");
        println!("CSS STATUS: {}", css_resp.status());
        println!("CSS HEADERS: {:?}", css_resp.headers());
        assert_eq!(css_resp.status(), reqwest::StatusCode::OK);
        let css_bytes = css_resp.bytes().await.unwrap();
        println!("CSS BYTES LEN: {}", css_bytes.len());
        assert!(css_bytes.len() > 100_000, "CSS should be ~363KB");
        println!("CSS VERIFIED SUCCESSFULLY");
    }
}

