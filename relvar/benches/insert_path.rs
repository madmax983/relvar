//! Focused insert-path benchmarks for v0.10 "insert/constraint path speed".
//!
//! Exercises the persistent-engine write path (`relvar::open`):
//! single autocommit inserts vs `bulk_insert`, with and without key / CHECK
//! constraints. Used for interleaved before/after A/B.

use criterion::{
    BatchSize, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use relvar::Tuple;
use relvar::constraints::{
    CheckConstraint, CheckConstraints, CmpOp, ConstraintExpression, ForeignKey,
    ForeignKeyConstraints, KeyConstraints, PrimaryKey, ValueOrRef,
};
use relvar::tuple;
use relvar::{PersistentEngine, RelationType, ScalarType, ScalarValue, TupleType};
use std::time::Duration;
use tempfile::TempDir;

type Db = relvar::Database<PersistentEngine>;

fn employee_type() -> RelationType {
    let heading = TupleType::new()
        .with_attribute("emp_id".to_string(), ScalarType::Int)
        .with_attribute("name".to_string(), ScalarType::String)
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("salary".to_string(), ScalarType::Float);
    RelationType::new(heading)
}

fn make_tuple(i: i64) -> Tuple {
    tuple! {
        emp_id: i,
        name: format!("Employee_{}", i),
        dept_id: i % 10,
        salary: 50000.0 + (i as f64 * 100.0)
    }
}

fn create_db() -> (TempDir, Db) {
    let temp_dir = TempDir::new().unwrap();
    let mut db = relvar::open(temp_dir.path()).unwrap();
    db.create_relvar("EMP", employee_type()).unwrap();
    (temp_dir, db)
}

fn with_pk(db: &mut Db) {
    let pk = PrimaryKey::new(vec!["emp_id".to_string()]).unwrap();
    db.set_key_constraints("EMP", KeyConstraints::new().with_primary_key(pk))
        .unwrap();
}

fn with_check(db: &mut Db) {
    let checks = CheckConstraints::new().with_constraint(CheckConstraint::new(
        "salary_positive",
        "Salary must be positive",
        ConstraintExpression::Cmp {
            left: "salary".to_string(),
            op: CmpOp::Gt,
            right: ValueOrRef::Value(ScalarValue::Float(0.0)),
        },
    ));
    db.set_check_constraints("EMP", checks).unwrap();
}

/// Referenced table for the FK benches: 100 departments.
fn with_fk(db: &mut Db) {
    let dept_heading = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
    db.create_relvar("DEPT", RelationType::new(dept_heading))
        .unwrap();
    let dept_tuples: Vec<Tuple> = (0..100).map(|i| tuple! { id: i as i64 }).collect();
    db.bulk_insert("DEPT", dept_tuples).unwrap();
    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    db.set_foreign_key_constraints("EMP", ForeignKeyConstraints::new().with_foreign_key(fk))
        .unwrap();
}

fn make_fk_tuple(i: i64) -> Tuple {
    tuple! {
        emp_id: i,
        name: format!("Employee_{}", i),
        dept_id: i % 100,
        salary: 50000.0 + (i as f64 * 100.0)
    }
}

fn bench_single_inserts(c: &mut Criterion) {
    let mut group = c.benchmark_group("v10_single_inserts");
    group.measurement_time(Duration::from_secs(2));
    group.sample_size(20);
    for count in [20, 100].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter_batched(
                create_db,
                |(_temp_dir, mut db)| {
                    for i in 0..count {
                        db.insert("EMP", make_tuple(i)).unwrap();
                    }
                    black_box(db);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_bulk_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("v10_bulk_insert");
    group.measurement_time(Duration::from_secs(2));
    group.sample_size(20);
    for count in [20, 100].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter_batched(
                create_db,
                |(_temp_dir, mut db)| {
                    let tuples: Vec<_> = (0..count).map(make_tuple).collect();
                    db.bulk_insert("EMP", tuples).unwrap();
                    black_box(db);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_single_inserts_key(c: &mut Criterion) {
    let mut group = c.benchmark_group("v10_single_inserts_key");
    group.measurement_time(Duration::from_secs(2));
    group.sample_size(20);
    for count in [20, 100].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter_batched(
                || {
                    let (t, mut db) = create_db();
                    with_pk(&mut db);
                    (t, db)
                },
                |(_temp_dir, mut db)| {
                    for i in 0..count {
                        db.insert("EMP", make_tuple(i)).unwrap();
                    }
                    black_box(db);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_bulk_insert_key(c: &mut Criterion) {
    let mut group = c.benchmark_group("v10_bulk_insert_key");
    group.measurement_time(Duration::from_secs(2));
    group.sample_size(20);
    for count in [20, 100].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter_batched(
                || {
                    let (t, mut db) = create_db();
                    with_pk(&mut db);
                    (t, db)
                },
                |(_temp_dir, mut db)| {
                    let tuples: Vec<_> = (0..count).map(make_tuple).collect();
                    db.bulk_insert("EMP", tuples).unwrap();
                    black_box(db);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_single_inserts_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("v10_single_inserts_check");
    group.measurement_time(Duration::from_secs(2));
    group.sample_size(20);
    for count in [20, 100].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter_batched(
                || {
                    let (t, mut db) = create_db();
                    with_check(&mut db);
                    (t, db)
                },
                |(_temp_dir, mut db)| {
                    for i in 0..count {
                        db.insert("EMP", make_tuple(i)).unwrap();
                    }
                    black_box(db);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_single_inserts_fk(c: &mut Criterion) {
    let mut group = c.benchmark_group("v11_single_inserts_fk");
    group.measurement_time(Duration::from_secs(2));
    group.sample_size(20);
    for count in [20, 100].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter_batched(
                || {
                    let (t, mut db) = create_db();
                    with_fk(&mut db);
                    (t, db)
                },
                |(_temp_dir, mut db)| {
                    for i in 0..count {
                        db.insert("EMP", make_fk_tuple(i)).unwrap();
                    }
                    black_box(db);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_bulk_insert_fk(c: &mut Criterion) {
    let mut group = c.benchmark_group("v11_bulk_insert_fk");
    group.measurement_time(Duration::from_secs(2));
    group.sample_size(20);
    for count in [20, 100].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter_batched(
                || {
                    let (t, mut db) = create_db();
                    with_fk(&mut db);
                    (t, db)
                },
                |(_temp_dir, mut db)| {
                    let tuples: Vec<_> = (0..count).map(make_fk_tuple).collect();
                    db.bulk_insert("EMP", tuples).unwrap();
                    black_box(db);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// In-memory FK bulk benchmark (v0.11): isolates constraint-evaluation CPU
/// work from persistent-engine fsync noise. `InMemoryEngine::load_relation`
/// clones the referenced relation, so the old per-tuple FK check paid one
/// full clone plus an O(R) scan per tuple; the prepared plan loads once and
/// probes a set.
fn mem_db_with_fk() -> relvar::Database<relvar::InMemoryEngine> {
    let mut db = relvar::in_memory();
    db.create_relvar(
        "DEPT",
        RelationType::new(TupleType::new().with_attribute("id".to_string(), ScalarType::Int)),
    )
    .unwrap();
    let dept_tuples: Vec<Tuple> = (0..100).map(|i| tuple! { id: i as i64 }).collect();
    db.bulk_insert("DEPT", dept_tuples).unwrap();
    db.create_relvar("EMP", employee_type()).unwrap();
    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    db.set_foreign_key_constraints("EMP", ForeignKeyConstraints::new().with_foreign_key(fk))
        .unwrap();
    db
}

fn bench_mem_fk_bulk(c: &mut Criterion) {
    let mut group = c.benchmark_group("v11_mem_fk_bulk");
    group.measurement_time(Duration::from_secs(2));
    group.sample_size(30);
    for count in [20i64, 100] {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter_batched(
                mem_db_with_fk,
                |mut db| {
                    let tuples: Vec<Tuple> = (0..count).map(make_fk_tuple).collect();
                    db.bulk_insert("EMP", tuples).unwrap();
                    black_box(db);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_single_inserts,
    bench_bulk_insert,
    bench_single_inserts_key,
    bench_bulk_insert_key,
    bench_single_inserts_check,
    bench_single_inserts_fk,
    bench_bulk_insert_fk,
    bench_mem_fk_bulk,
);
criterion_main!(benches);
