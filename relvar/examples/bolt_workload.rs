//! Deterministic profiling workload for callgrind / valgrind-dhat.
//!
//! Mixed OLTP-ish session on the persistent engine: bulk load with PK +
//! CHECK + FK, single autocommit inserts, update, delete, full scans.
//! Run: `cargo build --release --example bolt_workload` then
//! `valgrind --tool=callgrind target/release/examples/bolt_workload`.

use relvar::constraints::{
    CheckConstraint, CheckConstraints, CmpOp, ConstraintExpression, ForeignKey,
    ForeignKeyConstraints, KeyConstraints, PrimaryKey, ValueOrRef,
};
use relvar::tuple;
use relvar::{RelationType, ScalarType, ScalarValue, Tuple, TupleType};

fn emp(i: i64) -> Tuple {
    tuple! {
        emp_id: i,
        name: format!("Employee_{}", i),
        dept_id: i % 100,
        salary: 50000.0 + (i as f64 * 100.0)
    }
}

fn main() {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);
    let dir = tempfile::tempdir().unwrap();
    let mut db = relvar::open(dir.path()).unwrap();
    let heading = TupleType::new()
        .with_attribute("emp_id".to_string(), ScalarType::Int)
        .with_attribute("name".to_string(), ScalarType::String)
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("salary".to_string(), ScalarType::Float);
    db.create_relvar("EMP", RelationType::new(heading)).unwrap();
    let dh = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
    db.create_relvar("DEPT", RelationType::new(dh)).unwrap();
    db.bulk_insert("DEPT", (0..100).map(|i| tuple! { id: i as i64 }).collect())
        .unwrap();
    // A foreign key references a declared key of DEPT.
    let dept_pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    db.set_key_constraints("DEPT", KeyConstraints::new().with_primary_key(dept_pk))
        .unwrap();
    let pk = PrimaryKey::new(vec!["emp_id".to_string()]).unwrap();
    db.set_key_constraints("EMP", KeyConstraints::new().with_primary_key(pk))
        .unwrap();
    db.set_check_constraints(
        "EMP",
        CheckConstraints::new().with_constraint(CheckConstraint::new(
            "salary_positive",
            "Salary must be positive",
            ConstraintExpression::Cmp {
                left: "salary".to_string(),
                op: CmpOp::Gt,
                right: ValueOrRef::Value(ScalarValue::Float(0.0)),
            },
        )),
    )
    .unwrap();
    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    db.set_foreign_key_constraints("EMP", ForeignKeyConstraints::new().with_foreign_key(fk))
        .unwrap();

    // phase 1: bulk load
    db.bulk_insert("EMP", (0..n).map(emp).collect()).unwrap();
    // phase 2: single autocommit inserts
    for i in n..n + n / 10 {
        db.insert("EMP", emp(i)).unwrap();
    }
    // phase 3: scans
    for _ in 0..5 {
        std::hint::black_box(db.query("EMP").unwrap());
    }
    // phase 4: update + delete
    db.update(
        "EMP",
        |t| t.get_typed::<i64>("dept_id").unwrap() == 5,
        |t| {
            tuple! {
                emp_id: t.get_typed::<i64>("emp_id").unwrap(),
                name: t.get_typed::<String>("name").unwrap(),
                dept_id: t.get_typed::<i64>("dept_id").unwrap(),
                salary: 60000.0f64
            }
        },
    )
    .unwrap();
    db.delete("EMP", |t| t.get_typed::<i64>("emp_id").unwrap() < n / 20)
        .unwrap();
    std::hint::black_box(db.query("EMP").unwrap());
}
