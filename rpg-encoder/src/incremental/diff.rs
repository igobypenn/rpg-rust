use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::hash::{compute_file_hash, compute_hash};
use super::snapshot::{CachedUnit, RpgSnapshot, UnitType};
use crate::error::{Result, RpgError};
use crate::parser::{LanguageParser, ParserRegistry};

#[derive(Debug, Clone, Default)]
pub struct FileDiff {
    pub added: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
    pub modified: Vec<ModifiedFile>,
    pub stats: DiffStats,
}

impl FileDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.deleted.is_empty() && self.modified.is_empty()
    }
}

#[derive(Debug, Clone, Default)]
pub struct DiffStats {
    pub files_added: usize,
    pub files_deleted: usize,
    pub files_modified: usize,
    pub units_added: usize,
    pub units_changed: usize,
    pub units_deleted: usize,
}

#[derive(Debug, Clone)]
pub struct ModifiedFile {
    pub path: PathBuf,
    pub old_hash: String,
    pub new_hash: String,
    pub added_units: Vec<CodeUnit>,
    pub changed_units: Vec<CodeUnit>,
    pub deleted_units: Vec<CodeUnit>,
    pub unchanged_units: Vec<CodeUnit>,
}

#[derive(Debug, Clone)]
pub struct CodeUnit {
    pub name: String,
    pub unit_type: UnitType,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
    pub content_hash: String,
}

impl CodeUnit {
    pub fn new(
        name: String,
        unit_type: UnitType,
        start_line: usize,
        end_line: usize,
        content: String,
    ) -> Self {
        let content_hash = compute_hash(&content);
        Self {
            name,
            unit_type,
            start_line,
            end_line,
            content,
            content_hash,
        }
    }
}

pub fn generate_diff(
    old_snapshot: &RpgSnapshot,
    new_dir: &Path,
    registry: &ParserRegistry,
) -> Result<FileDiff> {
    let mut diff = FileDiff::default();
    let mut new_files: HashSet<PathBuf> = HashSet::new();

    for entry in walkdir::WalkDir::new(new_dir).follow_links(false) {
        let entry = entry.map_err(|e| RpgError::Incremental(format!("Walk error: {}", e)))?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let parser = match registry.get_parser(path) {
            Some(p) => p,
            None => continue,
        };

        let relative_path = path
            .strip_prefix(new_dir)
            .map_err(|_| RpgError::PathError {
                path: path.display().to_string(),
                operation: "strip prefix".to_string(),
            })?;

        new_files.insert(relative_path.to_path_buf());

        let new_hash = compute_file_hash(path)?;

        if let Some(old_hash) = old_snapshot.file_hashes.get(relative_path) {
            if &new_hash != old_hash {
                if let Some(modified) = generate_modified_file(
                    relative_path,
                    path,
                    old_hash.clone(),
                    new_hash,
                    old_snapshot,
                    parser,
                )? {
                    diff.stats.units_added += modified.added_units.len();
                    diff.stats.units_changed += modified.changed_units.len();
                    diff.stats.units_deleted += modified.deleted_units.len();
                    diff.modified.push(modified);
                    diff.stats.files_modified += 1;
                }
            }
        } else {
            diff.added.push(relative_path.to_path_buf());
            diff.stats.files_added += 1;
        }
    }

    for old_path in old_snapshot.file_hashes.keys() {
        if !new_files.contains(old_path) {
            diff.deleted.push(old_path.clone());
            diff.stats.files_deleted += 1;
        }
    }

    Ok(diff)
}

fn generate_modified_file(
    relative_path: &Path,
    full_path: &Path,
    old_hash: String,
    new_hash: String,
    old_snapshot: &RpgSnapshot,
    parser: &dyn LanguageParser,
) -> Result<Option<ModifiedFile>> {
    let source = std::fs::read_to_string(full_path)?;
    let new_units = parse_units_from_source(&source, full_path, parser)?;

    let old_units = old_snapshot
        .get_units_for_file(relative_path)
        .unwrap_or(&[]);

    let matched = match_units(old_units, &new_units);

    if matched.added.is_empty() && matched.changed.is_empty() && matched.deleted.is_empty() {
        return Ok(None);
    }

    Ok(Some(ModifiedFile {
        path: relative_path.to_path_buf(),
        old_hash,
        new_hash,
        added_units: matched.added,
        changed_units: matched.changed,
        deleted_units: matched
            .deleted
            .into_iter()
            .map(|u| CodeUnit {
                name: u.name.clone(),
                unit_type: u.unit_type,
                start_line: u.start_line,
                end_line: u.end_line,
                content: String::new(),
                content_hash: u.content_hash.clone(),
            })
            .collect(),
        unchanged_units: matched.unchanged.into_iter().map(|(_, new)| new).collect(),
    }))
}

fn parse_units_from_source(
    source: &str,
    file_path: &Path,
    parser: &dyn LanguageParser,
) -> Result<Vec<CodeUnit>> {
    let parse_result = parser
        .parse(source, file_path)
        .map_err(|e| RpgError::Incremental(format!("Parse error: {}", e)))?;

    let mut units = Vec::new();

    for def in &parse_result.definitions {
        let Some(unit_type) = UnitType::from_kind(def.kind.as_str()) else {
            continue;
        };

        let (start_line, end_line) = def
            .location
            .as_ref()
            .map(|l| (l.start_line, l.end_line))
            .unwrap_or((1, 1));

        let content = extract_unit_content(source, start_line, end_line);
        let content_hash = compute_hash(&content);

        units.push(CodeUnit {
            name: def.name.clone(),
            unit_type,
            start_line,
            end_line,
            content,
            content_hash,
        });
    }

    Ok(units)
}

