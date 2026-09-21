use crate::capture::browser::BrowserFinder;
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

pub struct CredentialManager;

impl CredentialManager {
    /// Launch a visible (headful) browser window for interactive user login
    pub async fn launch_interactive_browser(
        browser_path: &Path,
        login_url: &str,
        temp_base: &Path,
    ) -> Result<(u16, PathBuf)> {
        let port = BrowserFinder::find_available_port();
        let profile_dir = temp_base.join(format!("interactive_login_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&profile_dir)?;

        let safe_url = crate::http_url::require_http_url(login_url)?;

        Command::new(browser_path)
            .arg(format!("--remote-debugging-port={}", port))
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-blink-features=AutomationControlled")
            .arg(format!("--user-data-dir={}", profile_dir.to_string_lossy()))
            .arg("--")
            .arg(safe_url.as_str())
            .spawn()
            .with_context(|| format!("Failed to launch visible browser for login at {:?}", browser_path))?;

        let version_endpoint = format!("http://127.0.0.1:{}/json/version", port);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(800))
            .build()?;

        let mut ready = false;
        for _ in 0..30 {
            sleep(Duration::from_millis(200)).await;
            if let Ok(resp) = client.get(&version_endpoint).send().await {
                if resp.status().is_success() {
                    ready = true;
                    break;
                }
            }
        }

        if !ready {
            anyhow::bail!("Timed out waiting for interactive browser to open on port {}", port);
        }

        Ok((port, profile_dir))
    }

    /// Extract cookies and localStorage from the active browser session via CDP
    pub async fn extract_browser_credentials(port: u16) -> Result<(String, String)> {
        let http_client = reqwest::Client::new();
        let list_url = format!("http://127.0.0.1:{}/json/list", port);
        let targets = http_client
            .get(&list_url)
            .send()
            .await
            .context("Failed to query browser targets")?
            .json::<Vec<serde_json::Value>>()
            .await?;

        // Pick page target
        let page_target = targets
            .iter()
            .find(|t| t["type"].as_str() == Some("page"))
            .or_else(|| targets.first())
            .context("No page target found in browser")?;

        let ws_url = page_target["webSocketDebuggerUrl"]
            .as_str()
            .context("Target missing webSocketDebuggerUrl")?;

        let (ws_stream, _) = connect_async(ws_url)
            .await
            .with_context(|| format!("Failed to connect to CDP: {}", ws_url))?;

        let (mut write, mut read) = ws_stream.split();

        // 1. Enable domains
        let msg_net_enable = json!({ "id": 1, "method": "Network.enable", "params": {} });
        write.send(Message::Text(msg_net_enable.to_string().into())).await?;

        let msg_rt_enable = json!({ "id": 2, "method": "Runtime.enable", "params": {} });
        write.send(Message::Text(msg_rt_enable.to_string().into())).await?;

        // 2. Fetch all cookies
        let msg_get_cookies = json!({ "id": 3, "method": "Network.getCookies", "params": {} });
        write.send(Message::Text(msg_get_cookies.to_string().into())).await?;

        // 3. Fetch localStorage
        let msg_get_storage = json!({
            "id": 4,
            "method": "Runtime.evaluate",
            "params": {
                "expression": "try { JSON.stringify(window.localStorage); } catch (e) { '{}'; }",
                "returnByValue": true
            }
        });
        write.send(Message::Text(msg_get_storage.to_string().into())).await?;

        let mut cookies_json = "[]".to_string();
        let mut storage_json = "{}".to_string();
        let mut got_cookies = false;
        let mut got_storage = false;

        let deadline = tokio::time::Instant::now() + Duration::from_secs(6);
        while tokio::time::Instant::now() < deadline && (!got_cookies || !got_storage) {
            tokio::select! {
                Some(Ok(Message::Text(text))) = read.next() => {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                        if val["id"].as_u64() == Some(3) {
                            if let Some(cookies) = val["result"]["cookies"].as_array() {
                                cookies_json = serde_json::to_string(cookies).unwrap_or_else(|_| "[]".to_string());
                                got_cookies = true;
                            }
                        } else if val["id"].as_u64() == Some(4) {
                            if let Some(raw_str) = val["result"]["result"]["value"].as_str() {
                                storage_json = raw_str.to_string();
                                got_storage = true;
                            }
                        }
                    }
                }
                _ = sleep(Duration::from_millis(50)) => {}
            }
        }

        // Close interactive browser cleanly
        if let Some(target_id) = page_target["id"].as_str() {
            let _ = http_client.put(format!("http://127.0.0.1:{}/json/close/{}", port, target_id)).send().await;
        }

        if let Ok(version_resp) = http_client.get(format!("http://127.0.0.1:{}/json/version", port)).send().await {
            if let Ok(v_json) = version_resp.json::<serde_json::Value>().await {
                if let Some(browser_ws) = v_json["webSocketDebuggerUrl"].as_str() {
                    if let Ok((mut b_ws, _)) = connect_async(browser_ws).await {
                        let msg_b_close = json!({ "id": 100, "method": "Browser.close", "params": {} });
                        let _ = b_ws.send(Message::Text(msg_b_close.to_string().into())).await;
                    }
                }
            }
        }
        sleep(Duration::from_millis(200)).await;

        Ok((cookies_json, storage_json))
    }
}
