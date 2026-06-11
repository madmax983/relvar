use crate::{CmpOp, ConstraintExpression, ScalarValue, ValueOrRef};

#[test]
fn test_constraint_recursion() {
    let mut expr = ConstraintExpression::Cmp {
        left: "A".to_string(),
        op: CmpOp::Eq,
        right: ValueOrRef::Value(ScalarValue::Int(1)),
    };
    for _ in 0..100 {
        // Exceeds MAX_RECURSION_DEPTH of 64
        expr = ConstraintExpression::Not(Box::new(expr));
    }

    let bytes = postcard::to_stdvec(&expr).unwrap();
    let result: Result<ConstraintExpression, _> = postcard::from_bytes(&bytes);

    assert!(
        result.is_err(),
        "Deserialization should fail due to recursion limit"
    );
}
