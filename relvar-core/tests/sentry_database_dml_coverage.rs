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

// Data.rs Tests

#[test]
fn test_database_data_delete_validation_uncovered() {
    use relvar_core::database::Database;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    db.create_relvar("PARENT", rel_type.clone()).unwrap();

    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("parent_id", ScalarType::Int),
    );
    db.create_relvar("CHILD", child_type.clone()).unwrap();

    db.insert("PARENT", tuple! { id: 1i64 }).unwrap();
    db.insert("CHILD", tuple! { child_id: 10i64, parent_id: 1i64 })
        .unwrap();

    // Set FK constraint
    let mut fk = relvar_core::constraints::ForeignKeyConstraints::new();
    fk = fk.with_foreign_key(
        relvar_core::constraints::ForeignKey::new(
            vec!["parent_id".to_string()],
            "PARENT".to_string(),
            vec!["id".to_string()],
        )
        .unwrap(),
    );
    db.set_foreign_key_constraints("CHILD", fk).unwrap();

    // Trying to delete parent should fail due to FK referencing constraint
    // Because the child has an FK on parent_id -> id
    let result = db.delete("PARENT", |t| t.get_typed::<i64>("id").unwrap() == 1);

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(relvar_core::error::DatabaseError::Constraint(_))
    ));
}

#[test]
fn test_database_data_update_validation_uncovered() {
    use relvar_core::database::Database;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    db.create_relvar("PARENT", rel_type.clone()).unwrap();

    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("parent_id", ScalarType::Int),
    );
    db.create_relvar("CHILD", child_type.clone()).unwrap();

    db.insert("PARENT", tuple! { id: 1i64 }).unwrap();
    db.insert("CHILD", tuple! { child_id: 10i64, parent_id: 1i64 })
        .unwrap();

    // Set FK constraint
    let mut fk = relvar_core::constraints::ForeignKeyConstraints::new();
    fk = fk.with_foreign_key(
        relvar_core::constraints::ForeignKey::new(
            vec!["parent_id".to_string()],
            "PARENT".to_string(),
            vec!["id".to_string()],
        )
        .unwrap(),
    );
    db.set_foreign_key_constraints("CHILD", fk).unwrap();

    // Try to update parent's id, which would break child's FK
    let result = db.update(
        "PARENT",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |_t| {
            tuple! { id: 2i64 }
        },
    );

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(relvar_core::error::DatabaseError::Constraint(_))
    ));
}

#[test]
fn test_database_data_update_duplicate_keys_uncovered() {
    use relvar_core::database::Database;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    db.create_relvar("TEST", rel_type.clone()).unwrap();

    db.insert("TEST", tuple! { id: 1i64 }).unwrap();
    db.insert("TEST", tuple! { id: 2i64 }).unwrap();

    let mut keys = relvar_core::constraints::KeyConstraints::new();
    keys = keys.with_primary_key(
        relvar_core::constraints::PrimaryKey::new(vec!["id".to_string()]).unwrap(),
    );
    db.set_key_constraints("TEST", keys).unwrap();

    // Update id 2 to id 1, violating the key constraint implicitly through set duplication
    // We expect the update to complete, but the cardinality should drop,
    // which triggers our new check.
    let result = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 2,
        |_t| {
            tuple! { id: 1i64 }
        },
    );

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(relvar_core::error::DatabaseError::Constraint(_))
    ));
}
