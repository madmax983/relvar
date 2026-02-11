use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use relvar_core::types::{ScalarType, TupleType};
use relvar_core::values::{ScalarValue, Tuple};
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

criterion_group!(benches, bench_tuple_new);
criterion_main!(benches);
