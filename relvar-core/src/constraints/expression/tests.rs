#![allow(clippy::module_inception)]
use super::*;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;

    #[test]
    fn test_constraint_expression_eq() {
        let expr = ConstraintExpression::Cmp {
            left: "salary".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(50000)),
        };

        let tuple = tuple! { salary: 50000i64 };
        assert!(expr.evaluate(&tuple).unwrap());

        let tuple_bad = tuple! { salary: 30000i64 };
        assert!(!expr.evaluate(&tuple_bad).unwrap());
    }

    #[test]
    fn test_constraint_expression_ne() {
        let expr = ConstraintExpression::Cmp {
            left: "status".to_string(),
            op: CmpOp::Ne,
            right: ValueOrRef::Value(ScalarValue::String("inactive".to_string())),
        };

        let tuple = tuple! { status: "active" };
        assert!(expr.evaluate(&tuple).unwrap());

        let tuple_bad = tuple! { status: "inactive" };
        assert!(!expr.evaluate(&tuple_bad).unwrap());
    }

    #[test]
    fn test_constraint_expression_lt() {
        let expr = ConstraintExpression::Cmp {
            left: "age".to_string(),
            op: CmpOp::Lt,
            right: ValueOrRef::Value(ScalarValue::Int(65)),
        };

        let tuple = tuple! { age: 30i64 };
        assert!(expr.evaluate(&tuple).unwrap());

        let tuple_bad = tuple! { age: 70i64 };
        assert!(!expr.evaluate(&tuple_bad).unwrap());

        let tuple_equal = tuple! { age: 65i64 };
        assert!(!expr.evaluate(&tuple_equal).unwrap());
    }

    #[test]
    fn test_constraint_expression_le() {
        let expr = ConstraintExpression::Cmp {
            left: "score".to_string(),
            op: CmpOp::Le,
            right: ValueOrRef::Value(ScalarValue::Int(100)),
        };

        let tuple = tuple! { score: 95i64 };
        assert!(expr.evaluate(&tuple).unwrap());

        let tuple_equal = tuple! { score: 100i64 };
        assert!(expr.evaluate(&tuple_equal).unwrap());

        let tuple_bad = tuple! { score: 105i64 };
        assert!(!expr.evaluate(&tuple_bad).unwrap());
    }

    #[test]
    fn test_constraint_expression_gt() {
        let expr = ConstraintExpression::Cmp {
            left: "salary".to_string(),
            op: CmpOp::Gt,
            right: ValueOrRef::Value(ScalarValue::Int(0)),
        };

        let tuple = tuple! { salary: 50000i64 };
        assert!(expr.evaluate(&tuple).unwrap());

        let tuple_bad = tuple! { salary: -100i64 };
        assert!(!expr.evaluate(&tuple_bad).unwrap());

        let tuple_equal = tuple! { salary: 0i64 };
        assert!(!expr.evaluate(&tuple_equal).unwrap());
    }

    #[test]
    fn test_constraint_expression_ge() {
        let expr = ConstraintExpression::Cmp {
            left: "balance".to_string(),
            op: CmpOp::Ge,
            right: ValueOrRef::Value(ScalarValue::Int(0)),
        };

        let tuple = tuple! { balance: 100i64 };
        assert!(expr.evaluate(&tuple).unwrap());

        let tuple_equal = tuple! { balance: 0i64 };
        assert!(expr.evaluate(&tuple_equal).unwrap());

        let tuple_bad = tuple! { balance: -50i64 };
        assert!(!expr.evaluate(&tuple_bad).unwrap());
    }

    #[test]
    fn test_constraint_expression_and() {
        let expr = ConstraintExpression::And(
            Box::new(ConstraintExpression::Cmp {
                left: "age".to_string(),
                op: CmpOp::Gt,
                right: ValueOrRef::Value(ScalarValue::Int(0)),
            }),
            Box::new(ConstraintExpression::Cmp {
                left: "age".to_string(),
                op: CmpOp::Lt,
                right: ValueOrRef::Value(ScalarValue::Int(150)),
            }),
        );

        let valid = tuple! { age: 30i64 };
        assert!(expr.evaluate(&valid).unwrap());

        let invalid_low = tuple! { age: -5i64 };
        assert!(!expr.evaluate(&invalid_low).unwrap());

        let invalid_high = tuple! { age: 200i64 };
        assert!(!expr.evaluate(&invalid_high).unwrap());
    }

    #[test]
    fn test_constraint_expression_or() {
        let expr = ConstraintExpression::Or(
            Box::new(ConstraintExpression::Cmp {
                left: "status".to_string(),
                op: CmpOp::Eq,
                right: ValueOrRef::Value(ScalarValue::String("active".to_string())),
            }),
            Box::new(ConstraintExpression::Cmp {
                left: "status".to_string(),
                op: CmpOp::Eq,
                right: ValueOrRef::Value(ScalarValue::String("pending".to_string())),
            }),
        );

        let active = tuple! { status: "active" };
        assert!(expr.evaluate(&active).unwrap());

        let pending = tuple! { status: "pending" };
        assert!(expr.evaluate(&pending).unwrap());

        let inactive = tuple! { status: "inactive" };
        assert!(!expr.evaluate(&inactive).unwrap());
    }

    #[test]
    fn test_constraint_expression_not() {
        let expr = ConstraintExpression::Not(Box::new(ConstraintExpression::Cmp {
            left: "deleted".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Bool(true)),
        }));

        let not_deleted = tuple! { deleted: false };
        assert!(expr.evaluate(&not_deleted).unwrap());

        let deleted = tuple! { deleted: true };
        assert!(!expr.evaluate(&deleted).unwrap());
    }

    #[test]
    fn test_attr_comparison_ge() {
        let expr = ConstraintExpression::Cmp {
            left: "end_date".to_string(),
            op: CmpOp::Ge,
            right: ValueOrRef::Attribute("start_date".to_string()),
        };

        let valid = tuple! { start_date: 100i64, end_date: 200i64 };
        assert!(expr.evaluate(&valid).unwrap());

        let valid_equal = tuple! { start_date: 100i64, end_date: 100i64 };
        assert!(expr.evaluate(&valid_equal).unwrap());

        let invalid = tuple! { start_date: 200i64, end_date: 100i64 };
        assert!(!expr.evaluate(&invalid).unwrap());
    }

    #[test]
    fn test_attr_comparison_eq() {
        let expr = ConstraintExpression::Cmp {
            left: "password".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Attribute("password_confirmation".to_string()),
        };

        let valid = tuple! { password: "secret123", password_confirmation: "secret123" };
        assert!(expr.evaluate(&valid).unwrap());

        let invalid = tuple! { password: "secret123", password_confirmation: "different" };
        assert!(!expr.evaluate(&invalid).unwrap());
    }

    #[test]
    fn test_attr_comparison_lt() {
        let expr = ConstraintExpression::Cmp {
            left: "min_value".to_string(),
            op: CmpOp::Lt,
            right: ValueOrRef::Attribute("max_value".to_string()),
        };

        let valid = tuple! { min_value: 10i64, max_value: 100i64 };
        assert!(expr.evaluate(&valid).unwrap());

        let invalid = tuple! { min_value: 100i64, max_value: 10i64 };
        assert!(!expr.evaluate(&invalid).unwrap());

        let equal = tuple! { min_value: 50i64, max_value: 50i64 };
        assert!(!expr.evaluate(&equal).unwrap());
    }

    #[test]
    fn test_between_inclusive_replacement() {
        // Between replacement: And(Ge(min), Le(max))
        let expr = ConstraintExpression::And(
            Box::new(ConstraintExpression::Cmp {
                left: "age".to_string(),
                op: CmpOp::Ge,
                right: ValueOrRef::Value(ScalarValue::Int(18)),
            }),
            Box::new(ConstraintExpression::Cmp {
                left: "age".to_string(),
                op: CmpOp::Le,
                right: ValueOrRef::Value(ScalarValue::Int(65)),
            }),
        );

        let valid_low = tuple! { age: 18i64 };
        assert!(expr.evaluate(&valid_low).unwrap());

        let valid_mid = tuple! { age: 30i64 };
        assert!(expr.evaluate(&valid_mid).unwrap());

        let valid_high = tuple! { age: 65i64 };
        assert!(expr.evaluate(&valid_high).unwrap());

        let invalid_low = tuple! { age: 17i64 };
        assert!(!expr.evaluate(&invalid_low).unwrap());

        let invalid_high = tuple! { age: 66i64 };
        assert!(!expr.evaluate(&invalid_high).unwrap());
    }

    #[test]
    fn test_in_set_membership() {
        let expr = ConstraintExpression::In(
            "status".to_string(),
            vec![
                ScalarValue::String("active".to_string()),
                ScalarValue::String("pending".to_string()),
                ScalarValue::String("approved".to_string()),
            ]
            .into_iter()
            .collect(),
        );

        let valid_active = tuple! { status: "active" };
        assert!(expr.evaluate(&valid_active).unwrap());

        let valid_pending = tuple! { status: "pending" };
        assert!(expr.evaluate(&valid_pending).unwrap());

        let invalid = tuple! { status: "inactive" };
        assert!(!expr.evaluate(&invalid).unwrap());
    }

    #[test]
    fn test_in_with_integers() {
        let expr = ConstraintExpression::In(
            "priority".to_string(),
            vec![
                ScalarValue::Int(1),
                ScalarValue::Int(2),
                ScalarValue::Int(3),
            ]
            .into_iter()
            .collect(),
        );

        let valid = tuple! { priority: 2i64 };
        assert!(expr.evaluate(&valid).unwrap());

        let invalid = tuple! { priority: 5i64 };
        assert!(!expr.evaluate(&invalid).unwrap());
    }

    #[test]
    fn test_expression_serialization_simple() {
        let expr = ConstraintExpression::Cmp {
            left: "age".to_string(),
            op: CmpOp::Gt,
            right: ValueOrRef::Value(ScalarValue::Int(0)),
        };

        let json = serde_json::to_string(&expr).unwrap();
        let restored: ConstraintExpression = serde_json::from_str(&json).unwrap();
        assert_eq!(expr, restored);

        // Verify it still works after round-trip
        let tuple = tuple! { age: 30i64 };
        assert!(restored.evaluate(&tuple).unwrap());
    }

    #[test]
    fn test_expression_serialization_complex() {
        let expr = ConstraintExpression::And(
            Box::new(ConstraintExpression::Cmp {
                left: "age".to_string(),
                op: CmpOp::Gt,
                right: ValueOrRef::Value(ScalarValue::Int(0)),
            }),
            Box::new(ConstraintExpression::Cmp {
                left: "age".to_string(),
                op: CmpOp::Lt,
                right: ValueOrRef::Value(ScalarValue::Int(150)),
            }),
        );

        let json = serde_json::to_string(&expr).unwrap();
        let restored: ConstraintExpression = serde_json::from_str(&json).unwrap();
        assert_eq!(expr, restored);

        // Verify it still works after round-trip
        let valid = tuple! { age: 30i64 };
        assert!(restored.evaluate(&valid).unwrap());

        let invalid = tuple! { age: 200i64 };
        assert!(!restored.evaluate(&invalid).unwrap());
    }

    #[test]
    fn test_expression_serialization_attr_cmp() {
        let expr = ConstraintExpression::Cmp {
            left: "end_date".to_string(),
            op: CmpOp::Ge,
            right: ValueOrRef::Attribute("start_date".to_string()),
        };

        let json = serde_json::to_string(&expr).unwrap();
        let restored: ConstraintExpression = serde_json::from_str(&json).unwrap();
        assert_eq!(expr, restored);
    }

    #[test]
    fn test_type_mismatch_in_comparison() {
        // Comparing Int with String should fail with TypeMismatch
        let expr = ConstraintExpression::Cmp {
            left: "age".to_string(),
            op: CmpOp::Gt,
            right: ValueOrRef::Value(ScalarValue::String("not_a_number".to_string())),
        };

        let tuple = tuple! { age: 30i64 };
        let result = expr.evaluate(&tuple);
        assert!(result.is_err());
        assert!(matches!(result, Err(ExpressionError::TypeMismatch(_, _))));
    }

    #[test]
    fn test_type_mismatch_in_lt() {
        let expr = ConstraintExpression::Cmp {
            left: "name".to_string(),
            op: CmpOp::Lt,
            right: ValueOrRef::Value(ScalarValue::Int(42)),
        };

        let tuple = tuple! { name: "Alice" };
        let result = expr.evaluate(&tuple);
        assert!(result.is_err());
        assert!(matches!(result, Err(ExpressionError::TypeMismatch(_, _))));
    }

    #[test]
    fn test_type_mismatch_attr_to_attr() {
        let expr = ConstraintExpression::Cmp {
            left: "age".to_string(),
            op: CmpOp::Lt,
            right: ValueOrRef::Attribute("name".to_string()),
        };

        let tuple = tuple! { age: 30i64, name: "Alice" };
        let result = expr.evaluate(&tuple);
        assert!(result.is_err());
        assert!(matches!(result, Err(ExpressionError::TypeMismatch(_, _))));
    }

    #[test]
    fn test_referenced_attributes() {
        // Simple comparison
        let expr1 = ConstraintExpression::Cmp {
            left: "age".to_string(),
            op: CmpOp::Gt,
            right: ValueOrRef::Value(ScalarValue::Int(18)),
        };
        let attrs1 = expr1.referenced_attributes();
        assert_eq!(attrs1.len(), 1);
        assert!(attrs1.contains("age"));

        // Attribute to attribute comparison
        let expr2 = ConstraintExpression::Cmp {
            left: "start_date".to_string(),
            op: CmpOp::Le,
            right: ValueOrRef::Attribute("end_date".to_string()),
        };
        let attrs2 = expr2.referenced_attributes();
        assert_eq!(attrs2.len(), 2);
        assert!(attrs2.contains("start_date"));
        assert!(attrs2.contains("end_date"));

        // Nested expression
        let expr3 = ConstraintExpression::And(Box::new(expr1), Box::new(expr2));
        let attrs3 = expr3.referenced_attributes();
        assert_eq!(attrs3.len(), 3);
        assert!(attrs3.contains("age"));
        assert!(attrs3.contains("start_date"));
        assert!(attrs3.contains("end_date"));

        // Like expression
        let expr4 = ConstraintExpression::Like("name".to_string(), "A%".to_string());
        let attrs4 = expr4.referenced_attributes();
        assert_eq!(attrs4.len(), 1);
        assert!(attrs4.contains("name"));
    }
}

