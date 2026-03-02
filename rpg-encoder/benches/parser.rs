mod fixtures;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rpg_encoder::languages::RustParser;
use rpg_encoder::LanguageParser;
use std::path::PathBuf;

fn parser_init(c: &mut Criterion) {
    c.bench_function("parser/init/rust", |b| {
        b.iter(|| {
            let parser = RustParser::new().unwrap();
            black_box(parser)
        })
    });
}

fn parser_single_file(c: &mut Criterion) {
    let parser = RustParser::new().expect("Failed to create parser");
    let path = PathBuf::from("test.rs");

    let mut group = c.benchmark_group("parser/single_file");

    group.bench_function("empty", |b| {
        b.iter(|| parser.parse(black_box(fixtures::empty_rust_code()), &path))
    });

    group.bench_function("typical_function", |b| {
        b.iter(|| parser.parse(black_box(fixtures::typical_function()), &path))
    });

    group.bench_function("complex_struct", |b| {
        b.iter(|| parser.parse(black_box(fixtures::complex_struct()), &path))
    });

    group.bench_function("deeply_nested", |b| {
        b.iter(|| parser.parse(black_box(fixtures::deeply_nested()), &path))
    });

    group.bench_function("many_imports", |b| {
        b.iter(|| parser.parse(black_box(fixtures::many_imports()), &path))
    });

    group.finish();
}

fn parser_batch(c: &mut Criterion) {
    let parser = RustParser::new().expect("Failed to create parser");
    let _path = PathBuf::from("test.rs");

    let mut group = c.benchmark_group("parser/batch");
    group.throughput(Throughput::Elements(1));

    let small_fixture =
        fixtures::FixtureSet::generate("bench_small", fixtures::FixtureConfig::small())
            .expect("Failed to generate small fixture");

    let medium_fixture =
        fixtures::FixtureSet::generate("bench_medium", fixtures::FixtureConfig::medium())
            .expect("Failed to generate medium fixture");

    let small_files: Vec<_> = small_fixture.files.iter().take(10).collect();
    let medium_files: Vec<_> = medium_fixture.files.iter().take(50).collect();

    group.bench_function("parse_10_files", |b| {
        b.iter(|| {
            for file_path in &small_files {
                if let Ok(source) = std::fs::read_to_string(file_path) {
                    let _ = parser.parse(black_box(&source), file_path);
                }
            }
        })
    });

    group.bench_function("parse_50_files", |b| {
        b.iter(|| {
            for file_path in &medium_files {
                if let Ok(source) = std::fs::read_to_string(file_path) {
                    let _ = parser.parse(black_box(&source), file_path);
                }
            }
        })
    });

    group.finish();

    drop(small_fixture);
    drop(medium_fixture);
}

fn parser_by_file_size(c: &mut Criterion) {
    let parser = RustParser::new().expect("Failed to create parser");
    let path = PathBuf::from("test.rs");

    let mut group = c.benchmark_group("parser/by_size");

    let sizes = [
        (100, "~100 bytes"),
        (500, "~500 bytes"),
        (1000, "~1KB"),
        (5000, "~5KB"),
        (10000, "~10KB"),
    ];

    for (target_size, label) in sizes {
        let mut config = fixtures::FixtureConfig::small();
        config.file_count = 1;
        config.functions_per_file = target_size / 50;
        config.structs_per_file = target_size / 200;

        let fixture = fixtures::FixtureSet::generate(&format!("size_{}", target_size), config)
            .expect("Failed to generate fixture");

        if let Some(file_path) = fixture.files.first() {
            if let Ok(source) = std::fs::read_to_string(file_path) {
                group.throughput(Throughput::Bytes(source.len() as u64));
                group.bench_with_input(BenchmarkId::new("file", label), &source, |b, source| {
                    b.iter(|| parser.parse(black_box(source), &path))
                });
            }
        }

        drop(fixture);
    }

    group.finish();
}

fn parser_extraction_operations(c: &mut Criterion) {
    let parser = RustParser::new().expect("Failed to create parser");
    let path = PathBuf::from("test.rs");

    let mut group = c.benchmark_group("parser/extraction");

    let multi_def_code = (0..20)
        .map(|i| {
            format!(
                r#"
pub fn function_{}(x: i32) -> Result<String, String> {{
    Ok(format!("{{}}", x))
}}

pub struct Struct{} {{
    field1: i32,
    field2: String,
}}

impl Struct{} {{
    pub fn new() -> Self {{
        Self {{ field1: 0, field2: String::new() }}
    }}
}}
"#,
                i, i, i
            )
        })
        .collect::<String>();

    group.bench_function("20_functions_20_structs", |b| {
        b.iter(|| parser.parse(black_box(&multi_def_code), &path))
    });

    let call_graph_code = r#"
pub fn caller_a() {
    let _ = helper_1();
    let _ = helper_2();
    for _ in 0..10 {
        let _ = nested_call();
    }
}

pub fn caller_b() {
    caller_a();
    let x = StructA::new();
    x.method_call();
}

pub fn caller_c() -> Result<i32, String> {
    caller_b();
    caller_a();
    Ok(42)
}

pub fn helper_1() -> i32 { 1 }
pub fn helper_2() -> i32 { 2 }
pub fn nested_call() -> i32 { helper_1() + helper_2() }

pub struct StructA;
impl StructA {
    pub fn new() -> Self { Self }
    pub fn method_call(&self) {}
}
"#;

    group.bench_function("call_graph_extraction", |b| {
        b.iter(|| parser.parse(black_box(call_graph_code), &path))
    });

    let type_refs_code = r#"
use std::collections::HashMap;

pub fn process(
    data: Vec<String>,
    map: HashMap<String, i32>,
    opt: Option<Box<dyn std::error::Error>>,
) -> Result<Vec<HashMap<String, Vec<i32>>>, String> {
    let inner: Vec<i32> = data.iter().map(|s| s.len() as i32).collect();
    let mut result: HashMap<String, Vec<i32>> = HashMap::new();
    result.insert("key".to_string(), inner);
    Ok(vec![result])
}

pub struct ComplexType {
    field1: Vec<HashMap<String, Option<i32>>>,
    field2: Box<dyn Fn(i32) -> Result<String, String>>,
    field3: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
}
"#;

    group.bench_function("type_ref_extraction", |b| {
        b.iter(|| parser.parse(black_box(type_refs_code), &path))
    });

    group.finish();
}

criterion_group!(
    parser_benches,
    parser_init,
    parser_single_file,
    parser_batch,
    parser_by_file_size,
    parser_extraction_operations,
);

criterion_main!(parser_benches);
