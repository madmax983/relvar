use relvar_core::constraints::ConstraintManagerError;
use relvar_core::database::Database;
use relvar_core::error::DatabaseError;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::ScalarValue;

#[test]
fn test_delete_foreign_key_violation() {
    let mut db = Database::new(InMemoryEngine::new());

    let user_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    let post_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("user_id", ScalarType::Int),
    );

    db.create_relvar("USERS", user_type).unwrap();
    db.create_relvar("POSTS", post_type).unwrap();

    db.set_key_constraints(
        "USERS",
        relvar_core::constraints::KeyConstraints::new().with_primary_key(
            relvar_core::constraints::PrimaryKey::new(vec!["id".to_string()]).unwrap(),
        ),
    )
    .unwrap();
    db.set_foreign_key_constraints(
        "POSTS",
        relvar_core::constraints::ForeignKeyConstraints::new().with_foreign_key(
            relvar_core::constraints::ForeignKey::new(
                vec!["user_id".to_string()],
                "USERS".to_string(),
                vec!["id".to_string()],
            )
            .unwrap(),
        ),
    )
    .unwrap();

    db.insert("USERS", tuple! { id: 1i64 }).unwrap();
    db.insert("POSTS", tuple! { id: 100i64, user_id: 1i64 })
        .unwrap();

    let result = db.delete("USERS", |t| t.get_typed::<i64>("id").unwrap() == 1);

    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
}

#[test]
fn test_insert_key_constraint_violation() {
    let mut db = Database::new(InMemoryEngine::new());

    let user_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));

    db.create_relvar("USERS", user_type).unwrap();
    db.set_key_constraints(
        "USERS",
        relvar_core::constraints::KeyConstraints::new().with_primary_key(
            relvar_core::constraints::PrimaryKey::new(vec!["id".to_string()]).unwrap(),
        ),
    )
    .unwrap();

    db.insert("USERS", tuple! { id: 1i64 }).unwrap();

    let result = db.insert("USERS", tuple! { id: 1i64 });

    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
}

#[test]
fn test_insert_tuple_type_violation() {
    let mut db = Database::new(InMemoryEngine::new());

    let user_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));

    db.create_relvar("USERS", user_type).unwrap();

    // Type constraint violation: insert a string instead of int
    let tuple_wrong = tuple! { id: "wrong_type".to_string() };

    let result = db.insert("USERS", tuple_wrong);

    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::TupleMismatch
        ))
    ));
}

#[test]
fn test_insert_check_constraint_violation() {
    let mut db = Database::new(InMemoryEngine::new());

    let user_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("age", ScalarType::Int),
    );

    db.create_relvar("USERS", user_type).unwrap();

    let check_expr = relvar_core::constraints::ConstraintExpression::Cmp {
        left: "age".to_string(),
        op: relvar_core::constraints::CmpOp::Gt,
        right: relvar_core::constraints::ValueOrRef::Value(ScalarValue::Int(17)),
    };
    let constraint = relvar_core::constraints::CheckConstraint::new(
        "age_check".to_string(),
        "age >= 18".to_string(),
        check_expr,
    );
    db.set_check_constraints(
        "USERS",
        relvar_core::constraints::CheckConstraints::new().with_constraint(constraint),
    )
    .unwrap();

    let result = db.insert("USERS", tuple! { id: 1i64, age: 15i64 });

    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::CheckConstraintViolation(_)
        ))
    ));
}

#[test]
fn test_update_check_constraint_violation() {
    let mut db = Database::new(InMemoryEngine::new());

    let user_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("age", ScalarType::Int),
    );

    db.create_relvar("USERS", user_type).unwrap();

    let check_expr = relvar_core::constraints::ConstraintExpression::Cmp {
        left: "age".to_string(),
        op: relvar_core::constraints::CmpOp::Gt,
        right: relvar_core::constraints::ValueOrRef::Value(ScalarValue::Int(17)),
    };
    let constraint = relvar_core::constraints::CheckConstraint::new(
        "age_check".to_string(),
        "age >= 18".to_string(),
        check_expr,
    );
    db.set_check_constraints(
        "USERS",
        relvar_core::constraints::CheckConstraints::new().with_constraint(constraint),
    )
    .unwrap();

    db.insert("USERS", tuple! { id: 1i64, age: 20i64 }).unwrap();

    let result = db.update(
        "USERS",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |_t| tuple! { id: 1i64, age: 15i64 },
    );

    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::CheckConstraintViolation(_)
        ))
    ));
}
