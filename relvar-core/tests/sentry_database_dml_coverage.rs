use relvar_core::database::Database;
use relvar_core::error::DatabaseError;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};

#[test]
fn test_database_update_tuple_mismatch() {
    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("val", ScalarType::Int),
    );
    db.create_relvar("TEST", rel_type).unwrap();
    db.insert("TEST", tuple! { id: 1i64, val: 10i64 }).unwrap();

    // Update should fail due to returning a tuple of wrong type
    let result = db.update(
        "TEST",
        |t: &relvar_core::values::Tuple| t.get_typed::<i64>("id").unwrap() == 1,
        |_t: &relvar_core::values::Tuple| tuple! { id: 1i64, wrong_attr: 10i64 }, // Invalid tuple type
    );

    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::TupleMismatch)));
}

#[test]
fn test_database_delete_foreign_key_violation() {
    let mut db = Database::new(InMemoryEngine::new());

    // Create PARENT
    let parent_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    );
    db.create_relvar("PARENT", parent_type).unwrap();
    let pk = relvar_core::constraints::PrimaryKey::new(vec!["id".to_string()]).unwrap();
    db.set_key_constraints(
        "PARENT",
        relvar_core::constraints::KeyConstraints::new().with_primary_key(pk),
    )
    .unwrap();

    // Create CHILD
    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("parent_id", ScalarType::Int),
    );
    db.create_relvar("CHILD", child_type).unwrap();
    let fk = relvar_core::constraints::ForeignKey::new(
        vec!["parent_id".to_string()],
        "PARENT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    db.set_foreign_key_constraints(
        "CHILD",
        relvar_core::constraints::ForeignKeyConstraints::new().with_foreign_key(fk),
    )
    .unwrap();

    // Insert tuples
    db.insert("PARENT", tuple! { id: 1i64, name: "P1" })
        .unwrap();
    db.insert("CHILD", tuple! { child_id: 100i64, parent_id: 1i64 })
        .unwrap();

    // Attempt delete that violates FK constraint
    let err = db.delete("PARENT", |t| t.get_typed::<i64>("id").unwrap() == 1);

    assert!(err.is_err());
    assert!(matches!(
        err,
        Err(DatabaseError::Constraint(
            relvar_core::constraints::ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
}

#[test]
fn test_database_validate_insert_bulk_fails() {
    let mut db = Database::new(InMemoryEngine::new());

    // Create PARENT
    let parent_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    );
    db.create_relvar("PARENT", parent_type).unwrap();
    let pk = relvar_core::constraints::PrimaryKey::new(vec!["id".to_string()]).unwrap();
    db.set_key_constraints(
        "PARENT",
        relvar_core::constraints::KeyConstraints::new().with_primary_key(pk),
    )
    .unwrap();

    // Insert tuples
    db.insert("PARENT", tuple! { id: 1i64, name: "P1" })
        .unwrap();

    // insert duplicate id to fail at `validate_key_constraints_single_tuple` inside validate_insert
    let err = db.insert("PARENT", tuple! { id: 1i64, name: "P1_duplicate" });

    assert!(err.is_err());
}

#[test]
fn test_database_update_constraint_validation() {
    let mut db = Database::new(InMemoryEngine::new());

    // Create PARENT
    let parent_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    );
    db.create_relvar("PARENT", parent_type).unwrap();
    let pk = relvar_core::constraints::PrimaryKey::new(vec!["id".to_string()]).unwrap();
    db.set_key_constraints(
        "PARENT",
        relvar_core::constraints::KeyConstraints::new().with_primary_key(pk),
    )
    .unwrap();

    db.insert("PARENT", tuple! { id: 1i64, name: "P1" })
        .unwrap();

    // try to update to the same primary key... wait we need 2 tuples
    db.insert("PARENT", tuple! { id: 2i64, name: "P2" })
        .unwrap();

    // Update id 2 to id 1, violating PK via validate_relation_constraints
    let err = db.update(
        "PARENT",
        |t| t.get_typed::<i64>("id").unwrap() == 2,
        |t| {
            let mut map = std::collections::BTreeMap::new();
            map.insert("id".to_string(), relvar_core::values::ScalarValue::Int(1));
            map.insert(
                "name".to_string(),
                relvar_core::values::ScalarValue::String("P2_renamed".to_string()),
            );
            relvar_core::values::Tuple::new(std::sync::Arc::new(t.tuple_type().clone()), map)
                .unwrap()
        },
    );

    assert!(err.is_err());
    assert!(matches!(
        err,
        Err(DatabaseError::Constraint(
            relvar_core::constraints::ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
}
