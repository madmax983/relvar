use super::common::setup_db;
use crate::algebra::Aggregation;
use crate::constraints::{CmpOp, ConstraintExpression, ValueOrRef};
use crate::query::{Query, QueryError};
use crate::values::ScalarValue;

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

#[test]
fn test_query_deserialization_depth_limit() {
    let mut q = Query::Scan("test".to_string());
    for _ in 0..100 {
        q = Query::Project {
            input: Box::new(q),
            attributes: vec!["a".to_string()],
        };
    }

    // Serialize with postcard
    let bytes = postcard::to_stdvec(&q).unwrap();

    // Deserialize
    std::mem::forget(q); // Prevent Drop stack overflow
    let result: Result<Query, _> = postcard::from_bytes(&bytes);

    assert!(
        result.is_err(),
        "Deserialization should fail due to recursion limit"
    );

    if let Ok(q2) = result {
        std::mem::forget(q2); // Should not reach here, but prevent Drop if it does
    }
}
