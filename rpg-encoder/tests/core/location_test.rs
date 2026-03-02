use rpg_encoder::SourceLocation;
use std::path::PathBuf;

#[test]
fn test_location_new() {
    let file = PathBuf::from("src/main.rs");
    let loc = SourceLocation::new(file.clone(), 1, 5, 10, 20);

    assert_eq!(loc.file, file);
    assert_eq!(loc.start_line, 1);
    assert_eq!(loc.start_column, 5);
    assert_eq!(loc.end_line, 10);
    assert_eq!(loc.end_column, 20);
}

#[test]
fn test_location_single_line() {
    let file = PathBuf::from("test.rs");
    let loc = SourceLocation::new(file, 5, 1, 5, 10);

    assert_eq!(loc.start_line, loc.end_line);
    assert!(loc.start_column < loc.end_column);
}

#[test]
fn test_location_multiline() {
    let loc = SourceLocation::new(PathBuf::from("test.rs"), 1, 1, 10, 1);

    assert!(loc.start_line < loc.end_line);
}

#[test]
fn test_location_clone() {
    let loc = SourceLocation::new(PathBuf::from("test.rs"), 1, 1, 5, 10);
    let cloned = loc.clone();

    assert_eq!(loc.file, cloned.file);
    assert_eq!(loc.start_line, cloned.start_line);
}
