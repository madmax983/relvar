use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

fn create_employee_relation(size: usize) -> Relation {
    let heading = TupleType::new()
        .with_attribute("emp_id".to_string(), ScalarType::Int)
        .with_attribute("salary".to_string(), ScalarType::Float);

    let mut relation = Relation::new(RelationType::new(heading));

    for i in 0..size {
        let tuple = tuple! {
            emp_id: i as i64,
            salary: 50000.0 + (i as f64 * 1000.0)
        };
        relation.insert(tuple).unwrap();
    }
    relation
}

fn bench_extend_into(c: &mut Criterion) {
    let mut group = c.benchmark_group("extend_into");

    for size in [100, 500, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let relation = create_employee_relation(size);

            b.iter(|| {
                let rel = relation.clone();
                let result = rel.extend_into("annual_salary", ScalarType::Float, |t| {
                    ScalarValue::Float(t.get_typed::<f64>("salary").unwrap() * 12.0)
                });
                let _ = black_box(result);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_extend_into);
criterion_main!(benches);
