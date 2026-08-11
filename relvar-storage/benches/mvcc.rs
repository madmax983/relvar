//! MVCC performance benchmarks
//!
//! Benchmarks for Multi-Version Concurrency Control operations using public API only.
//! Internal MVCC mechanisms are tested but not directly benchmarked to maintain
//! TTM compliance (no exposure of physical implementation details).

use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use relvar_core::StorageEngine;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;
use relvar_storage::persistent_engine::PersistentEngine;
use tempfile::TempDir;

fn create_test_relation_type() -> RelationType {
    let tuple_type = TupleType::new()
        .with_attribute("id".to_string(), ScalarType::Int)
        .with_attribute("name".to_string(), ScalarType::String)
        .with_attribute("value".to_string(), ScalarType::Int);
    RelationType::new(tuple_type)
}

/// Benchmark relation load performance (exercises MVCC visibility checks)
fn bench_load_relation(c: &mut Criterion) {
    c.bench_function("mvcc_load_relation_100_tuples", |b| {
        b.iter_batched(
            || {
                // Setup: Create engine with pre-populated data
                let temp_dir = TempDir::new().unwrap();
                let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
                let rel_type = create_test_relation_type();
                engine.create_relation("TEST", rel_type.clone()).unwrap();

                // Insert 100 tuples
                let mut tuples = Vec::new();
                for i in 1..=100 {
                    tuples.push(tuple! {
                        id: i,
                        name: format!("Test{}", i),
                        value: i * 10
                    });
                }
                let relation = Relation::from_tuples(rel_type, tuples).unwrap();
                engine.store_relation("TEST", &relation).unwrap();

                (engine, temp_dir)
            },
            |(engine, _temp_dir): (PersistentEngine, tempfile::TempDir)| {
                // Benchmark: Load relation (exercises MVCC visibility checks)
                let _relation = black_box(engine.load_relation("TEST").unwrap());
            },
            BatchSize::SmallInput,
        );
    });
}

/// Benchmark relation store performance (exercises MVCC versioning)
fn bench_store_relation(c: &mut Criterion) {
    for tuple_count in [10, 100, 1000].iter() {
        c.bench_function(
            &format!("mvcc_store_relation_{}_tuples", tuple_count),
            |b| {
                b.iter_batched(
                    || {
                        // Setup
                        let temp_dir = TempDir::new().unwrap();
                        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
                        let rel_type = create_test_relation_type();
                        engine.create_relation("TEST", rel_type.clone()).unwrap();

                        // Create relation with N tuples
                        let mut tuples = Vec::new();
                        for i in 1..=*tuple_count {
                            tuples.push(tuple! {
                                id: i,
                                name: format!("Test{}", i),
                                value: i * 10
                            });
                        }
                        let relation = Relation::from_tuples(rel_type, tuples).unwrap();

                        (engine, relation, temp_dir)
                    },
                    |(mut engine, relation, _temp_dir): (PersistentEngine, relvar_core::values::Relation, tempfile::TempDir)| {
                        // Benchmark: Store relation
                        engine.store_relation("TEST", &relation).unwrap();
                        black_box(());
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
}

/// Benchmark checkpoint performance (exercises GC)
fn bench_checkpoint(c: &mut Criterion) {
    c.bench_function("mvcc_checkpoint_with_gc", |b| {
        b.iter_batched(
            || {
                // Setup: Create engine with data
                let temp_dir = TempDir::new().unwrap();
                let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
                let rel_type = create_test_relation_type();
                engine.create_relation("TEST", rel_type.clone()).unwrap();

                // Store some data
                let mut tuples = Vec::new();
                for i in 1..=100 {
                    tuples.push(tuple! {
                        id: i,
                        name: format!("Test{}", i),
                        value: i
                    });
                }
                let relation = Relation::from_tuples(rel_type, tuples).unwrap();
                engine.store_relation("TEST", &relation).unwrap();

                (engine, temp_dir)
            },
            |(mut engine, _temp_dir): (PersistentEngine, tempfile::TempDir)| {
                // Benchmark: Checkpoint (includes GC)
                engine.checkpoint().unwrap();
                black_box(());
            },
            BatchSize::SmallInput,
        );
    });
}

/// Benchmark create relation performance
fn bench_create_relation(c: &mut Criterion) {
    c.bench_function("mvcc_create_relation", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let engine = PersistentEngine::open(temp_dir.path()).unwrap();
                (engine, temp_dir)
            },
            |(mut engine, _temp_dir): (PersistentEngine, tempfile::TempDir)| {
                let rel_type = create_test_relation_type();
                engine.create_relation("TEST", rel_type).unwrap();
                black_box(());
            },
            BatchSize::SmallInput,
        );
    });
}

/// Benchmark repeated loads (cache behavior)
fn bench_repeated_loads(c: &mut Criterion) {
    c.bench_function("mvcc_repeated_loads_10x", |b| {
        b.iter_batched(
            || {
                // Setup
                let temp_dir = TempDir::new().unwrap();
                let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
                let rel_type = create_test_relation_type();
                engine.create_relation("TEST", rel_type.clone()).unwrap();

                let mut tuples = Vec::new();
                for i in 1..=100 {
                    tuples.push(tuple! {
                        id: i,
                        name: format!("Test{}", i),
                        value: i * 10
                    });
                }
                let relation = Relation::from_tuples(rel_type, tuples).unwrap();
                engine.store_relation("TEST", &relation).unwrap();

                (engine, temp_dir)
            },
            |(engine, _temp_dir): (PersistentEngine, tempfile::TempDir)| {
                // Benchmark: 10 repeated loads
                for _ in 0..10 {
                    let _relation = black_box(engine.load_relation("TEST").unwrap());
                }
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    benches,
    bench_load_relation,
    bench_store_relation,
    bench_checkpoint,
    bench_create_relation,
    bench_repeated_loads,
);

criterion_main!(benches);
