use relvar_core::constraints::{ConstraintExpression, CmpOp, ValueOrRef};
use relvar_core::database::Database;
use relvar_core::query::Query;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::tuple;

#[test]
fn test_query_restrict_unwrap_or_default_hides_errors() {
    let mut db = Database::new(InMemoryEngine::new());
    let heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);
    db.create_relvar("USERS", RelationType::new(heading)).unwrap();
    db.insert("USERS", tuple! { id: 1i64, name: "Alice".to_string() }).unwrap();
    db.insert("USERS", tuple! { id: 2i64, name: "Bob".to_string() }).unwrap();

    let bad_predicate = ConstraintExpression::Cmp {
        left: "id".to_string(),
        op: CmpOp::Eq,
        right: ValueOrRef::Attribute("name".to_string()),
    };

    let query = Query::Scan("USERS".to_string()).restrict(bad_predicate);

    let result = query.execute(&db);

    assert!(result.is_err(), "Expected query execution to fail due to type mismatch in predicate, but it succeeded!");
}