#[cfg(test)]
mod like_tests {
    use super::*;
    use crate::tuple;

    #[test]
    fn test_like_exact_match() {
        let expr = ConstraintExpression::Like("name".to_string(), "Alice".to_string());
        let tuple = tuple! { name: "Alice" };
        assert!(expr.evaluate(&tuple).unwrap());

        let tuple_bad = tuple! { name: "Bob" };
        assert!(!expr.evaluate(&tuple_bad).unwrap());
    }

    #[test]
    fn test_like_wildcard_percent() {
        let expr = ConstraintExpression::Like("name".to_string(), "Al%".to_string());

        assert!(expr.evaluate(&tuple! { name: "Alice" }).unwrap());
        assert!(expr.evaluate(&tuple! { name: "Alan" }).unwrap());
        assert!(expr.evaluate(&tuple! { name: "Al" }).unwrap()); // Empty match
        assert!(!expr.evaluate(&tuple! { name: "Bob" }).unwrap());
    }

    #[test]
    fn test_like_wildcard_underscore() {
        let expr = ConstraintExpression::Like("code".to_string(), "A_C".to_string());

        assert!(expr.evaluate(&tuple! { code: "ABC" }).unwrap());
        assert!(expr.evaluate(&tuple! { code: "ADC" }).unwrap());
        assert!(!expr.evaluate(&tuple! { code: "AC" }).unwrap());
        assert!(!expr.evaluate(&tuple! { code: "ABBC" }).unwrap());
    }

