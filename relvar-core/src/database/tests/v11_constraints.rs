// v0.11 regression tests: constraint-evaluation refactor.
// The insert path validates through prepared per-batch plans
// (`InsertContentPlan` / `InsertKeyPlan`); these tests pin the observable
// behavior the refactor must preserve: error identity and ordering,
// within-batch duplicate detection, and batch atomicity.

use super::common::*;
use crate::constraints::ConstraintManagerError;
use crate::constraints::expression::{CmpOp, ConstraintExpression, ValueOrRef};
use crate::constraints::{CheckConstraint, CheckConstraints, ForeignKey, ForeignKeyConstraints};
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::InMemoryEngine;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::ScalarValue;

fn child_fk() -> ForeignKeyConstraints {
    let fk = ForeignKey::new(
        vec!["parent_id".to_string()],
        "PARENT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    ForeignKeyConstraints::new().with_foreign_key(fk)
}

fn salary_check() -> CheckConstraints {
    CheckConstraints::new().with_constraint(CheckConstraint::new(
        "positive_salary",
        "Salary must be positive",
        ConstraintExpression::Cmp {
            left: "salary".to_string(),
            op: CmpOp::Gt,
            right: ValueOrRef::Value(ScalarValue::Int(0)),
        },
    ))
}

fn emp_with_check_db() -> Database<InMemoryEngine> {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("salary", ScalarType::Int),
    );
    db.create_relvar("EMPLOYEES", rel_type).unwrap();
    db.set_check_constraints("EMPLOYEES", salary_check())
        .unwrap();
    db
}

#[test]
fn v11_bulk_insert_fk_violation_rejects_whole_batch() {
    let mut db = setup_parent_child_db();
    db.insert("PARENT", tuple! { id: 10i64, name: "Engineering" })
        .unwrap();
    db.set_foreign_key_constraints("CHILD", child_fk()).unwrap();

    let result = db.bulk_insert(
        "CHILD",
        vec![
            tuple! { child_id: 1i64, parent_id: 10i64 },
            tuple! { child_id: 2i64, parent_id: 99i64 },
            tuple! { child_id: 3i64, parent_id: 10i64 },
        ],
    );
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
    // Batch atomicity: the valid tuples were not applied either.
    assert_eq!(db.query("CHILD").unwrap().cardinality(), 0);
}

#[test]
fn v11_bulk_insert_valid_fk_batch_succeeds() {
    let mut db = setup_parent_child_db();
    db.insert("PARENT", tuple! { id: 10i64, name: "Engineering" })
        .unwrap();
    db.insert("PARENT", tuple! { id: 20i64, name: "Sales" })
        .unwrap();
    db.set_foreign_key_constraints("CHILD", child_fk()).unwrap();

    db.bulk_insert(
        "CHILD",
        vec![
            tuple! { child_id: 1i64, parent_id: 10i64 },
            tuple! { child_id: 2i64, parent_id: 20i64 },
        ],
    )
    .unwrap();
    assert_eq!(db.query("CHILD").unwrap().cardinality(), 2);
}

#[test]
fn v11_bulk_insert_composite_fk_batch() {
    use crate::constraints::{KeyConstraints, PrimaryKey};

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar(
        "DEPT",
        RelationType::new(
            TupleType::new()
                .with_attribute("d1", ScalarType::Int)
                .with_attribute("d2", ScalarType::Int),
        ),
    )
    .unwrap();
    db.create_relvar(
        "EMP",
        RelationType::new(
            TupleType::new()
                .with_attribute("e1", ScalarType::Int)
                .with_attribute("f1", ScalarType::Int)
                .with_attribute("f2", ScalarType::Int),
        ),
    )
    .unwrap();
    let pk = PrimaryKey::new(vec!["d1".to_string(), "d2".to_string()]).unwrap();
    db.set_key_constraints("DEPT", KeyConstraints::new().with_primary_key(pk))
        .unwrap();
    db.insert("DEPT", tuple! { d1: 1i64, d2: 2i64 }).unwrap();

    let fk = ForeignKey::new(
        vec!["f1".to_string(), "f2".to_string()],
        "DEPT".to_string(),
        vec!["d1".to_string(), "d2".to_string()],
    )
    .unwrap();
    db.set_foreign_key_constraints("EMP", ForeignKeyConstraints::new().with_foreign_key(fk))
        .unwrap();

    db.insert("EMP", tuple! { e1: 1i64, f1: 1i64, f2: 2i64 })
        .unwrap();
    let result = db.insert("EMP", tuple! { e1: 2i64, f1: 1i64, f2: 99i64 });
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
}

