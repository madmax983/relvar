use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use relvar_core::database::Database;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};

fn create_database(size: usize) -> Database<InMemoryEngine> {
    let mut db = Database::new(InMemoryEngine::new());
    let heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("value", ScalarType::Int);

    let rel_type = RelationType::new(heading);
    db.create_relvar("TEST", rel_type).unwrap();

    for i in 0..size {
        db.insert("TEST", tuple! { id: i as i64, value: (i % 2) as i64 })
            .unwrap();
    }

    db
}

fn bench_delete(c: &mut Criterion) {
    let mut group = c.benchmark_group("delete");

    for size in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched(
                || create_database(size),
                |mut db| {
                    // Delete where value == 0 (approx 50%)
                    db.delete("TEST", |t| t.get_typed::<i64>("value").unwrap() == 0)
                        .unwrap()
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, bench_delete);
criterion_main!(benches);
