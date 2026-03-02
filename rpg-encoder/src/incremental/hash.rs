use sha2::{Digest, Sha256};
use std::path::Path;

use crate::error::Result;

pub fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn compute_file_hash(path: &Path) -> Result<String> {
    let content = std::fs::read_to_string(path)?;
    Ok(compute_hash(&content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash_consistency() {
        let content = "fn main() {}";
        let hash1 = compute_hash(content);
        let hash2 = compute_hash(content);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_compute_hash_different() {
        let hash1 = compute_hash("fn foo() {}");
        let hash2 = compute_hash("fn bar() {}");
        assert_ne!(hash1, hash2);
    }
}
