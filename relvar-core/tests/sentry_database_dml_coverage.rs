use relvar_core::database::Database;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::types::{RelationType, ScalarType, TupleType};

#[test]
fn test_compute_relation_after_update() {
    let mut db = Database::new(InMemoryEngine::new());
    let tuple_type = TupleType::new().with_attribute("id", ScalarType::Int);
    let relation_type = RelationType::new(tuple_type);
    db.create_relvar("TEST", relation_type).unwrap();

    // We can't use compute_relation_after_update easily since it's private to dml,
    // let's do this via Database::update which calls compute_relation_after_update.
    db.insert("TEST", relvar_core::tuple! { id: 1i64 }).unwrap();
    db.insert("TEST", relvar_core::tuple! { id: 2i64 }).unwrap();

    let update_count = db
        .update(
            "TEST",
            |t| t.get_typed::<i64>("id").unwrap_or(0) == 1,
            |_t| relvar_core::tuple! { id: 3i64 },
        )
        .unwrap();

    assert_eq!(update_count, 1);

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 2);
}

#[test]
fn test_compute_relation_after_update_mismatch() {
    let mut db = Database::new(InMemoryEngine::new());
    let tuple_type = TupleType::new().with_attribute("id", ScalarType::Int);
    let relation_type = RelationType::new(tuple_type);
    db.create_relvar("TEST", relation_type).unwrap();

    db.insert("TEST", relvar_core::tuple! { id: 1i64 }).unwrap();

    let res = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap_or(0) == 1,
        |_t| relvar_core::tuple! { id: "not_an_int".to_string() },
    );

    assert!(res.is_err());
}
