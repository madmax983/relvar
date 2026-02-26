use relvar_core::constraints::expression::{ConstraintExpression, CmpOp, ValueOrRef, ExpressionError};
use relvar_core::values::ScalarValue;
use relvar_core::tuple;

#[test]
fn test_constraint_recursion_evaluate_stack_overflow() {
    // Construct a deeply nested expression: NOT(NOT(...(true)...))
    // Use a depth > 32 (RECURSION_LIMIT) but small enough to not blow stack on Drop.
    // 1000 frames is safe for stack (default is usually ~2MB, 1000 frames is ~100KB).
    // But it will trigger the RecursionGuard (limit 32).

    let depth = 100;
    let mut expr = ConstraintExpression::Cmp {
        left: "age".to_string(),
        op: CmpOp::Gt,
        right: ValueOrRef::Value(ScalarValue::Int(0)),
    };

    for _ in 0..depth {
        expr = ConstraintExpression::Not(Box::new(expr));
    }

    // Evaluate it
    let tuple = tuple! { age: 10i64 };

    // This should now return an error instead of crashing or succeeding
    // because RecursionGuard limits depth to 32.
    let result = expr.evaluate(&tuple);

    match result {
        Ok(_) => panic!("Should have failed with RecursionLimitExceeded"),
        Err(e) => {
            assert!(matches!(e, ExpressionError::RecursionLimitExceeded), "Expected RecursionLimitExceeded, got: {:?}", e);
        }
    }
}
