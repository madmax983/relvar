use crate::constraints::{ForeignKey, ForeignKeyConstraints, KeyConstraints, PrimaryKey};
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::InMemoryEngine;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};

// Cover lines 153, 156-158 in database/data.rs (validate_referencing_foreign_keys during delete)
// Cover lines 260, 265-267 in database/data.rs (validate_referencing_foreign_keys during update)
#[test]
fn test_sentry_database_dml_fk_violations() {
    let mut db = Database::new(InMemoryEngine::new());

    // Parent
    db.create_relvar(
        "PARENT",
        RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int)),
    )
    .unwrap();
    db.set_key_constraints(
        "PARENT",
        KeyConstraints::new().with_primary_key(PrimaryKey::new(vec!["id".to_string()]).unwrap()),
    )
    .unwrap();

    // Child
    db.create_relvar(
        "CHILD",
        RelationType::new(
            TupleType::new()
                .with_attribute("child_id", ScalarType::Int)
                .with_attribute("parent_id", ScalarType::Int),
        ),
    )
    .unwrap();
    db.set_key_constraints(
        "CHILD",
        KeyConstraints::new()
            .with_primary_key(PrimaryKey::new(vec!["child_id".to_string()]).unwrap()),
    )
    .unwrap();

    let fk = ForeignKey::new(
        vec!["parent_id".to_string()],
        "PARENT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    db.set_foreign_key_constraints("CHILD", ForeignKeyConstraints::new().with_foreign_key(fk))
        .unwrap();

    db.insert("PARENT", tuple! { id: 1i64 }).unwrap();
    db.insert("CHILD", tuple! { child_id: 10i64, parent_id: 1i64 })
        .unwrap();

    // Deleting PARENT should fail because CHILD references it
    let res = db.delete("PARENT", |_| true);
    assert!(matches!(
        res,
        Err(DatabaseError::Constraint(
            crate::constraints::ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));

    // Updating PARENT's ID should fail because CHILD references the old ID
    let res = db.update("PARENT", |_| true, |_| tuple! { id: 2i64 });
    assert!(matches!(
        res,
        Err(DatabaseError::Constraint(
            crate::constraints::ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
}

// Cover compute_relation_after_update failure (TupleMismatch) lines 259-260
#[test]
fn test_sentry_database_update_tuple_mismatch() {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar(
        "TEST",
        RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int)),
    )
    .unwrap();
    db.insert("TEST", tuple! { id: 1i64 }).unwrap();

    let res = db.update("TEST", |_| true, |_| tuple! { id: "not_an_int" });
    assert!(matches!(res, Err(DatabaseError::TupleMismatch)));
}

// Cover validate_insert validation failures (lines 297-299, 305-307)
#[test]
fn test_sentry_database_validate_insert_errors() {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar(
        "TEST",
        RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int)),
    )
    .unwrap();
    db.set_key_constraints(
        "TEST",
        KeyConstraints::new().with_primary_key(PrimaryKey::new(vec!["id".to_string()]).unwrap()),
    )
    .unwrap();

    // Type mismatch during insert (TupleMismatch)
    let res = db.insert("TEST", tuple! { id: "string" });
    assert!(res.is_err());

    // Primary key violation during insert
    db.insert("TEST", tuple! { id: 1i64 }).unwrap();
    let res = db.insert("TEST", tuple! { id: 1i64 });
    assert!(matches!(
        res,
        Err(DatabaseError::Constraint(
            crate::constraints::ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
}

// Cover validate_relation_constraints_bulk_errors (lines 328-330)
#[test]
fn test_sentry_database_update_bulk_constraint_error() {
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar(
        "TEST",
        RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("val", ScalarType::Int),
        ),
    )
    .unwrap();
    db.set_key_constraints(
        "TEST",
        KeyConstraints::new().with_primary_key(PrimaryKey::new(vec!["id".to_string()]).unwrap()),
    )
    .unwrap();

    db.insert("TEST", tuple! { id: 1i64, val: 10i64 }).unwrap();
    db.insert("TEST", tuple! { id: 2i64, val: 20i64 }).unwrap();

    // Update id 2 to 1, causing PK violation on bulk validation since val is different, the tuples are distinct in the set but conflict on PK
    let res = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 2,
        |_| tuple! { id: 1i64, val: 20i64 },
    );
    assert!(matches!(
        res,
        Err(DatabaseError::Constraint(
            crate::constraints::ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
}
