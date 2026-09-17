use anyhow::{Context, Result};
use std::collections::HashSet;
use url::Url;

pub struct UrlNormalizer {
    tracking_params: HashSet<&'static str>,
}

impl Default for UrlNormalizer {
    fn default() -> Self {
        let mut set = HashSet::new();
        set.insert("utm_source");
        set.insert("utm_medium");
        set.insert("utm_campaign");
        set.insert("utm_term");
        set.insert("utm_content");
        set.insert("fbclid");
        set.insert("gclid");
        set.insert("yclid");
        set.insert("spm");
        set.insert("_ga");
        set.insert("ref");
        Self { tracking_params: set }
    }
}

impl UrlNormalizer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn normalize(&self, raw_url: &str) -> Result<String> {
        let mut parsed = Url::parse(raw_url).context("Failed to parse URL")?;

        // Remove fragment
        parsed.set_fragment(None);

        // Strip standard ports
        if let Some(port) = parsed.port() {
            if (parsed.scheme() == "http" && port == 80) || (parsed.scheme() == "https" && port == 443) {
                let _ = parsed.set_port(None);
            }
        }

        // Strip tracking parameters and sort queries deterministically
        let mut clean_query: Vec<(String, String)> = parsed
            .query_pairs()
            .filter(|(k, _)| !self.tracking_params.contains(k.to_lowercase().as_str()))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        if clean_query.is_empty() {
            parsed.set_query(None);
        } else {
            clean_query.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            let mut serializer = url::form_urlencoded::Serializer::new(String::new());
            for (k, v) in &clean_query {
                serializer.append_pair(k, v);
            }
            parsed.set_query(Some(&serializer.finish()));
        }

        // Normalize trailing slash
        let path = parsed.path().to_string();
        if path.len() > 1 && path.ends_with('/') {
            parsed.set_path(&path[..path.len() - 1]);
        }

        Ok(parsed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalizer() {
        let normalizer = UrlNormalizer::new();

        let u1 = "https://EXAMPLE.com/blog/post/?utm_source=twitter&b=2&a=1#comments";
        let norm1 = normalizer.normalize(u1).unwrap();
        assert_eq!(norm1, "https://example.com/blog/post?a=1&b=2");

        let u2 = "http://example.com:80/about/";
        let norm2 = normalizer.normalize(u2).unwrap();
        assert_eq!(norm2, "http://example.com/about");
    }
}
