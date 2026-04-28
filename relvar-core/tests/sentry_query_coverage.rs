use relvar_core::algebra::{Aggregation, AggregationFn};
use relvar_core::database::Database;
use relvar_core::query::Query;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};

#[test]
fn test_query_summarize_coverage() {
    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("val", ScalarType::Int),
    );
    db.create_relvar("TEST", rel_type).unwrap();

    db.insert("TEST", tuple! { id: 1i64, val: 10i64 }).unwrap();
    db.insert("TEST", tuple! { id: 1i64, val: 20i64 }).unwrap();
    db.insert("TEST", tuple! { id: 2i64, val: 30i64 }).unwrap();

    let query = Query::scan("TEST").summarize(
        vec!["id"],
        vec![Aggregation {
            result_name: "sum".to_string(),
            result_type: ScalarType::Int,
            function: AggregationFn::Sum("val".to_string()),
        }],
    );

    let result = query.execute(&db).unwrap();
    assert_eq!(result.cardinality(), 2);
}

#[test]
fn test_query_join_coverage() {
    let mut db = Database::new(InMemoryEngine::new());

    let rel_type1 = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    );
    db.create_relvar("TEST1", rel_type1).unwrap();
    db.insert("TEST1", tuple! { id: 1i64, name: "Alice".to_string() })
        .unwrap();

    let rel_type2 = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("age", ScalarType::Int),
    );
    db.create_relvar("TEST2", rel_type2).unwrap();
    db.insert("TEST2", tuple! { id: 1i64, age: 30i64 }).unwrap();

    let query = Query::scan("TEST1").join(Query::scan("TEST2"));

    let result = query.execute(&db).unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_query_project_coverage() {
    let mut db = Database::new(InMemoryEngine::new());

    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    );
    db.create_relvar("TEST", rel_type).unwrap();
    db.insert("TEST", tuple! { id: 1i64, name: "Alice".to_string() })
        .unwrap();

    let query = Query::scan("TEST").project(vec!["id"]);

    let result = query.execute(&db).unwrap();
    assert_eq!(result.cardinality(), 1);
    assert!(result.relation_type().has_attribute("id"));
    assert!(!result.relation_type().has_attribute("name"));
}

#[test]
fn test_query_rename_coverage() {
    let mut db = Database::new(InMemoryEngine::new());

    let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    db.create_relvar("TEST", rel_type).unwrap();
    db.insert("TEST", tuple! { id: 1i64 }).unwrap();

    let query = Query::scan("TEST").rename(vec![("id", "new_id")]);

    let result = query.execute(&db).unwrap();
    assert_eq!(result.cardinality(), 1);
    assert!(result.relation_type().has_attribute("new_id"));
}
