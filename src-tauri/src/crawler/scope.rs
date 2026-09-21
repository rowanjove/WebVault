use super::normalizer::UrlNormalizer;
use regex::Regex;
use url::Url;

#[derive(Debug, Clone)]
pub enum ScopeRule {
    CurrentPage(String),
    Prefix { origin: String, path_prefix: String },
    Host(String),
    Domain(String),
    Custom {
        includes: Vec<Regex>,
        excludes: Vec<Regex>,
    },
}

/// Multi-part public suffixes so "domain" scope does not crawl all of `.com.cn` / `.co.uk`.
const MULTI_PART_SUFFIXES: &[&str] = &[
    "com.cn", "net.cn", "org.cn", "gov.cn", "edu.cn", "co.cn",
    "com.hk", "com.tw", "com.sg", "com.my", "com.vn",
    "co.uk", "org.uk", "ac.uk", "gov.uk", "me.uk",
    "co.jp", "ne.jp", "or.jp", "ac.jp", "go.jp",
    "com.au", "net.au", "org.au", "co.nz",
    "co.kr", "com.br", "com.mx", "co.id", "co.th",
    "com.ar", "co.in", "com.tr", "com.ua",
];

pub fn registrable_domain(host: &str) -> String {
    let host = host.trim_end_matches('.').to_lowercase();
    if host.is_empty() {
        return host;
    }
    let parts: Vec<&str> = host.split('.').filter(|p| !p.is_empty()).collect();
    if parts.len() < 2 {
        return host;
    }
    let last2 = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
    if MULTI_PART_SUFFIXES.contains(&last2.as_str()) {
        if parts.len() >= 3 {
            format!("{}.{}", parts[parts.len() - 3], last2)
        } else {
            last2
        }
    } else {
        last2
    }
}

fn url_origin(u: &Url) -> String {
    match u.origin() {
        url::Origin::Tuple(scheme, host, port) => {
            let host = host.to_string();
            let default = match scheme.as_str() {
                "http" => 80,
                "https" => 443,
                _ => port,
            };
            if port == default {
                format!("{}://{}", scheme, host)
            } else {
                format!("{}://{}:{}", scheme, host, port)
            }
        }
        url::Origin::Opaque(_) => format!("{}://{}", u.scheme(), u.host_str().unwrap_or_default()),
    }
}

impl ScopeRule {
    pub fn from_profile(
        scope_type: &str,
        root_url: &str,
        include_rules: &[String],
        exclude_rules: &[String],
    ) -> Self {
        let parsed_root = Url::parse(root_url).ok();
        let host = parsed_root
            .as_ref()
            .and_then(|u| u.host_str())
            .unwrap_or("")
            .to_lowercase();

        match scope_type {
            "current" => {
                let norm = UrlNormalizer::new()
                    .normalize(root_url)
                    .unwrap_or_else(|_| root_url.to_string());
                ScopeRule::CurrentPage(norm)
            }
            "prefix" => {
                if let Some(ref u) = parsed_root {
                    let mut path = u.path().to_string();
                    if path.is_empty() {
                        path = "/".to_string();
                    }
                    ScopeRule::Prefix {
                        origin: url_origin(u),
                        path_prefix: path,
                    }
                } else {
                    ScopeRule::Host(host)
                }
            }
            "domain" => ScopeRule::Domain(registrable_domain(&host)),
            "custom" => {
                let inc = include_rules
                    .iter()
                    .filter_map(|r| Regex::new(r).ok())
                    .collect();
                let exc = exclude_rules
                    .iter()
                    .filter_map(|r| Regex::new(r).ok())
                    .collect();
                ScopeRule::Custom {
                    includes: inc,
                    excludes: exc,
                }
            }
            _ => ScopeRule::Host(host),
        }
    }

    pub fn is_in_scope(&self, candidate_url: &str) -> bool {
        let parsed = match Url::parse(candidate_url) {
            Ok(u) => u,
            Err(_) => return false,
        };

        let cand_host = parsed.host_str().unwrap_or("").to_lowercase();

        match self {
            ScopeRule::CurrentPage(exact) => {
                let norm = UrlNormalizer::new()
                    .normalize(candidate_url)
                    .unwrap_or_else(|_| candidate_url.to_string());
                &norm == exact
            }
            ScopeRule::Prefix { origin, path_prefix } => {
                if url_origin(&parsed) != *origin {
                    return false;
                }
                let prefix = path_prefix.trim_end_matches('/');
                if prefix.is_empty() {
                    return true;
                }
                let path = parsed.path();
                path == prefix || path.starts_with(&format!("{}/", prefix))
            }
            ScopeRule::Host(expected_host) => cand_host == *expected_host,
            ScopeRule::Domain(expected_domain) => {
                cand_host == *expected_domain || cand_host.ends_with(&format!(".{}", expected_domain))
            }
            ScopeRule::Custom { includes, excludes } => {
                for exc in excludes {
                    if exc.is_match(candidate_url) {
                        return false;
                    }
                }
                if includes.is_empty() {
                    true
                } else {
                    includes.iter().any(|inc| inc.is_match(candidate_url))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_rules() {
        let host_scope = ScopeRule::from_profile("host", "https://example.com/docs", &[], &[]);
        assert!(host_scope.is_in_scope("https://example.com/docs/intro"));
        assert!(host_scope.is_in_scope("https://example.com/about"));
        assert!(!host_scope.is_in_scope("https://other.com/docs"));

        let prefix_scope = ScopeRule::from_profile("prefix", "https://example.com/docs", &[], &[]);
        assert!(prefix_scope.is_in_scope("https://example.com/docs/intro"));
        assert!(!prefix_scope.is_in_scope("https://example.com/about"));
        assert!(!prefix_scope.is_in_scope("https://example.com.evil.com/docs"));

        let domain_scope = ScopeRule::from_profile("domain", "https://app.example.com", &[], &[]);
        assert!(domain_scope.is_in_scope("https://api.example.com/data"));
        assert!(domain_scope.is_in_scope("https://example.com"));
        assert!(!domain_scope.is_in_scope("https://example.org"));

        let cn_scope = ScopeRule::from_profile("domain", "https://www.foo.com.cn/", &[], &[]);
        assert!(cn_scope.is_in_scope("https://cdn.foo.com.cn/a"));
        assert!(!cn_scope.is_in_scope("https://other.com.cn/a"));

        let current = ScopeRule::from_profile("current", "https://example.com", &[], &[]);
        assert!(current.is_in_scope("https://example.com/"));
        assert!(current.is_in_scope("https://example.com"));
        assert!(!current.is_in_scope("https://example.com/other"));
    }

    #[test]
    fn test_registrable_domain() {
        assert_eq!(registrable_domain("www.foo.com.cn"), "foo.com.cn");
        assert_eq!(registrable_domain("app.example.com"), "example.com");
        assert_eq!(registrable_domain("www.bbc.co.uk"), "bbc.co.uk");
    }
}
