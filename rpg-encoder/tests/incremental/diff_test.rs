
use rpg_encoder::{CodeUnit, DiffStats, FileDiff, UnitType};
use std::path::PathBuf;

#[test]
fn test_code_unit_new() {
    let unit = CodeUnit::new(
        "test_fn".to_string(),
        UnitType::Function,
        1,
        10,
        "fn test_fn() {}".to_string(),
    );

    assert_eq!(unit.name, "test_fn");
    assert_eq!(unit.unit_type, UnitType::Function);
    assert_eq!(unit.start_line, 1);
    assert_eq!(unit.end_line, 10);
    assert!(!unit.content_hash.is_empty());
}

#[test]
fn test_code_unit_hash_consistency() {
    let content = "fn main() {}";
    let unit1 = CodeUnit::new(
        "main".to_string(),
        UnitType::Function,
        1,
        1,
        content.to_string(),
    );
    let unit2 = CodeUnit::new(
        "main".to_string(),
        UnitType::Function,
        1,
        1,
        content.to_string(),
    );

    assert_eq!(unit1.content_hash, unit2.content_hash);
}

#[test]
fn test_code_unit_hash_different() {
    let unit1 = CodeUnit::new(
        "fn1".to_string(),
        UnitType::Function,
        1,
        1,
        "fn fn1() {}".to_string(),
    );
    let unit2 = CodeUnit::new(
        "fn2".to_string(),
        UnitType::Function,
        1,
        1,
        "fn fn2() {}".to_string(),
    );

    assert_ne!(unit1.content_hash, unit2.content_hash);
}

#[test]
fn test_file_diff_default() {
    let diff = FileDiff::default();

    assert!(diff.added.is_empty());
    assert!(diff.deleted.is_empty());
    assert!(diff.modified.is_empty());
    assert!(diff.stats.files_added == 0);
}

#[test]
fn test_file_diff_is_empty() {
    let mut diff = FileDiff::default();
    assert!(diff.is_empty());

    diff.added.push(PathBuf::from("new.rs"));
    assert!(!diff.is_empty());
}

#[test]
fn test_diff_stats_default() {
    let stats = DiffStats::default();

    assert_eq!(stats.files_added, 0);
    assert_eq!(stats.files_deleted, 0);
    assert_eq!(stats.files_modified, 0);
    assert_eq!(stats.units_added, 0);
    assert_eq!(stats.units_changed, 0);
    assert_eq!(stats.units_deleted, 0);
}

#[test]
fn test_unit_type_variants() {
    assert_eq!(UnitType::Function, UnitType::Function);
    assert_ne!(UnitType::Function, UnitType::Struct);

    let variants = [
        UnitType::Function,
        UnitType::Struct,
        UnitType::Enum,
        UnitType::Trait,
        UnitType::Impl,
        UnitType::Module,
    ];
    assert_eq!(variants.len(), 6);
}

#[test]
fn test_file_diff_with_modifications() {
    let mut diff = FileDiff::default();

    diff.added.push(PathBuf::from("new_file.rs"));
    diff.deleted.push(PathBuf::from("old_file.rs"));
    diff.stats.files_added = 1;
    diff.stats.files_deleted = 1;

    assert_eq!(diff.added.len(), 1);
    assert_eq!(diff.deleted.len(), 1);
    assert!(!diff.is_empty());
}
