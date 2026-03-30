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

    let result = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |_t| tuple! { id: 1i64, val: "not an int".to_string() },
    );

    assert!(matches!(result, Err(DatabaseError::TupleMismatch)));
}
