use crate::archive::service::CapturePersistenceService;
use crate::archive::singlefile::SingleFileExporter;
use crate::archive::warc::WarcWriter;
use crate::capture::browser::BrowserFinder;
use crate::capture::cdp::CdpClient;
use crate::database::Database;
use crate::search::SearchEngine;
use anyhow::{Context, Result};
use std::path::PathBuf;

fn get_default_data_dir() -> PathBuf {
    if let Ok(env_dir) = std::env::var("WEBVAULT_DATA_DIR") {
        let p = PathBuf::from(env_dir);
        let _ = std::fs::create_dir_all(&p);
        return p;
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let p_app = PathBuf::from(&appdata).join("com.webvault.app").join("WebVault");
            if p_app.exists() {
                return p_app;
            }
            let p_dev = PathBuf::from(&appdata).join("com.webvault.dev").join("WebVault");
            if p_dev.exists() {
                return p_dev;
            }
            let local = PathBuf::from("./data/WebVault");
            if local.exists() {
                return local;
            }
            let _ = std::fs::create_dir_all(&p_app);
            return p_app;
        }
    }

    let local = PathBuf::from("./data/WebVault");
    let _ = std::fs::create_dir_all(&local);
    local
}

pub async fn handle_cli(args: Vec<String>) -> Result<bool> {
    if args.len() <= 1 {
        return Ok(false);
    }

    let cmd = args[1].as_str();
    match cmd {
        "--help" | "-h" | "help" => {
            print_help();
            Ok(true)
        }
        "--version" | "-V" | "-v" | "version" => {
            println!("WebVault Local Web Time Machine v0.1.1");
            Ok(true)
        }
        "stats" => {
            let data_dir = get_default_data_dir();
            let db = Database::init(&data_dir)?;
            let conn = db.lock_conn()?;

            let site_count: i64 = conn.query_row("SELECT COUNT(*) FROM sites", [], |r| r.get(0)).unwrap_or(0);
            let page_count: i64 = conn.query_row("SELECT COUNT(*) FROM pages", [], |r| r.get(0)).unwrap_or(0);
            let capture_count: i64 = conn.query_row("SELECT COUNT(*) FROM captures", [], |r| r.get(0)).unwrap_or(0);
            let resource_count: i64 = conn.query_row("SELECT COUNT(*) FROM resources", [], |r| r.get(0)).unwrap_or(0);
            let storage_bytes: i64 = conn.query_row("SELECT COALESCE(SUM(warc_length), 0) FROM captures", [], |r| r.get(0)).unwrap_or(0);

            println!("=== WebVault Storage & Index Stats ===");
            println!("  Storage Directory : {}", data_dir.display());
            println!("  Total Sites       : {}", site_count);
            println!("  Total Pages       : {}", page_count);
            println!("  Total Captures    : {}", capture_count);
            println!("  Total Resources   : {}", resource_count);
            println!("  WARC Storage Size : {:.2} MB ({} bytes)", storage_bytes as f64 / 1_048_576.0, storage_bytes);
            Ok(true)
        }
        "list-sites" | "sites" => {
            let data_dir = get_default_data_dir();
            let db = Database::init(&data_dir)?;
            let sites = db.list_sites()?;
            println!("{:<36}  {:<25}  {:<8}  {}", "SITE ID", "SITE NAME", "CAPTURES", "ROOT URL");
            println!("{}", "-".repeat(95));
            for s in sites {
                println!("{:<36}  {:<25}  {:<8}  {}", s.id, s.name, s.capture_count.unwrap_or(0), s.root_url);
            }
            Ok(true)
        }
        "search" => {
            if args.len() < 3 {
                eprintln!("Error: Missing search keyword. Usage: webvault search <query>");
                return Ok(true);
            }
            let query = &args[2];
            let data_dir = get_default_data_dir();
            let db = Database::init(&data_dir)?;
            let results = SearchEngine::search(&db, query, None)?;

            println!("Found {} results matching '{}':", results.len(), query);
            for (idx, item) in results.iter().enumerate() {
                println!("[{}] {} (Score: {:.2})", idx + 1, item.title, item.score.unwrap_or(0.0));
                println!("    URL: {}", item.url);
                println!("    Capture ID: {}", item.capture_id);
                println!("    Snippet: {}", item.snippet.replace('\n', " "));
                println!();
            }
            Ok(true)
        }
        "serve" => {
            let mut port: u16 = 0;
            let mut i = 2;
            while i < args.len() {
                if args[i] == "--port" && i + 1 < args.len() {
                    port = args[i + 1].parse().unwrap_or(0);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            let data_dir = get_default_data_dir();
            let db = Database::init(&data_dir)?;
            let server = if port == 0 {
                crate::replay::server::ReplayServer::start(db).await?
            } else {
                crate::replay::server::ReplayServer::bind(db, port).await?
            };
            println!("WebVault replay server listening on http://127.0.0.1:{}", server.port);
            println!("Replay URL example: http://127.0.0.1:{}/replay/<capture_id>?t={}", server.port, server.token);
            std::future::pending::<()>().await;
            Ok(true)
        }
        "export" => {
            let mut site_id: Option<String> = None;
            let mut capture_id: Option<String> = None;
            let mut format = "singlefile".to_string();
            let mut output_path = String::new();
            let mut i = 2;
            while i < args.len() {
                if args[i] == "--site-id" && i + 1 < args.len() {
                    site_id = Some(args[i + 1].clone());
                    i += 2;
                } else if args[i] == "--format" && i + 1 < args.len() {
                    format = args[i + 1].clone();
                    i += 2;
                } else if args[i] == "--output" && i + 1 < args.len() {
                    output_path = args[i + 1].clone();
                    i += 2;
                } else if !args[i].starts_with('-') && capture_id.is_none() && site_id.is_none() {
                    capture_id = Some(args[i].clone());
                    i += 1;
                } else {
                    i += 1;
                }
            }

            if let Some(sid) = site_id {
                let data_dir = get_default_data_dir();
                if output_path.is_empty() {
                    output_path = format!("{}.wacz", sid);
                }
                let warc_rel = format!("archives/{}/data.warc.gz", sid);
                let warc_full = data_dir.join(&warc_rel);
                if !warc_full.exists() {
                    anyhow::bail!("No WARC archive exists for this site yet");
                }
                let out = PathBuf::from(&output_path);
                crate::archive::wacz::WaczPackage::create_wacz(
                    &warc_full,
                    &out,
                    &format!("WebVault Export - {}", sid),
                    "Exported from WebVault CLI",
                    "",
                    None,
                )?;
                println!("WACZ exported: {}", output_path);
                return Ok(true);
            }

            let capture_id = match capture_id {
                Some(id) => id,
                None => {
                    eprintln!("Error: Missing capture ID. Usage: webvault export <capture_id> [--format <html|pdf>] [--output <path>]");
                    eprintln!("       webvault export --site-id <SITE_ID> --output archive.wacz");
                    return Ok(true);
                }
            };
            if output_path.is_empty() {
                let ext = if format == "pdf" { "pdf" } else { "html" };
                output_path = format!("webvault_{}_{}.{}", capture_id, chrono::Utc::now().timestamp(), ext);
            }

            let data_dir = get_default_data_dir();
            let db = Database::init(&data_dir)?;
            let path = PathBuf::from(&output_path);

            println!("Exporting capture {} to {} (format: {})...", capture_id, output_path, format);
            if format == "pdf" {
                let replay_server = crate::replay::server::ReplayServer::start(db.clone()).await?;
                SingleFileExporter::export_pdf(&db, &capture_id, replay_server.port, &replay_server.token, &path).await?;
            } else {
                SingleFileExporter::export_single_file_html(&db, &capture_id, &path)?;
            }
            println!("Export completed successfully: {}", output_path);
            Ok(true)
        }
        "capture" => {
            if args.len() < 3 {
                eprintln!("Error: Missing URL. Usage: webvault capture <url> [--autoscroll]");
                return Ok(true);
            }
            let target_url = &args[2];
            let autoscroll = args.iter().any(|a| a == "--autoscroll");

            println!("Initializing browser for capture: {} (autoscroll: {})...", target_url, autoscroll);
            let browser_path = BrowserFinder::find_browser()
                .context("No supported Chromium browser found (Chrome/Edge/Brave)")?;

            let data_dir = get_default_data_dir();
            let db = Database::init(&data_dir)?;

            let port = BrowserFinder::find_available_port();
            let temp_dir = data_dir.join("temp");
            let browser_proc = BrowserFinder::launch(&browser_path, port, &temp_dir).await?;

            println!("Launching CDP session on 127.0.0.1:{}...", browser_proc.port);
            let cdp = CdpClient::new(browser_proc.port);
            let result = cdp.capture_page(target_url, autoscroll, 30).await?;

            println!("Page loaded (HTTP {}) with {} resources captured.", result.status_code, result.resource_count);
            println!("Writing WARC record and updating local catalog...");

            let parsed_url = url::Url::parse(target_url)?;
            let host = parsed_url.host_str().unwrap_or("localhost");
            let root_url = format!("{}://{}", parsed_url.scheme(), host);
            let site = db.create_site("CLI Archived Site", &root_url)?;

            let warc_rel_path = format!("archives/{}/data.warc.gz", site.id);
            let warc_full_path = data_dir.join(&warc_rel_path);
            let writer = WarcWriter::new(&warc_full_path);

            let persisted = CapturePersistenceService::persist_capture(
                &db,
                &site.id,
                "cli",
                &result,
                &writer,
            )?;

            println!("Capture successful!");
            println!("  Capture ID    : {}", persisted.capture_id);
            println!("  Page Title    : {}", result.title);
            println!("  Completeness  : {:.1}%", result.capture_score);
            println!("  Saved in WARC : {}", persisted.warc_path.display());
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn print_help() {
    println!(
        r#"WebVault - Local Web Time Machine (v0.1.1)
High-Fidelity Offline Archiving, Replay & Time-Travel Engine

Usage:
  webvault <command> [arguments...]

Available Commands:
  capture <url> [--autoscroll]              Capture and archive a web page via CDP
  export <capture_id> [--format <html|pdf>] Export snapshot as single-file HTML or PDF
  export --site-id <SITE_ID> --output f.wacz  Export a site archive as WACZ
  serve [--port 8080]                       Start local offline replay HTTP server
  search <query>                            Search text and titles in historical archives
  list-sites                                List all archived sites and domains
  stats                                     Display total captures, pages, and storage usage
  version, --version, -V                    Show application version
  help, --help, -h                          Show this help message

Examples:
  webvault capture https://news.ycombinator.com/ --autoscroll
  webvault search "AI Agent"
  webvault export cap_123 --format html
"#
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cli_help_and_version() {
        let help_res = handle_cli(vec!["webvault".to_string(), "--help".to_string()]).await.unwrap();
        assert!(help_res);

        let ver_res = handle_cli(vec!["webvault".to_string(), "-V".to_string()]).await.unwrap();
        assert!(ver_res);

        let passthrough = handle_cli(vec!["webvault".to_string()]).await.unwrap();
        assert!(!passthrough);
    }
}
