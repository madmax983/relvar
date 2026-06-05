use criterion::{criterion_group, criterion_main, Criterion};
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;
use relvar_core::tuple;
use relvar_core::algebra::Aggregation;

pub fn group_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("group");

    let heading = TupleType::new()
        .with_attribute("a", ScalarType::Int)
        .with_attribute("b", ScalarType::Int)
        .with_attribute("c", ScalarType::Int);
    let rel_type = RelationType::new(heading);
    let mut rel = Relation::new(rel_type);

    for i in 0..1000 {
        rel.insert(tuple! {
            a: (i % 10) as i64,
            b: i as i64,
            c: (i * 3) as i64,
        }).unwrap();
    }

    group.bench_function("group by a", |b| {
        b.iter(|| {
            rel.group(&["b", "c"], "rva")
        });
    });

    group.finish();
}

criterion_group!(benches, group_bench);
criterion_main!(benches);
