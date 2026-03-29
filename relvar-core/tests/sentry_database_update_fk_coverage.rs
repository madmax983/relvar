use relvar_core::constraints::ConstraintManagerError;
use relvar_core::constraints::{ForeignKey, ForeignKeyConstraints, KeyConstraints, PrimaryKey};
use relvar_core::database::Database;
use relvar_core::error::DatabaseError;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};

#[test]
fn test_database_update_validates_referencing_foreign_keys() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    // Create PARENT
    let parent_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    );
    db.create_relvar("PARENT", parent_type).unwrap();
    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    db.set_key_constraints("PARENT", KeyConstraints::new().with_primary_key(pk))
        .unwrap();

    // Create CHILD
    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("parent_id", ScalarType::Int),
    );
    db.create_relvar("CHILD", child_type).unwrap();
    let fk = ForeignKey::new(
        vec!["parent_id".to_string()],
        "PARENT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    db.set_foreign_key_constraints("CHILD", ForeignKeyConstraints::new().with_foreign_key(fk))
        .unwrap();

    // Insert tuples
    db.insert("PARENT", tuple! { id: 1i64, name: "P1" })
        .unwrap();
    db.insert("PARENT", tuple! { id: 2i64, name: "P2" })
        .unwrap();
    db.insert("CHILD", tuple! { child_id: 100i64, parent_id: 1i64 })
        .unwrap();

    // Attempt update that violates FK constraint (updating parent being referenced)
    let err = db.update(
        "PARENT",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |_t| tuple! { id: 99i64, name: "P1_new" },
    );
    assert!(err.is_err());
    assert!(matches!(
        err,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));

    // Valid update doesn't trigger FK violation
    let count = db
        .update(
            "PARENT",
            |t| t.get_typed::<i64>("id").unwrap() == 1,
            |_t| tuple! { id: 1i64, name: "P1_renamed" },
        )
        .unwrap();
    assert_eq!(count, 1);

    // Update on unreferenced parent
    let count = db
        .update(
            "PARENT",
            |t| t.get_typed::<i64>("id").unwrap() == 2,
            |_t| tuple! { id: 22i64, name: "P2_new" },
        )
        .unwrap();
    assert_eq!(count, 1);
}
