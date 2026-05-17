use crate::database::Database;
use crate::storage_engine::InMemoryEngine;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, ScalarValue};

pub fn setup_db() -> Database<InMemoryEngine> {
    let mut db = Database::new(InMemoryEngine::new());

    let users_heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("name", ScalarType::String)
        .with_attribute("age", ScalarType::Int);

    db.create_relvar("USERS", RelationType::new(users_heading.clone()))
        .unwrap();

    let users_rel_type = RelationType::new(users_heading.clone());
    let mut users_rel = Relation::new(users_rel_type);
    users_rel
        .insert(tuple![
            id: ScalarValue::Int(1),
            name: ScalarValue::String("Alice".into()),
            age: ScalarValue::Int(30)
        ])
        .unwrap();
    users_rel
        .insert(tuple![
            id: ScalarValue::Int(2),
            name: ScalarValue::String("Bob".into()),
            age: ScalarValue::Int(25)
        ])
        .unwrap();
    for t in users_rel.tuples() {
        db.insert("USERS", t.clone()).unwrap();
    }

    let orders_heading = TupleType::new()
        .with_attribute("order_id", ScalarType::Int)
        .with_attribute("id", ScalarType::Int) // FK to users
        .with_attribute("amount", ScalarType::Int);

    let orders_rel_type = RelationType::new(orders_heading.clone());
    db.create_relvar("ORDERS", orders_rel_type.clone()).unwrap();
    let mut orders_rel = Relation::new(orders_rel_type);
    orders_rel
        .insert(tuple![
            order_id: ScalarValue::Int(101),
            id: ScalarValue::Int(1),
            amount: ScalarValue::Int(500)
        ])
        .unwrap();
    for t in orders_rel.tuples() {
        db.insert("ORDERS", t.clone()).unwrap();
    }

    db
}

use crate::algebra::Aggregation;
use crate::constraints::{CmpOp, ConstraintExpression, ValueOrRef};
use crate::query::{Query, QueryError};

#[test]
fn test_scan() {
    let db = setup_db();
    let query = Query::scan("USERS");

    let result = query.execute(&db).unwrap();
    assert_eq!(result.cardinality(), 2);

    // Error case
    let err_query = Query::scan("MISSING");
    assert!(matches!(
        err_query.execute(&db),
        Err(QueryError::Database(_))
    ));
}

#[test]
fn test_restrict() {
    let db = setup_db();
    let query = Query::scan("USERS").restrict(ConstraintExpression::Cmp {
        left: "id".to_string(),
        op: CmpOp::Eq,
        right: ValueOrRef::Value(ScalarValue::Int(1)),
    });

    let result = query.execute(&db).unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_project() {
    let db = setup_db();
    let query = Query::scan("USERS").project(vec!["name"]);

    let result = query.execute(&db).unwrap();
    assert_eq!(result.degree(), 1);
    assert_eq!(result.cardinality(), 2);
}

#[test]
fn test_rename() {
    let db = setup_db();
    let query = Query::scan("USERS").rename(vec![("name", "full_name")]);

    let result = query.execute(&db).unwrap();
    assert!(result.relation_type().heading().has_attribute("full_name"));
    assert!(!result.relation_type().heading().has_attribute("name"));
}

#[test]
fn test_join() {
    let db = setup_db();
    let q_users = Query::scan("USERS");
    let q_orders = Query::scan("ORDERS");

    let query = q_users.join(q_orders);
    let result = query.execute(&db).unwrap();

    assert_eq!(result.cardinality(), 1); // Only Alice has an order
    assert_eq!(result.degree(), 5); // id, name, age, order_id, amount
}

#[test]
fn test_summarize() {
    let db = setup_db();
    let query = Query::scan("USERS").summarize(vec!["age"], vec![Aggregation::count("count")]);
    let result = query.execute(&db).unwrap();

    // One person with age 25, one with age 30
    assert_eq!(result.cardinality(), 2);
}

#[test]
fn test_explain() {
    let query = Query::scan("USERS")
        .restrict(ConstraintExpression::Cmp {
            left: "id".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(1)),
        })
        .project(vec!["name"]);

    let explain_str = query.explain();
    assert!(explain_str.contains("Project([\"name\"])"));
    assert!(explain_str.contains("Restrict"));
    assert!(explain_str.contains("Scan(USERS)"));

    let q_join = Query::scan("A").join(Query::scan("B"));
    assert!(q_join.explain().contains("Join"));

    let q_rename = Query::scan("A").rename(vec![("old", "new")]);
    assert!(q_rename.explain().contains("Rename([(\"old\", \"new\")])"));

    let q_summarize = Query::scan("A").summarize(vec!["a"], vec![Aggregation::count("cnt")]);
    assert!(q_summarize.explain().contains("Summarize"));
}

#[test]
fn test_query_error_propagation() {
    let db = setup_db();

    // Algebra error in Summarize (grouping by missing attribute)
    let q_bad_sum = Query::scan("USERS").summarize(vec!["nonexistent_col"], vec![]);
    let err = q_bad_sum.execute(&db);
    assert!(matches!(err, Err(QueryError::Algebra(_))));
}
