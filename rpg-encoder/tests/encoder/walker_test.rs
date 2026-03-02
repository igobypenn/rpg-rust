use rpg_encoder::encoder::FileWalker;
use std::io::Write;
use tempfile::TempDir;

fn create_test_files(dir: &std::path::Path) -> std::io::Result<()> {
    let src = dir.join("src");
    std::fs::create_dir_all(&src)?;

    let main = src.join("main.rs");
    let mut f = std::fs::File::create(&main)?;
    writeln!(f, "fn main() {{}}")?;

    let lib = src.join("lib.rs");
    let mut f = std::fs::File::create(&lib)?;
    writeln!(f, "pub fn helper() {{}}")?;

    Ok(())
}

fn create_rpgignore(dir: &std::path::Path) -> std::io::Result<()> {
    let ignore = dir.join(".rpgignore");
    let mut f = std::fs::File::create(&ignore)?;
    writeln!(f, "*.md")?;
    Ok(())
}

#[test]
fn test_walk_directory_finds_files() {
    let dir = TempDir::new().unwrap();
    create_test_files(dir.path()).unwrap();

    let walker = FileWalker::new(dir.path());
    let files = walker.walk().unwrap();

    assert!(!files.is_empty());
}

#[test]
fn test_rpgignore_patterns() {
    let dir = TempDir::new().unwrap();
    create_test_files(dir.path()).unwrap();
    create_rpgignore(dir.path()).unwrap();

    std::fs::write(dir.path().join("README.md"), "# Readme").unwrap();

    let walker = FileWalker::new(dir.path());
    let files = walker.walk().unwrap();

    let has_md = files
        .iter()
        .any(|f| f.extension().map(|e| e == "md").unwrap_or(false));
    assert!(!has_md, "Should ignore .md files");
}

#[test]
fn test_max_depth() {
    let dir = TempDir::new().unwrap();

    let deep = dir.path().join("a").join("b").join("c");
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(deep.join("deep.rs"), "fn deep() {}").unwrap();

    let walker = FileWalker::new(dir.path()).with_max_depth(2);
    let files = walker.walk().unwrap();

    let has_deep = files
        .iter()
        .any(|f| f.to_str().unwrap().contains("deep.rs"));
    assert!(!has_deep, "Should respect max_depth");
}

#[test]
fn test_hidden_files_can_be_included() {
    let dir = TempDir::new().unwrap();
    create_test_files(dir.path()).unwrap();

    let hidden_dir = dir.path().join(".hidden_dir");
    std::fs::create_dir_all(&hidden_dir).unwrap();
    std::fs::write(hidden_dir.join("secret.rs"), "fn secret() {}").unwrap();

    let walker = FileWalker::new(dir.path());
    let files = walker.walk().unwrap();

    // FileWalker is configured with hidden(false) so it includes hidden files
    let has_hidden = files
        .iter()
        .any(|f| f.to_str().unwrap().contains(".hidden_dir"));
    assert!(has_hidden, "Should include hidden directories by default");
}