pub(super) fn extract_unit_content(source: &str, start_line: usize, end_line: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let start = start_line.saturating_sub(1);
    let end = end_line.min(lines.len());

    lines[start..end].join("\n")
}

struct UnitMatchResult {
    added: Vec<CodeUnit>,
    changed: Vec<CodeUnit>,
    deleted: Vec<CachedUnit>,
    unchanged: Vec<(CachedUnit, CodeUnit)>,
}

fn match_units(old: &[CachedUnit], new: &[CodeUnit]) -> UnitMatchResult {
    let mut result = UnitMatchResult {
        added: Vec::new(),
        changed: Vec::new(),
        deleted: Vec::new(),
        unchanged: Vec::new(),
    };

    let mut old_matched: HashSet<usize> = HashSet::new();
    let mut old_index: HashMap<(String, UnitType), Vec<usize>> = HashMap::new();

    for (i, old_unit) in old.iter().enumerate() {
        old_index
            .entry((old_unit.name.clone(), old_unit.unit_type))
            .or_default()
            .push(i);
    }

    for new_unit in new {
        let key = (new_unit.name.clone(), new_unit.unit_type);
        let mut found_match = false;

        if let Some(indices) = old_index.get(&key) {
            for &i in indices {
                if old_matched.contains(&i) {
                    continue;
                }

                old_matched.insert(i);
                found_match = true;

                if old[i].content_hash == new_unit.content_hash {
                    result.unchanged.push((old[i].clone(), new_unit.clone()));
                } else {
                    result.changed.push(new_unit.clone());
                }
                break;
            }
        }

        if !found_match {
            result.added.push(new_unit.clone());
        }
    }

    for (i, old_unit) in old.iter().enumerate() {
        if !old_matched.contains(&i) {
            result.deleted.push(old_unit.clone());
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_diff_empty() {
        let diff = FileDiff::default();
        assert!(diff.is_empty());
    }

    #[test]
    fn test_diff_stats_default() {
        let stats = DiffStats::default();
        assert_eq!(stats.files_added, 0);
        assert_eq!(stats.files_deleted, 0);
        assert_eq!(stats.files_modified, 0);
    }

    #[test]
    fn test_match_units_identical() {
        let old = vec![CachedUnit::new(
            "foo".to_string(),
            UnitType::Function,
            "hash_a".to_string(),
            1,
            5,
        )];
        let new = vec![CodeUnit {
            name: "foo".to_string(),
            unit_type: UnitType::Function,
            start_line: 1,
            end_line: 5,
            content: "fn foo() {}".to_string(),
            content_hash: "hash_a".to_string(),
        }];

        let result = match_units(&old, &new);
        assert_eq!(result.unchanged.len(), 1);
        assert!(result.added.is_empty());
        assert!(result.changed.is_empty());
        assert!(result.deleted.is_empty());
    }

    #[test]
    fn test_match_units_added() {
        let old: Vec<CachedUnit> = vec![];
        let new = vec![CodeUnit {
            name: "bar".to_string(),
            unit_type: UnitType::Function,
            start_line: 1,
            end_line: 3,
            content: "fn bar() {}".to_string(),
            content_hash: "hash_b".to_string(),
        }];

        let result = match_units(&old, &new);
        assert_eq!(result.added.len(), 1);
        assert_eq!(result.added[0].name, "bar");
        assert!(result.unchanged.is_empty());
        assert!(result.changed.is_empty());
        assert!(result.deleted.is_empty());
    }

    #[test]
    fn test_match_units_removed() {
        let old = vec![CachedUnit::new(
            "old_fn".to_string(),
            UnitType::Function,
            "hash_old".to_string(),
            1,
            4,
        )];
        let new: Vec<CodeUnit> = vec![];

        let result = match_units(&old, &new);
        assert_eq!(result.deleted.len(), 1);
        assert_eq!(result.deleted[0].name, "old_fn");
        assert!(result.added.is_empty());
        assert!(result.unchanged.is_empty());
    }

    #[test]
    fn test_match_units_changed() {
        let old = vec![CachedUnit::new(
            "func".to_string(),
            UnitType::Function,
            "hash_old".to_string(),
            1,
            5,
        )];
        let new = vec![CodeUnit {
            name: "func".to_string(),
            unit_type: UnitType::Function,
            start_line: 1,
            end_line: 10,
            content: "fn func() { /* changed */ }".to_string(),
            content_hash: "hash_new".to_string(),
        }];

        let result = match_units(&old, &new);
        assert_eq!(result.changed.len(), 1);
        assert_eq!(result.changed[0].name, "func");
        assert!(result.unchanged.is_empty());
        assert!(result.added.is_empty());
        assert!(result.deleted.is_empty());
    }

    #[test]
    fn test_extract_unit_content() {
        let source = "line1\nline2\nline3\nline4";
        let content = extract_unit_content(source, 2, 3);
        assert_eq!(content, "line2\nline3");
    }
}
