use relvar_core::constraints::{CheckConstraint, CheckConstraints};
use relvar_core::constraints::{CmpOp, ConstraintExpression, ValueOrRef};
use relvar_core::values::ScalarValue;

#[test]
fn test_check_constraints_properties() {
    let constraint = CheckConstraint::new(
        "my_check",
        "description here",
        ConstraintExpression::Cmp {
            left: "val".to_string(),
            op: CmpOp::Gt,
            right: ValueOrRef::Value(ScalarValue::Int(0)),
        },
    );

    let constraints = CheckConstraints::new().with_constraint(constraint);
    let list = constraints.constraints();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name(), "my_check");
}

#[test]
fn test_check_constraint_properties() {
    let expr = ConstraintExpression::Cmp {
        left: "val".to_string(),
        op: CmpOp::Gt,
        right: ValueOrRef::Value(ScalarValue::Int(0)),
    };

    let constraint = CheckConstraint::new(
        "my_check",
        "description here",
        expr.clone(),
    );

    assert_eq!(constraint.description(), "description here");

    // Test the expression() method
    if let ConstraintExpression::Cmp { left, op, right } = constraint.expression() {
        assert_eq!(left, "val");
        assert!(matches!(op, CmpOp::Gt));
        assert!(matches!(right, ValueOrRef::Value(ScalarValue::Int(0))));
    } else {
        panic!("Expression mismatch");
    }
}
