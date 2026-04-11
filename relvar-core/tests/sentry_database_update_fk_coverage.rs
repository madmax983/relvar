use relvar_core::constraints::{
    CandidateKey, ConstraintManagerError, ForeignKey, ForeignKeyConstraints, KeyConstraints,
};
use relvar_core::database::Database;
use relvar_core::error::DatabaseError;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};

#[test]
fn test_update_referenced_parent_fails() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    let parent_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    );
    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("parent_id", ScalarType::Int),
    );

    db.create_relvar("PARENT", parent_type).unwrap();
    db.create_relvar("CHILD", child_type).unwrap();

    let pk = CandidateKey::new(vec!["id".to_string()]).unwrap();
    db.set_key_constraints("PARENT", KeyConstraints::new().with_primary_key(pk))
        .unwrap();

    let fk = ForeignKey::new(
        vec!["parent_id".to_string()],
        "PARENT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    db.set_foreign_key_constraints("CHILD", ForeignKeyConstraints::new().with_foreign_key(fk))
        .unwrap();

    // Insert parent
    db.insert("PARENT", tuple! { id: 1i64, name: "Parent1" })
        .unwrap();

    // Insert child referencing Parent1
    db.insert("CHILD", tuple! { child_id: 100i64, parent_id: 1i64 })
        .unwrap();

    // Update Parent1's id (referenced by child) - should fail
    let result = db.update(
        "PARENT",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |t| tuple! { id: 2i64, name: t.get_typed::<String>("name").unwrap() },
    );

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
}
