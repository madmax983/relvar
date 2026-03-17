use relvar_core::constraints::{CheckConstraint, CheckConstraints};
use relvar_core::constraints::{ConstraintExpression, CmpOp, ValueOrRef};
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
