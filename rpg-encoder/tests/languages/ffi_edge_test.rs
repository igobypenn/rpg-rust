use rpg_encoder::languages::ffi::{FfiBinding, FfiDetector, FfiKind};
use std::path::PathBuf;

#[test]
fn test_ffi_binding_new() {
    let binding = FfiBinding::new("rust", "c", "my_function", FfiKind::Export);

    assert_eq!(binding.source_lang, "rust");
    assert_eq!(binding.target_lang, "c");
    assert_eq!(binding.symbol, "my_function");
    assert_eq!(binding.kind, FfiKind::Export);
    assert!(binding.location.is_none());
    assert!(binding.native_signature.is_none());
}

#[test]
fn test_ffi_binding_with_location() {
    let location = rpg_encoder::SourceLocation::new(PathBuf::from("test.rs"), 10, 1, 20, 2);

    let binding =
        FfiBinding::new("rust", "c", "fn", FfiKind::Import).with_location(location.clone());

    assert!(binding.location.is_some());
    let loc = binding.location.unwrap();
    assert_eq!(loc.start_line, 10);
}

#[test]
fn test_ffi_binding_with_signature() {
    let binding = FfiBinding::new("rust", "c", "fn", FfiKind::Import)
        .with_signature("int (*)(char*, size_t)");

    assert!(binding.native_signature.is_some());
    assert_eq!(binding.native_signature.unwrap(), "int (*)(char*, size_t)");
}

#[test]
fn test_ffi_binding_to_metadata() {
    let binding =
        FfiBinding::new("rust", "c", "test_fn", FfiKind::Export).with_signature("void test_fn()");

    let metadata = binding.to_metadata();

    assert_eq!(metadata.get("ffi_source").unwrap(), "rust");
    assert_eq!(metadata.get("ffi_target").unwrap(), "c");
    assert!(metadata.contains_key("ffi_kind"));
    assert!(metadata.contains_key("ffi_signature"));
}

#[test]
fn test_ffi_kind_variants() {
    assert_eq!(FfiKind::Export, FfiKind::Export);
    assert_ne!(FfiKind::Export, FfiKind::Import);
    assert_ne!(FfiKind::Callback, FfiKind::TypeBinding);
}

#[test]
fn test_ffi_detect_extern_blocks_empty() {
    let source = "fn main() {}";
    let file = PathBuf::from("test.rs");
    let bindings = FfiDetector::detect_extern_blocks(source, &file, &["C"]);

    assert!(bindings.is_empty());
}

#[test]
fn test_ffi_detect_no_mangle_empty() {
    let source = "fn main() {}";
    let file = PathBuf::from("test.rs");
    let bindings = FfiDetector::detect_no_mangle(source, &file);

    assert!(bindings.is_empty());
}

#[test]
fn test_ffi_detect_cgo_exports_empty() {
    let source = "package main\nfunc main() {}";
    let file = PathBuf::from("test.go");
    let bindings = FfiDetector::detect_cgo_exports(source, &file);

    assert!(bindings.is_empty());
}

#[test]
fn test_ffi_detect_python_ctypes_empty() {
    let source = "def main(): pass";
    let file = PathBuf::from("test.py");
    let bindings = FfiDetector::detect_python_ctypes(source, &file);

    assert!(bindings.is_empty());
}

#[test]
fn test_ffi_detect_ruby_ffi_empty() {
    let source = "def main; end";
    let file = PathBuf::from("test.rb");
    let bindings = FfiDetector::detect_ruby_ffi(source, &file);

    assert!(bindings.is_empty());
}

#[test]
fn test_ffi_detect_cpp_extern_c_empty() {
    let source = "int main() { return 0; }";
    let file = PathBuf::from("test.cpp");
    let bindings = FfiDetector::detect_cpp_extern_c(source, &file);

    assert!(bindings.is_empty());
}
