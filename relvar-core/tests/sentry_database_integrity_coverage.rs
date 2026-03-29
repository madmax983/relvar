use relvar_core::constraints::{
    AttributeConstraints, CheckConstraints, ForeignKeyConstraints, KeyConstraints,
};
use relvar_core::database::Database;
use relvar_core::error::DatabaseError;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};

fn setup() -> Database<InMemoryEngine> {
    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("val", ScalarType::Int),
    );
    db.create_relvar("TEST", rel_type).unwrap();
    db.insert("TEST", tuple! { id: 1i64, val: 10i64 }).unwrap();
    db.insert("TEST", tuple! { id: 2i64, val: 20i64 }).unwrap();
    db
}

#[test]
fn test_database_set_key_constraints_fails() {
    let mut db = setup();

    db.insert("TEST", tuple! { id: 3i64, val: 10i64 }).unwrap();

    // Try to set primary key on val, which has duplicates (10i64)
    let pk = relvar_core::constraints::PrimaryKey::new(vec!["val".to_string()]).unwrap();
    let constraints = KeyConstraints::new().with_primary_key(pk);

    let result = db.set_key_constraints("TEST", constraints);
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_set_foreign_key_constraints_fails() {
    let mut db = setup();

    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("test_id", ScalarType::Int),
    );
    db.create_relvar("CHILD", child_type).unwrap();

    // Insert an orphan record
    db.insert("CHILD", tuple! { child_id: 100i64, test_id: 999i64 })
        .unwrap();

    let fk = relvar_core::constraints::ForeignKey::new(
        vec!["test_id".to_string()],
        "TEST".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let constraints = ForeignKeyConstraints::new().with_foreign_key(fk);

    let result = db.set_foreign_key_constraints("CHILD", constraints);
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_set_type_constraints_fails() {
    let mut db = setup();

    let type_cons = relvar_core::constraints::TypeConstraint::Range {
        min: relvar_core::values::ScalarValue::Int(0),
        max: relvar_core::values::ScalarValue::Int(15),
    };

    let constraints =
        AttributeConstraints::new("val".to_string(), ScalarType::Int).with_constraint(type_cons);

    // Should fail because one of the tuples has val=20
    let result = db.set_type_constraints("TEST", "val", constraints);
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_set_check_constraints_fails() {
    let mut db = setup();

    let check_expr = relvar_core::constraints::ConstraintExpression::Cmp {
        left: "val".to_string(),
        op: relvar_core::constraints::CmpOp::Lt,
        right: relvar_core::constraints::ValueOrRef::Value(relvar_core::values::ScalarValue::Int(
            15,
        )),
    };

    let constraints =
        CheckConstraints::new().with_constraint(relvar_core::constraints::CheckConstraint::new(
            "val_lt_15".to_string(),
            "must be less than 15".to_string(),
            check_expr,
        ));

    // Should fail because one tuple has val=20
    let result = db.set_check_constraints("TEST", constraints);
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_set_key_constraints_nonexistent_fails() {
    let mut db = setup();
    let constraints = KeyConstraints::new();
    let result = db.set_key_constraints("NONEXISTENT", constraints);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            relvar_core::constraints::ConstraintManagerError::RelationNotFound(_)
        ))
    ));
}

#[test]
fn test_database_set_foreign_key_constraints_nonexistent_fails() {
    let mut db = setup();
    let constraints = ForeignKeyConstraints::new();
    let result = db.set_foreign_key_constraints("NONEXISTENT", constraints);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            relvar_core::constraints::ConstraintManagerError::RelationNotFound(_)
        ))
    ));
}

#[test]
fn test_database_set_type_constraints_nonexistent_fails() {
    let mut db = setup();
    let constraints = AttributeConstraints::new("val".to_string(), ScalarType::Int);
    let result = db.set_type_constraints("NONEXISTENT", "val", constraints);
    assert!(result.is_err());
    // The type constraint internally triggers a fetch which eventually resolves through map_err returning just Constraint(_)
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_set_check_constraints_nonexistent_fails() {
    let mut db = setup();
    let constraints = CheckConstraints::new();
    let result = db.set_check_constraints("NONEXISTENT", constraints);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            relvar_core::constraints::ConstraintManagerError::RelationNotFound(_)
        ))
    ));
}
