use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Clone)]
pub struct CapturedRequest {
    pub request_id: String,
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub post_data: Option<Vec<u8>>,
    pub timestamp: i64,
}

#[derive(Debug, Clone)]
pub struct CapturedResponse {
    pub request_id: String,
    pub url: String,
    pub status: u16,
    pub status_text: String,
    pub mime_type: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub size: usize,
    pub timestamp: i64,
}

pub struct CaptureResult {
    pub url: String,
    pub title: String,
    pub html: String,
    pub clean_text: String,
    pub links: Vec<String>,
    pub screenshot_bytes: Option<Vec<u8>>,
    pub network_records: Vec<(CapturedRequest, CapturedResponse)>,
    pub status_code: u16,
    pub resource_count: usize,
    pub missing_resource_count: usize,
    pub capture_score: f64,
}

pub struct CdpClient {
    port: u16,
    next_id: AtomicU64,
}

impl CdpClient {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            next_id: AtomicU64::new(1),
        }
    }

    fn next_msg_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Capture a URL using CDP
    pub async fn capture_page(
        &self,
        target_url: &str,
        autoscroll: bool,
        timeout_secs: u64,
    ) -> Result<CaptureResult> {
        self.capture_page_with_credentials(target_url, autoscroll, timeout_secs, None, None, None).await
    }

    /// Capture a URL with optional injected login credentials (cookies and localStorage) and user scripts
    pub async fn capture_page_with_credentials(
        &self,
        target_url: &str,
        autoscroll: bool,
        timeout_secs: u64,
        cookies_json: Option<&str>,
        storage_json: Option<&str>,
        user_scripts: Option<&[String]>,
    ) -> Result<CaptureResult> {
        let http_client = reqwest::Client::new();
        // Open a blank tab first so Network.enable can attach before any page resources load.
        let encoded_blank: String = url::form_urlencoded::byte_serialize(b"about:blank").collect();
        let new_target_url = format!("http://127.0.0.1:{}/json/new?{}", self.port, encoded_blank);
        let target_info = http_client
            .put(&new_target_url)
            .send()
            .await
            .context("Failed to create new browser page target")?
            .json::<serde_json::Value>()
            .await?;

        let target_id = target_info["id"]
            .as_str()
            .context("Missing target id")?
            .to_string();
        let ws_url = target_info["webSocketDebuggerUrl"]
            .as_str()
            .context("Missing webSocketDebuggerUrl in target info")?;

        let (ws_stream, _) = connect_async(ws_url)
            .await
            .with_context(|| format!("Failed to connect to page CDP websocket: {}", ws_url))?;

        let (write, mut read) = ws_stream.split();
        let write = Arc::new(Mutex::new(write));

        // Helper to send CDP command
        let send_cmd = {
            let write = write.clone();
            move |id: u64, method: &str, params: serde_json::Value| {
                let write = write.clone();
                let method = method.to_string();
                async move {
                    let msg = json!({
                        "id": id,
                        "method": method,
                        "params": params
                    });
                    let mut lock = write.lock().await;
                    lock.send(Message::Text(msg.to_string().into())).await
                }
            }
        };

        let mut id = self.next_msg_id();
        send_cmd(id, "Page.enable", json!({})).await?;

        // Mask automated browser indicators to avoid cloudflare/anti-bot blockage on popular sites
        let stealth_script = r#"
            (() => {
                try {
                    Object.defineProperty(navigator, 'webdriver', {
                        get: () => undefined,
                        configurable: true
                    });
                    delete Object.getPrototypeOf(navigator).webdriver;
                } catch(e) {}

                try {
                    const makePlugin = (name, filename, description) => {
                        const plugin = Object.create(Plugin.prototype);
                        Object.defineProperties(plugin, {
                            name: { value: name, enumerable: true },
                            filename: { value: filename, enumerable: true },
                            description: { value: description, enumerable: true },
                            length: { value: 1, enumerable: true },
                        });
                        return plugin;
                    };
                    const plugins = [
                        makePlugin('PDF Viewer', 'internal-pdf-viewer', 'Portable Document Format'),
                        makePlugin('Chrome PDF Viewer', 'internal-pdf-viewer', 'Portable Document Format'),
                        makePlugin('Chromium PDF Viewer', 'internal-pdf-viewer', 'Portable Document Format'),
                        makePlugin('Microsoft Edge PDF Viewer', 'internal-pdf-viewer', 'Portable Document Format'),
                        makePlugin('WebKit built-in PDF', 'internal-pdf-viewer', 'Portable Document Format')
                    ];
                    Object.defineProperty(navigator, 'plugins', {
                        get: () => plugins,
                        enumerable: true,
                        configurable: true
                    });
                } catch(e) {}

                try {
                    Object.defineProperty(navigator, 'languages', {
                        get: () => ['zh-CN', 'zh', 'en-US', 'en'],
                        enumerable: true,
                        configurable: true
                    });
                } catch(e) {}

                try {
                    if (!window.chrome) {
                        window.chrome = {};
                    }
                    if (!window.chrome.runtime) {
                        window.chrome.runtime = {
                            PlatformOs: { MAC: 'mac', WIN: 'win', ANDROID: 'android', CROS: 'cros', LINUX: 'linux', OPENBSD: 'openbsd' },
                            PlatformArch: { ARM: 'arm', X86_32: 'x86-32', X86_64: 'x86-64', MIPS: 'mips', MIPS64: 'mips64' },
                            PlatformNaclArch: { ARM: 'arm', X86_32: 'x86-32', X86_64: 'x86-64', MIPS: 'mips', MIPS64: 'mips64' },
                            connect: () => {},
                            sendMessage: () => {}
                        };
                    }
                    if (!window.chrome.app) {
                        window.chrome.app = {
                            isInstalled: false,
                            InstallState: { DISABLED: 'disabled', INSTALLED: 'installed', NOT_INSTALLED: 'not_installed' },
                            RunningState: { CANNOT_RUN: 'cannot_run', READY_TO_RUN: 'ready_to_run', RUNNING: 'running' }
                        };
                    }
                } catch(e) {}

                try {
                    if (window.navigator && window.navigator.permissions) {
                        const origQuery = window.navigator.permissions.query;
                        window.navigator.permissions.query = (params) => {
                            if (params && params.name === 'notifications') {
                                return Promise.resolve({
                                    state: window.Notification && Notification.permission === 'default' ? 'prompt' : (Notification.permission || 'prompt'),
                                    onchange: null
                                });
                            }
                            return origQuery.call(window.navigator.permissions, params);
                        };
                    }
                } catch(e) {}
            })()
        "#;
        id = self.next_msg_id();
        let _ = send_cmd(id, "Page.addScriptToEvaluateOnNewDocument", json!({ "source": stealth_script })).await;

        id = self.next_msg_id();
        let _ = send_cmd(id, "Emulation.setDeviceMetricsOverride", json!({
            "width": 1920,
            "height": 1080,
            "deviceScaleFactor": 1,
            "mobile": false
        })).await;

        id = self.next_msg_id();
        let user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
        let _ = send_cmd(id, "Network.setUserAgentOverride", json!({
            "userAgent": user_agent,
            "acceptLanguage": "zh-CN,zh;q=0.9,en;q=0.8",
            "platform": "Win32",
            "userAgentMetadata": {
                "brands": [
                    { "brand": "Chromium", "version": "124" },
                    { "brand": "Google Chrome", "version": "124" },
                    { "brand": "Not-A.Brand", "version": "99" }
                ],
                "fullVersion": "124.0.0.0",
                "platform": "Windows",
                "platformVersion": "10.0.0",
                "architecture": "x86",
                "model": "",
                "mobile": false,
                "bitness": "64",
                "wow64": false
            }
        })).await;

        if let Some(scripts) = user_scripts {
            for script in scripts {
                id = self.next_msg_id();
                let _ = send_cmd(id, "Page.addScriptToEvaluateOnNewDocument", json!({ "source": script })).await;
            }
        }
        id = self.next_msg_id();
        send_cmd(id, "Network.enable", json!({ "maxTotalBufferSize": 104857600, "maxResourceBufferSize": 52428800 })).await?;
        id = self.next_msg_id();
        let _ = send_cmd(id, "Network.setCacheDisabled", json!({ "cacheDisabled": true })).await;

        if let Some(cj) = cookies_json {
            if let Ok(cookies_val) = serde_json::from_str::<serde_json::Value>(cj) {
                if let Some(arr) = cookies_val.as_array() {
                    id = self.next_msg_id();
                    let _ = send_cmd(id, "Network.setCookies", json!({ "cookies": arr })).await;
                }
            }
        }

        // Block common ad/analytics trackers to keep archive focused and reduce bloat
        let blocked_patterns = vec![
            "*google-analytics.com*",
            "*googletagmanager.com*",
            "*doubleclick.net*",
            "*googlesyndication.com*",
            "*adservice.google.*",
            "*facebook.net/en_US/fbevents.js*",
            "*connect.facebook.net*",
            "*scorecardresearch.com*",
            "*adnxs.com*",
            "*clarity.ms*",
        ];
        id = self.next_msg_id();
        let _ = send_cmd(id, "Network.setBlockedURLs", json!({ "urls": blocked_patterns })).await;

        id = self.next_msg_id();
        send_cmd(id, "Runtime.enable", json!({})).await?;

        if let Some(sj) = storage_json {
            let script = format!(
                r#"try {{ const data = {}; for (const [k, v] of Object.entries(data)) {{ localStorage.setItem(k, v); }} }} catch(e) {{}}"#,
                sj
            );
            id = self.next_msg_id();
            let _ = send_cmd(id, "Page.addScriptToEvaluateOnNewDocument", json!({ "source": script })).await;
        }

        id = self.next_msg_id();
        send_cmd(id, "DOM.enable", json!({})).await?;

        let requests: Arc<Mutex<HashMap<String, CapturedRequest>>> = Arc::new(Mutex::new(HashMap::new()));
        let responses: Arc<Mutex<HashMap<String, CapturedResponse>>> = Arc::new(Mutex::new(HashMap::new()));
        let pending_bodies: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let main_status: Arc<Mutex<u16>> = Arc::new(Mutex::new(200));

        // Wait for page load and events
        let start_time = tokio::time::Instant::now();
        let deadline = start_time + Duration::from_secs(timeout_secs);
        id = self.next_msg_id();
        send_cmd(id, "Page.navigate", json!({ "url": target_url })).await?;

        let mut loaded = false;
        let mut cf_detected = false;
        let mut cf_resolved = false;
        let mut cf_check_sent_id: Option<u64> = None;
        let mut cf_retry_count: u32 = 0;
        let mut cf_next_check: Option<tokio::time::Instant> = None;
        let mut scroll_triggered = false;
        let mut settle_time: Option<tokio::time::Instant> = None;

        let requests_ref = requests.clone();
        let responses_ref = responses.clone();
        let pending_bodies_ref = pending_bodies.clone();
        let main_status_ref = main_status.clone();
        let target_url_str = target_url.to_string();

        let cf_probe_js = r#"
            (() => {
                const title = (document.title || '').toLowerCase();
                const text = document.body ? (document.body.innerText || '').toLowerCase() : '';
                const hasCfTitle = title.includes('just a moment') || title.includes('attention required') || title.includes('cloudflare');
                const hasCfElements = !!document.querySelector('#challenge-running, #challenge-stage, #cf-wrapper, iframe[src*="cloudflare"], iframe[src*="turnstile"]');
                const hasCfText = text.includes('checking your browser') || text.includes('verifying you are human') || text.includes('enable javascript and cookies');
                const iframe = document.querySelector('iframe[src*="cloudflare"], iframe[src*="turnstile"]');
                let iframeBox = null;
                if (iframe) {
                    try {
                        const rect = iframe.getBoundingClientRect();
                        if (rect.width > 0 && rect.height > 0) {
                            iframeBox = { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
                        }
                    } catch(e) {}
                }
                return JSON.stringify({
                    is_cf: hasCfTitle || hasCfElements || hasCfText,
                    title: document.title || '',
                    iframe_box: iframeBox
                });
            })()
        "#;

        while tokio::time::Instant::now() < deadline {
            tokio::select! {
                Some(msg_result) = read.next() => {
                    if let Ok(Message::Text(text)) = msg_result {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                            if let Some(method) = val["method"].as_str() {
                                match method {
                                    "Network.requestWillBeSent" => {
                                        let req = &val["params"]["request"];
                                        let req_id = val["params"]["requestId"].as_str().unwrap_or_default().to_string();
                                        let url = req["url"].as_str().unwrap_or_default().to_string();
                                        let method = req["method"].as_str().unwrap_or("GET").to_string();

                                        let mut headers = HashMap::new();
                                        if let Some(h) = req["headers"].as_object() {
                                            for (k, v) in h {
                                                if let Some(vs) = v.as_str() {
                                                    headers.insert(k.to_lowercase(), vs.to_string());
                                                }
                                            }
                                        }

                                        let post_data = req["postData"].as_str().map(|s| s.as_bytes().to_vec());
                                        let mut reqs = requests_ref.lock().await;
                                        reqs.insert(req_id.clone(), CapturedRequest {
                                            request_id: req_id,
                                            url,
                                            method,
                                            headers,
                                            post_data,
                                            timestamp: chrono::Utc::now().timestamp_millis(),
                                        });
                                    }
                                    "Network.responseReceived" => {
                                        let resp = &val["params"]["response"];
                                        let req_id = val["params"]["requestId"].as_str().unwrap_or_default().to_string();
                                        let url = resp["url"].as_str().unwrap_or_default().to_string();
                                        let status = resp["status"].as_u64().unwrap_or(200) as u16;
                                        let status_text = resp["statusText"].as_str().unwrap_or("OK").to_string();
                                        let mime_type = resp["mimeType"].as_str().unwrap_or("application/octet-stream").to_string();

                                        if url == target_url_str || url.starts_with(&target_url_str) {
                                            let mut st = main_status_ref.lock().await;
                                            *st = status;
                                        }

                                        let mut headers = HashMap::new();
                                        if let Some(h) = resp["headers"].as_object() {
                                            for (k, v) in h {
                                                if let Some(vs) = v.as_str() {
                                                    headers.insert(k.to_lowercase(), vs.to_string());
                                                }
                                            }
                                        }

                                        let mut resps = responses_ref.lock().await;
                                        resps.insert(req_id, CapturedResponse {
                                            request_id: val["params"]["requestId"].as_str().unwrap_or_default().to_string(),
                                            url,
                                            status,
                                            status_text,
                                            mime_type,
                                            headers,
                                            body: Vec::new(),
                                            size: 0,
                                            timestamp: chrono::Utc::now().timestamp_millis(),
                                        });
                                    }
                                    "Network.loadingFinished" => {
                                        let req_id = val["params"]["requestId"].as_str().unwrap_or_default().to_string();
                                        let mut pb = pending_bodies_ref.lock().await;
                                        pb.push(req_id);
                                    }
                                    "Page.loadEventFired" | "Page.domContentEventFired" => {
                                        loaded = true;
                                    }
                                    _ => {}
                                }
                            } else if let Some(msg_id) = val["id"].as_u64() {
                                if Some(msg_id) == cf_check_sent_id {
                                    cf_check_sent_id = None;
                                    if let Some(res_val) = val["result"]["result"]["value"].as_str() {
                                        if let Ok(cf_status) = serde_json::from_str::<serde_json::Value>(res_val) {
                                            let is_cf = cf_status["is_cf"].as_bool().unwrap_or(false);
                                            let title = cf_status["title"].as_str().unwrap_or_default();
                                            if is_cf {
                                                cf_detected = true;
                                                tracing::info!("Cloudflare challenge detected (title: '{}', retry #{}), waiting for resolution...", title, cf_retry_count);

                                                if let Some(box_val) = cf_status.get("iframe_box") {
                                                    if !box_val.is_null() {
                                                        let x = box_val["x"].as_f64().unwrap_or(0.0);
                                                        let y = box_val["y"].as_f64().unwrap_or(0.0);
                                                        let click_x = x + 28.0;
                                                        let click_y = y + 28.0;
                                                        if click_x > 0.0 && click_y > 0.0 {
                                                            let c_id1 = self.next_msg_id();
                                                            let _ = send_cmd(c_id1, "Input.dispatchMouseEvent", json!({
                                                                "type": "mouseMoved",
                                                                "x": click_x,
                                                                "y": click_y
                                                            })).await;
                                                            let c_id2 = self.next_msg_id();
                                                            let _ = send_cmd(c_id2, "Input.dispatchMouseEvent", json!({
                                                                "type": "mousePressed",
                                                                "x": click_x,
                                                                "y": click_y,
                                                                "button": "left",
                                                                "clickCount": 1
                                                            })).await;
                                                            let c_id3 = self.next_msg_id();
                                                            let _ = send_cmd(c_id3, "Input.dispatchMouseEvent", json!({
                                                                "type": "mouseReleased",
                                                                "x": click_x,
                                                                "y": click_y,
                                                                "button": "left",
                                                                "clickCount": 1
                                                            })).await;
                                                        }
                                                    }
                                                }

                                                if cf_retry_count < 15 {
                                                    cf_retry_count += 1;
                                                    cf_next_check = Some(tokio::time::Instant::now() + Duration::from_millis(800));
                                                } else {
                                                    tracing::warn!("Cloudflare challenge wait limit reached (15 retries), proceeding with capture.");
                                                    cf_detected = false;
                                                    cf_resolved = true;
                                                }
                                            } else {
                                                if cf_detected {
                                                    tracing::info!("Cloudflare challenge resolved! Resuming capture.");
                                                }
                                                cf_detected = false;
                                                cf_resolved = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                _ = sleep(Duration::from_millis(100)) => {
                    let timed_out_wait = tokio::time::Instant::now().duration_since(start_time) >= Duration::from_secs(6);
                    if (loaded || timed_out_wait) && !scroll_triggered {
                        if !cf_resolved {
                            if cf_check_sent_id.is_none() {
                                if let Some(next_t) = cf_next_check {
                                    if tokio::time::Instant::now() >= next_t {
                                        cf_next_check = None;
                                        let cid = self.next_msg_id();
                                        cf_check_sent_id = Some(cid);
                                        let _ = send_cmd(cid, "Runtime.evaluate", json!({
                                            "expression": cf_probe_js,
                                            "returnByValue": true
                                        })).await;
                                    }
                                } else if cf_retry_count == 0 {
                                    let cid = self.next_msg_id();
                                    cf_check_sent_id = Some(cid);
                                    let _ = send_cmd(cid, "Runtime.evaluate", json!({
                                        "expression": cf_probe_js,
                                        "returnByValue": true
                                    })).await;
                                }
                            }
                        } else {
                            scroll_triggered = true;

                        // 1. Dismiss common cookie consent banners
                        let cookie_dismiss_js = r#"
                            (() => {
                                const selectors = [
                                    '#onetrust-accept-btn-handler', '#accept-cookies', '#cookie-accept',
                                    '.cookie-banner button', '.cookie-consent button', 'button[id*="cookie" i]',
                                    'button[class*="cookie" i]', 'button[aria-label*="cookie" i]', 'a[id*="cookie" i]'
                                ];
                                for (const sel of selectors) {
                                    try {
                                        const el = document.querySelector(sel);
                                        if (el && typeof el.click === 'function') { el.click(); break; }
                                    } catch (e) {}
                                }
                            })()
                        "#;
                        id = self.next_msg_id();
                        let _ = send_cmd(id, "Runtime.evaluate", json!({ "expression": cookie_dismiss_js })).await;

                        // 2. Perform autoscroll WHILE network listening remains active to trigger lazyloaded images
                        if autoscroll {
                            let scroll_and_lazy_js = r#"
                                (async () => {
                                    const totalHeight = document.body.scrollHeight;
                                    const step = Math.min(window.innerHeight * 0.8, 600);
                                    let current = 0;
                                    while (current < totalHeight && current < 20000) {
                                        window.scrollBy(0, step);
                                        current += step;
                                        await new Promise(r => setTimeout(r, 120));
                                    }
                                    // Trigger all lazyloaded image attributes into src
                                    document.querySelectorAll('img').forEach(img => {
                                        const lz = img.getAttribute('data-src') || img.getAttribute('data-original') || img.getAttribute('data-lazy-src');
                                        if (lz && (!img.src || img.src.includes('data:image'))) {
                                            img.src = lz;
                                        }
                                    });
                                    await new Promise(r => setTimeout(r, 300));
                                    window.scrollTo(0, 0);
                                })()
                            "#;
                            id = self.next_msg_id();
                            let _ = send_cmd(id, "Runtime.evaluate", json!({ "expression": scroll_and_lazy_js, "awaitPromise": true })).await;
                        }

                        // Give network 2.0s after scrolling to settle and download thumbnails
                        settle_time = Some(tokio::time::Instant::now() + Duration::from_millis(2200));
                        }
                    }

                    if let Some(st) = settle_time {
                        if tokio::time::Instant::now() >= st {
                            break;
                        }
                    }
                }
            }
        }

        let pb_list = {
            let pb = pending_bodies.lock().await;
            pb.clone()
        };

        let mut msg_to_req = HashMap::new();
        for req_id in pb_list {
            id = self.next_msg_id();
            if send_cmd(id, "Network.getResponseBody", json!({ "requestId": req_id })).await.is_ok() {
                msg_to_req.insert(id, req_id);
            }
        }

        let body_deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while !msg_to_req.is_empty() && tokio::time::Instant::now() < body_deadline {
            match tokio::time::timeout(Duration::from_millis(50), read.next()).await {
                Ok(Some(Ok(Message::Text(text)))) => {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(msg_id) = val["id"].as_u64() {
                            if let Some(req_id) = msg_to_req.remove(&msg_id) {
                                if let Some(body_str) = val["result"]["body"].as_str() {
                                    let is_base64 = val["result"]["base64Encoded"].as_bool().unwrap_or(false);
                                    let bytes = if is_base64 {
                                        use base64::Engine;
                                        base64::engine::general_purpose::STANDARD
                                            .decode(body_str)
                                            .unwrap_or_else(|_| body_str.as_bytes().to_vec())
                                    } else {
                                        body_str.as_bytes().to_vec()
                                    };
                                    let mut resps = responses.lock().await;
                                    if let Some(resp) = resps.get_mut(&req_id) {
                                        resp.size = bytes.len();
                                        resp.body = bytes;
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Some(Err(_))) | Ok(None) => break,
                _ => {}
            }
        }

        // Extract Title, DOM outerHTML, clean innerText, Links, and Canvas snapshot
        id = self.next_msg_id();
        let eval_js_id = id;
        let extraction_script = r#"
            (() => {
                // Snapshot HTML5 / WebGL canvas elements before DOM serialization
                try {
                    const canvases = document.querySelectorAll('canvas');
                    canvases.forEach((cvs) => {
                        try {
                            const dataUrl = cvs.toDataURL('image/png');
                            if (dataUrl && dataUrl.length > 30) {
                                cvs.setAttribute('data-webvault-canvas-snapshot', dataUrl);
                            }
                        } catch (e) {
                            // Ignore tainted/security restricted canvases
                        }
                    });
                } catch (e) {}

                return JSON.stringify({
                    title: document.title || '',
                    html: document.documentElement.outerHTML || '',
                    text: document.body ? document.body.innerText : '',
                    links: Array.from(document.querySelectorAll('a[href]')).map(a => a.href)
                });
            })()
        "#;
        let _ = send_cmd(id, "Runtime.evaluate", json!({ "expression": extraction_script, "returnByValue": true })).await;

        let mut title = String::new();
        let mut html = String::new();
        let mut clean_text = String::new();
        let mut links = Vec::new();

        let eval_timeout = tokio::time::Instant::now() + Duration::from_secs(4);
        while tokio::time::Instant::now() < eval_timeout {
            if let Ok(Some(Ok(Message::Text(text)))) = tokio::time::timeout(Duration::from_millis(200), read.next()).await {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if val["id"].as_u64() == Some(eval_js_id) {
                        if let Some(res_str) = val["result"]["result"]["value"].as_str() {
                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(res_str) {
                                title = parsed["title"].as_str().unwrap_or("").to_string();
                                html = parsed["html"].as_str().unwrap_or("").to_string();
                                clean_text = parsed["text"].as_str().unwrap_or("").to_string();
                                if let Some(arr) = parsed["links"].as_array() {
                                    for l in arr {
                                        if let Some(ls) = l.as_str() {
                                            links.push(ls.to_string());
                                        }
                                    }
                                }
                            }
                        }
                        break;
                    }
                }
            }
        }

        // Capture Full-Page Screenshot
        id = self.next_msg_id();
        let ss_msg_id = id;
        let _ = send_cmd(id, "Page.captureScreenshot", json!({ "format": "jpeg", "quality": 80 })).await;

        let mut screenshot_bytes = None;
        let ss_timeout = tokio::time::Instant::now() + Duration::from_secs(5);
        while tokio::time::Instant::now() < ss_timeout {
            if let Ok(Some(Ok(Message::Text(text)))) = tokio::time::timeout(Duration::from_millis(200), read.next()).await {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if val["id"].as_u64() == Some(ss_msg_id) {
                        if let Some(data_b64) = val["result"]["data"].as_str() {
                            use base64::Engine;
                            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data_b64) {
                                screenshot_bytes = Some(bytes);
                            }
                        }
                        break;
                    }
                }
            }
        }

        // Close page target via HTTP
        let close_url = format!("http://127.0.0.1:{}/json/close/{}", self.port, target_id);
        let _ = http_client.put(&close_url).send().await;

        // Assemble network records
        let reqs = requests.lock().await;
        let resps = responses.lock().await;
        let mut network_records = Vec::new();

        let mut captured_count = 0;
        let mut missing_count = 0;

        for (req_id, req) in reqs.iter() {
            if let Some(resp) = resps.get(req_id) {
                if !resp.body.is_empty() {
                    captured_count += 1;
                } else {
                    missing_count += 1;
                }
                network_records.push((req.clone(), resp.clone()));
            } else {
                missing_count += 1;
            }
        }

        let total_res = captured_count + missing_count;
        let capture_score = if total_res > 0 {
            (captured_count as f64 / total_res as f64) * 100.0
        } else {
            100.0
        };

        let status_code = *main_status.lock().await;

        Ok(CaptureResult {
            url: target_url.to_string(),
            title,
            html,
            clean_text,
            links,
            screenshot_bytes,
            network_records,
            status_code,
            resource_count: total_res,
            missing_resource_count: missing_count,
            capture_score,
        })
    }

    /// Navigate to a page and render to PDF using CDP Page.printToPDF
    pub async fn print_to_pdf(&self, page_url: &str) -> Result<Vec<u8>> {
        let http_client = reqwest::Client::new();
        let encoded_target: String = url::form_urlencoded::byte_serialize(page_url.as_bytes()).collect();
        let new_target_url = format!("http://127.0.0.1:{}/json/new?{}", self.port, encoded_target);
        let target_info = http_client
            .put(&new_target_url)
            .send()
            .await
            .context("Failed to create new browser page target for PDF export")?
            .json::<serde_json::Value>()
            .await?;

        let target_id = target_info["id"]
            .as_str()
            .context("Missing target id")?
            .to_string();
        let ws_url = target_info["webSocketDebuggerUrl"]
            .as_str()
            .context("Missing webSocketDebuggerUrl in target info")?;

        let (ws_stream, _) = connect_async(ws_url)
            .await
            .with_context(|| format!("Failed to connect to page CDP websocket: {}", ws_url))?;

        let (mut write, mut read) = ws_stream.split();

        // Enable Page domain
        let enable_msg = json!({
            "id": 1,
            "method": "Page.enable",
            "params": {}
        });
        write.send(Message::Text(enable_msg.to_string().into())).await?;

        // Wait for page to finish loading (up to 8s)
        let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
        while tokio::time::Instant::now() < deadline {
            tokio::select! {
                Some(Ok(Message::Text(text))) = read.next() => {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                        if val["method"].as_str() == Some("Page.loadEventFired") {
                            break;
                        }
                    }
                }
                _ = sleep(Duration::from_millis(100)) => {}
            }
        }

        // Give a brief moment for layout/fonts to stabilize
        sleep(Duration::from_millis(600)).await;

        let print_msg = json!({
            "id": 2,
            "method": "Page.printToPDF",
            "params": {
                "printBackground": true,
                "preferCSSPageSize": true
            }
        });
        write.send(Message::Text(print_msg.to_string().into())).await?;

        let mut pdf_data = None;
        let pdf_deadline = tokio::time::Instant::now() + Duration::from_secs(12);
        while tokio::time::Instant::now() < pdf_deadline {
            tokio::select! {
                Some(Ok(Message::Text(text))) = read.next() => {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                        if val["id"].as_u64() == Some(2) {
                            if let Some(b64) = val["result"]["data"].as_str() {
                                use base64::Engine;
                                if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
                                    pdf_data = Some(bytes);
                                    break;
                                }
                            }
                        }
                    }
                }
                _ = sleep(Duration::from_millis(50)) => {}
            }
        }

        // Close target
        let _ = http_client
            .put(format!("http://127.0.0.1:{}/json/close/{}", self.port, target_id))
            .send()
            .await;

        pdf_data.context("Failed to generate PDF via CDP: timed out or received empty response")
    }
}

