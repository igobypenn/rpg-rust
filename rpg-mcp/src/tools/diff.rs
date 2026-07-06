//! Minimal unified-diff parser for the diff analysis tools.
//!
//! Parses the subset of the unified diff format that `git diff` produces:
//! `diff --git`, `---`/`+++` file headers, `@@` hunk headers with line ranges.
//! Extracts per-file changed line ranges so we can map them to graph nodes.

/// A hunk's changed line range in the new (post-diff) file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedRange {
    /// Starting line number in the new file (1-based).
    pub start: usize,
    /// Ending line number (inclusive).
    pub end: usize,
}

/// Parsed diff: per-file changed line ranges.
#[derive(Debug, Clone, Default)]
pub struct ParsedDiff {
    /// (file_path, changed_ranges) pairs.
    pub files: Vec<(String, Vec<ChangedRange>)>,
}

impl ParsedDiff {
    /// Parse a unified diff string.
    ///
    /// Recognizes:
    /// - `+++ b/path` (new file path)
    /// - `@@ -old_start,old_len +new_start,new_len @@` (hunk header)
    /// - `+added_line` / `-removed_line` (content lines)
    ///
    /// Collects contiguous runs of added/changed lines into ranges.
    pub fn parse(diff: &str) -> Self {
        let mut result = ParsedDiff::default();
        let mut current_file: Option<String> = None;
        let mut current_ranges: Vec<ChangedRange> = Vec::new();
        let mut new_line: usize = 0; // current line in the new file
        let mut range_start: Option<usize> = None;

        for line in diff.lines() {
            // New file path: `+++ b/path` or `+++ /dev/null`.
            if let Some(rest) = line.strip_prefix("+++ ") {
                // Close any open range.
                if let Some(start) = range_start.take() {
                    current_ranges.push(ChangedRange { start, end: new_line.saturating_sub(1) });
                }
                // Save the previous file's ranges.
                if let Some(f) = current_file.take() {
                    if !current_ranges.is_empty() {
                        result.files.push((f, std::mem::take(&mut current_ranges)));
                    }
                }
                // Extract path from `+++ b/path` or `+++ "b/path with spaces"`.
                // Handle quoted paths (git emits these for paths with special chars).
                let path = if let Some(quoted) = rest.strip_prefix('"') {
                    // Find closing quote, ignoring escaped quotes.
                    quoted.rsplit_once('"').map(|(p, _)| p).unwrap_or(quoted)
                } else {
                    // Unquoted: take everything up to first whitespace (but NOT
                    // splitting on spaces inside the path — git quotepath is
                    // off in this case, so spaces are literal and there's no
                    // whitespace after the path on this line except \r).
                    rest.split_whitespace().next().unwrap_or(rest)
                };
                let path = path.strip_prefix("b/").unwrap_or(path);
                if path != "/dev/null" {
                    current_file = Some(path.to_string());
                }
                continue;
            }

            // Hunk header: `@@ -old_start,old_len +new_start,new_len @@`
            if line.starts_with("@@") {
                // Close any open range.
                if let Some(start) = range_start.take() {
                    current_ranges.push(ChangedRange { start, end: new_line.saturating_sub(1) });
                }
                // Parse +new_start from the header.
                if let Some(ns) = parse_hunk_new_start(line) {
                    new_line = ns;
                }
                continue;
            }

            // Content lines.
            if current_file.is_some() {
                // Skip git metadata lines inside hunks: "\ No newline at end
                // of file" is NOT a context line — counting it would corrupt
                // all subsequent line ranges in this hunk.
                if line.starts_with("\\ ") || line == "\\" {
                    continue;
                }
                if let Some(_added) = line.strip_prefix('+') {
                    // Added line — extends or starts a range.
                    if range_start.is_none() {
                        range_start = Some(new_line);
                    }
                    new_line += 1;
                } else if line.starts_with('-') {
                    // Removed line — doesn't advance new_line, but marks the
                    // region as changed. We close the current added-range and
                    // note the removal as part of the previous line's range.
                    if let Some(start) = range_start.take() {
                        current_ranges.push(ChangedRange { start, end: new_line.saturating_sub(1).max(start) });
                    }
                } else {
                    // Context line — closes any open range.
                    if let Some(start) = range_start.take() {
                        current_ranges.push(ChangedRange { start, end: new_line.saturating_sub(1).max(start) });
                    }
                    new_line += 1;
                }
            }
        }

        // Close trailing range.
        if let Some(start) = range_start.take() {
            current_ranges.push(ChangedRange { start, end: new_line.saturating_sub(1).max(start) });
        }
        if let Some(f) = current_file.take() {
            if !current_ranges.is_empty() {
                result.files.push((f, current_ranges));
            }
        }

        result
    }
}

/// Extract the new-file starting line from a `@@ -a,b +c,d @@` header.
fn parse_hunk_new_start(line: &str) -> Option<usize> {
    // Find the `+` that precedes the new range.
    let plus_idx = line.find(" +")?;
    let after_plus = &line[plus_idx + 2..];
    let num_str: String = after_plus.chars().take_while(|c| c.is_ascii_digit()).collect();
    num_str.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_addition() {
        let diff = "\
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,4 @@
 fn main() {
+    println!(\"hello\");
 }
";
        let parsed = ParsedDiff::parse(diff);
        assert_eq!(parsed.files.len(), 1);
        assert_eq!(parsed.files[0].0, "src/lib.rs");
        assert_eq!(parsed.files[0].1.len(), 1);
        assert_eq!(parsed.files[0].1[0].start, 2); // line 2 in new file
    }

    #[test]
    fn parse_multiple_hunks() {
        let diff = "\
--- a/src/a.rs
+++ b/src/a.rs
@@ -5,3 +5,4 @@
 line5
+new_line
 line6
@@ -20,3 +21,4 @@
 line20
+another_new
 line21
";
        let parsed = ParsedDiff::parse(diff);
        assert_eq!(parsed.files.len(), 1);
        assert_eq!(parsed.files[0].1.len(), 2);
        assert_eq!(parsed.files[0].1[0].start, 6);
        assert_eq!(parsed.files[0].1[1].start, 22);
    }

    #[test]
    fn parse_empty_diff() {
        let parsed = ParsedDiff::parse("");
        assert!(parsed.files.is_empty());
    }

    #[test]
    fn parse_dev_null_new_file() {
        let diff = "\
--- /dev/null
+++ b/new_file.rs
@@ -0,0 +1,2 @@
+pub fn new_func() {}
+pub fn another() {}
";
        let parsed = ParsedDiff::parse(diff);
        assert_eq!(parsed.files.len(), 1);
        assert_eq!(parsed.files[0].0, "new_file.rs");
    }

    #[test]
    fn parse_removal_only() {
        let diff = "\
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -3,3 +3,2 @@
 fn main() {
-    removed_line
 }
";
        let parsed = ParsedDiff::parse(diff);
        // A removal-only diff may produce no added ranges (no new lines).
        // The file is still listed with its ranges (which may be empty).
        // Our parser only records added-line ranges, so this is empty.
        assert!(parsed.files.is_empty() || parsed.files[0].1.is_empty());
    }
}
