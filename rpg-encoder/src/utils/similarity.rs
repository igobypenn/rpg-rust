//! Shared utility module for semantic similarity.
//!
//! Provides:
//! - `jaccard_similarity`: Compare text against pre-lowered query words (optimized for pre-lowered text)
//! - `semantic_similarity`: Compare two feature strings (optimized to take temporary lowercase allocation)
//!
//! Also useful for comparing feature vectors in evolution.rs.

use std::collections::HashSet;

/// Compute Jaccard similarity between text words and query words.
///
/// Both inputs should be pre-lowered strings for efficiency.
/// Returns a similarity score between 0.0 and 1.0.
pub fn jaccard_similarity(text: &str, query_words: &HashSet<&str>) -> f32 {
    if text.is_empty() && query_words.is_empty() {
        return 1.0;
    }
    if text.is_empty() || query_words.is_empty() {
        return 0.0;
    }

    let text_words: HashSet<&str> = text.split_whitespace().collect();
    if text_words.is_empty() {
        return 0.0;
    }

    let intersection = text_words.intersection(query_words).count();
    let union = text_words.union(query_words).count();

    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

/// Compute semantic similarity between two feature strings.
///
/// Uses Jaccard similarity based on word overlap.
/// Returns a value between 0.0 (no similarity) and 1.0 (identical).
pub fn semantic_similarity(a: &str, b: &str) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();

    jaccard_similarity(&a_lower, &b_lower.split_whitespace().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hashset<'a>(words: &[&'a str]) -> HashSet<&'a str> {
        words.iter().copied().collect()
    }

    #[test]
    fn test_jaccard_similarity_empty() {
        let query_words = make_hashset(&["test"]);
        assert_eq!(jaccard_similarity("", &query_words), 0.0);

        let empty: HashSet<&str> = HashSet::new();
        assert_eq!(jaccard_similarity("hello world", &empty), 0.0);
    }

    #[test]
    fn test_jaccard_similarity_basic() {
        let query_words = make_hashset(&["hello", "test"]);
        // hello world vs {hello, test}: intersection=1, union=3
        assert!((jaccard_similarity("hello world", &query_words) - 0.333).abs() < 0.01);
        // foo bar vs {hello, test}: intersection=0, union=4
        assert_eq!(jaccard_similarity("foo bar", &query_words), 0.0);
    }

    #[test]
    fn test_jaccard_similarity_partial_match() {
        // {hello, world, test} vs {hello, world}: intersection=2, union=3
        let query_words = make_hashset(&["hello", "world", "test"]);
        assert!((jaccard_similarity("hello world", &query_words) - 0.666).abs() < 0.01);
        assert!((jaccard_similarity("world hello", &query_words) - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_semantic_similarity_empty() {
        assert_eq!(semantic_similarity("", ""), 1.0);
        assert_eq!(semantic_similarity("hello", ""), 0.0);
        assert_eq!(semantic_similarity("", "hello"), 0.0);
    }

    #[test]
    fn test_semantic_similarity_identical() {
        assert_eq!(semantic_similarity("hello world", "hello world"), 1.0);
    }

    #[test]
    fn test_semantic_similarity_partial() {
        // hello world vs hello universe: intersection=1, union=3
        assert!((semantic_similarity("hello world", "hello universe") - 0.333).abs() < 0.01);
    }
}
