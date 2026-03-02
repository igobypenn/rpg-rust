//! Snapshot tests for Rust parser output
//!
//! These tests capture parser output in JSON and YAML formats for easy
//! review of parsing behavior changes.

use insta::{assert_json_snapshot, assert_yaml_snapshot};
use rpg_encoder::languages::RustParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rust")
}

fn parse_fixture(name: &str) -> rpg_encoder::parser::ParseResult {
    let fixture_path = fixtures_dir().join(name);
    let source = std::fs::read_to_string(&fixture_path)
        .unwrap_or_else(|_| panic!("Failed to load fixture: {}", name));
    let parser = RustParser::new().expect("Failed to create parser");
    parser
        .parse(&source, &fixture_path)
        .expect("Failed to parse fixture")
}

#[test]
fn snapshot_basic_json() {
    let result = parse_fixture("basic.rs");
    assert_json_snapshot!(result);
}

#[test]
fn snapshot_basic_yaml() {
    let result = parse_fixture("basic.rs");
    assert_yaml_snapshot!(result);
}

#[test]
fn snapshot_types_json() {
    let result = parse_fixture("types.rs");
    assert_json_snapshot!(result);
}

#[test]
fn snapshot_types_yaml() {
    let result = parse_fixture("types.rs");
    assert_yaml_snapshot!(result);
}

#[test]
fn snapshot_ffi_json() {
    let result = parse_fixture("ffi.rs");
    assert_json_snapshot!(result);
}

#[test]
fn snapshot_ffi_yaml() {
    let result = parse_fixture("ffi.rs");
    assert_yaml_snapshot!(result);
}
