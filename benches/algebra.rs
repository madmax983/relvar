use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use relvar::algebra::extend::ExtendOps;
use relvar::algebra::summarize::{Aggregation, AggregationFn, SummarizeOps};
use relvar::tuple;
use relvar::types::{RelationType, ScalarType, TupleType};
use relvar::values::{Relation, ScalarValue};

fn create_employee_relation(size: usize) -> Relation {
    let heading = TupleType::new()
        .with_attribute("emp_id".to_string(), ScalarType::Int)
        .with_attribute("name".to_string(), ScalarType::String)
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("salary".to_string(), ScalarType::Float);

    let mut relation = Relation::new(RelationType::new(heading));

    for i in 0..size {
        let tuple = tuple! {
            emp_id: i as i64,
            name: format!("Employee_{}", i),
            dept_id: (i % 10) as i64,
            salary: 50000.0 + (i as f64 * 100.0)
        };
        relation.insert(tuple).unwrap();
    }

    relation
}

fn create_department_relation(size: usize) -> Relation {
    let heading = TupleType::new()
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("dept_name".to_string(), ScalarType::String);

    let mut relation = Relation::new(RelationType::new(heading));

    for i in 0..size {
        let tuple = tuple! {
            dept_id: i as i64,
            dept_name: format!("Department_{}", i)
        };
        relation.insert(tuple).unwrap();
    }

    relation
}

// Restrict benchmark
fn bench_restrict(c: &mut Criterion) {
    let mut group = c.benchmark_group("restrict");

    for size in [100, 500, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let relation = create_employee_relation(size);

            b.iter(|| {
                let result = relation.restrict(|t| {
                    t.get_typed::<i64>("dept_id").unwrap() == 5
                        && t.get_typed::<f64>("salary").unwrap() > 60000.0
                });
                black_box(result);
            });
        });
    }
    group.finish();
}

// Project benchmark
fn bench_project(c: &mut Criterion) {
    let mut group = c.benchmark_group("project");

    for size in [100, 500, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let relation = create_employee_relation(size);

            b.iter(|| {
                let result = relation.project(&["emp_id", "name"]);
                black_box(result);
            });
        });
    }
    group.finish();
}

// Rename benchmark
fn bench_rename(c: &mut Criterion) {
    let mut group = c.benchmark_group("rename");

    for size in [100, 500, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let relation = create_employee_relation(size);

            b.iter(|| {
                let result = relation.rename(&[
                    ("emp_id", "employee_id"),
                    ("dept_id", "department_id"),
                ]);
                black_box(result);
            });
        });
    }
    group.finish();
}

// Join benchmark
fn bench_join(c: &mut Criterion) {
    let mut group = c.benchmark_group("join");

    for size in [10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let employees = create_employee_relation(size * 10);
            let departments = create_department_relation(size);

            b.iter(|| {
                let result = employees.join(&departments);
                black_box(result);
            });
        });
    }
    group.finish();
}

// Union benchmark
fn bench_union(c: &mut Criterion) {
    let mut group = c.benchmark_group("union");

    for size in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let rel1 = create_employee_relation(size);
            let rel2 = create_employee_relation(size / 2);

            b.iter(|| {
                let result = rel1.union(&rel2).unwrap();
                black_box(result);
            });
        });
    }
    group.finish();
}

// Intersect benchmark
fn bench_intersect(c: &mut Criterion) {
    let mut group = c.benchmark_group("intersect");

    for size in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let rel1 = create_employee_relation(size);
            let rel2 = create_employee_relation(size / 2);

            b.iter(|| {
                let result = rel1.intersect(&rel2).unwrap();
                black_box(result);
            });
        });
    }
    group.finish();
}

// Difference benchmark
fn bench_difference(c: &mut Criterion) {
    let mut group = c.benchmark_group("difference");

    for size in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let rel1 = create_employee_relation(size);
            let rel2 = create_employee_relation(size / 2);

            b.iter(|| {
                let result = rel1.difference(&rel2).unwrap();
                black_box(result);
            });
        });
    }
    group.finish();
}

// Extend benchmark
fn bench_extend(c: &mut Criterion) {
    let mut group = c.benchmark_group("extend");

    for size in [100, 500, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let relation = create_employee_relation(size);

            b.iter(|| {
                let result = relation
                    .extend("annual_salary", ScalarType::Float, |t| {
                        ScalarValue::Float(t.get_typed::<f64>("salary").unwrap() * 12.0)
                    })
                    .unwrap();
                black_box(result);
            });
        });
    }
    group.finish();
}

// Summarize benchmark
fn bench_summarize(c: &mut Criterion) {
    let mut group = c.benchmark_group("summarize");

    for size in [100, 500, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let relation = create_employee_relation(size);

            b.iter(|| {
                let result = relation
                    .summarize(
                        &["dept_id"],
                        &[
                            Aggregation::count("count"),
                            Aggregation {
                                result_name: "avg_salary".to_string(),
                                result_type: ScalarType::Float,
                                function: AggregationFn::Custom(Box::new(|tuples| {
                                    if tuples.is_empty() {
                                        return ScalarValue::Float(0.0);
                                    }
                                    let sum: f64 = tuples
                                        .iter()
                                        .map(|t| t.get_typed::<f64>("salary").unwrap())
                                        .sum();
                                    ScalarValue::Float(sum / tuples.len() as f64)
                                })),
                            },
                        ],
                    )
                    .unwrap();
                black_box(result);
            });
        });
    }
    group.finish();
}

// Chained operations benchmark (realistic query)
fn bench_chained_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("chained_operations");

    for size in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let employees = create_employee_relation(size);

            b.iter(|| {
                let result = employees
                    .restrict(|t| t.get_typed::<f64>("salary").unwrap() > 60000.0)
                    .extend("bonus", ScalarType::Float, |t| {
                        ScalarValue::Float(t.get_typed::<f64>("salary").unwrap() * 0.1)
                    })
                    .unwrap()
                    .project(&["emp_id", "name", "salary", "bonus"]);
                black_box(result);
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_restrict,
    bench_project,
    bench_rename,
    bench_join,
    bench_union,
    bench_intersect,
    bench_difference,
    bench_extend,
    bench_summarize,
    bench_chained_operations
);
criterion_main!(benches);
