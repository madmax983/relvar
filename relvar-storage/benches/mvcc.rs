use criterion::{black_box, criterion_group, criterion_main, Criterion};
use relvar_core::storage_engine::StorageEngine;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;
use relvar_storage::PersistentEngine;
use tempfile::TempDir;

fn create_test_relation_type() -> RelationType {
    let heading = TupleType::new()
        .with_attribute("id".to_string(), ScalarType::Int)
        .with_attribute("val".to_string(), ScalarType::Int);
    RelationType::new(heading)
}

fn create_test_relation(count: i64) -> Relation {
    let rel_type = create_test_relation_type();
    let mut relation = Relation::new(rel_type);
    for i in 0..count {
        relation
            .insert(tuple! { id: i, val: i * 2 })
            .unwrap();
    }
    relation
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("mvcc");

    group.bench_function("load_relation", |b| {
        b.iter_with_setup(
            || {
                let temp_dir = TempDir::new().unwrap();
                let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
                let rel_type = create_test_relation_type();
                engine.create_relation("TEST", rel_type).unwrap();
                let relation = create_test_relation(100);
                engine.store_relation("TEST", &relation).unwrap();
                (engine, temp_dir)
            },
            |(engine, _temp_dir): (PersistentEngine, TempDir)| {
                let _relation = black_box(engine.load_relation("TEST").unwrap());
            },
        );
    });

    group.bench_function("store_relation", |b| {
        b.iter_with_setup(
            || {
                let temp_dir = TempDir::new().unwrap();
                let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
                let rel_type = create_test_relation_type();
                engine.create_relation("TEST", rel_type).unwrap();
                let relation = create_test_relation(100);
                (engine, relation, temp_dir)
            },
            |(mut engine, relation, _temp_dir): (PersistentEngine, Relation, TempDir)| {
                engine.store_relation("TEST", &relation).unwrap();
            },
        );
    });

    group.bench_function("checkpoint", |b| {
        b.iter_with_setup(
            || {
                let temp_dir = TempDir::new().unwrap();
                let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
                let rel_type = create_test_relation_type();
                engine.create_relation("TEST", rel_type).unwrap();
                for _ in 0..10 {
                    let relation = create_test_relation(10);
                    engine.store_relation("TEST", &relation).unwrap();
                }
                (engine, temp_dir)
            },
            |(mut engine, _temp_dir): (PersistentEngine, TempDir)| {
                engine.checkpoint().unwrap();
            },
        );
    });

    group.bench_function("create_relation", |b| {
        b.iter_with_setup(
            || {
                let temp_dir = TempDir::new().unwrap();
                let engine = PersistentEngine::open(temp_dir.path()).unwrap();
                (engine, temp_dir)
            },
            |(mut engine, _temp_dir): (PersistentEngine, TempDir)| {
                let rel_type = create_test_relation_type();
                engine.create_relation("TEST", rel_type).unwrap();
            },
        );
    });

    group.bench_function("load_relation_multiple_versions", |b| {
        b.iter_with_setup(
            || {
                let temp_dir = TempDir::new().unwrap();
                let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
                let rel_type = create_test_relation_type();
                engine.create_relation("TEST", rel_type).unwrap();
                for i in 0..5 {
                    let relation = create_test_relation(i * 10);
                    engine.store_relation("TEST", &relation).unwrap();
                }
                (engine, temp_dir)
            },
            |(engine, _temp_dir): (PersistentEngine, TempDir)| {
                for _ in 0..5 {
                    let _relation = black_box(engine.load_relation("TEST").unwrap());
                }
            },
        );
    });

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
