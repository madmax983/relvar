use relvar_core::constraints::{
    AttributeConstraints, CheckConstraint, CheckConstraints, CmpOp, ConstraintExpression,
    ForeignKeyConstraints, TypeConstraint, ValueOrRef,
};
use relvar_core::database::Database;
use relvar_core::error::DatabaseError;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::ScalarValue;

fn setup() -> Database<InMemoryEngine> {
    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("val", ScalarType::Int),
    );
    db.create_relvar("TEST", rel_type).unwrap();
    db.insert("TEST", tuple! { id: 1i64, val: 10i64 }).unwrap();
    db
}

#[test]
fn test_database_set_key_constraints_fails() {
    let mut db = setup();
    let pk = relvar_core::constraints::PrimaryKey::new(vec!["id".to_string()]).unwrap();
    let key_constraints = relvar_core::constraints::KeyConstraints::new().with_primary_key(pk);

    // Duplicate 1i64 to make it fail
    db.insert("TEST", tuple! { id: 1i64, val: 20i64 }).unwrap();

    let result = db.set_key_constraints("TEST", key_constraints);
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

    db.insert("CHILD", tuple! { child_id: 100i64, test_id: 999i64 })
        .unwrap();

    let fk = relvar_core::constraints::ForeignKey::new(
        vec!["test_id".to_string()],
        "TEST".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);

    let result = db.set_foreign_key_constraints("CHILD", fk_constraints);
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_set_type_constraints_fails() {
    let mut db = setup();

    let type_cons = TypeConstraint::Range {
        min: ScalarValue::Int(100), // value 10 is in DB, so this fails
        max: ScalarValue::Int(200),
    };

    let result = db.set_type_constraints(
        "TEST",
        "val",
        AttributeConstraints::new("val".to_string(), ScalarType::Int).with_constraint(type_cons),
    );

    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_set_check_constraints_fails() {
    let mut db = setup();

    let check_expr = ConstraintExpression::Cmp {
        left: "val".to_string(),
        op: CmpOp::Gt,
        right: ValueOrRef::Value(ScalarValue::Int(100)), // 10 is in DB
    };
    let checks = CheckConstraints::new().with_constraint(CheckConstraint::new(
        "val_large",
        "must be > 100",
        check_expr,
    ));

    let result = db.set_check_constraints("TEST", checks);
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_set_key_constraints_relation_not_found() {
    let mut db = setup();
    let pk = relvar_core::constraints::PrimaryKey::new(vec!["id".to_string()]).unwrap();
    let key_constraints = relvar_core::constraints::KeyConstraints::new().with_primary_key(pk);

    let result = db.set_key_constraints("NOT_FOUND", key_constraints);
    assert!(result.is_err());
}

#[test]
fn test_database_set_foreign_key_constraints_relation_not_found() {
    let mut db = setup();
    let fk = relvar_core::constraints::ForeignKey::new(
        vec!["test_id".to_string()],
        "TEST".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);

    let result = db.set_foreign_key_constraints("NOT_FOUND", fk_constraints);
    assert!(result.is_err());
}

#[test]
fn test_database_set_type_constraints_relation_not_found() {
    let mut db = setup();
    let type_cons = TypeConstraint::Range {
        min: ScalarValue::Int(100),
        max: ScalarValue::Int(200),
    };

    let result = db.set_type_constraints(
        "NOT_FOUND",
        "val",
        AttributeConstraints::new("val".to_string(), ScalarType::Int).with_constraint(type_cons),
    );

    assert!(result.is_err());
}

#[test]
fn test_database_set_check_constraints_relation_not_found() {
    let mut db = setup();
    let check_expr = ConstraintExpression::Cmp {
        left: "val".to_string(),
        op: CmpOp::Gt,
        right: ValueOrRef::Value(ScalarValue::Int(100)),
    };
    let checks = CheckConstraints::new().with_constraint(CheckConstraint::new(
        "val_large",
        "must be > 100",
        check_expr,
    ));

    let result = db.set_check_constraints("NOT_FOUND", checks);
    assert!(result.is_err());
}

#[test]
fn test_database_set_key_constraints_success() {
    let mut db = setup();
    let pk = relvar_core::constraints::PrimaryKey::new(vec!["id".to_string()]).unwrap();
    let key_constraints = relvar_core::constraints::KeyConstraints::new().with_primary_key(pk);

    let result = db.set_key_constraints("TEST", key_constraints);
    assert!(result.is_ok());
}

#[test]
fn test_database_set_foreign_key_constraints_success() {
    let mut db = setup();

    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("test_id", ScalarType::Int),
    );
    db.create_relvar("CHILD", child_type).unwrap();

    let fk = relvar_core::constraints::ForeignKey::new(
        vec!["test_id".to_string()],
        "TEST".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);

    let result = db.set_foreign_key_constraints("CHILD", fk_constraints);
    assert!(result.is_ok());
}

#[test]
fn test_database_set_type_constraints_success() {
    let mut db = setup();

    let type_cons = TypeConstraint::Range {
        min: ScalarValue::Int(0),
        max: ScalarValue::Int(200),
    };

    let result = db.set_type_constraints(
        "TEST",
        "val",
        AttributeConstraints::new("val".to_string(), ScalarType::Int).with_constraint(type_cons),
    );

    assert!(result.is_ok());
}

#[test]
fn test_database_set_check_constraints_success() {
    let mut db = setup();

    let check_expr = ConstraintExpression::Cmp {
        left: "val".to_string(),
        op: CmpOp::Gt,
        right: ValueOrRef::Value(ScalarValue::Int(0)),
    };
    let checks = CheckConstraints::new().with_constraint(CheckConstraint::new(
        "val_large",
        "must be > 0",
        check_expr,
    ));

    let result = db.set_check_constraints("TEST", checks);
    assert!(result.is_ok());
}

#[test]
fn test_database_get_constraints() {
    let mut db = setup();

    // Check getting constraints when they don't exist
    assert!(db.get_key_constraints("TEST").is_none());
    assert!(db.get_foreign_key_constraints("TEST").is_none());

    // Set constraints and get them
    let pk = relvar_core::constraints::PrimaryKey::new(vec!["id".to_string()]).unwrap();
    let key_constraints = relvar_core::constraints::KeyConstraints::new().with_primary_key(pk);
    db.set_key_constraints("TEST", key_constraints).unwrap();
    assert!(db.get_key_constraints("TEST").is_some());
}
