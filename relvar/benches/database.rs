use criterion::{
    BatchSize, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use relvar::constraints::{KeyConstraints, PrimaryKey};
use relvar::tuple;
use relvar::{PersistentEngine, RelationType, ScalarType, TupleType};
use tempfile::TempDir;

fn create_employee_type() -> RelationType {
    let heading = TupleType::new()
        .with_attribute("emp_id".to_string(), ScalarType::Int)
        .with_attribute("name".to_string(), ScalarType::String)
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("salary".to_string(), ScalarType::Float);
    RelationType::new(heading)
}

fn create_database_with_relvar() -> (TempDir, relvar::Database<relvar::PersistentEngine>) {
    let temp_dir = TempDir::new().unwrap();
    let mut db = relvar::open(temp_dir.path()).unwrap();
    let emp_type = create_employee_type();
    db.create_relvar("EMP", emp_type).unwrap();
    (temp_dir, db)
}

// Database creation benchmark
fn bench_database_open(c: &mut Criterion) {
    let mut group = c.benchmark_group("database_open");

    group.bench_function("open_new", |b| {
        b.iter(|| {
            let temp_dir = TempDir::new().unwrap();
            let db = relvar::open(temp_dir.path()).unwrap();
            black_box(db);
        });
    });

    group.bench_function("open_existing", |b| {
        let temp_dir = TempDir::new().unwrap();
        {
            let _db = relvar::open(temp_dir.path()).unwrap();
        }

        b.iter(|| {
            let db = relvar::open(temp_dir.path()).unwrap();
            black_box(db);
        });
    });

    group.finish();
}

// Relvar creation benchmark
fn bench_create_relvar(c: &mut Criterion) {
    let mut group = c.benchmark_group("create_relvar");

    group.bench_function("create", |b| {
        b.iter(|| {
            let temp_dir = TempDir::new().unwrap();
            let mut db = relvar::open(temp_dir.path()).unwrap();
            let emp_type = create_employee_type();
            db.create_relvar("EMP", emp_type).unwrap();
            black_box(db);
        });
    });

    group.finish();
}

// Insert benchmark
fn bench_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert");

    for count in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter_batched(
                || {
                    // Setup: create database and relvar
                    create_database_with_relvar()
                },
                |(_temp_dir, mut db)| {
                    // Measured: just the insert operations
                    for i in 0..count {
                        let tuple = tuple! {
                            emp_id: i as i64,
                            name: format!("Employee_{}", i),
                            dept_id: (i % 10) as i64,
                            salary: 50000.0 + (i as f64 * 100.0)
                        };
                        db.insert("EMP", tuple).unwrap();
                    }
                    black_box(db);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

// Insert with key constraint benchmark
fn bench_insert_with_key_constraint(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_with_key_constraint");

    for count in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter_batched(
                || {
                    // Setup: create database, relvar, and add constraint
                    let (_temp_dir, mut db) = create_database_with_relvar();
                    let pk = PrimaryKey::new(vec!["emp_id".to_string()]).unwrap();
                    db.set_key_constraints("EMP", KeyConstraints::new().with_primary_key(pk))
                        .unwrap();
                    (_temp_dir, db)
                },
                |(_temp_dir, mut db)| {
                    // Measured: just the inserts with constraint checking
                    for i in 0..count {
                        let tuple = tuple! {
                            emp_id: i as i64,
                            name: format!("Employee_{}", i),
                            dept_id: (i % 10) as i64,
                            salary: 50000.0 + (i as f64 * 100.0)
                        };
                        db.insert("EMP", tuple).unwrap();
                    }
                    black_box(db);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

// In-memory keyed database setup for the index-scaling benchmarks (#23).
//
// Uses InMemoryEngine to isolate the constraint/index path from storage
// I/O: the measured cost is key validation, not the WAL.
fn create_keyed_memory_database() -> relvar::Database<relvar::InMemoryEngine> {
    let mut db = relvar::Database::new(relvar::InMemoryEngine::new());
    db.create_relvar("EMP", create_employee_type()).unwrap();
    let pk = PrimaryKey::new(vec!["emp_id".to_string()]).unwrap();
    db.set_key_constraints("EMP", KeyConstraints::new().with_primary_key(pk))
        .unwrap();
    db
}

fn make_employee_tuple(i: i64) -> relvar::Tuple {
    tuple! {
        emp_id: i,
        name: format!("Employee_{}", i),
        dept_id: i % 10,
        salary: 50000.0 + (i as f64 * 100.0)
    }
}

// Keyed single-insert scaling benchmark (#23).
//
// The key index keeps per-insert validation O(1), so total time should
// scale linearly with the insert count even as the relation grows --
// the O(n^2) full-relation scan per insert is gone.
fn bench_keyed_insert_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("keyed_insert_scaling");

    for count in [100, 500, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter_batched(
                || {
                    // Setup: fresh keyed database (not measured)
                    create_keyed_memory_database()
                },
                |mut db| {
                    // Measured: single keyed inserts into a growing relation
                    for i in 0..count {
                        db.insert("EMP", make_employee_tuple(i as i64)).unwrap();
                    }
                    black_box(db);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

// Bulk insert scaling benchmark (#40).
//
// One bulk_insert call per batch: validation is O(batch size) against the
// key index, so time should scale linearly with the batch size.
fn bench_bulk_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulk_insert");

    for count in [100, 500, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter_batched(
                || {
                    // Setup: fresh keyed database and the batch (not measured)
                    let db = create_keyed_memory_database();
                    let batch: Vec<relvar::Tuple> =
                        (0..count).map(|i| make_employee_tuple(i as i64)).collect();
                    (db, batch)
                },
                |(mut db, batch)| {
                    // Measured: a single bulk_insert call
                    db.bulk_insert("EMP", batch).unwrap();
                    black_box(db);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

// Bulk insert against pre-populated relations (#23, #40).
//
// A fixed-size batch goes into relations of varying existing size. The key
// index means validation never loads the relation, so the batch cost
// should stay flat as existing data grows.
fn bench_bulk_insert_with_existing_data(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulk_insert_with_existing_data");
    const BATCH: i64 = 500;

    for existing in [0, 500, 2000, 5000].iter() {
        group.throughput(Throughput::Elements(BATCH as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(existing),
            existing,
            |b, &existing| {
                b.iter_batched(
                    || {
                        // Setup: keyed database pre-populated with `existing`
                        // tuples, plus a fresh 500-tuple batch (not measured)
                        let mut db = create_keyed_memory_database();
                        if existing > 0 {
                            let seed: Vec<relvar::Tuple> =
                                (0..existing).map(make_employee_tuple).collect();
                            db.bulk_insert("EMP", seed).unwrap();
                        }
                        let batch: Vec<relvar::Tuple> = (existing..existing + BATCH)
                            .map(make_employee_tuple)
                            .collect();
                        (db, batch)
                    },
                    |(mut db, batch)| {
                        // Measured: bulk_insert into the populated relation
                        db.bulk_insert("EMP", batch).unwrap();
                        black_box(db);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

// Query benchmark
fn bench_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("query");

    for count in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            let (_temp_dir, mut db) = create_database_with_relvar();

            // Pre-populate
            for i in 0..count {
                let tuple = tuple! {
                    emp_id: i as i64,
                    name: format!("Employee_{}", i),
                    dept_id: (i % 10) as i64,
                    salary: 50000.0 + (i as f64 * 100.0)
                };
                db.insert("EMP", tuple).unwrap();
            }

            b.iter(|| {
                let result = db.query("EMP").unwrap();
                black_box(result);
            });
        });
    }
    group.finish();
}

// Query with operations benchmark
fn bench_query_with_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_with_operations");

    for count in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            let (_temp_dir, mut db) = create_database_with_relvar();

            // Pre-populate
            for i in 0..count {
                let tuple = tuple! {
                    emp_id: i as i64,
                    name: format!("Employee_{}", i),
                    dept_id: (i % 10) as i64,
                    salary: 50000.0 + (i as f64 * 100.0)
                };
                db.insert("EMP", tuple).unwrap();
            }

            b.iter(|| {
                let result = db
                    .query("EMP")
                    .unwrap()
                    .restrict(|t| t.get_typed::<i64>("dept_id").unwrap() == 5)
                    .project(&["emp_id", "name", "salary"]);
                black_box(result);
            });
        });
    }
    group.finish();
}

// Delete benchmark
fn bench_delete(c: &mut Criterion) {
    let mut group = c.benchmark_group("delete");

    for count in [10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter_batched(
                || {
                    // Setup: create database and pre-populate
                    let (_temp_dir, mut db) = create_database_with_relvar();
                    for i in 0..count * 2 {
                        let tuple = tuple! {
                            emp_id: i as i64,
                            name: format!("Employee_{}", i),
                            dept_id: (i % 10) as i64,
                            salary: 50000.0 + (i as f64 * 100.0)
                        };
                        db.insert("EMP", tuple).unwrap();
                    }
                    (_temp_dir, db)
                },
                |(_temp_dir, mut db)| {
                    // Measured: just the delete operation
                    db.delete("EMP", |t| {
                        t.get_typed::<i64>("emp_id").unwrap() < count as i64
                    })
                    .unwrap();
                    black_box(db);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

// Update benchmark
fn bench_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("update");

    for count in [10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter_batched(
                || {
                    // Setup: create database and pre-populate
                    let (_temp_dir, mut db) = create_database_with_relvar();
                    for i in 0..count {
                        let tuple = tuple! {
                            emp_id: i as i64,
                            name: format!("Employee_{}", i),
                            dept_id: (i % 10) as i64,
                            salary: 50000.0 + (i as f64 * 100.0)
                        };
                        db.insert("EMP", tuple).unwrap();
                    }
                    (_temp_dir, db)
                },
                |(_temp_dir, mut db)| {
                    // Measured: just the update operation
                    db.update(
                        "EMP",
                        |_| true,
                        |t| {
                            let current_salary = t.get_typed::<f64>("salary").unwrap();
                            relvar::tuple! {
                                emp_id: t.get_typed::<i64>("emp_id").unwrap(),
                                name: t.get_typed::<String>("name").unwrap(),
                                dept_id: t.get_typed::<i64>("dept_id").unwrap(),
                                salary: current_salary * 1.1
                            }
                        },
                    )
                    .unwrap();
                    black_box(db);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

// Transaction benchmark
fn bench_transaction(c: &mut Criterion) {
    let mut group = c.benchmark_group("transaction");

    group.bench_function("commit", |b| {
        b.iter(|| {
            let (_temp_dir, mut db) = create_database_with_relvar();

            db.begin().unwrap();
            for i in 0..10 {
                let tuple = tuple! {
                    emp_id: i as i64,
                    name: format!("Employee_{}", i),
                    dept_id: (i % 10) as i64,
                    salary: 50000.0
                };
                db.insert("EMP", tuple).unwrap();
            }
            db.commit().unwrap();
            black_box(db);
        });
    });

    group.bench_function("rollback", |b| {
        b.iter(|| {
            let (_temp_dir, mut db) = create_database_with_relvar();

            // Insert some data first
            for i in 0..10 {
                let tuple = tuple! {
                    emp_id: i as i64,
                    name: format!("Employee_{}", i),
                    dept_id: (i % 10) as i64,
                    salary: 50000.0
                };
                db.insert("EMP", tuple).unwrap();
            }

            db.begin().unwrap();
            for i in 10..20 {
                let tuple = tuple! {
                    emp_id: i as i64,
                    name: format!("Employee_{}", i),
                    dept_id: (i % 10) as i64,
                    salary: 50000.0
                };
                db.insert("EMP", tuple).unwrap();
            }
            db.rollback().unwrap();
            black_box(db);
        });
    });

    group.finish();
}

// End-to-end realistic workload
fn bench_realistic_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_workload");

    group.bench_function("mixed_operations", |b| {
        b.iter_batched(
            || {
                // Setup: create database with relvar
                create_database_with_relvar()
            },
            |(_temp_dir, mut db)| {
                // Measured: complete realistic workload
                // Insert 100 records
                for i in 0..100 {
                    let tuple = tuple! {
                        emp_id: i as i64,
                        name: format!("Employee_{}", i),
                        dept_id: (i % 10) as i64,
                        salary: 50000.0 + (i as f64 * 100.0)
                    };
                    db.insert("EMP", tuple).unwrap();
                }

                // Query 5 times
                for _ in 0..5 {
                    let _result = db
                        .query("EMP")
                        .unwrap()
                        .restrict(|t| t.get_typed::<f64>("salary").unwrap() > 55000.0);
                }

                // Update 10 records
                db.update(
                    "EMP",
                    |t| t.get_typed::<i64>("dept_id").unwrap() == 5,
                    |t| {
                        relvar::tuple! {
                            emp_id: t.get_typed::<i64>("emp_id").unwrap(),
                            name: t.get_typed::<String>("name").unwrap(),
                            dept_id: t.get_typed::<i64>("dept_id").unwrap(),
                            salary: 60000.0f64
                        }
                    },
                )
                .unwrap();

                // Delete 5 records
                db.delete("EMP", |t| t.get_typed::<i64>("emp_id").unwrap() < 5)
                    .unwrap();

                black_box(db);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

// =============================================================================
// Virtual Relvar Benchmarks (TTM RM Prescription 10)
// =============================================================================

/// Helper to create a database with an EMP relvar pre-populated with data
fn create_populated_database(
    count: usize,
) -> (TempDir, relvar::Database<relvar::PersistentEngine>) {
    let (_temp_dir, mut db) = create_database_with_relvar();
    for i in 0..count {
        let tuple = tuple! {
            emp_id: i as i64,
            name: format!("Employee_{}", i),
            dept_id: (i % 10) as i64,
            salary: 50000.0 + (i as f64 * 1000.0)
        };
        db.insert("EMP", tuple).unwrap();
    }
    (_temp_dir, db)
}

/// Benchmark virtual relvar creation
fn bench_virtual_relvar_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("virtual_relvar_creation");

    group.bench_function("create_simple_virtual_relvar", |b| {
        b.iter_batched(
            || create_populated_database(100),
            |(_temp_dir, mut db)| {
                let emp_type = create_employee_type();

                fn evaluator(
                    db: &relvar::Database<PersistentEngine>,
                ) -> Result<relvar::Relation, relvar::DatabaseError> {
                    Ok(db
                        .query("EMP")?
                        .restrict(|t| t.get_typed::<f64>("salary").unwrap() > 60000.0))
                }

                db.define_virtual_relvar("HIGH_EARNERS", emp_type, evaluator)
                    .unwrap();
                black_box(db);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark virtual relvar query (re-evaluation)
fn bench_virtual_relvar_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("virtual_relvar_query");

    for count in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(
            BenchmarkId::new("restrict_virtual_relvar", count),
            count,
            |b, &count| {
                b.iter_batched(
                    || {
                        // Setup: create database with data and virtual relvar
                        let (_temp_dir, mut db) = create_populated_database(count);

                        // Define virtual relvar type (same as EMP)
                        let emp_type = create_employee_type();

                        fn high_earners_evaluator(
                            db: &relvar::Database<PersistentEngine>,
                        ) -> Result<relvar::Relation, relvar::DatabaseError>
                        {
                            Ok(db
                                .query("EMP")?
                                .restrict(|t| t.get_typed::<f64>("salary").unwrap() > 60000.0))
                        }

                        db.define_virtual_relvar("HIGH_EARNERS", emp_type, high_earners_evaluator)
                            .unwrap();
                        (_temp_dir, db)
                    },
                    |(_temp_dir, db)| {
                        // Measured: just the virtual relvar query
                        let result = db.query("HIGH_EARNERS").unwrap();
                        black_box(result);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_database_open,
    bench_create_relvar,
    bench_insert,
    bench_insert_with_key_constraint,
    bench_keyed_insert_scaling,
    bench_bulk_insert,
    bench_bulk_insert_with_existing_data,
    bench_query,
    bench_query_with_operations,
    bench_delete,
    bench_update,
    bench_transaction,
    bench_realistic_workload,
    // Virtual relvar benchmarks (TTM RM Prescription 10)
    bench_virtual_relvar_creation,
    bench_virtual_relvar_query
);
criterion_main!(benches);
