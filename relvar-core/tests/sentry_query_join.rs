use relvar_core::database::Database;
use relvar_core::query::Query;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::types::{RelationType, ScalarType, TupleType};

#[test]
fn test_query_join_coverage() {
    let mut db = Database::new(InMemoryEngine::new());

    // Create first relvar
    let type1 = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);
    db.create_relvar("r1", RelationType::new(type1.clone()))
        .unwrap();
    db.insert("r1", relvar_core::tuple! { id: 1i64, name: "A" })
        .unwrap();

    // Create second relvar
    let type2 = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("value", ScalarType::Float);
    db.create_relvar("r2", RelationType::new(type2.clone()))
        .unwrap();
    db.insert("r2", relvar_core::tuple! { id: 1i64, value: 3.5 })
        .unwrap();

    // Test the Join query variant
    let q = Query::Join {
        left: Box::new(Query::Scan("r1".to_string())),
        right: Box::new(Query::Scan("r2".to_string())),
    };

    let res = q.execute(&db).unwrap();
    assert_eq!(res.cardinality(), 1);
    assert_eq!(res.degree(), 3);
}

#[test]
fn test_query_rename_coverage() {
    let mut db = Database::new(InMemoryEngine::new());

    let type1 = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);
    db.create_relvar("r1", RelationType::new(type1.clone()))
        .unwrap();
    db.insert("r1", relvar_core::tuple! { id: 1i64, name: "A" })
        .unwrap();

    let q = Query::Rename {
        input: Box::new(Query::Scan("r1".to_string())),
        mappings: vec![("name".to_string(), "first_name".to_string())],
    };

    let res = q.execute(&db).unwrap();
    assert_eq!(res.cardinality(), 1);
    assert_eq!(res.degree(), 2);
    assert!(res.relation_type().heading().has_attribute("first_name"));
    assert!(!res.relation_type().heading().has_attribute("name"));
}

#[test]
fn test_query_project_coverage() {
    let mut db = Database::new(InMemoryEngine::new());

    let type1 = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);
    db.create_relvar("r1", RelationType::new(type1.clone()))
        .unwrap();
    db.insert("r1", relvar_core::tuple! { id: 1i64, name: "A" })
        .unwrap();

    let q = Query::Project {
        input: Box::new(Query::Scan("r1".to_string())),
        attributes: vec!["id".to_string()],
    };

    let res = q.execute(&db).unwrap();
    assert_eq!(res.cardinality(), 1);
    assert_eq!(res.degree(), 1);
    assert!(res.relation_type().heading().has_attribute("id"));
    assert!(!res.relation_type().heading().has_attribute("name"));
}

#[test]
fn test_query_summarize_coverage() {
    let mut db = Database::new(InMemoryEngine::new());

    let type1 = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("val", ScalarType::Int);
    db.create_relvar("r1", RelationType::new(type1.clone()))
        .unwrap();
    db.insert("r1", relvar_core::tuple! { id: 1i64, val: 10i64 })
        .unwrap();
    db.insert("r1", relvar_core::tuple! { id: 1i64, val: 20i64 })
        .unwrap();
    db.insert("r1", relvar_core::tuple! { id: 2i64, val: 50i64 })
        .unwrap();

    let q = Query::Summarize {
        input: Box::new(Query::Scan("r1".to_string())),
        group_by: vec!["id".to_string()],
        aggregations: vec![relvar_core::algebra::Aggregation::sum("sum_val", "val")],
    };

    let res = q.execute(&db).unwrap();
    assert_eq!(res.cardinality(), 2);
    assert_eq!(res.degree(), 2);
}
