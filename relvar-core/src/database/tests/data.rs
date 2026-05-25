use super::common::*;
use crate::database::Database;
use crate::storage_engine::InMemoryEngine;
use crate::tuple;

#[test]
fn test_delete() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("TEST", test_rel_type()).unwrap();
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

    let deleted = db
        .delete("TEST", |t| t.get_typed::<i64>("id").unwrap() == 1)
        .unwrap();
    assert_eq!(deleted, 1);

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_update() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("TEST", test_rel_type()).unwrap();
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    let updated = db
        .update(
            "TEST",
            |t| t.get_typed::<i64>("id").unwrap() == 1,
            |t| tuple! { id: t.get_typed::<i64>("id").unwrap(), name: "Alicia" },
        )
        .unwrap();
    assert_eq!(updated, 1);

    let result = db.query("TEST").unwrap();
    let tuple = result.tuples().next().unwrap();
    assert_eq!(tuple.get_typed::<String>("name").unwrap(), "Alicia");
}

#[test]
fn test_delete_returns_count() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();
    db.insert("TEST", tuple! { id: 3i64, name: "Charlie" })
        .unwrap();

    // Delete some tuples
    let count = db
        .delete("TEST", |t| t.get_typed::<i64>("id").unwrap() > 1)
        .unwrap();
    assert_eq!(count, 2);

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_update_returns_count() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

    // Update tuples
    let count = db
        .update(
            "TEST",
            |t| t.get_typed::<i64>("id").unwrap() == 1,
            |_| {
                tuple! { id: 1i64, name: "Alicia" }
            },
        )
        .unwrap();

    assert_eq!(count, 1);

    let result = db.query("TEST").unwrap();
    assert!(result.contains(&tuple! { id: 1i64, name: "Alicia" }));
}

#[test]
fn test_delete_no_matches_returns_zero() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Delete with no matches
    let count = db
        .delete("TEST", |t| t.get_typed::<i64>("id").unwrap() > 100)
        .unwrap();
    assert_eq!(count, 0);

    // Relation should be unchanged
    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_query_relation_not_found() {
    let db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let err = db.query("NONEXISTENT").unwrap_err();
    assert!(matches!(
        err,
        crate::error::DatabaseError::Storage(
            crate::storage_engine::StorageError::RelationNotFound(_)
        )
    ));
}

#[test]
fn test_delete_relation_not_found() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let err = db.delete("NONEXISTENT", |_| true).unwrap_err();
    assert!(matches!(
        err,
        crate::error::DatabaseError::Storage(
            crate::storage_engine::StorageError::RelationNotFound(_)
        )
    ));
}

#[test]
fn test_update_relation_not_found() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let err = db
        .update("NONEXISTENT", |_| true, |t| t.clone())
        .unwrap_err();
    assert!(matches!(
        err,
        crate::error::DatabaseError::Storage(
            crate::storage_engine::StorageError::RelationNotFound(_)
        )
    ));
}

#[test]
fn test_ensure_not_virtual_insert_update_delete() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("BASE", test_rel_type()).unwrap();
    db.insert("BASE", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    db.define_virtual_relvar("VIRTUAL", test_rel_type(), |db| db.query("BASE"))
        .unwrap();

    let err = db
        .insert("VIRTUAL", tuple! { id: 2i64, name: "Bob" })
        .unwrap_err();
    assert!(matches!(
        err,
        crate::error::DatabaseError::CannotModifyVirtualRelvar(_)
    ));

    let err = db.update("VIRTUAL", |_| true, |t| t.clone()).unwrap_err();
    assert!(matches!(
        err,
        crate::error::DatabaseError::CannotModifyVirtualRelvar(_)
    ));

    let err = db.delete("VIRTUAL", |_| true).unwrap_err();
    assert!(matches!(
        err,
        crate::error::DatabaseError::CannotModifyVirtualRelvar(_)
    ));
}

