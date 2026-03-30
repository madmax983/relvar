use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue, Tuple};

fn create_test_relation(size: usize) -> Relation {
    let mut heading = TupleType::new();
    for i in 0..5 {
        heading = heading.with_attribute(format!("attr_{}", i), ScalarType::Int);
    }

    let rel_type = RelationType::new(heading.clone());
    let mut rel = Relation::new(rel_type);

    for r in 0..size {
        let mut values = std::collections::BTreeMap::new();
        for i in 0..5 {
            values.insert(format!("attr_{}", i), ScalarValue::Int(r as i64 + i as i64));
        }
        let tuple = Tuple::new(heading.clone(), values).unwrap();
        rel.insert(tuple).unwrap();
    }

    rel
}

fn bench_rename(c: &mut Criterion) {
    let mut group = c.benchmark_group("rename");

    let mappings = vec![
        ("attr_0", "renamed_0"),
        ("attr_2", "renamed_2"),
        ("attr_4", "renamed_4"),
    ];
    let mappings_refs: Vec<(&str, &str)> = mappings.iter().map(|(a, b)| (*a, *b)).collect();

    for size in [100, 1000, 5000].iter() {
        group.bench_with_input(BenchmarkId::new("rename", size), size, |b, &size| {
            b.iter_with_setup(
                || create_test_relation(size),
                |rel| rel.rename(&mappings_refs),
            );
        });

        group.bench_with_input(BenchmarkId::new("rename_into", size), size, |b, &size| {
            b.iter_with_setup(
                || create_test_relation(size),
                |rel| rel.rename_into(&mappings_refs),
            );
        });
    }
    group.finish();
}

criterion_group!(benches, bench_rename);
criterion_main!(benches);
