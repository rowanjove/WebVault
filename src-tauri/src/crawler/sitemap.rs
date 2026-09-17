use regex::Regex;
use std::time::Duration;
use url::Url;

pub struct SitemapFinder;

impl SitemapFinder {
    pub async fn discover_sitemap_urls(root_url: &str, limit: usize) -> Vec<String> {
        let base_parsed = match Url::parse(root_url) {
            Ok(u) => u,
            Err(_) => return Vec::new(),
        };

        let host = match base_parsed.host_str() {
            Some(h) => h,
            None => return Vec::new(),
        };

        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
        {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let candidates = vec![
            format!("{}://{}/sitemap.xml", base_parsed.scheme(), host),
            format!("{}://{}/sitemap_index.xml", base_parsed.scheme(), host),
        ];

        let mut discovered = Vec::new();
        let loc_regex = match Regex::new(r"(?i)<loc>\s*(https?://[^<\s]+)\s*</loc>") {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        for candidate in candidates {
            if let Ok(resp) = client.get(&candidate).send().await {
                if resp.status().is_success() {
                    if let Ok(text) = resp.text().await {
                        for cap in loc_regex.captures_iter(&text) {
                            if let Some(matched) = cap.get(1) {
                                let found_url = matched.as_str().trim().to_string();
                                if !discovered.contains(&found_url) {
                                    discovered.push(found_url);
                                    if discovered.len() >= limit {
                                        return discovered;
                                    }
                                }
                            }
                        }
                        if !discovered.is_empty() {
                            break;
                        }
                    }
                }
            }
        }

        discovered
    }
}