    #[test]
    fn test_like_mixed_wildcards() {
        // Matches "data_" followed by anything, then ".csv"
        // Note: "_" is a wildcard, so it matches any character, including "_"
        let expr = ConstraintExpression::Like("file".to_string(), "data_%.csv".to_string());

        assert!(expr.evaluate(&tuple! { file: "data_2024.csv" }).unwrap());
        assert!(expr.evaluate(&tuple! { file: "data_final.csv" }).unwrap());

        // "dataA.csv" should match: "data" matches "data", "_" matches "A", "%" matches empty, ".csv" matches
        assert!(expr.evaluate(&tuple! { file: "dataA.csv" }).unwrap());

        // "data.csv" should NOT match: "data" matches, "_" matches ".", "%" matches empty, ".csv" needs ".csv" but remaining is "csv". "." != "c"
        assert!(!expr.evaluate(&tuple! { file: "data.csv" }).unwrap());
    }

    #[test]
    fn test_like_unicode() {
        let expr = ConstraintExpression::Like("word".to_string(), "caf_".to_string());

        // 'é' is one char
        assert!(expr.evaluate(&tuple! { word: "café" }).unwrap());
        // "caff" is 4 chars, so it matches "caf_"
        assert!(expr.evaluate(&tuple! { word: "caff" }).unwrap());
        // "caf" is 3 chars, so it does NOT match "caf_"
        assert!(!expr.evaluate(&tuple! { word: "caf" }).unwrap());

        // Ensure we handle multibyte chars correctly in pattern too
        let expr_unicode_pattern =
            ConstraintExpression::Like("word".to_string(), "café%".to_string());
        assert!(
            expr_unicode_pattern
                .evaluate(&tuple! { word: "café au lait" })
                .unwrap()
        );
    }

