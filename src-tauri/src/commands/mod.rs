use crate::archive::service::CapturePersistenceService;
use crate::archive::singlefile::SingleFileExporter;
use crate::archive::wacz::WaczPackage;
use crate::archive::warc::WarcWriter;
use crate::capture::browser::BrowserFinder;
use crate::capture::cdp::CdpClient;
use crate::capture::credentials::CredentialManager;
use crate::crawler::frontier::CrawlFrontier;
use crate::crawler::scope::ScopeRule;
use crate::database::models::*;
use crate::database::Database;
use crate::diff::DiffEngine;
use crate::monitor::MonitorEngine;
use crate::search::SearchEngine;
use crate::wayback::WaybackProvider;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

pub struct AppState {
    pub db: Database,
    pub browser_path: Option<PathBuf>,
    pub replay_port: u16,
    pub wayback: WaybackProvider,
    pub active_jobs: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

#[tauri::command]
pub async fn get_system_stats(state: State<'_, AppState>) -> Result<SystemStats, String> {
    let browser_detected = state.browser_path.is_some();
    let browser_str = state
        .browser_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "Not detected".to_string());

    state
        .db
        .get_system_stats(browser_detected, &browser_str)
        .map_err(|e| e.to_string())
}

// Sites
#[tauri::command]
pub async fn list_sites(state: State<'_, AppState>) -> Result<Vec<Site>, String> {
    state.db.list_sites().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_site(
    state: State<'_, AppState>,
    name: String,
    root_url: String,
) -> Result<Site, String> {
    state.db.create_site(&name, &root_url).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_site(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.db.delete_site(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_site_profile(
    state: State<'_, AppState>,
    site_id: String,
) -> Result<CrawlProfile, String> {
    state.db.get_site_profile(&site_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_site_profile(
    state: State<'_, AppState>,
    profile: CrawlProfile,
) -> Result<(), String> {
    state.db.update_site_profile(&profile).map_err(|e| e.to_string())
}

// Pages & Captures
#[tauri::command]
pub async fn list_pages(
    state: State<'_, AppState>,
    site_id: String,
) -> Result<Vec<PageItem>, String> {
    state.db.list_pages(&site_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_captures(
    state: State<'_, AppState>,
    page_id: String,
) -> Result<Vec<CaptureItem>, String> {
    state.db.list_captures(&page_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_capture_details(
    state: State<'_, AppState>,
    capture_id: String,
) -> Result<serde_json::Value, String> {
    let (capture, resources, rendered_text) = state
        .db
        .get_capture_details(&capture_id)
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "capture": capture,
        "resources": resources,
        "rendered_text": rendered_text,
    }))
}

// Single Page Capture
#[tauri::command]
pub async fn start_single_capture(
    state: State<'_, AppState>,
    site_id: String,
    url: String,
) -> Result<String, String> {
    let browser_path = state
        .browser_path
        .clone()
        .ok_or_else(|| "No supported Chromium or Edge browser detected on system".to_string())?;

    let db = state.db.clone();
    let job_id = format!("job_{}", uuid::Uuid::new_v4().simple());
    let now = chrono::Utc::now().timestamp_millis();

    if let Ok(conn) = db.lock_conn() {
        let _ = conn.execute(
            r#"
            INSERT INTO crawl_jobs (id, site_id, status, started_at, pages_discovered, trigger_type)
            VALUES (?1, ?2, 'running', ?3, 1, 'single_capture')
            "#,
            rusqlite::params![job_id, site_id, now],
        );
    }

    let job_id_clone = job_id.clone();
    let url_clone = url.clone();

    tokio::spawn(async move {
        let temp_dir = db.base_dir.join("temp");
        let site_profile_dir = db.base_dir.join("browser_profiles").join(&site_id);
        let launch_options = crate::capture::browser::BrowserLaunchOptions {
            headless: true,
            user_data_dir: Some(site_profile_dir),
            window_width: 1920,
            window_height: 1080,
        };
        let port = BrowserFinder::find_available_port();
        let browser = match BrowserFinder::launch_with_options(&browser_path, port, &temp_dir, &launch_options).await {
            Ok(b) => b,
            Err(e) => {
                tracing::error!("Failed to launch browser: {}", e);
                if let Ok(conn) = db.lock_conn() {
                    let finish_now = chrono::Utc::now().timestamp_millis();
                    let _ = conn.execute(
                        "UPDATE crawl_jobs SET status = 'failed', error_count = 1, finished_at = ?1 WHERE id = ?2",
                        rusqlite::params![finish_now, job_id_clone],
                    );
                }
                return;
            }
        };

        let client = CdpClient::new(browser.port);
        let creds = db.get_site_credentials(&site_id).ok();
        let (cookies_json, storage_json) = if let Some(ref list) = creds {
            if let Some(first) = list.first() {
                (Some(first.cookies_json.as_str()), Some(first.storage_json.as_str()))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };
        let user_scripts: Vec<String> = db.list_user_scripts(Some(&site_id))
            .unwrap_or_default()
            .into_iter()
            .filter(|s| s.enabled)
            .map(|s| s.script_content)
            .collect();
        let scripts_ref = if user_scripts.is_empty() { None } else { Some(user_scripts.as_slice()) };
        let capture_res = match client.capture_page_with_credentials(&url_clone, true, 25, cookies_json, storage_json, scripts_ref).await {
            Ok(res) => res,
            Err(e) => {
                tracing::error!("Capture failed for {}: {}", url_clone, e);
                if let Ok(conn) = db.lock_conn() {
                    let finish_now = chrono::Utc::now().timestamp_millis();
                    let _ = conn.execute(
                        "UPDATE crawl_jobs SET status = 'failed', error_count = 1, finished_at = ?1 WHERE id = ?2",
                        rusqlite::params![finish_now, job_id_clone],
                    );
                }
                return;
            }
        };

        let warc_rel_path = format!("archives/{}/data.warc.gz", site_id);
        let warc_full_path = db.base_dir.join(&warc_rel_path);
        let writer = WarcWriter::new(&warc_full_path);

        match CapturePersistenceService::persist_capture(&db, &site_id, &job_id_clone, &capture_res, &writer) {
            Ok(persisted) => {
                if let Ok(conn) = db.lock_conn() {
                    let finish_now = chrono::Utc::now().timestamp_millis();
                    let _ = conn.execute(
                        r#"
                        UPDATE crawl_jobs SET
                            status = 'completed',
                            pages_captured = 1,
                            resources_captured = ?1,
                            bytes_written = ?2,
                            finished_at = ?3
                        WHERE id = ?4
                        "#,
                        rusqlite::params![
                            capture_res.resource_count as i64,
                            persisted.bytes_written as i64,
                            finish_now,
                            job_id_clone,
                        ],
                    );
                }
            }
            Err(e) => {
                tracing::error!("Failed to persist capture for {}: {}", url_clone, e);
                if let Ok(conn) = db.lock_conn() {
                    let finish_now = chrono::Utc::now().timestamp_millis();
                    let _ = conn.execute(
                        "UPDATE crawl_jobs SET status = 'failed', error_count = 1, finished_at = ?1 WHERE id = ?2",
                        rusqlite::params![finish_now, job_id_clone],
                    );
                }
            }
        }
    });

    Ok(job_id)
}

// Multi-page Crawler Job
#[tauri::command]
pub async fn start_crawl_job(
    state: State<'_, AppState>,
    site_id: String,
) -> Result<String, String> {
    let browser_path = state
        .browser_path
        .clone()
        .ok_or_else(|| "No browser detected".to_string())?;

    let db = state.db.clone();
    let job_id = format!("job_{}", uuid::Uuid::new_v4().simple());
    let now = chrono::Utc::now().timestamp_millis();

    if let Ok(conn) = db.lock_conn() {
        let _ = conn.execute(
            r#"
            INSERT INTO crawl_jobs (id, site_id, status, started_at)
            VALUES (?1, ?2, 'running', ?3)
            "#,
            rusqlite::params![job_id, site_id, now],
        );
    }

    let cancel_flag = Arc::new(AtomicBool::new(false));
    {
        let mut jobs = state.active_jobs.lock().await;
        jobs.insert(job_id.clone(), cancel_flag.clone());
    }

    let job_id_clone = job_id.clone();
    tokio::spawn(async move {
        let profile = db.get_site_profile(&site_id).unwrap_or_default();
        let site_info = {
            let conn = db.lock_conn().ok();
            conn.and_then(|c| {
                c.query_row(
                    "SELECT root_url FROM sites WHERE id = ?1",
                    rusqlite::params![site_id],
                    |r| r.get::<_, String>(0),
                ).ok()
            })
        };

        let root_url = match site_info {
            Some(u) => u,
            None => return,
        };

        let scope = ScopeRule::from_profile(
            &profile.scope_type,
            &root_url,
            &profile.include_rules,
            &profile.exclude_rules,
        );

        // Initialize Frontier
        let mut frontier = CrawlFrontier::new(
            &root_url,
            scope,
            profile.max_depth,
            profile.max_pages as usize,
        );

        // Discover Sitemap URLs
        let sitemap_limit = (profile.max_pages as usize).min(100);
        let sitemap_urls = crate::crawler::sitemap::SitemapFinder::discover_sitemap_urls(&root_url, sitemap_limit).await;
        for sm_url in sitemap_urls {
            frontier.add_url(&sm_url, 1, Some(root_url.clone()));
        }

        // Persist initial queue in crawl_queue table
        if let Ok(conn) = db.lock_conn() {
            let now_ts = chrono::Utc::now().timestamp_millis();
            let norm_root = crate::crawler::normalizer::UrlNormalizer::new().normalize(&root_url).unwrap_or_else(|_| root_url.clone());
            let _ = conn.execute(
                r#"
                INSERT OR IGNORE INTO crawl_queue (id, job_id, url, normalized_url, depth, status, discovered_at)
                VALUES (?1, ?2, ?3, ?4, 0, 'pending', ?5)
                "#,
                rusqlite::params![format!("q_{}", uuid::Uuid::new_v4().simple()), job_id_clone, root_url, norm_root, now_ts],
            );
        }

        let temp_dir = db.base_dir.join("temp");
        let site_profile_dir = db.base_dir.join("browser_profiles").join(&site_id);
        let launch_options = crate::capture::browser::BrowserLaunchOptions {
            headless: true,
            user_data_dir: Some(site_profile_dir),
            window_width: 1920,
            window_height: 1080,
        };
        let port = BrowserFinder::find_available_port();
        let browser = match BrowserFinder::launch_with_options(&browser_path, port, &temp_dir, &launch_options).await {
            Ok(b) => b,
            Err(e) => {
                tracing::error!("Failed to launch crawler browser: {}", e);
                return;
            }
        };

        let client = Arc::new(CdpClient::new(browser.port));
        let warc_rel_path = format!("archives/{}/data.warc.gz", site_id);
        let warc_full_path = db.base_dir.join(&warc_rel_path);
        let writer = Arc::new(tokio::sync::Mutex::new(WarcWriter::new(&warc_full_path)));

        let concurrency = (profile.concurrency as usize).clamp(1, 4);
        let max_bytes = (profile.max_size_mb as u64) * 1024 * 1024;

        let frontier = Arc::new(tokio::sync::Mutex::new(frontier));
        let total_pages_captured = Arc::new(std::sync::atomic::AtomicI64::new(0));
        let total_resources_captured = Arc::new(std::sync::atomic::AtomicI64::new(0));
        let total_bytes_written = Arc::new(std::sync::atomic::AtomicU64::new(0));

        let sem = Arc::new(tokio::sync::Semaphore::new(concurrency));
        let mut join_set = tokio::task::JoinSet::new();

        let (site_cookies, site_storage) = {
            if let Ok(creds) = db.get_site_credentials(&site_id) {
                if let Some(first) = creds.first() {
                    (Some(first.cookies_json.clone()), Some(first.storage_json.clone()))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        };
        let site_cookies = Arc::new(site_cookies);
        let site_storage = Arc::new(site_storage);

        let site_scripts: Vec<String> = db.list_user_scripts(Some(&site_id))
            .unwrap_or_default()
            .into_iter()
            .filter(|s| s.enabled)
            .map(|s| s.script_content)
            .collect();
        let site_scripts = Arc::new(site_scripts);

        loop {
            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }
            if total_bytes_written.load(Ordering::Relaxed) >= max_bytes {
                tracing::info!("Crawl job {} reached max size limit ({} MB)", job_id_clone, profile.max_size_mb);
                break;
            }

            // Check if frontier has items
            let next_item = {
                let mut f = frontier.lock().await;
                f.pop_next()
            };

            match next_item {
                Some(item) => {
                    let permit = match sem.clone().acquire_owned().await {
                        Ok(p) => p,
                        Err(_) => break,
                    };
                    let client = client.clone();
                    let writer = writer.clone();
                    let db = db.clone();
                    let site_id = site_id.clone();
                    let job_id = job_id_clone.clone();
                    let frontier = frontier.clone();
                    let total_pages_captured = total_pages_captured.clone();
                    let total_resources_captured = total_resources_captured.clone();
                    let total_bytes_written = total_bytes_written.clone();
                    let autoscroll = profile.autoscroll;
                    let site_cookies = site_cookies.clone();
                    let site_storage = site_storage.clone();
                    let site_scripts = site_scripts.clone();

                    // Update crawl_queue status to processing
                    if let Ok(conn) = db.lock_conn() {
                        let now_ts = chrono::Utc::now().timestamp_millis();
                        let _ = conn.execute(
                            "UPDATE crawl_queue SET status = 'processing', started_at = ?1 WHERE job_id = ?2 AND normalized_url = ?3",
                            rusqlite::params![now_ts, job_id, item.normalized_url],
                        );
                    }

                    join_set.spawn(async move {
                        let _permit = permit;
                        let scripts_slice = if site_scripts.is_empty() { None } else { Some(site_scripts.as_slice()) };
                        let capture_res = client.capture_page_with_credentials(
                            &item.url,
                            autoscroll,
                            20,
                            site_cookies.as_ref().as_deref(),
                            site_storage.as_ref().as_deref(),
                            scripts_slice,
                        ).await;
                        match capture_res {
                            Ok(capture_res) => {
                                let persist_res = {
                                    let w = writer.lock().await;
                                    CapturePersistenceService::persist_capture(
                                        &db,
                                        &site_id,
                                        &job_id,
                                        &capture_res,
                                        &*w,
                                    )
                                };

                                match persist_res {
                                    Ok(persisted) => {
                                        let cap_now = chrono::Utc::now().timestamp_millis();
                                        total_pages_captured.fetch_add(1, Ordering::Relaxed);
                                        total_resources_captured.fetch_add(capture_res.resource_count as i64, Ordering::Relaxed);
                                        total_bytes_written.fetch_add(persisted.bytes_written, Ordering::Relaxed);

                                        if let Ok(conn) = db.lock_conn() {
                                            let _ = conn.execute(
                                                "UPDATE crawl_queue SET status = 'completed', finished_at = ?1 WHERE job_id = ?2 AND normalized_url = ?3",
                                                rusqlite::params![cap_now, job_id, item.normalized_url],
                                            );
                                        }

                                        // Add discovered links to frontier and crawl_queue
                                        {
                                            let mut f = frontier.lock().await;
                                            for link in capture_res.links {
                                                if f.add_url(&link, item.depth + 1, Some(item.url.clone())) {
                                                    if let Ok(conn) = db.lock_conn() {
                                                        let q_id = format!("q_{}", uuid::Uuid::new_v4().simple());
                                                        let norm_link = crate::crawler::normalizer::UrlNormalizer::new()
                                                            .normalize(&link)
                                                            .unwrap_or_else(|_| link.clone());
                                                        let now_ts = chrono::Utc::now().timestamp_millis();
                                                        let _ = conn.execute(
                                                            r#"
                                                            INSERT OR IGNORE INTO crawl_queue (id, job_id, url, normalized_url, parent_url, depth, status, discovered_at)
                                                            VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7)
                                                            "#,
                                                            rusqlite::params![q_id, job_id, link, norm_link, item.url, item.depth + 1, now_ts],
                                                        );
                                                    }
                                                }
                                            }
                                        }

                                        // Update job stats in DB
                                        let discovered = frontier.lock().await.discovered_count();
                                        if let Ok(conn) = db.lock_conn() {
                                            let _ = conn.execute(
                                                r#"
                                                UPDATE crawl_jobs SET
                                                    pages_discovered = ?1,
                                                    pages_captured = ?2,
                                                    resources_captured = ?3,
                                                    bytes_written = ?4
                                                WHERE id = ?5
                                                "#,
                                                rusqlite::params![
                                                    discovered as i64,
                                                    total_pages_captured.load(Ordering::Relaxed),
                                                    total_resources_captured.load(Ordering::Relaxed),
                                                    total_bytes_written.load(Ordering::Relaxed) as i64,
                                                    job_id
                                                ],
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to persist crawler capture for {}: {}", item.url, e);
                                        if let Ok(conn) = db.lock_conn() {
                                            let _ = conn.execute(
                                                "UPDATE crawl_queue SET status = 'failed', error_message = ?1 WHERE job_id = ?2 AND normalized_url = ?3",
                                                rusqlite::params![e.to_string(), job_id, item.normalized_url],
                                            );
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed capturing {}: {}", item.url, e);
                                if let Ok(conn) = db.lock_conn() {
                                    let now_ts = chrono::Utc::now().timestamp_millis();
                                    let _ = conn.execute(
                                        "UPDATE crawl_queue SET status = 'failed', retry_count = retry_count + 1, finished_at = ?1 WHERE job_id = ?2 AND normalized_url = ?3",
                                        rusqlite::params![now_ts, job_id, item.normalized_url],
                                    );
                                    let _ = conn.execute(
                                        "UPDATE crawl_jobs SET error_count = error_count + 1 WHERE id = ?1",
                                        rusqlite::params![job_id],
                                    );
                                }
                            }
                        }
                    });
                }
                None => {
                    if join_set.is_empty() {
                        break;
                    }
                    tokio::select! {
                        _ = join_set.join_next() => {}
                        _ = tokio::time::sleep(tokio::time::Duration::from_millis(50)) => {}
                    }
                }
            }
        }

        // Await any remaining in-flight tasks
        while let Some(_) = join_set.join_next().await {}

        // Finish job
        if let Ok(conn) = db.lock_conn() {
            let finish_now = chrono::Utc::now().timestamp_millis();
            let _ = conn.execute(
                "UPDATE crawl_jobs SET status = 'completed', finished_at = ?1 WHERE id = ?2",
                rusqlite::params![finish_now, job_id_clone],
            );
        }
    });

    Ok(job_id)
}

#[tauri::command]
pub async fn list_jobs(state: State<'_, AppState>) -> Result<Vec<CrawlJob>, String> {
    let conn = state.db.lock_conn().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, site_id, profile_id, status, started_at, finished_at,
                   pages_discovered, pages_captured, resources_captured, bytes_written, error_count, trigger_type
            FROM crawl_jobs ORDER BY started_at DESC LIMIT 50
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(CrawlJob {
                id: row.get(0)?,
                site_id: row.get(1)?,
                profile_id: row.get(2)?,
                status: row.get(3)?,
                started_at: row.get(4)?,
                finished_at: row.get(5)?,
                pages_discovered: row.get(6)?,
                pages_captured: row.get(7)?,
                resources_captured: row.get(8)?,
                bytes_written: row.get(9)?,
                error_count: row.get(10)?,
                trigger_type: row.get(11)?,
                current_url: None,
                logs: None,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut jobs = Vec::new();
    for r in rows {
        jobs.push(r.map_err(|e| e.to_string())?);
    }
    Ok(jobs)
}

#[tauri::command]
pub async fn cancel_job(state: State<'_, AppState>, job_id: String) -> Result<(), String> {
    let mut jobs = state.active_jobs.lock().await;
    if let Some(flag) = jobs.remove(&job_id) {
        flag.store(true, Ordering::Relaxed);
    }
    let conn = state.db.lock_conn().map_err(|e| e.to_string())?;
    let _ = conn.execute(
        "UPDATE crawl_jobs SET status = 'cancelled' WHERE id = ?1",
        rusqlite::params![job_id],
    );
    Ok(())
}

// Replay
#[tauri::command]
pub async fn get_replay_url(state: State<'_, AppState>, capture_id: String) -> Result<String, String> {
    Ok(format!("http://127.0.0.1:{}/replay/{}", state.replay_port, capture_id))
}

// Search
#[tauri::command]
pub async fn search_archive(
    state: State<'_, AppState>,
    query: String,
    site_id: Option<String>,
) -> Result<Vec<SearchResultItem>, String> {
    SearchEngine::search(&state.db, &query, site_id.as_deref()).map_err(|e| e.to_string())
}

// Diff
#[tauri::command]
pub async fn compare_captures(
    state: State<'_, AppState>,
    old_capture_id: String,
    new_capture_id: String,
) -> Result<DiffResult, String> {
    DiffEngine::compare(&state.db, &old_capture_id, &new_capture_id).map_err(|e| e.to_string())
}

// Monitor
#[tauri::command]
pub async fn list_monitor_rules(state: State<'_, AppState>) -> Result<Vec<MonitorRule>, String> {
    let monitor = MonitorEngine::new(state.db.clone());
    monitor.list_rules().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_monitor_rule(
    state: State<'_, AppState>,
    rule: MonitorRule,
) -> Result<MonitorRule, String> {
    let monitor = MonitorEngine::new(state.db.clone());
    monitor.create_rule(&rule).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_monitor_rule(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    let monitor = MonitorEngine::new(state.db.clone());
    monitor.toggle_rule(&id, enabled).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_monitor_rule(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let monitor = MonitorEngine::new(state.db.clone());
    monitor.delete_rule(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_change_events(state: State<'_, AppState>) -> Result<Vec<ChangeEventItem>, String> {
    let monitor = MonitorEngine::new(state.db.clone());
    monitor.list_change_events().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn trigger_monitor_check(state: State<'_, AppState>) -> Result<(), String> {
    let monitor = MonitorEngine::new(state.db.clone());
    monitor.check_pending_rules().await.map_err(|e| e.to_string())
}

// Wayback
#[tauri::command]
pub async fn query_wayback(
    state: State<'_, AppState>,
    url: String,
) -> Result<Vec<WaybackCaptureItem>, String> {
    state.wayback.query_cdx(&url).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_wayback_capture(
    state: State<'_, AppState>,
    site_id: String,
    capture: WaybackCaptureItem,
) -> Result<(), String> {
    state
        .wayback
        .import_capture(&state.db, &site_id, &capture)
        .await
        .map_err(|e| e.to_string())
}

// Export / Import
#[tauri::command]
pub async fn export_wacz(
    state: State<'_, AppState>,
    site_id: String,
    output_path: String,
) -> Result<String, String> {
    let warc_rel = format!("archives/{}/data.warc.gz", site_id);
    let warc_full = state.db.base_dir.join(&warc_rel);

    if !warc_full.exists() {
        return Err("No WARC archive exists for this site yet".to_string());
    }

    let (pages_lines, pages_jsonl, cdx_content) = {
        let conn = state.db.lock_conn().map_err(|e| e.to_string())?;

        // 1. Generate pages.jsonl
        let mut stmt_pages = conn.prepare(
            r#"
            SELECT p.id, p.url, p.title, COALESCE(MAX(c.captured_at), p.last_seen)
            FROM pages p
            LEFT JOIN captures c ON c.page_id = p.id
            WHERE p.site_id = ?1
            GROUP BY p.id
            ORDER BY p.first_seen ASC
            "#,
        ).map_err(|e| e.to_string())?;

        let page_rows = stmt_pages.query_map(rusqlite::params![site_id], |row| {
            let id: String = row.get(0)?;
            let url: String = row.get(1)?;
            let title: Option<String> = row.get(2)?;
            let captured_at: i64 = row.get(3)?;
            let ts = chrono::DateTime::from_timestamp_millis(captured_at)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default();
            Ok(serde_json::json!({
                "id": id,
                "url": url,
                "title": title.unwrap_or_default(),
                "ts": ts,
            }).to_string())
        }).map_err(|e| e.to_string())?;

        let mut pages_lines = Vec::new();
        for r in page_rows {
            if let Ok(line) = r {
                pages_lines.push(line);
            }
        }
        let pages_jsonl = pages_lines.join("\n");

        // 2. Generate CDXJ index
        let mut cdx_lines = Vec::new();
        let mut stmt_caps = conn.prepare(
            r#"
            SELECT p.url, c.captured_at, c.mime_type, c.status_code, c.text_hash, c.warc_length, c.warc_offset, c.warc_file
            FROM captures c
            JOIN pages p ON c.page_id = p.id
            WHERE p.site_id = ?1
            "#,
        ).map_err(|e| e.to_string())?;

        let cap_rows = stmt_caps.query_map(rusqlite::params![site_id], |row| {
            let url: String = row.get(0)?;
            let captured_at: i64 = row.get(1)?;
            let mime: Option<String> = row.get(2)?;
            let status: i32 = row.get(3)?;
            let hash: Option<String> = row.get(4)?;
            let length: i64 = row.get(5)?;
            let offset: i64 = row.get(6)?;
            let warc_file: String = row.get(7)?;
            let ts = chrono::DateTime::from_timestamp_millis(captured_at)
                .map(|dt| dt.format("%Y%m%d%H%M%S").to_string())
                .unwrap_or_else(|| "20260101000000".to_string());
            let filename = std::path::Path::new(&warc_file)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("data.warc.gz");
            Ok(format!(
                "{} {} {{\"url\": {:?}, \"mime\": {:?}, \"status\": \"{}\", \"digest\": {:?}, \"length\": \"{}\", \"offset\": \"{}\", \"filename\": {:?}}}",
                url, ts, url, mime.unwrap_or_else(|| "text/html".to_string()), status, hash.unwrap_or_default(), length, offset, filename
            ))
        }).map_err(|e| e.to_string())?;

        for r in cap_rows {
            if let Ok(l) = r {
                cdx_lines.push(l);
            }
        }

        let mut stmt_res = conn.prepare(
            r#"
            SELECT r.url, c.captured_at, r.mime_type, r.status_code, r.sha256, r.warc_length, r.warc_offset, r.warc_file
            FROM resources r
            JOIN captures c ON r.capture_id = c.id
            JOIN pages p ON c.page_id = p.id
            WHERE p.site_id = ?1
            "#,
        ).map_err(|e| e.to_string())?;

        let res_rows = stmt_res.query_map(rusqlite::params![site_id], |row| {
            let url: String = row.get(0)?;
            let captured_at: i64 = row.get(1)?;
            let mime: Option<String> = row.get(2)?;
            let status: i32 = row.get(3)?;
            let hash: Option<String> = row.get(4)?;
            let length: Option<i64> = row.get(5)?;
            let offset: i64 = row.get(6)?;
            let warc_file: Option<String> = row.get(7)?;
            let ts = chrono::DateTime::from_timestamp_millis(captured_at)
                .map(|dt| dt.format("%Y%m%d%H%M%S").to_string())
                .unwrap_or_else(|| "20260101000000".to_string());
            let filename = warc_file.as_ref()
                .and_then(|f| std::path::Path::new(f).file_name().and_then(|n| n.to_str()))
                .unwrap_or("data.warc.gz");
            Ok(format!(
                "{} {} {{\"url\": {:?}, \"mime\": {:?}, \"status\": \"{}\", \"digest\": {:?}, \"length\": \"{}\", \"offset\": \"{}\", \"filename\": {:?}}}",
                url, ts, url, mime.unwrap_or_else(|| "application/octet-stream".to_string()), status, hash.unwrap_or_default(), length.unwrap_or(0), offset, filename
            ))
        }).map_err(|e| e.to_string())?;

        for r in res_rows {
            if let Ok(l) = r {
                cdx_lines.push(l);
            }
        }
        cdx_lines.sort();
        let cdx_content = cdx_lines.join("\n");
        (pages_lines, pages_jsonl, cdx_content)
    };

    let raw_path = std::path::PathBuf::from(&output_path);
    let resolved_path = if raw_path.is_relative() {
        let export_dir = state.db.base_dir.join("exports");
        let _ = std::fs::create_dir_all(&export_dir);
        export_dir.join(raw_path)
    } else {
        raw_path
    };
    if let Some(parent) = resolved_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    WaczPackage::create_wacz(
        &warc_full,
        &resolved_path,
        &format!("WebVault Export - {}", site_id),
        "Exported from WebVault Local Web Time Machine",
        &cdx_content,
        if pages_lines.is_empty() { None } else { Some(&pages_jsonl) },
    )
    .map_err(|e| e.to_string())?;

    Ok(resolved_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn import_wacz(state: State<'_, AppState>, file_path: String) -> Result<Site, String> {
    let temp_dest = state.db.base_dir.join("temp").join(format!("imp_{}", uuid::Uuid::new_v4().simple()));
    let unpacked = WaczPackage::unpack_wacz(&file_path, &temp_dest)
        .map_err(|e| e.to_string())?;

    // 1. Determine root url and initial pages from pages_jsonl or cdx
    let mut initial_root_url = String::new();
    let mut parsed_pages = Vec::new();

    if let Some(ref pj) = unpacked.pages_jsonl {
        for line in pj.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                let url = v["url"].as_str().unwrap_or_default().to_string();
                let title = v["title"].as_str().unwrap_or_default().to_string();
                let ts = v["ts"].as_str().unwrap_or_default();
                let epoch_ms = chrono::DateTime::parse_from_rfc3339(ts)
                    .map(|dt| dt.timestamp_millis())
                    .unwrap_or_else(|_| chrono::Utc::now().timestamp_millis());
                if !url.is_empty() {
                    if initial_root_url.is_empty() {
                        initial_root_url = url.clone();
                    }
                    parsed_pages.push((url, title, epoch_ms));
                }
            }
        }
    }

    if initial_root_url.is_empty() {
        initial_root_url = format!("https://imported.local/{}", uuid::Uuid::new_v4().simple());
    }

    let site = state
        .db
        .create_site(&unpacked.title, &initial_root_url)
        .map_err(|e| e.to_string())?;

    let site_warc_rel = format!("archives/{}/data.warc.gz", site.id);
    let site_warc_full = state.db.base_dir.join(&site_warc_rel);
    if let Some(p) = site_warc_full.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    std::fs::copy(&unpacked.warc_path, &site_warc_full).map_err(|e| e.to_string())?;

    // 2. Index pages and captures into SQLite
    let normalizer = crate::crawler::normalizer::UrlNormalizer::new();
    let now = chrono::Utc::now().timestamp_millis();

    if let Ok(conn) = state.db.lock_conn() {
        // A. Insert parsed pages from pages.jsonl
        for (url, title, ts) in parsed_pages {
            let norm_url = normalizer.normalize(&url).unwrap_or_else(|_| url.clone());
            let page_id = format!("page_{}", uuid::Uuid::new_v4().simple());
            let cap_id = format!("cap_{}", uuid::Uuid::new_v4().simple());

            let _ = conn.execute(
                r#"
                INSERT INTO pages (id, site_id, url, normalized_url, title, first_seen, last_seen, capture_count)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)
                ON CONFLICT(site_id, normalized_url) DO UPDATE SET
                    capture_count = capture_count + 1,
                    title = excluded.title
                "#,
                rusqlite::params![page_id, site.id, url, norm_url, title, ts, ts],
            );

            let actual_page_id: String = conn.query_row(
                "SELECT id FROM pages WHERE site_id = ?1 AND normalized_url = ?2",
                rusqlite::params![site.id, norm_url],
                |r| r.get(0),
            ).unwrap_or(page_id);

            let _ = conn.execute(
                r#"
                INSERT INTO captures (
                    id, page_id, job_id, captured_at, status_code, mime_type,
                    warc_file, warc_offset, warc_length, capture_score
                ) VALUES (?1, ?2, 'wacz_import', ?3, 200, 'text/html', ?4, 0, 0, 100.0)
                "#,
                rusqlite::params![cap_id, actual_page_id, ts, site_warc_rel],
            );
        }

        // B. If cdx_content is present, parse records and map to captures/resources
        if let Some(ref cdx) = unpacked.cdx_content {
            for line in cdx.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') || line.starts_with(" CDX") { continue; }
                if let Some(json_start) = line.find('{') {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line[json_start..]) {
                        let url = v["url"].as_str().unwrap_or_default();
                        let mime = v["mime"].as_str().unwrap_or("application/octet-stream");
                        let status: i32 = v["status"].as_str().and_then(|s| s.parse().ok()).unwrap_or(200);
                        let offset: i64 = v["offset"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0);
                        let length: i64 = v["length"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0);
                        let digest = v["digest"].as_str();

                        if url.is_empty() { continue; }

                        let norm_url = normalizer.normalize(url).unwrap_or_else(|_| url.to_string());
                        if mime.contains("text/html") {
                            let page_id = format!("page_{}", uuid::Uuid::new_v4().simple());
                            let cap_id = format!("cap_{}", uuid::Uuid::new_v4().simple());

                            let _ = conn.execute(
                                r#"
                                INSERT INTO pages (id, site_id, url, normalized_url, title, first_seen, last_seen, capture_count)
                                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)
                                ON CONFLICT(site_id, normalized_url) DO UPDATE SET capture_count = capture_count + 1
                                "#,
                                rusqlite::params![page_id, site.id, url, norm_url, url, now, now],
                            );

                            let actual_page_id: String = conn.query_row(
                                "SELECT id FROM pages WHERE site_id = ?1 AND normalized_url = ?2",
                                rusqlite::params![site.id, norm_url],
                                |r| r.get(0),
                            ).unwrap_or(page_id);

                            let _ = conn.execute(
                                r#"
                                INSERT INTO captures (
                                    id, page_id, job_id, captured_at, status_code, mime_type,
                                    warc_file, warc_offset, warc_length, text_hash, capture_score
                                ) VALUES (?1, ?2, 'wacz_import', ?3, ?4, ?5, ?6, ?7, ?8, ?9, 100.0)
                                "#,
                                rusqlite::params![cap_id, actual_page_id, now, status, mime, site_warc_rel, offset, length, digest],
                            );
                        }
                    }
                }
            }
        }
    }

    let _ = std::fs::remove_dir_all(&temp_dest);
    Ok(site)
}

#[tauri::command]
pub async fn launch_interactive_login(
    state: State<'_, AppState>,
    _site_id: String,
    login_url: String,
) -> Result<u16, String> {
    let browser_path = state
        .browser_path
        .as_ref()
        .ok_or_else(|| "No browser detected. Please configure browser path in settings.".to_string())?;
    let temp_dir = state.db.base_dir.join("temp");
    let (port, _profile_dir) = CredentialManager::launch_interactive_browser(browser_path, &login_url, &temp_dir)
        .await
        .map_err(|e| e.to_string())?;
    Ok(port)
}

#[tauri::command]
pub async fn save_site_credential(
    state: State<'_, AppState>,
    site_id: String,
    port: u16,
    name: String,
) -> Result<SiteCredential, String> {
    let (cookies_json, storage_json) = CredentialManager::extract_browser_credentials(port)
        .await
        .map_err(|e| e.to_string())?;
    state
        .db
        .save_site_credential(&site_id, &name, &cookies_json, &storage_json)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_site_credentials(
    state: State<'_, AppState>,
    site_id: String,
) -> Result<Vec<SiteCredential>, String> {
    state.db.get_site_credentials(&site_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_site_credential(
    state: State<'_, AppState>,
    credential_id: String,
) -> Result<(), String> {
    state.db.delete_site_credential(&credential_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_batch_urls(
    state: State<'_, AppState>,
    site_id: String,
    urls: Vec<String>,
) -> Result<usize, String> {
    state.db.import_batch_urls(&site_id, &urls).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_page_offline(
    state: State<'_, AppState>,
    capture_id: String,
    format: String,
    output_path: String,
) -> Result<String, String> {
    let raw_path = std::path::PathBuf::from(&output_path);
    let resolved_path = if raw_path.is_relative() {
        let export_dir = state.db.base_dir.join("exports");
        let _ = std::fs::create_dir_all(&export_dir);
        export_dir.join(raw_path)
    } else {
        raw_path
    };
    if let Some(parent) = resolved_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match format.to_lowercase().as_str() {
        "singlefile" | "html" => {
            SingleFileExporter::export_single_file_html(&state.db, &capture_id, &resolved_path)
                .map_err(|e| e.to_string())?;
        }
        "pdf" => {
            SingleFileExporter::export_pdf(&state.db, &capture_id, state.replay_port, &resolved_path)
                .await
                .map_err(|e| e.to_string())?;
        }
        other => return Err(format!("Unsupported export format: {}. Use 'singlefile' or 'pdf'.", other)),
    }
    Ok(resolved_path.to_string_lossy().to_string())
}

// Tags
#[tauri::command]
pub async fn create_tag(
    state: State<'_, AppState>,
    name: String,
    color: String,
) -> Result<TagItem, String> {
    state.db.create_tag(&name, &color).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_tags(state: State<'_, AppState>) -> Result<Vec<TagItem>, String> {
    state.db.list_tags().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_tag(state: State<'_, AppState>, tag_id: String) -> Result<(), String> {
    state.db.delete_tag(&tag_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_page_tag(
    state: State<'_, AppState>,
    page_id: String,
    tag_id: String,
) -> Result<(), String> {
    state.db.add_page_tag(&page_id, &tag_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_page_tag(
    state: State<'_, AppState>,
    page_id: String,
    tag_id: String,
) -> Result<(), String> {
    state.db.remove_page_tag(&page_id, &tag_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_page_tags(
    state: State<'_, AppState>,
    page_id: String,
) -> Result<Vec<TagItem>, String> {
    state.db.get_page_tags(&page_id).map_err(|e| e.to_string())
}

// Bookmarks
#[tauri::command]
pub async fn create_bookmark(
    state: State<'_, AppState>,
    capture_id: String,
    note: String,
) -> Result<BookmarkItem, String> {
    state.db.create_bookmark(&capture_id, &note).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_bookmarks(state: State<'_, AppState>) -> Result<Vec<BookmarkItem>, String> {
    state.db.list_bookmarks().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_bookmark(
    state: State<'_, AppState>,
    bookmark_id: String,
) -> Result<(), String> {
    state.db.delete_bookmark(&bookmark_id).map_err(|e| e.to_string())
}

// User Scripts
#[tauri::command]
pub async fn save_user_script(
    state: State<'_, AppState>,
    site_id: Option<String>,
    name: String,
    script_content: String,
    enabled: bool,
) -> Result<UserScriptItem, String> {
    state
        .db
        .save_user_script(site_id.as_deref(), &name, &script_content, enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_user_scripts(
    state: State<'_, AppState>,
    site_id: Option<String>,
) -> Result<Vec<UserScriptItem>, String> {
    state.db.list_user_scripts(site_id.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_user_script(
    state: State<'_, AppState>,
    script_id: String,
) -> Result<(), String> {
    state.db.delete_user_script(&script_id).map_err(|e| e.to_string())
}

// RSS Feeds
#[tauri::command]
pub async fn add_rss_feed(
    state: State<'_, AppState>,
    site_id: String,
    feed_url: String,
    title: Option<String>,
) -> Result<RssFeedItem, String> {
    state.db.add_rss_feed(&site_id, &feed_url, title.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_rss_feeds(
    state: State<'_, AppState>,
    site_id: String,
) -> Result<Vec<RssFeedItem>, String> {
    state.db.list_rss_feeds(&site_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_rss_feed(
    state: State<'_, AppState>,
    feed_id: String,
) -> Result<(), String> {
    state.db.delete_rss_feed(&feed_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn sync_rss_feed(
    state: State<'_, AppState>,
    feed_id: String,
) -> Result<usize, String> {
    state.db.sync_rss_feed(&feed_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_folder(path: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    let target_dir = if p.is_file() {
        p.parent().map(|d| d.to_path_buf()).unwrap_or_else(|| p.clone())
    } else if !p.exists() {
        let _ = std::fs::create_dir_all(&p);
        p.clone()
    } else {
        p.clone()
    };

    #[cfg(target_os = "windows")]
    {
        if p.is_file() {
            std::process::Command::new("explorer")
                .arg(format!("/select,{}", p.to_string_lossy().replace('/', "\\")))
                .spawn()
                .map_err(|e| format!("无法打开目录: {}", e))?;
        } else {
            std::process::Command::new("explorer")
                .arg(target_dir.to_string_lossy().replace('/', "\\"))
                .spawn()
                .map_err(|e| format!("无法打开目录: {}", e))?;
        }
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(target_dir)
            .spawn()
            .map_err(|e| format!("无法打开目录: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(target_dir)
            .spawn()
            .map_err(|e| format!("无法打开目录: {}", e))?;
    }
    Ok(())
}