#[test]
fn v11_bulk_insert_within_batch_duplicate_key_rejected() {
    use crate::constraints::{KeyConstraints, PrimaryKey};

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar(
        "T",
        RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int)),
    )
    .unwrap();
    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    db.set_key_constraints("T", KeyConstraints::new().with_primary_key(pk))
        .unwrap();

    let result = db.bulk_insert("T", vec![tuple! { id: 1i64 }, tuple! { id: 1i64 }]);
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
    assert_eq!(db.query("T").unwrap().cardinality(), 0);
}

#[test]
fn v11_bulk_insert_check_violation_rejects_whole_batch() {
    let mut db = emp_with_check_db();

    let result = db.bulk_insert(
        "EMPLOYEES",
        vec![
            tuple! { id: 1i64, salary: 100i64 },
            tuple! { id: 2i64, salary: -50i64 },
        ],
    );
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::CheckConstraintViolation(_)
        ))
    ));
    assert_eq!(db.query("EMPLOYEES").unwrap().cardinality(), 0);
}

#[test]
fn v11_single_insert_fk_violation() {
    let mut db = setup_parent_child_db();
    db.insert("PARENT", tuple! { id: 10i64, name: "Engineering" })
        .unwrap();
    db.set_foreign_key_constraints("CHILD", child_fk()).unwrap();

    let result = db.insert("CHILD", tuple! { child_id: 2i64, parent_id: 99i64 });
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
}

#[test]
fn v11_content_checks_run_before_key_checks_for_whole_batch() {
    use crate::constraints::{KeyConstraints, PrimaryKey};

    // CHILD has a primary key on child_id and an FK on parent_id.
    let mut db = setup_parent_child_db();
    let pk = PrimaryKey::new(vec!["child_id".to_string()]).unwrap();
    db.set_key_constraints("CHILD", KeyConstraints::new().with_primary_key(pk))
        .unwrap();
    db.insert("PARENT", tuple! { id: 10i64, name: "Engineering" })
        .unwrap();
    db.set_foreign_key_constraints("CHILD", child_fk()).unwrap();
    db.insert("CHILD", tuple! { child_id: 1i64, parent_id: 10i64 })
        .unwrap();

    // First tuple duplicates an existing key; second tuple violates the FK.
    // All content checks run before any key check, so the FK violation
    // surfaces even though the key violation comes first in tuple order.
    let result = db.bulk_insert(
        "CHILD",
        vec![
            tuple! { child_id: 1i64, parent_id: 10i64 },
            tuple! { child_id: 2i64, parent_id: 99i64 },
        ],
    );
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
    assert_eq!(db.query("CHILD").unwrap().cardinality(), 1);
}

#[test]
fn v11_type_constraint_violation_still_enforced() {
    use crate::constraints::AttributeConstraints;
    use crate::constraints::type_constraint::TypeConstraint;

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar(
        "T",
        RelationType::new(TupleType::new().with_attribute("age", ScalarType::Int)),
    )
    .unwrap();
    let attr_constraints = AttributeConstraints::new("age".to_string(), ScalarType::Int)
        .with_constraint(TypeConstraint::Range {
            min: ScalarValue::Int(1),
            max: ScalarValue::Int(i64::MAX),
        });
    db.set_type_constraints("T", "age", attr_constraints)
        .unwrap();

    db.insert("T", tuple! { age: 5i64 }).unwrap();
    let result = db.insert("T", tuple! { age: -5i64 });
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::TypeConstraintViolation(_)
        ))
    ));
    assert_eq!(db.query("T").unwrap().cardinality(), 1);
}
