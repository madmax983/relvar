//! Set operators and semijoins in the query AST (TTM RM Prescription 18).
//!
//! `UNION`, `INTERSECT` and `MINUS` require operands of the same type; a
//! heading mismatch is a type error, never a silent empty result.
//! `MATCHING` (semijoin) and `NOT MATCHING` (semidifference) keep the left
//! operand's heading; their common attributes must have the same type.

use crate::algebra::JoinError;
use crate::constraints::{CmpOp, ConstraintExpression, ValueOrRef};
use crate::database::Database;
use crate::query::{Query, QueryError};
use crate::storage_engine::InMemoryEngine;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::ScalarValue;
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

/// P{pid}: 1, 2, 3.   Q{pid}: 2, 3, 4.   C{cid, pid}: (10, 1), (11, 3).
/// S{pid: String}: a relvar whose `pid` has a different type.
fn set_db() -> Database<InMemoryEngine> {
    let mut db = Database::new(InMemoryEngine::new());
    let pid = || RelationType::new(TupleType::new().with_attribute("pid", ScalarType::Int));
    db.create_relvar("P", pid()).unwrap();
    db.create_relvar("Q", pid()).unwrap();
    db.create_relvar(
        "C",
        RelationType::new(
            TupleType::new()
                .with_attribute("cid", ScalarType::Int)
                .with_attribute("pid", ScalarType::Int),
        ),
    )
    .unwrap();
    db.create_relvar(
        "S",
        RelationType::new(TupleType::new().with_attribute("pid", ScalarType::String)),
    )
    .unwrap();
    for p in [1i64, 2, 3] {
        db.insert("P", tuple! { pid: p }).unwrap();
    }
    for q in [2i64, 3, 4] {
        db.insert("Q", tuple! { pid: q }).unwrap();
    }
    db.insert("C", tuple! { cid: 10i64, pid: 1i64 }).unwrap();
    db.insert("C", tuple! { cid: 11i64, pid: 3i64 }).unwrap();
    db.insert("S", tuple! { pid: "1" }).unwrap();
    db
}

fn pids(query: &Query, db: &Database<InMemoryEngine>) -> Vec<i64> {
    let mut values: Vec<i64> = query
        .execute(db)
        .unwrap()
        .tuples()
        .filter_map(|t| t.get_typed::<i64>("pid"))
        .collect();
    values.sort_unstable();
    values
}

#[test]
fn union_combines_both_bodies() {
    let db = set_db();
    assert_eq!(
        pids(&Query::scan("P").union(Query::scan("Q")), &db),
        vec![1, 2, 3, 4]
    );
}

#[test]
fn intersect_keeps_common_tuples() {
    let db = set_db();
    assert_eq!(
        pids(&Query::scan("P").intersect(Query::scan("Q")), &db),
        vec![2, 3]
    );
}

#[test]
fn minus_removes_right_tuples() {
    let db = set_db();
    assert_eq!(
        pids(&Query::scan("P").minus(Query::scan("Q")), &db),
        vec![1]
    );
}

#[test]
fn matching_keeps_left_tuples_with_a_partner() {
    let db = set_db();
    let query = Query::scan("P").matching(Query::scan("C").project(vec!["pid"]));
    assert_eq!(pids(&query, &db), vec![1, 3]);
    // The heading is the left operand's.
    assert_eq!(query.execute(&db).unwrap().degree(), 1);
}

#[test]
fn not_matching_keeps_left_tuples_without_a_partner() {
    let db = set_db();
    let query = Query::scan("P").not_matching(Query::scan("C"));
    assert_eq!(pids(&query, &db), vec![2]);
}

#[test]
fn matching_with_no_common_attributes_is_all_or_nothing() {
    let db = set_db();
    let renamed_c = Query::scan("C").rename(vec![("pid", "other")]);
    assert_eq!(
        pids(&Query::scan("P").matching(renamed_c.clone()), &db),
        vec![1, 2, 3]
    );
    assert!(pids(&Query::scan("P").not_matching(renamed_c), &db).is_empty());
}

