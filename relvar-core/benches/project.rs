use criterion::{criterion_group, criterion_main, Criterion};
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;
use relvar_core::tuple;

pub fn project_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("project");

    let heading = TupleType::new()
        .with_attribute("a", ScalarType::Int)
        .with_attribute("b", ScalarType::Int)
        .with_attribute("c", ScalarType::Int)
        .with_attribute("d", ScalarType::Int)
        .with_attribute("e", ScalarType::Int);
    let rel_type = RelationType::new(heading);
    let mut rel = Relation::new(rel_type);

    for i in 0..1000 {
        rel.insert(tuple! {
            a: i as i64,
            b: (i * 2) as i64,
            c: (i * 3) as i64,
            d: (i * 4) as i64,
            e: (i * 5) as i64,
        }).unwrap();
    }

    group.bench_function("project 2 attributes", |b| {
        b.iter(|| {
            rel.project(&["a", "d"])
        });
    });

    group.finish();
}

criterion_group!(benches, project_bench);
criterion_main!(benches);
