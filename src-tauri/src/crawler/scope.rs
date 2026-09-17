use regex::Regex;
use url::Url;

#[derive(Debug, Clone)]
pub enum ScopeRule {
    CurrentPage(String),
    Prefix(String),
    Host(String),
    Domain(String),
    Custom {
        includes: Vec<Regex>,
        excludes: Vec<Regex>,
    },
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
            "current" => ScopeRule::CurrentPage(root_url.to_string()),
            "prefix" => ScopeRule::Prefix(root_url.to_string()),
            "domain" => {
                // If host is foo.bar.com, get root domain bar.com or keep host
                let parts: Vec<&str> = host.split('.').collect();
                let domain = if parts.len() >= 2 {
                    parts[parts.len() - 2..].join(".")
                } else {
                    host
                };
                ScopeRule::Domain(domain)
            }
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
            _ => ScopeRule::Host(host), // Default is Host
        }
    }

    pub fn is_in_scope(&self, candidate_url: &str) -> bool {
        let parsed = match Url::parse(candidate_url) {
            Ok(u) => u,
            Err(_) => return false,
        };

        let cand_host = parsed.host_str().unwrap_or("").to_lowercase();

        match self {
            ScopeRule::CurrentPage(exact) => candidate_url == exact,
            ScopeRule::Prefix(prefix) => candidate_url.starts_with(prefix),
            ScopeRule::Host(expected_host) => cand_host == *expected_host,
            ScopeRule::Domain(expected_domain) => {
                cand_host == *expected_domain || cand_host.ends_with(&format!(".{}", expected_domain))
            }
            ScopeRule::Custom { includes, excludes } => {
                // If excludes match, reject
                for exc in excludes {
                    if exc.is_match(candidate_url) {
                        return false;
                    }
                }
                // If includes is empty, accept; otherwise must match at least one include
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

        let domain_scope = ScopeRule::from_profile("domain", "https://app.example.com", &[], &[]);
        assert!(domain_scope.is_in_scope("https://api.example.com/data"));
        assert!(domain_scope.is_in_scope("https://example.com"));
        assert!(!domain_scope.is_in_scope("https://example.org"));
    }
}