#[test]
fn union_heading_mismatch_is_a_type_error() {
    let db = set_db();
    let result = Query::scan("P").union(Query::scan("C")).execute(&db);
    assert!(matches!(result, Err(QueryError::Union(_))));
}

#[test]
fn intersect_heading_mismatch_is_a_type_error() {
    let db = set_db();
    let result = Query::scan("P").intersect(Query::scan("S")).execute(&db);
    assert!(matches!(result, Err(QueryError::Intersect(_))));
}

#[test]
fn minus_heading_mismatch_is_a_type_error() {
    let db = set_db();
    let result = Query::scan("P").minus(Query::scan("C")).execute(&db);
    assert!(matches!(result, Err(QueryError::Difference(_))));
}

#[test]
fn matching_common_attribute_type_mismatch_is_a_type_error() {
    let db = set_db();
    let result = Query::scan("P").matching(Query::scan("S")).execute(&db);
    assert!(matches!(
        result,
        Err(QueryError::Join(JoinError::AttributeTypeMismatch { attribute, .. })) if attribute == "pid"
    ));
}

#[test]
fn not_matching_common_attribute_type_mismatch_is_a_type_error() {
    let db = set_db();
    let result = Query::scan("P").not_matching(Query::scan("S")).execute(&db);
    assert!(matches!(
        result,
        Err(QueryError::Join(JoinError::AttributeTypeMismatch { .. }))
    ));
}

#[test]
fn operand_errors_propagate() {
    let db = set_db();
    for query in [
        Query::scan("P").union(Query::scan("MISSING")),
        Query::scan("MISSING").intersect(Query::scan("P")),
        Query::scan("P").minus(Query::scan("MISSING")),
        Query::scan("P").matching(Query::scan("MISSING")),
        Query::scan("MISSING").not_matching(Query::scan("P")),
    ] {
        assert!(matches!(query.execute(&db), Err(QueryError::Database(_))));
    }
}

#[test]
fn restricted_not_matching_expresses_an_orphan_check() {
    // P WHERE pid = 2 NOT MATCHING C: parent 2 has no child.
    let db = set_db();
    let query = Query::scan("P")
        .restrict(ConstraintExpression::Cmp {
            left: "pid".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(2)),
        })
        .not_matching(Query::scan("C").project(vec!["pid"]));
    assert_eq!(pids(&query, &db), vec![2]);
}

#[test]
fn explain_names_every_set_operator() {
    let cases = [
        (Query::scan("P").union(Query::scan("Q")), "Union"),
        (Query::scan("P").intersect(Query::scan("Q")), "Intersect"),
        (Query::scan("P").minus(Query::scan("Q")), "Minus"),
        (Query::scan("P").matching(Query::scan("Q")), "Matching"),
        (
            Query::scan("P").not_matching(Query::scan("Q")),
            "NotMatching",
        ),
    ];
    for (query, name) in cases {
        let explanation = query.explain();
        assert!(explanation.starts_with(name), "{explanation}");
        assert!(explanation.contains("  Scan(P)"), "{explanation}");
        assert!(explanation.contains("  Scan(Q)"), "{explanation}");
    }
}

#[test]
fn set_operators_round_trip_through_json() {
    let db = set_db();
    let query = Query::scan("P")
        .union(Query::scan("Q"))
        .minus(Query::scan("P").intersect(Query::scan("Q")))
        .not_matching(Query::scan("C").matching(Query::scan("P")));
    let text = serde_json::to_string(&query).unwrap();
    let back: Query = serde_json::from_str(&text).unwrap();
    assert_eq!(pids(&back, &db), pids(&query, &db));
}

#[test]
fn deeply_nested_set_operators_are_rejected_on_deserialization() {
    let mut query = Query::scan("P");
    for _ in 0..100 {
        query = Query::Union {
            left: Box::new(Query::scan("P")),
            right: Box::new(query),
        };
    }
    let bytes = postcard::to_stdvec(&query).unwrap();
    core::mem::forget(query); // avoid a deep recursive drop in the test
    let result: Result<Query, _> = postcard::from_bytes(&bytes);
    assert!(result.is_err());
    if let Ok(back) = result {
        core::mem::forget(back);
    }
}
