use crate::archive::warc::WarcReader;
use crate::database::Database;
use axum::extract::{Path, Request, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use std::collections::HashMap;
use tokio::net::TcpListener;

#[derive(Clone)]
pub struct ReplayState {
    pub db: Database,
    pub port: u16,
    pub token: String,
    pub allow_live_fallback: bool,
}

pub struct ReplayServer {
    pub port: u16,
    pub token: String,
}

impl ReplayServer {
    pub async fn start(db: Database) -> anyhow::Result<Self> {
        Self::bind(db, 0).await
    }

    pub async fn bind(db: Database, port: u16) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port)).await?;
        let port = listener.local_addr()?.port();
        let token = uuid::Uuid::new_v4().simple().to_string();

        let state = ReplayState {
            db,
            port,
            token: token.clone(),
            allow_live_fallback: false,
        };

        let app = Router::new()
            .route("/replay/{capture_id}", get(replay_main_page))
            .route("/replay/{capture_id}/{*url}", get(replay_sub_resource))
            .route("/screenshot/{capture_id}", get(serve_screenshot))
            .route("/api/v1/health", get(api_health))
            .fallback(get(fallback_replay_resource))
            .layer(middleware::from_fn_with_state(state.clone(), replay_auth))
            .with_state(state);

        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("Replay server error: {}", e);
            }
        });

        Ok(Self { port, token })
    }
}

pub fn request_has_token(token: &str, headers: &HeaderMap, query: Option<&str>) -> bool {
    if token.is_empty() {
        return false;
    }
    if let Some(q) = query {
        for pair in q.split('&') {
            if let Some(value) = pair.strip_prefix("t=") {
                if value == token {
                    return true;
                }
            }
        }
    }
    if let Some(cookie) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
        for part in cookie.split(';') {
            if let Some(value) = part.trim().strip_prefix("wv_t=") {
                if value == token {
                    return true;
                }
            }
        }
    }
    false
}

async fn replay_auth(State(state): State<ReplayState>, req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    if path == "/api/v1/health" {
        return next.run(req).await;
    }
    let query = req.uri().query().map(|s| s.to_string());
    if !request_has_token(&state.token, req.headers(), query.as_deref()) {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }
    next.run(req).await
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
        rewrite_html_for_replay(&capture_id, &target_url, &text, &state.token).into_bytes()
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
    if let Ok(cookie) = HeaderValue::from_str(&format!("wv_t={}; Path=/; HttpOnly; SameSite=Lax", state.token)) {
        response_headers.insert(header::SET_COOKIE, cookie);
    }

    let status_code = StatusCode::from_u16(status).unwrap_or(StatusCode::OK);
    (status_code, response_headers, final_body).into_response()
}

