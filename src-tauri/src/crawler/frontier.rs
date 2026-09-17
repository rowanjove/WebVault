use super::normalizer::UrlNormalizer;
use super::scope::ScopeRule;
use super::trap::TrapDetector;
use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub url: String,
    pub normalized_url: String,
    pub depth: i32,
    pub parent_url: Option<String>,
}

pub struct CrawlFrontier {
    queue: VecDeque<QueueItem>,
    visited_urls: HashSet<String>,
    discovered_urls: HashSet<String>,
    normalizer: UrlNormalizer,
    scope: ScopeRule,
    trap_detector: TrapDetector,
    max_pages: usize,
}

impl CrawlFrontier {
    pub fn new(root_url: &str, scope: ScopeRule, max_depth: i32, max_pages: usize) -> Self {
        let normalizer = UrlNormalizer::new();
        let mut frontier = Self {
            queue: VecDeque::new(),
            visited_urls: HashSet::new(),
            discovered_urls: HashSet::new(),
            normalizer,
            scope,
            trap_detector: TrapDetector::new(max_depth),
            max_pages,
        };

        frontier.add_url(root_url, 0, None);
        frontier
    }

    pub fn add_url(&mut self, raw_url: &str, depth: i32, parent_url: Option<String>) -> bool {
        if self.discovered_urls.len() >= self.max_pages {
            return false;
        }

        // Only allow http and https schemes
        let parsed = match url::Url::parse(raw_url) {
            Ok(u) => u,
            Err(_) => return false,
        };
        if parsed.scheme() != "http" && parsed.scheme() != "https" {
            return false;
        }

        let normalized = match self.normalizer.normalize(raw_url) {
            Ok(u) => u,
            Err(_) => return false,
        };

        if self.discovered_urls.contains(&normalized) || self.visited_urls.contains(&normalized) {
            return false;
        }

        if !self.scope.is_in_scope(&normalized) {
            return false;
        }

        if self.trap_detector.is_trap(&normalized, depth) {
            return false;
        }

        self.discovered_urls.insert(normalized.clone());
        self.queue.push_back(QueueItem {
            url: raw_url.to_string(),
            normalized_url: normalized,
            depth,
            parent_url,
        });

        true
    }

    pub fn pop_next(&mut self) -> Option<QueueItem> {
        let item = self.queue.pop_front()?;
        self.visited_urls.insert(item.normalized_url.clone());
        Some(item)
    }

    pub fn has_next(&self) -> bool {
        !self.queue.is_empty()
    }

    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    pub fn discovered_count(&self) -> usize {
        self.discovered_urls.len()
    }

    pub fn visited_count(&self) -> usize {
        self.visited_urls.len()
    }

    pub fn mark_visited(&mut self, norm_url: &str) {
        self.visited_urls.insert(norm_url.to_string());
    }

    pub fn is_visited(&self, norm_url: &str) -> bool {
        self.visited_urls.contains(norm_url)
    }

    pub fn is_discovered(&self, norm_url: &str) -> bool {
        self.discovered_urls.contains(norm_url)
    }
}
