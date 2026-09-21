pub mod archive;
pub mod backup;
pub mod capture;
pub mod cli;
pub mod commands;
pub mod crawler;
pub mod database;
pub mod diff;
pub mod http_url;
pub mod monitor;
pub mod protect;
pub mod replay;
pub mod search;
pub mod wayback;

#[cfg(test)]
mod product_flow_test;

use capture::browser::BrowserFinder;
use commands::*;
use database::Database;
use replay::server::ReplayServer;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;
use wayback::WaybackProvider;

fn init_logging(base_dir: &std::path::Path) {
    let log_dir = base_dir.join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let env_filter = tracing_subscriber::EnvFilter::new("webvault=debug,info");
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("webvault.log"))
    {
        Ok(file) => {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_ansi(false)
                .with_writer(std::sync::Mutex::new(TeeWriter { file }))
                .try_init();
        }
        Err(_) => {
            let _ = tracing_subscriber::fmt().with_env_filter(env_filter).try_init();
        }
    }
}

struct TeeWriter {
    file: std::fs::File,
}

impl std::io::Write for TeeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = std::io::stderr().write_all(buf);
        self.file.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let _ = std::io::stderr().flush();
        self.file.flush()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("./data"));
            let base_dir = app_data.join("WebVault");
            std::fs::create_dir_all(&base_dir)?;
            init_logging(&base_dir);

            let db = Database::init(&base_dir)?;
            let browser_path = BrowserFinder::find_browser();
            let wayback = WaybackProvider::new();

            // Run async setup for replay server inside tokio runtime
            let db_clone = db.clone();
            let app_handle = app.handle().clone();

            tauri::async_runtime::block_on(async move {
                let replay_server = ReplayServer::start(db_clone).await.expect("Failed to start ReplayServer");
                let replay_port = replay_server.port;
                let replay_token = replay_server.token.clone();
                let monitor_db = db.clone();
                let monitor_browser = browser_path.clone();
                tauri::async_runtime::spawn(async move {
                    let monitor = crate::monitor::MonitorEngine::new(monitor_db).with_browser(monitor_browser);
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                        if let Err(e) = monitor.check_pending_rules().await {
                            tracing::warn!("Monitor check encountered an error: {:?}", e);
                        }
                    }
                });

                let state = AppState {
                    db,
                    browser_path,
                    replay_port,
                    replay_token,
                    wayback,
                    active_jobs: Arc::new(Mutex::new(HashMap::new())),
                };

                app_handle.manage(state);
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_system_stats,
            list_sites,
            create_site,
            delete_site,
            get_site_profile,
            update_site_profile,
            list_pages,
            list_captures,
            get_capture_details,
            start_single_capture,
            start_crawl_job,
            list_jobs,
            cancel_job,
            get_replay_url,
            search_archive,
            compare_captures,
            list_monitor_rules,
            create_monitor_rule,
            toggle_monitor_rule,
            delete_monitor_rule,
            list_change_events,
            trigger_monitor_check,
            query_wayback,
            import_wayback_capture,
            export_wacz,
            import_wacz,
            export_backup,
            import_backup,
            launch_interactive_login,
            save_site_credential,
            get_site_credentials,
            delete_site_credential,
            import_batch_urls,
            export_page_offline,
            create_tag,
            list_tags,
            delete_tag,
            add_page_tag,
            remove_page_tag,
            get_page_tags,
            create_bookmark,
            list_bookmarks,
            delete_bookmark,
            save_user_script,
            list_user_scripts,
            delete_user_script,
            add_rss_feed,
            list_rss_feeds,
            delete_rss_feed,
            sync_rss_feed,
            open_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running WebVault application");
}