/// Rewrite HTML content for accurate archive replay
fn rewrite_html_for_replay(capture_id: &str, target_url: &str, html: &str, token: &str) -> String {
    let mut rewritten = html.to_string();

    // 1. Enforce no-referrer meta policy to bypass CDN hotlink protection
    if let Ok(ref_re) = Regex::new(r#"(?i)<meta\b[^>]*\bname=["']referrer["'][^>]*>"#) {
        if ref_re.is_match(&rewritten) {
            rewritten = ref_re.replace_all(&rewritten, r#"<meta name="referrer" content="no-referrer">"#).to_string();
        }
    }

    // 2. Inject Sandbox client shim, base tag and Fetch/XHR proxy
    let replay_prefix = format!("/replay/{}/", capture_id);
    let page_origin = url::Url::parse(target_url)
        .ok()
        .map(url_origin_string)
        .unwrap_or_default();
    let base_href = replay_base_href(capture_id, target_url);
    let shim_script = format!(
        r#"<meta name="referrer" content="no-referrer"><base href="{1}"><script>(()=>{{const p='{0}';const o='{2}';const tk='{3}';window.addEventListener('contextmenu',e=>{{e.preventDefault();e.stopPropagation();}},true);try{{window.localStorage.getItem('_test');}}catch(e){{const m={{}};const s={{getItem:k=>k in m?m[k]:null,setItem:(k,v)=>{{m[k]=String(v);}},removeItem:k=>{{delete m[k];}},clear:()=>{{for(let k in m)delete m[k];}},key:i=>Object.keys(m)[i]||null,get length(){{return Object.keys(m).length;}}}};try{{Object.defineProperty(window,'localStorage',{{value:s,configurable:true}});}}catch(er){{}}try{{Object.defineProperty(window,'sessionStorage',{{value:s,configurable:true}});}}catch(er){{}}}}const withT=u=>{{if(!tk||typeof u!=='string'||u.indexOf('t=')>=0)return u;return u+(u.indexOf('?')>=0?'&':'?')+'t='+tk;}};const rw=u=>{{if(typeof u!=='string')return u;let n=u;if(u.startsWith('//'))n=p+'https:'+u;else if(u.startsWith('http://')||u.startsWith('https://'))n=p+u;else if(u.startsWith('/')&&!u.startsWith('/replay/'))n=p+o+u;return withT(n);}};const of=window.fetch;window.fetch=function(u,i){{if(typeof u==='string')u=rw(u);return of.call(this,u,i);}};const ox=XMLHttpRequest.prototype.open;XMLHttpRequest.prototype.open=function(m,u,...r){{if(typeof u==='string')u=rw(u);return ox.call(this,m,u,...r);}};window.addEventListener('DOMContentLoaded',()=>{{document.querySelectorAll('canvas[data-webvault-canvas-snapshot]').forEach(c=>{{const s=c.getAttribute('data-webvault-canvas-snapshot');if(s){{const img=new Image();img.onload=()=>{{const ctx=c.getContext('2d');if(ctx)ctx.drawImage(img,0,0,c.width,c.height);}};img.src=s;}}}});}});}})();</script>"#,
        replay_prefix, base_href, page_origin, token
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
            return with_replay_token(trimmed, token);
        }
        if let Some(ref base) = parsed_base {
            if let Ok(joined) = base.join(trimmed) {
                let s = joined.to_string();
                if s.starts_with("http://") || s.starts_with("https://") {
                    return with_replay_token(&format!("{}{}", replay_prefix, s), token);
                }
            }
        }
        if trimmed.starts_with("//") {
            return with_replay_token(&format!("{}https:{}", replay_prefix, trimmed), token);
        }
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            return with_replay_token(&format!("{}{}", replay_prefix, trimmed), token);
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

    // Keep in-page navigation inside the replay sandbox
    if let Ok(nav_re) = Regex::new(r#"(?i)<(a|area|form)\b([^>]*)\b(href|action)=["']([^"']+)["']([^>]*)>"#) {
        rewritten = nav_re.replace_all(&rewritten, |caps: &regex::Captures| {
            let tag = &caps[1];
            let before = &caps[2];
            let attr = &caps[3];
            let raw = &caps[4];
            let after = &caps[5];
            format!(r#"<{}{}{}="{}"{}>"#, tag, before, attr, rewrite_single_url(raw), after)
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

fn replay_base_href(capture_id: &str, target_url: &str) -> String {
    let prefix = format!("/replay/{}/", capture_id);
    let Ok(u) = url::Url::parse(target_url) else {
        return prefix;
    };
    let mut path = u.path().to_string();
    if !path.ends_with('/') {
        if let Some(idx) = path.rfind('/') {
            path.truncate(idx + 1);
        } else {
            path = "/".to_string();
        }
    }
    format!("{}{}{}", prefix, url_origin_string(u), path)
}

fn url_origin_string(u: url::Url) -> String {
    match u.origin() {
        url::Origin::Tuple(scheme, host, port) => {
            let host = host.to_string();
            let default = match scheme.as_str() {
                "http" => 80,
                "https" => 443,
                _ => port,
            };
            if port == default {
                format!("{}://{}", scheme, host)
            } else {
                format!("{}://{}:{}", scheme, host, port)
            }
        }
        url::Origin::Opaque(_) => format!("{}://{}", u.scheme(), u.host_str().unwrap_or_default()),
    }
}

fn with_replay_token(url: &str, token: &str) -> String {
    if token.is_empty() || url.contains("t=") {
        return url.to_string();
    }
    if url.contains('?') {
        format!("{}&t={}", url, token)
    } else {
        format!("{}?t={}", url, token)
    }
}

fn rewrite_css_urls(capture_id: &str, css_url: &str, css: &str, token: &str) -> String {
    let replay_prefix = format!("/replay/{}/", capture_id);
    let parsed_base = url::Url::parse(css_url).ok();
    let rewrite_single_url = |val: &str| -> String {
        let trimmed = val.trim();
        if trimmed.is_empty() || trimmed.starts_with("data:") || trimmed.starts_with('#') {
            return trimmed.to_string();
        }
        if trimmed.starts_with(&replay_prefix) {
            return with_replay_token(trimmed, token);
        }
        if let Some(ref base) = parsed_base {
            if let Ok(joined) = base.join(trimmed) {
                let s = joined.to_string();
                if s.starts_with("http://") || s.starts_with("https://") {
                    return with_replay_token(&format!("{}{}", replay_prefix, s), token);
                }
            }
        }
        if trimmed.starts_with("//") {
            return with_replay_token(&format!("{}https:{}", replay_prefix, trimmed), token);
        }
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            return with_replay_token(&format!("{}{}", replay_prefix, trimmed), token);
        }
        trimmed.to_string()
    };
    if let Ok(css_url_re) = Regex::new(r#"(?i)url\(\s*['"]?((?:https?://|//|/|\./|\.\./)[^'")\s]+)['"]?\s*\)"#) {
        return css_url_re
            .replace_all(css, |caps: &regex::Captures| {
                format!("url('{}')", rewrite_single_url(&caps[1]))
            })
            .to_string();
    }
    css.to_string()
}

fn is_static_asset(url: &str) -> bool {
    let lower = url.split('?').next().unwrap_or(url).to_lowercase();
    [
        ".css", ".js", ".mjs", ".woff2", ".woff", ".ttf", ".otf", ".eot", ".png", ".jpg", ".jpeg",
        ".gif", ".webp", ".avif", ".svg", ".ico", ".bmp", ".map",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
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
                        || k_lower == "set-cookie"
                        || k_lower == "location"
                        || k_lower == "www-authenticate"
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
                let body = if mime.contains("text/css") || target_url.to_lowercase().contains(".css") {
                    rewrite_css_urls(&capture_id, &target_url, &String::from_utf8_lossy(&body), &state.token).into_bytes()
                } else if mime.contains("text/html") {
                    rewrite_html_for_replay(&capture_id, &target_url, &String::from_utf8_lossy(&body), &state.token).into_bytes()
                } else {
                    body
                };
                return (st, response_headers, body).into_response();
            }
        }
    }

    if state.allow_live_fallback
        && is_static_asset(&target_url)
        && (target_url.starts_with("http://") || target_url.starts_with("https://"))
    {
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
                    let is_css = response_headers
                        .get(header::CONTENT_TYPE)
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.contains("text/css"))
                        .unwrap_or(false)
                        || target_url.to_lowercase().contains(".css");
                    let body = if is_css {
                        rewrite_css_urls(&capture_id, &target_url, &String::from_utf8_lossy(&bytes), &state.token).into_bytes()
                    } else {
                        bytes.to_vec()
                    };
                    return (StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK), response_headers, body).into_response();
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

async fn api_health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "service": "WebVault Local Web Time Machine",
        "version": "0.1.1"
    }))
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

        let denied = client
            .get(format!("http://127.0.0.1:{}/replay/cap_x", server.port))
            .send()
            .await
            .expect("replay without token");
        assert_eq!(denied.status(), reqwest::StatusCode::UNAUTHORIZED);

        let allowed = client
            .get(format!("http://127.0.0.1:{}/replay/cap_x?t={}", server.port, server.token))
            .send()
            .await
            .expect("replay with token");
        assert_ne!(allowed.status(), reqwest::StatusCode::UNAUTHORIZED);

        assert!(request_has_token("abc", &HeaderMap::new(), Some("t=abc")));
        assert!(!request_has_token("abc", &HeaderMap::new(), Some("t=nope")));

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
        let rewritten = rewrite_html_for_replay(cap_id, target_url, sample_html, "");
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
        assert_eq!(
            replay_base_href("cap_1", "https://example.com/foo/bar.html"),
            "/replay/cap_1/https://example.com/foo/"
        );
        assert_eq!(
            replay_base_href("cap_1", "https://example.com:8443/foo/bar.html"),
            "/replay/cap_1/https://example.com:8443/foo/"
        );
        let with_link = rewrite_html_for_replay(
            "cap_1",
            "https://example.com/page",
            r#"<html><body><a href="https://example.com/next">n</a></body></html>"#,
            "tok",
        );
        assert!(with_link.contains(r#"href="/replay/cap_1/https://example.com/next?t=tok""#));
        assert!(with_link.contains("contextmenu"));
        assert!(is_static_asset("https://cdn.example.com/app.css"));
        assert!(!is_static_asset("https://example.com/article/1"));
        let css = rewrite_css_urls(
            "cap_1",
            "https://example.com/css/app.css",
            "body{background:url('../img/bg.png')}",
            "",
        );
        assert!(css.contains("/replay/cap_1/https://example.com/img/bg.png"));
    }

}