#[test]
fn test_update_tuple_mismatch_error() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    let err = db
        .update(
            "TEST",
            |t| t.get_typed::<i64>("id").unwrap() == 1,
            |_| {
                tuple! { id: 1i64, name: 42i64 }
            },
        )
        .unwrap_err();

    assert!(matches!(err, crate::error::DatabaseError::TupleMismatch));
}

#[test]
fn test_update_constraint_violation() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    let mut keys = crate::constraints::key::KeyConstraints::new();
    keys = keys.with_candidate_key(
        crate::constraints::key::CandidateKey::new(vec!["id".to_string()]).unwrap(),
    );
    db.set_key_constraints("TEST", keys).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

    let err = db
        .update(
            "TEST",
            |t| t.get_typed::<i64>("id").unwrap() == 2,
            |t| {
                let mut new_t = t.clone();
                new_t
                    .set("id".to_string(), crate::values::ScalarValue::Int(1))
                    .unwrap();
                new_t
            },
        )
        .unwrap_err();

    assert!(matches!(
        err,
        crate::error::DatabaseError::Constraint(
            crate::constraints::ConstraintManagerError::CandidateKeyViolation
        )
    ));
}

#[test]
fn test_delete_foreign_key_violation() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("PARENT", test_rel_type()).unwrap();
    let mut keys = crate::constraints::key::KeyConstraints::new();
    keys = keys.with_candidate_key(
        crate::constraints::key::CandidateKey::new(vec!["id".to_string()]).unwrap(),
    );
    db.set_key_constraints("PARENT", keys).unwrap();

    db.create_relvar("CHILD", test_rel_type()).unwrap();
    let fk = crate::constraints::foreign_key::ForeignKey::new(
        vec!["id".to_string()],
        "PARENT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let fks = crate::constraints::foreign_key::ForeignKeyConstraints::new().with_foreign_key(fk);
    db.set_foreign_key_constraints("CHILD", fks).unwrap();

    db.insert("PARENT", tuple! { id: 1i64, name: "Parent" })
        .unwrap();
    db.insert("CHILD", tuple! { id: 1i64, name: "Child" })
        .unwrap();

    let err = db
        .delete("PARENT", |t| t.get_typed::<i64>("id").unwrap() == 1)
        .unwrap_err();

    assert!(matches!(
        err,
        crate::error::DatabaseError::Constraint(
            crate::constraints::ConstraintManagerError::ForeignKeyViolation(_)
        )
    ));
}

#[test]
fn test_update_foreign_key_violation() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("PARENT", test_rel_type()).unwrap();
    let mut keys = crate::constraints::key::KeyConstraints::new();
    keys = keys.with_candidate_key(
        crate::constraints::key::CandidateKey::new(vec!["id".to_string()]).unwrap(),
    );
    db.set_key_constraints("PARENT", keys).unwrap();

    db.create_relvar("CHILD", test_rel_type()).unwrap();
    let fk = crate::constraints::foreign_key::ForeignKey::new(
        vec!["id".to_string()],
        "PARENT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let fks = crate::constraints::foreign_key::ForeignKeyConstraints::new().with_foreign_key(fk);
    db.set_foreign_key_constraints("CHILD", fks).unwrap();

    db.insert("PARENT", tuple! { id: 1i64, name: "Parent" })
        .unwrap();
    db.insert("CHILD", tuple! { id: 1i64, name: "Child" })
        .unwrap();

    let err = db
        .update(
            "PARENT",
            |t| t.get_typed::<i64>("id").unwrap() == 1,
            |t| {
                let mut new_t = t.clone();
                new_t
                    .set("id".to_string(), crate::values::ScalarValue::Int(2))
                    .unwrap();
                new_t
            },
        )
        .unwrap_err();

    assert!(matches!(
        err,
        crate::error::DatabaseError::Constraint(
            crate::constraints::ConstraintManagerError::ForeignKeyViolation(_)
        )
    ));
}

