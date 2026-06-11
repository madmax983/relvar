use crate::query::Query;
use crate::{CmpOp, ConstraintExpression, ScalarValue, ValueOrRef};

#[test]
fn test_query_recursion() {
    let mut query = Query::Scan("A".to_string());
    for _ in 0..100 {
        // Exceeds MAX_RECURSION_DEPTH of 64
        query = Query::Restrict {
            input: Box::new(query),
            predicate: ConstraintExpression::Cmp {
                left: "A".to_string(),
                op: CmpOp::Eq,
                right: ValueOrRef::Value(ScalarValue::Int(1)),
            },
        };
    }

    let bytes = postcard::to_stdvec(&query).unwrap();
    let result: Result<Query, _> = postcard::from_bytes(&bytes);

    assert!(
        result.is_err(),
        "Deserialization should fail due to recursion limit"
    );
}
