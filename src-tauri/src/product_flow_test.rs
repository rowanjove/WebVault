#![cfg(test)]

use crate::archive::service::CapturePersistenceService;
use crate::archive::warc::WarcWriter;
use crate::backup::{export_backup, import_backup};
use crate::capture::cdp::CaptureResult;
use crate::database::Database;
use crate::monitor::MonitorEngine;
use crate::replay::server::ReplayServer;
use crate::search::SearchEngine;
use std::sync::{Arc, Mutex};

fn mock_capture(url: &str, title: &str, text: &str) -> CaptureResult {
    CaptureResult {
        url: url.to_string(),
        title: title.to_string(),
        html: format!("<html><body><p>{}</p></body></html>", text),
        clean_text: text.to_string(),
        links: Vec::new(),
        screenshot_bytes: Some(b"\xff\xd8fakejpg".to_vec()),
        network_records: Vec::new(),
        status_code: 200,
        resource_count: 1,
        missing_resource_count: 0,
        capture_score: 100.0,
    }
}

#[tokio::test]
async fn test_product_flow_persist_search_replay_wacz_backup_delete() {
    let temp = std::env::temp_dir().join(format!("webvault_flow_{}", uuid::Uuid::new_v4().simple()));
    let db = Database::init(&temp).unwrap();
    let site = db.create_site("Example", "https://example.com").unwrap();
    let capture = mock_capture("https://example.com/", "Example Domain", "天气很好 Example Domain");
    let writer = WarcWriter::new(temp.join("archives").join(&site.id).join("data.warc.gz"));
    let persisted = CapturePersistenceService::persist_capture(&db, &site.id, "job_test", &capture, &writer).unwrap();

    let found = SearchEngine::search(&db, "天气", None).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].title, "Example Domain");
    assert!(found[0].snippet.contains("天气") || found[0].snippet.contains("很好"));

    let server = ReplayServer::start(db.clone()).await.unwrap();
    let client = reqwest::Client::new();
    let denied = client
        .get(format!("http://127.0.0.1:{}/replay/{}", server.port, persisted.capture_id))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), reqwest::StatusCode::UNAUTHORIZED);

    let ok = client
        .get(format!(
            "http://127.0.0.1:{}/replay/{}?t={}",
            server.port, persisted.capture_id, server.token
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), reqwest::StatusCode::OK);
    let html = ok.text().await.unwrap();
    assert!(html.contains("天气很好") || html.contains("Example"));

    let missing = client
        .get(format!(
            "http://127.0.0.1:{}/replay/{}/https://example.com/not-captured.png?t={}",
            server.port, persisted.capture_id, server.token
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), reqwest::StatusCode::NOT_FOUND);

    let wacz_path = temp.join("export.wacz");
    crate::archive::wacz::WaczPackage::create_wacz(
        &temp.join("archives").join(&site.id).join("data.warc.gz"),
        &wacz_path,
        "Example Export",
        "test",
        "",
        None,
    )
    .unwrap();
    assert!(wacz_path.exists());

    let zip_path = temp.join("backup.zip");
    export_backup(&db, &zip_path).unwrap();
    assert!(zip_path.exists());

    let restore_dir = temp.join("restore");
    let restore_db = Database::init(&restore_dir).unwrap();
    std::fs::copy(&zip_path, restore_dir.join("backup.zip")).unwrap();
    import_backup(&restore_db, &restore_dir.join("backup.zip")).unwrap();
    assert_eq!(restore_db.list_sites().unwrap().len(), 1);

    db.delete_site(&site.id).unwrap();
    assert!(db.list_sites().unwrap().is_empty());
    assert!(!temp.join("archives").join(&site.id).exists());
    assert!(!temp.join("browser_profiles").join(&site.id).exists());

    let _ = std::fs::remove_dir_all(&temp);
}

#[tokio::test]
async fn test_monitor_http_baseline_no_change_then_change() {
    let html = Arc::new(Mutex::new("<html><body><p>version one</p></body></html>".to_string()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let html_clone = html.clone();
    tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/",
            axum::routing::get(move || {
                let html = html_clone.clone();
                async move { html.lock().unwrap().clone() }
            }),
        );
        let _ = axum::serve(listener, app).await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let temp = std::env::temp_dir().join(format!("webvault_mon_http_{}", uuid::Uuid::new_v4().simple()));
    let db = Database::init(&temp).unwrap();
    let site = db.create_site("MonSite", "http://127.0.0.1").unwrap();
    let monitor = MonitorEngine::new(db.clone());
    let url = format!("http://127.0.0.1:{}/", port);
    let rule = crate::database::models::MonitorRule {
        id: String::new(),
        site_id: site.id.clone(),
        url: url.clone(),
        schedule: "1h".to_string(),
        strategy: "content_hash".to_string(),
        selector: None,
        keyword: None,
        enabled: true,
        last_checked: None,
        next_check: Some(0),
        last_status: None,
    };
    let created = monitor.create_rule(&rule).unwrap();
    {
        let conn = db.lock_conn().unwrap();
        conn.execute(
            "UPDATE monitor_rules SET next_check = 0 WHERE id = ?1",
            rusqlite::params![created.id],
        )
        .unwrap();
    }

    monitor.check_pending_rules().await.unwrap();
    let status: String = {
        let conn = db.lock_conn().unwrap();
        conn.query_row(
            "SELECT last_status FROM monitor_rules WHERE id = ?1",
            rusqlite::params![created.id],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(status, "Baseline recorded");
    assert_eq!(db.list_pages(&site.id).unwrap().len(), 1);

    {
        let conn = db.lock_conn().unwrap();
        conn.execute(
            "UPDATE monitor_rules SET next_check = 0 WHERE id = ?1",
            rusqlite::params![created.id],
        )
        .unwrap();
    }
    monitor.check_pending_rules().await.unwrap();
    let status: String = {
        let conn = db.lock_conn().unwrap();
        conn.query_row(
            "SELECT last_status FROM monitor_rules WHERE id = ?1",
            rusqlite::params![created.id],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(status, "Checked - No Change");
    assert_eq!(db.list_pages(&site.id).unwrap()[0].capture_count, 1);

    *html.lock().unwrap() = "<html><body><p>version two changed</p></body></html>".to_string();
    {
        let conn = db.lock_conn().unwrap();
        conn.execute(
            "UPDATE monitor_rules SET next_check = 0 WHERE id = ?1",
            rusqlite::params![created.id],
        )
        .unwrap();
    }
    monitor.check_pending_rules().await.unwrap();
    let status: String = {
        let conn = db.lock_conn().unwrap();
        conn.query_row(
            "SELECT last_status FROM monitor_rules WHERE id = ?1",
            rusqlite::params![created.id],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert!(status.starts_with("Change Detected"), "status was {}", status);
    assert_eq!(db.list_pages(&site.id).unwrap()[0].capture_count, 2);
    assert_eq!(monitor.list_change_events().unwrap().len(), 1);

    let _ = std::fs::remove_dir_all(&temp);
}
