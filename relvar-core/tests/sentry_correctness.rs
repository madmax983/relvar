use proptest::prelude::*;
use relvar_core::constraints::expression::{CmpOp, ConstraintExpression, ValueOrRef};
use relvar_core::tuple;
use relvar_core::values::ScalarValue;

proptest! {
    #[test]
    fn test_scalar_float_ord_consistency(f1 in any::<f64>(), f2 in any::<f64>()) {
        let v1 = ScalarValue::Float(f1);
        let v2 = ScalarValue::Float(f2);

        // Check partial_cmp is consistent with cmp (Ord)
        prop_assert_eq!(v1.partial_cmp(&v2), Some(v1.cmp(&v2)));

        // Check antisymmetry: a <= b && b <= a => a == b (in terms of Eq)
        // Note: ScalarValue::Eq uses bit equality for floats (NaN == NaN)
        // But Ord uses bit comparison too.
        // -0.0 < 0.0. a <= b True. b <= a False. Consistent.
        // NaN == NaN. a <= b True. b <= a True. a == b True. Consistent.

        if v1 <= v2 && v2 <= v1 {
            prop_assert_eq!(&v1, &v2);
        }

        // Check consistency with PartialEq
        if v1 == v2 {
            prop_assert_eq!(v1.cmp(&v2), std::cmp::Ordering::Equal);
        }
    }

    #[test]
    fn test_scalar_float_transitivity(f1 in any::<f64>(), f2 in any::<f64>(), f3 in any::<f64>()) {
        let v1 = ScalarValue::Float(f1);
        let v2 = ScalarValue::Float(f2);
        let v3 = ScalarValue::Float(f3);

        if v1 <= v2 && v2 <= v3 {
            prop_assert!(v1 <= v3);
        }
    }
}

#[test]
fn test_constraint_in_type_inconsistency() {
    // Demonstrates that IN swallows type mismatches as false, while Cmp returns error.

    // Tuple has 'status' as String
    let tuple = tuple! { status: "active" };

    // Constraint: status IN (1, 2)
    // 1 and 2 are Ints. 'status' is String.
    let in_expr = ConstraintExpression::In(
        "status".to_string(),
        vec![ScalarValue::Int(1), ScalarValue::Int(2)],
    );

    // IN evaluation should succeed and return false (no match)
    // It does NOT return TypeMismatch error.
    let result = in_expr.evaluate(&tuple);
    assert!(result.is_ok());
    assert!(!result.unwrap());

    // Constraint: status = 1
    let cmp_expr = ConstraintExpression::Cmp {
        left: "status".to_string(),
        op: CmpOp::Eq,
        right: ValueOrRef::Value(ScalarValue::Int(1)),
    };

    // Cmp evaluation should FAIL with TypeMismatch
    let result = cmp_expr.evaluate(&tuple);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Type mismatch"));
}

#[test]
fn test_constraint_deep_nesting() {
    // Construct a deeply nested expression:
    // x > 0 AND (x > 0 AND (x > 0 ...))

    // Using a relatively safe depth that would still stress recursion logic
    let depth = 500;
    let mut expr = ConstraintExpression::Cmp {
        left: "x".to_string(),
        op: CmpOp::Gt,
        right: ValueOrRef::Value(ScalarValue::Int(0)),
    };

    for _ in 0..depth {
        expr = ConstraintExpression::And(
            Box::new(ConstraintExpression::Cmp {
                left: "x".to_string(),
                op: CmpOp::Gt,
                right: ValueOrRef::Value(ScalarValue::Int(0)),
            }),
            Box::new(expr),
        );
    }

    let tuple = tuple! { x: 10i64 };

    // This should not stack overflow
    let result = expr.evaluate(&tuple);
    assert!(result.is_ok());
    assert!(result.unwrap());
}