    #[test]
    fn test_like_edge_cases() {
        // Empty pattern matches only empty string
        let expr_empty = ConstraintExpression::Like("text".to_string(), "".to_string());
        assert!(expr_empty.evaluate(&tuple! { text: "" }).unwrap());
        assert!(!expr_empty.evaluate(&tuple! { text: "a" }).unwrap());

        // Pattern % matches everything
        let expr_all = ConstraintExpression::Like("text".to_string(), "%".to_string());
        assert!(expr_all.evaluate(&tuple! { text: "" }).unwrap());
        assert!(expr_all.evaluate(&tuple! { text: "anything" }).unwrap());

        // Pattern with just wildcards
        let expr_wild = ConstraintExpression::Like("text".to_string(), "_%_".to_string());
        assert!(expr_wild.evaluate(&tuple! { text: "ab" }).unwrap()); // min 2 chars
        assert!(!expr_wild.evaluate(&tuple! { text: "a" }).unwrap());
    }

    #[test]
    fn test_like_type_mismatch() {
        let expr = ConstraintExpression::Like("age".to_string(), "1%".to_string());
        let tuple = tuple! { age: 10i64 };
        let result = expr.evaluate(&tuple);
        assert!(result.is_err());
        assert!(matches!(result, Err(ExpressionError::TypeMismatch(_, _))));
    }
}

#[cfg(test)]
mod recursion_tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_constraint_expression_deserialization_depth_limit() {
        let mut q = ConstraintExpression::Not(Box::new(ConstraintExpression::In(
            "test".to_string(),
            HashSet::new(),
        )));
        for _ in 0..100 {
            q = ConstraintExpression::Not(Box::new(q));
        }

        // Serialize with postcard
        let bytes = postcard::to_stdvec(&q).unwrap();

        // Deserialize
        std::mem::forget(q); // Prevent Drop stack overflow
        let result: Result<ConstraintExpression, _> = postcard::from_bytes(&bytes);

        assert!(
            result.is_err(),
            "Deserialization should fail due to recursion limit"
        );

        if let Ok(q2) = result {
            std::mem::forget(q2); // Should not reach here, but prevent Drop if it does
        }
    }
}