#[test]
fn test_insert_check_constraint_violation() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("TEST", test_rel_type()).unwrap();

    let mut check = crate::constraints::check::CheckConstraints::new();
    check = check.with_constraint(crate::constraints::check::CheckConstraint::new(
        "id_positive",
        "id must be positive",
        crate::constraints::ConstraintExpression::Cmp {
            left: "id".to_string(),
            op: crate::constraints::expression::CmpOp::Gt,
            right: crate::constraints::expression::ValueOrRef::Value(
                crate::values::ScalarValue::Int(0),
            ),
        },
    ));
    db.set_check_constraints("TEST", check).unwrap();

    let err = db
        .insert("TEST", tuple! { id: -1i64, name: "Alice" })
        .unwrap_err();
    assert!(matches!(
        err,
        crate::error::DatabaseError::Constraint(
            crate::constraints::ConstraintManagerError::CheckConstraintViolation(_)
        )
    ));
}

#[test]
fn test_update_check_constraint_violation() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("TEST", test_rel_type()).unwrap();

    let mut check = crate::constraints::check::CheckConstraints::new();
    check = check.with_constraint(crate::constraints::check::CheckConstraint::new(
        "id_positive",
        "id must be positive",
        crate::constraints::ConstraintExpression::Cmp {
            left: "id".to_string(),
            op: crate::constraints::expression::CmpOp::Gt,
            right: crate::constraints::expression::ValueOrRef::Value(
                crate::values::ScalarValue::Int(0),
            ),
        },
    ));
    db.set_check_constraints("TEST", check).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    let err = db
        .update(
            "TEST",
            |t| t.get_typed::<i64>("id").unwrap() == 1,
            |t| {
                let mut new_t = t.clone();
                new_t
                    .set("id".to_string(), crate::values::ScalarValue::Int(-1))
                    .unwrap();
                new_t
            },
        )
        .unwrap_err();

    assert!(matches!(
        err,
        crate::error::DatabaseError::Constraint(
            crate::constraints::ConstraintManagerError::CheckConstraintViolation(_)
        )
    ));
}

#[test]
fn test_insert_type_constraint_violation() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("TEST", test_rel_type()).unwrap();

    let mut tc = crate::constraints::type_constraint::AttributeConstraints::new(
        "id".to_string(),
        crate::types::ScalarType::Int,
    );
    tc = tc.with_constraint(crate::constraints::type_constraint::TypeConstraint::Range {
        min: crate::values::ScalarValue::Int(0),
        max: crate::values::ScalarValue::Int(100),
    });
    db.set_type_constraints("TEST", "id", tc).unwrap();

    let err = db
        .insert("TEST", tuple! { id: -1i64, name: "Alice" })
        .unwrap_err();
    assert!(matches!(
        err,
        crate::error::DatabaseError::Constraint(
            crate::constraints::ConstraintManagerError::TypeConstraintViolation(_)
        )
    ));
}

#[test]
fn test_update_type_constraint_violation() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("TEST", test_rel_type()).unwrap();

    let mut tc = crate::constraints::type_constraint::AttributeConstraints::new(
        "id".to_string(),
        crate::types::ScalarType::Int,
    );
    tc = tc.with_constraint(crate::constraints::type_constraint::TypeConstraint::Range {
        min: crate::values::ScalarValue::Int(0),
        max: crate::values::ScalarValue::Int(100),
    });
    db.set_type_constraints("TEST", "id", tc).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    let err = db
        .update(
            "TEST",
            |t| t.get_typed::<i64>("id").unwrap() == 1,
            |t| {
                let mut new_t = t.clone();
                new_t
                    .set("id".to_string(), crate::values::ScalarValue::Int(-1))
                    .unwrap();
                new_t
            },
        )
        .unwrap_err();

    assert!(matches!(
        err,
        crate::error::DatabaseError::Constraint(
            crate::constraints::ConstraintManagerError::TypeConstraintViolation(_)
        )
    ));
}
