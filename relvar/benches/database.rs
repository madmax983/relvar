use criterion::{
    BatchSize, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use relvar::constraints::{ForeignKey, ForeignKeyConstraints, KeyConstraints, PrimaryKey};
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
                    .project(&["emp_id", "name", "salary"])
                    .unwrap();
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

// Multiple assignment benchmark (TTM RM Prescription 21).
//
// Assigns EMP and a DEPT relvar referencing each other (EMP.dept_id ->
// DEPT, DEPT.manager -> EMP) as one statement. Validation is O(total
// cardinality): one key pass per relvar plus one hash pass per foreign key.
fn create_cyclic_memory_database() -> relvar::Database<relvar::InMemoryEngine> {
    let mut db = create_keyed_memory_database();
    let dept_type = RelationType::new(
        TupleType::new()
            .with_attribute("dept_id", ScalarType::Int)
            .with_attribute("manager", ScalarType::Int),
    );
    db.create_relvar("DEPT", dept_type).unwrap();
    let pk = PrimaryKey::new(vec!["dept_id".to_string()]).unwrap();
    db.set_key_constraints("DEPT", KeyConstraints::new().with_primary_key(pk))
        .unwrap();
    let fk = |from: &str, to: &str, attr: &str| {
        ForeignKeyConstraints::new().with_foreign_key(
            ForeignKey::new(
                vec![from.to_string()],
                to.to_string(),
                vec![attr.to_string()],
            )
            .unwrap(),
        )
    };
    db.set_foreign_key_constraints("EMP", fk("dept_id", "DEPT", "dept_id"))
        .unwrap();
    db.set_foreign_key_constraints("DEPT", fk("manager", "EMP", "emp_id"))
        .unwrap();
    db
}

fn bench_multiple_assign(c: &mut Criterion) {
    let mut group = c.benchmark_group("multiple_assign");

    for count in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter_batched(
                || {
                    // Setup (not measured): the database and both new values.
                    let db = create_cyclic_memory_database();
                    let emp = relvar::Relation::from_tuples(
                        db.get_relvar_type("EMP").unwrap(),
                        (0..count).map(|i| make_employee_tuple(i as i64)),
                    )
                    .unwrap();
                    let dept = relvar::Relation::from_tuples(
                        db.get_relvar_type("DEPT").unwrap(),
                        (0..10i64).map(|d| tuple! { dept_id: d, manager: d }),
                    )
                    .unwrap();
                    (db, emp, dept)
                },
                |(mut db, emp, dept)| {
                    db.multiple_assign(vec![("EMP", emp), ("DEPT", dept)])
                        .unwrap();
                    // Returned, so criterion drops it outside the measurement.
                    black_box(db)
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// P{pid} with `parents` tuples and C{cid, pid} with one child per parent,
/// both keyed, optionally guarded by the declarative assertion
/// IS_EMPTY(C NOT MATCHING P) ("every child has a parent").
fn create_parent_child_memory_database(
    parents: i64,
    with_assertion: bool,
) -> relvar::Database<relvar::InMemoryEngine> {
    use relvar::constraints::{AssertionCondition, DeclarativeAssertion};
    let mut db = relvar::Database::new(relvar::InMemoryEngine::new());
    db.create_relvar(
        "P",
        RelationType::new(TupleType::new().with_attribute("pid", ScalarType::Int)),
    )
    .unwrap();
    db.create_relvar(
        "C",
        RelationType::new(
            TupleType::new()
                .with_attribute("cid", ScalarType::Int)
                .with_attribute("pid", ScalarType::Int),
        ),
    )
    .unwrap();
    let key = |attribute: &str| {
        KeyConstraints::new()
            .with_primary_key(PrimaryKey::new(vec![attribute.to_string()]).unwrap())
    };
    db.set_key_constraints("P", key("pid")).unwrap();
    db.set_key_constraints("C", key("cid")).unwrap();
    db.bulk_insert("P", (0..parents).map(|p| tuple! { pid: p }).collect())
        .unwrap();
    db.bulk_insert(
        "C",
        (0..parents).map(|p| tuple! { cid: p, pid: p }).collect(),
    )
    .unwrap();
    if with_assertion {
        db.add_declarative_assertion(DeclarativeAssertion::new(
            "child_has_parent",
            "Every child references an existing parent",
            AssertionCondition::IsEmpty(
                relvar::Query::scan("C").not_matching(relvar::Query::scan("P")),
            ),
        ))
        .unwrap();
    }
    db
}

/// Cost of a declarative assertion per statement: one valid child insert
/// with and without IS_EMPTY(C NOT MATCHING P). The assertion is evaluated
/// on the whole post-statement state, so its cost grows with the relvars.
fn bench_declarative_assertion_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("declarative_assertion_insert");

    for parents in [100i64, 1000, 10000] {
        for (label, with_assertion) in [("no_assertion", false), ("is_empty_not_matching", true)] {
            group.bench_with_input(BenchmarkId::new(label, parents), &parents, |b, &parents| {
                b.iter_batched(
                    // Setup (not measured): the populated database.
                    || create_parent_child_memory_database(parents, with_assertion),
                    |mut db| {
                        db.insert("C", tuple! { cid: parents, pid: 0i64 }).unwrap();
                        // Returned, so criterion drops it outside the measurement.
                        black_box(db)
                    },
                    BatchSize::SmallInput,
                );
            });
        }
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
    bench_multiple_assign,
    bench_declarative_assertion_insert,
    // Virtual relvar benchmarks (TTM RM Prescription 10)
    bench_virtual_relvar_creation,
    bench_virtual_relvar_query
);
criterion_main!(benches);
