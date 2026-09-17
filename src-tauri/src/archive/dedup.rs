use sha2::{Digest, Sha256};

pub struct Deduplicator;

impl Deduplicator {
    pub fn compute_sha256(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    pub fn compute_text_hash(text: &str) -> String {
        // Strip excessive whitespace to normalize text comparison
        let normalized: String = text
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ");
        Self::compute_sha256(normalized.as_bytes())
    }
}
