mod fixtures;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rpg_encoder::{FileWalker, GraphBuilder, RpgEncoder};
use std::path::Path;

fn walker_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("encoder/walker");

    let small_fixture =
        fixtures::FixtureSet::generate("walker_small", fixtures::FixtureConfig::small())
            .expect("Failed to generate small fixture");

    let medium_fixture =
        fixtures::FixtureSet::generate("walker_medium", fixtures::FixtureConfig::medium())
            .expect("Failed to generate medium fixture");

    group.throughput(Throughput::Elements(small_fixture.files.len() as u64));
    group.bench_function("walk_small_repo", |b| {
        b.iter(|| {
            let walker = FileWalker::new(&small_fixture.base_path);
            black_box(walker.walk())
        })
    });

    group.throughput(Throughput::Elements(medium_fixture.files.len() as u64));
    group.bench_function("walk_medium_repo", |b| {
        b.iter(|| {
            let walker = FileWalker::new(&medium_fixture.base_path);
            black_box(walker.walk())
        })
    });

    group.finish();

    drop(small_fixture);
    drop(medium_fixture);
}

fn builder_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("encoder/builder");

    group.bench_function("new_builder", |b| b.iter(|| GraphBuilder::new()));

    group.bench_function("add_file_single", |b| {
        b.iter_batched(
            || GraphBuilder::new().with_repo("test", Path::new(".")),
            |builder| {
                let builder = builder.add_file(Path::new("src/main.rs"), "rust");
                black_box(builder)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.bench_function("add_100_files", |b| {
        b.iter_batched(
            || GraphBuilder::new().with_repo("test", Path::new(".")),
            |builder| {
                let mut builder = builder;
                for i in 0..100 {
                    let path = format!("src/module_{:04}.rs", i);
                    builder = builder.add_file(Path::new(&path), "rust");
                }
                black_box(builder)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.bench_function("link_imports/100_nodes", |b| {
        b.iter_batched(
            || {
                let mut builder = GraphBuilder::new().with_repo("test", Path::new("."));
                for i in 0..50 {
                    builder = builder.add_file(Path::new(&format!("src/mod{}.rs", i)), "rust");
                }
                builder
            },
            |builder| {
                let builder = builder.link_imports();
                black_box(builder)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.bench_function("link_calls/100_nodes", |b| {
        b.iter_batched(
            || {
                let mut builder = GraphBuilder::new().with_repo("test", Path::new("."));
                for i in 0..50 {
                    builder = builder.add_file(Path::new(&format!("src/mod{}.rs", i)), "rust");
                }
                builder
            },
            |builder| {
                let builder = builder.link_calls();
                black_box(builder)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.bench_function("link_all/100_nodes", |b| {
        b.iter_batched(
            || {
                let mut builder = GraphBuilder::new().with_repo("test", Path::new("."));
                for i in 0..50 {
                    builder = builder.add_file(Path::new(&format!("src/mod{}.rs", i)), "rust");
                }
                builder
            },
            |builder| {
                let builder = builder.link_all();
                black_box(builder)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn full_encode_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("encoder/full_pipeline");

    let small_config = fixtures::FixtureConfig::small();
    let small_fixture = fixtures::FixtureSet::generate("encode_small", small_config.clone())
        .expect("Failed to generate small fixture");

    let medium_config = fixtures::FixtureConfig::medium();
    let medium_fixture = fixtures::FixtureSet::generate("encode_medium", medium_config.clone())
        .expect("Failed to generate medium fixture");

    group.throughput(Throughput::Elements(small_fixture.files.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("encode", "small_10_files"),
        &small_fixture.base_path,
        |b, path| {
            b.iter_batched(
                || RpgEncoder::new().expect("Failed to create encoder"),
                |mut encoder| {
                    let result = encoder.encode(path).expect("Encode failed");
                    black_box(result)
                },
                criterion::BatchSize::SmallInput,
            )
        },
    );

    group.throughput(Throughput::Elements(medium_fixture.files.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("encode", "medium_50_files"),
        &medium_fixture.base_path,
        |b, path| {
            b.iter_batched(
                || RpgEncoder::new().expect("Failed to create encoder"),
                |mut encoder| {
                    let result = encoder.encode(path).expect("Encode failed");
                    black_box(result)
                },
                criterion::BatchSize::SmallInput,
            )
        },
    );

    group.finish();

    drop(small_fixture);
    drop(medium_fixture);
}

fn encode_by_complexity(c: &mut Criterion) {
    let mut group = c.benchmark_group("encoder/by_complexity");

    let configs = [
        (
            "simple",
            fixtures::FixtureConfig {
                file_count: 20,
                functions_per_file: 2,
                structs_per_file: 1,
                imports_per_file: 1,
                call_depth: 0,
                nesting_depth: 0,
            },
        ),
        (
            "typical",
            fixtures::FixtureConfig {
                file_count: 20,
                functions_per_file: 5,
                structs_per_file: 2,
                imports_per_file: 3,
                call_depth: 2,
                nesting_depth: 1,
            },
        ),
        (
            "complex",
            fixtures::FixtureConfig {
                file_count: 20,
                functions_per_file: 10,
                structs_per_file: 4,
                imports_per_file: 6,
                call_depth: 4,
                nesting_depth: 3,
            },
        ),
    ];

    for (name, config) in configs {
        let fixture =
            fixtures::FixtureSet::generate(&format!("complexity_{}", name), config.clone())
                .expect("Failed to generate fixture");

        group.throughput(Throughput::Elements(fixture.files.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("encode", name),
            &fixture.base_path.clone(),
            |b, path| {
                b.iter_batched(
                    || RpgEncoder::new().expect("Failed to create encoder"),
                    |mut encoder| {
                        let result = encoder.encode(path).expect("Encode failed");
                        black_box(result)
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );

        drop(fixture);
    }

    group.finish();
}

fn encode_output_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("encoder/output");

    let fixture = fixtures::FixtureSet::generate("output_bench", fixtures::FixtureConfig::medium())
        .expect("Failed to generate fixture");

    let mut encoder = RpgEncoder::new().expect("Failed to create encoder");
    let _ = encoder.encode(&fixture.base_path).expect("Encode failed");

    group.bench_function("to_json", |b| b.iter(|| encoder.to_json()));

    group.bench_function("to_json_compact", |b| b.iter(|| encoder.to_json_compact()));

    group.finish();

    drop(fixture);
}

fn incremental_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("encoder/incremental");

    let small_fixture =
        fixtures::FixtureSet::generate("incr_small", fixtures::FixtureConfig::small())
            .expect("Failed to generate small fixture");

    group.bench_function("encode_cold_start", |b| {
        b.iter_batched(
            || RpgEncoder::new().expect("Failed to create encoder"),
            |mut encoder| {
                encoder
                    .encode(&small_fixture.base_path)
                    .expect("Encode failed")
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.bench_function("encode_warm_cache", |b| {
        let mut encoder = RpgEncoder::new().expect("Failed to create encoder");
        let _ = encoder.encode(&small_fixture.base_path);

        b.iter(|| {
            encoder
                .encode(&small_fixture.base_path)
                .expect("Encode failed")
        })
    });

    group.finish();

    drop(small_fixture);
}

criterion_group!(
    encoder_benches,
    walker_benchmark,
    builder_benchmark,
    full_encode_benchmark,
    encode_by_complexity,
    encode_output_benchmark,
    incremental_benchmark,
);

criterion_main!(encoder_benches);
