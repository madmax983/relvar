use relvar_core::constraints::{ConstraintExpression, CmpOp, ValueOrRef};
use relvar_core::values::ScalarValue;
use serde_json::json;

#[test]
fn test_constraint_deserialization_coverage() {
    // We want to test deserialization of all variants to hit the TryFrom match arms.

    // 1. Cmp
    let cmp_json = json!({
        "Cmp": {
            "left": "age",
            "op": "Eq",
            "right": {"Value": {"Int": 42}}
        }
    });
    let cmp_expr: ConstraintExpression = serde_json::from_value(cmp_json).unwrap();
    assert_eq!(
        cmp_expr,
        ConstraintExpression::Cmp {
            left: "age".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(42))
        }
    );

    // 2. And
    let and_json = json!({
        "And": [
            {
                "Cmp": {
                    "left": "a",
                    "op": "Eq",
                    "right": {"Value": {"Int": 1}}
                }
            },
            {
                "Cmp": {
                    "left": "b",
                    "op": "Eq",
                    "right": {"Value": {"Int": 2}}
                }
            }
        ]
    });
    let and_expr: ConstraintExpression = serde_json::from_value(and_json).unwrap();
    assert!(matches!(and_expr, ConstraintExpression::And(_, _)));

    // 3. Or
    let or_json = json!({
        "Or": [
            {
                "Cmp": {
                    "left": "a",
                    "op": "Eq",
                    "right": {"Value": {"Int": 1}}
                }
            },
            {
                "Cmp": {
                    "left": "b",
                    "op": "Eq",
                    "right": {"Value": {"Int": 2}}
                }
            }
        ]
    });
    let or_expr: ConstraintExpression = serde_json::from_value(or_json).unwrap();
    assert!(matches!(or_expr, ConstraintExpression::Or(_, _)));

    // 4. Not
    let not_json = json!({
        "Not": {
            "Cmp": {
                "left": "a",
                "op": "Eq",
                "right": {"Value": {"Int": 1}}
            }
        }
    });
    let not_expr: ConstraintExpression = serde_json::from_value(not_json).unwrap();
    assert!(matches!(not_expr, ConstraintExpression::Not(_)));

    // 5. In
    let in_json = json!({
        "In": [
            "status",
            [
                {"String": "active"}
            ]
        ]
    });
    let in_expr: ConstraintExpression = serde_json::from_value(in_json).unwrap();
    assert_eq!(
        in_expr,
        ConstraintExpression::In("status".to_string(), vec![ScalarValue::String("active".to_string())])
    );

    // 6. Like
    let like_json = json!({
        "Like": [
            "name",
            "A%"
        ]
    });
    let like_expr: ConstraintExpression = serde_json::from_value(like_json).unwrap();
    assert_eq!(
        like_expr,
        ConstraintExpression::Like("name".to_string(), "A%".to_string())
    );
}
