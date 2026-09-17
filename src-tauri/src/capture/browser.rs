use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use tokio::time::{sleep, Duration};

pub struct BrowserProcess {
    pub port: u16,
    pub child: Option<Child>,
    pub user_data_dir: PathBuf,
    pub is_temporary_profile: bool,
    pub ws_url: String,
}

impl Drop for BrowserProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if self.is_temporary_profile {
            let _ = std::fs::remove_dir_all(&self.user_data_dir);
        }
    }
}

#[derive(Debug, Clone)]
pub struct BrowserLaunchOptions {
    pub headless: bool,
    pub user_data_dir: Option<PathBuf>,
    pub window_width: u32,
    pub window_height: u32,
}

impl Default for BrowserLaunchOptions {
    fn default() -> Self {
        Self {
            headless: true,
            user_data_dir: None,
            window_width: 1920,
            window_height: 1080,
        }
    }
}

pub struct BrowserFinder;

impl BrowserFinder {
    pub fn find_browser() -> Option<PathBuf> {
        let candidates = [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        ];

        for c in &candidates {
            let p = PathBuf::from(c);
            if p.exists() {
                return Some(p);
            }
        }

        let user_profile = std::env::var("USERPROFILE").unwrap_or_default();
        let user_candidates = [
            format!(r"{}\AppData\Local\Google\Chrome\Application\chrome.exe", user_profile),
            format!(r"{}\AppData\Local\Microsoft\Edge\Application\msedge.exe", user_profile),
        ];

        for c in &user_candidates {
            let p = PathBuf::from(c);
            if p.exists() {
                return Some(p);
            }
        }

        let all_candidates = [
            PathBuf::from("/usr/bin/google-chrome"),
            PathBuf::from("/usr/bin/google-chrome-stable"),
            PathBuf::from("/usr/bin/chromium-browser"),
            PathBuf::from("/usr/bin/chromium"),
            PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
            PathBuf::from("/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"),
        ];

        for p in &all_candidates {
            if p.exists() {
                return Some(p.clone());
            }
        }
        None
    }

    pub fn find_available_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .map(|addr| addr.port())
            .unwrap_or(9222)
    }

    pub fn build_command_args(
        port: u16,
        user_data_dir: &Path,
        options: &BrowserLaunchOptions,
    ) -> Vec<String> {
        let mut args = vec![format!("--remote-debugging-port={}", port)];
        if options.headless {
            args.push("--headless=new".to_string());
        }
        // Note: Do NOT add --disable-gpu; hardware acceleration is essential so WebGL returns real GPU vendor instead of SwiftShader
        args.extend(vec![
            format!("--window-size={},{}", options.window_width, options.window_height),
            "--disable-blink-features=AutomationControlled".to_string(),
            "--lang=zh-CN,zh,en-US,en".to_string(),
            "--no-first-run".to_string(),
            "--no-default-browser-check".to_string(),
            "--mute-audio".to_string(),
            format!("--user-data-dir={}", user_data_dir.to_string_lossy()),
            "about:blank".to_string(),
        ]);
        args
    }

    pub async fn launch_with_options(
        browser_path: &Path,
        port: u16,
        temp_base: &Path,
        options: &BrowserLaunchOptions,
    ) -> Result<BrowserProcess> {
        let (user_data_dir, is_temporary_profile) = match &options.user_data_dir {
            Some(dir) => {
                std::fs::create_dir_all(dir)?;
                (dir.clone(), false)
            }
            None => {
                let dir = temp_base.join(format!("cdp_profile_{}", uuid::Uuid::new_v4().simple()));
                std::fs::create_dir_all(&dir)?;
                (dir, true)
            }
        };

        let args = Self::build_command_args(port, &user_data_dir, options);
        let mut cmd = Command::new(browser_path);
        cmd.args(&args);

        let child = cmd
            .spawn()
            .with_context(|| format!("Failed to launch browser executable at {:?}", browser_path))?;

        let version_endpoint = format!("http://127.0.0.1:{}/json/version", port);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(800))
            .build()?;

        let mut ws_url = None;
        for _ in 0..30 {
            sleep(Duration::from_millis(200)).await;
            if let Ok(resp) = client.get(&version_endpoint).send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(ws) = json["webSocketDebuggerUrl"].as_str() {
                        ws_url = Some(ws.to_string());
                        break;
                    }
                }
            }
        }

        let ws_url = ws_url.context("Failed to connect to browser CDP port within timeout")?;

        Ok(BrowserProcess {
            port,
            child: Some(child),
            user_data_dir,
            is_temporary_profile,
            ws_url,
        })
    }

    pub async fn launch(browser_path: &Path, port: u16, temp_base: &Path) -> Result<BrowserProcess> {
        Self::launch_with_options(browser_path, port, temp_base, &BrowserLaunchOptions::default()).await
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_stealth_browser_args_headless() {
        let options = BrowserLaunchOptions::default();
        let user_data = PathBuf::from("C:/fake/user_data");
        let args = BrowserFinder::build_command_args(9222, &user_data, &options);

        // Verify key anti-bot evasion arguments
        assert!(args.iter().any(|a| a == "--headless=new"), "Must use modern --headless=new");
        assert!(args.iter().any(|a| a == "--disable-blink-features=AutomationControlled"), "Must disable automation flag");
        assert!(args.iter().any(|a| a.starts_with("--remote-debugging-port=9222")));

        // Critical: Verify that --disable-gpu is NOT present to prevent SwiftShader leak
        assert!(!args.iter().any(|a| a == "--disable-gpu"), "Must NOT disable GPU to prevent SwiftShader detection");
    }

    #[test]
    fn test_stealth_browser_args_headed() {
        let mut options = BrowserLaunchOptions::default();
        options.headless = false;
        let user_data = PathBuf::from("C:/fake/user_data");
        let args = BrowserFinder::build_command_args(9222, &user_data, &options);

        // Headed mode should NOT contain headless flag
        assert!(!args.iter().any(|a| a.contains("headless")));
        assert!(args.iter().any(|a| a == "--disable-blink-features=AutomationControlled"));
    }
}
