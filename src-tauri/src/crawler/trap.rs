use std::collections::HashMap;
use url::Url;

pub struct TrapDetector {
    pub max_depth: i32,
    pub max_query_params: usize,
    pub max_url_length: usize,
    path_counts: HashMap<String, usize>,
    max_urls_per_path: usize,
}

impl Default for TrapDetector {
    fn default() -> Self {
        Self {
            max_depth: 5,
            max_query_params: 10,
            max_url_length: 1024,
            path_counts: HashMap::new(),
            max_urls_per_path: 100,
        }
    }
}

impl TrapDetector {
    pub fn new(max_depth: i32) -> Self {
        Self {
            max_depth,
            ..Default::default()
        }
    }

    pub fn is_trap(&mut self, url_str: &str, depth: i32) -> bool {
        // Depth limit
        if depth > self.max_depth {
            return true;
        }

        // Max URL length
        if url_str.len() > self.max_url_length {
            return true;
        }

        let parsed = match Url::parse(url_str) {
            Ok(u) => u,
            Err(_) => return true,
        };

        // Query params count explosion
        let query_count = parsed.query_pairs().count();
        if query_count > self.max_query_params {
            return true;
        }

        // Repeating path segments loop (e.g., /category/shoes/category/shoes/...)
        let segments: Vec<&str> = parsed.path_segments().map(|s| s.collect()).unwrap_or_default();
        if self.has_repeating_segments(&segments) {
            return true;
        }

        // Per-path explosion check
        let path = parsed.path().to_string();
        let count = self.path_counts.entry(path).or_insert(0);
        *count += 1;
        if *count > self.max_urls_per_path {
            return true;
        }

        false
    }

    fn has_repeating_segments(&self, segments: &[&str]) -> bool {
        if segments.len() < 4 {
            return false;
        }
        // Check if any segment appears 3 or more times
        let mut counts = HashMap::new();
        for seg in segments {
            if !seg.is_empty() {
                *counts.entry(*seg).or_insert(0) += 1;
            }
        }
        counts.values().any(|&c| c >= 3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trap_detection() {
        let mut detector = TrapDetector::new(3);

        assert!(!detector.is_trap("https://example.com/page1", 1));
        assert!(detector.is_trap("https://example.com/page1", 4)); // depth exceeded

        // Repeating path loop
        assert!(detector.is_trap("https://example.com/a/b/a/b/a/b", 2));
    }
}
