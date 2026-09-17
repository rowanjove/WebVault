pub mod archive;
pub mod capture;
pub mod cli;
pub mod commands;
pub mod crawler;
pub mod database;
pub mod diff;
pub mod monitor;
pub mod replay;
pub mod search;
pub mod wayback;

use capture::browser::BrowserFinder;
use commands::*;
use database::Database;
use replay::server::ReplayServer;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;
use wayback::WaybackProvider;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing subscriber
    let _ = tracing_subscriber::fmt()
        .with_env_filter("webvault=debug,info")
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_data = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("./data"));
            let base_dir = app_data.join("WebVault");
            std::fs::create_dir_all(&base_dir)?;

            let db = Database::init(&base_dir)?;
            let browser_path = BrowserFinder::find_browser();
            let wayback = WaybackProvider::new();

            // Run async setup for replay server inside tokio runtime
            let db_clone = db.clone();
            let app_handle = app.handle().clone();

            tauri::async_runtime::block_on(async move {
                let replay_server = ReplayServer::start(db_clone).await.expect("Failed to start ReplayServer");
                let replay_port = replay_server.port;
                let monitor_db = db.clone();
                tauri::async_runtime::spawn(async move {
                    let monitor = crate::monitor::MonitorEngine::new(monitor_db);
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
