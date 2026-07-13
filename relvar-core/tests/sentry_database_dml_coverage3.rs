use relvar_core::database::Database;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::tuple;
use relvar_core::error::DatabaseError;

fn test_rel_type() -> RelationType {
    RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    )
}

#[test]
fn should_return_error_when_updated_tuple_mismatches_type() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" }).unwrap();

    let result = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |_| tuple! { id: 1i64, extra: "unexpected" }, // Wrong tuple type
    );
    assert!(matches!(result, Err(DatabaseError::TupleMismatch)));
}
