use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue, Tuple};
use std::collections::HashMap;
use std::sync::Arc;

fn bench_tuple_new(c: &mut Criterion) {
    let mut group = c.benchmark_group("tuple_creation");

    // Setup a complex tuple type (50 attributes)
    let mut heading = TupleType::new();
    for i in 0..50 {
        heading = heading.with_attribute(format!("attr_{}", i), ScalarType::Int);
    }
    let heading_arc = Arc::new(heading.clone());

    // Create values
    let mut values = HashMap::new();
    for i in 0..50 {
        values.insert(format!("attr_{}", i), ScalarValue::Int(i));
    }

    for count in [1000, 10000].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(
            BenchmarkId::new("clone_tuple_type", count),
            count,
            |b, &count| {
                b.iter(|| {
                    for _ in 0..count {
                        // Old behavior: Clone the TupleType for each tuple
                        let _ = Tuple::new(heading.clone(), values.clone());
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("reuse_arc_tuple_type", count),
            count,
            |b, &count| {
                b.iter(|| {
                    for _ in 0..count {
                        // New behavior: Pass a clone of the Arc (cheap)
                        let _ = Tuple::new(heading_arc.clone(), values.clone());
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_relation_from_tuples(c: &mut Criterion) {
    let mut group = c.benchmark_group("relation_from_tuples");

    // Setup a complex tuple type (50 attributes) to make type checking expensive
    let mut heading = TupleType::new();
    for i in 0..50 {
        heading = heading.with_attribute(format!("attr_{}", i), ScalarType::Int);
    }
    let rel_type = RelationType::new(heading.clone());
    let heading_arc = Arc::new(heading);

    // Create values
    let mut values = HashMap::new();
    for i in 0..50 {
        values.insert(format!("attr_{}", i), ScalarValue::Int(i));
    }

    // Create a vector of identical tuples sharing the same Arc<TupleType>
    // This simulates bulk loading or import scenarios
    let mut tuples = Vec::new();
    // We create enough tuples to measure performance
    let base_tuple = Tuple::new(heading_arc, values).unwrap();

    // We will benchmark with different sizes
    for size in [100, 1000, 10000].iter() {
        tuples.clear();
        for _ in 0..*size {
            tuples.push(base_tuple.clone());
        }

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("from_tuples", size), size, |b, &_size| {
            // Clone the vector for each iteration to isolate from_tuples cost
            // (though vector clone is cheap compared to relation construction if tuples are Arcs)
            // But from_tuples moves tuples out of iterator.
            // We need to provide fresh tuples each time.
            b.iter_with_setup(
                || tuples.clone(),
                |tuples| {
                    let _ = Relation::from_tuples(rel_type.clone(), tuples);
                },
            );
        });
    }

    group.finish();
}

criterion_group!(benches, bench_tuple_new, bench_relation_from_tuples);
criterion_main!(benches);
