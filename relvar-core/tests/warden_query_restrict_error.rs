use relvar_core::query::Query;
use relvar_core::constraints::{ConstraintExpression, CmpOp, ValueOrRef};
use relvar_core::values::ScalarValue;
use relvar_core::types::{RelationType, TupleType, ScalarType};
use relvar_core::database::Database;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::tuple;

#[test]
fn test_query_restrict_propagates_errors() {
    let mut db = Database::new(InMemoryEngine::new());

    let heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);

    db.create_relvar("USERS", RelationType::new(heading)).unwrap();
    db.insert("USERS", tuple! { id: 1i64, name: "Alice" }).unwrap();

    // Type mismatch error
    let query = Query::scan("USERS")
        .restrict(ConstraintExpression::Cmp {
            left: "name".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(1)),
        });

    let result = query.execute(&db);
    assert!(result.is_err());

    // Attribute not found error
    let query_missing = Query::scan("USERS")
        .restrict(ConstraintExpression::Cmp {
            left: "missing_attr".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(1)),
        });

    let result_missing = query_missing.execute(&db);
    assert!(result_missing.is_err());
}
