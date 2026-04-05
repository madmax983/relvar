//! Pre-compiled Execution Plans for Constraints.
//!
//! This module contains `PreparedConstraint`, which is an optimized, read-only
//! version of a `ConstraintExpression`. By pre-compiling `IN` lists to HashSets
//! and string patterns to compiled regexers, evaluating these constraints during
//! filtering loops is O(1) instead of O(N) per tuple.
use super::expression::{CmpOp, ConstraintExpression, ExpressionError, ValueOrRef};
use crate::values::{ScalarValue, Tuple};
use std::collections::HashSet;

/// A prepared, optimized version of `ConstraintExpression`.
///
/// This structure is designed for efficient evaluation, particularly for
/// operations that benefit from pre-computation or pre-allocation, such as
/// `IN` (using `HashSet`) and `LIKE` (using `Vec<char>`).
#[derive(Debug, Clone)]
pub enum PreparedConstraintExpression {
    /// Comparison: left_attr op right_value_or_ref
    Cmp {
        /// Left attribute name
        left: String,
        /// Comparison operator
        op: CmpOp,
        /// Right operand (value or attribute reference)
        right: ValueOrRef,
    },

    /// Logical AND: both expressions must be true
    And(
        Box<PreparedConstraintExpression>,
        Box<PreparedConstraintExpression>,
    ),
    /// Logical OR: at least one expression must be true
    Or(
        Box<PreparedConstraintExpression>,
        Box<PreparedConstraintExpression>,
    ),
    /// Logical NOT: inverts the expression
    Not(Box<PreparedConstraintExpression>),

    /// Set membership: attribute IN (`HashSet<ScalarValue>`)
    /// Optimized for O(1) lookup.
    In(String, HashSet<ScalarValue>),

    /// Pattern matching: attribute LIKE pattern
    /// Optimized by pre-parsing pattern into chars.
    Like(String, Vec<char>),
}

impl PreparedConstraintExpression {
    /// Evaluates this prepared expression against a tuple.
    pub fn evaluate(&self, tuple: &Tuple) -> Result<bool, ExpressionError> {
        match self {
            PreparedConstraintExpression::Cmp { left, op, right } => {
                let (left_val, right_val) =
                    ConstraintExpression::get_comparison_operands(tuple, left, right)?;

                Self::evaluate_cmp(left_val, op, right_val)
            }
            PreparedConstraintExpression::And(left, right) => {
                Ok(left.evaluate(tuple)? && right.evaluate(tuple)?)
            }
            PreparedConstraintExpression::Or(left, right) => {
                Ok(left.evaluate(tuple)? || right.evaluate(tuple)?)
            }
            PreparedConstraintExpression::Not(expr) => Ok(!expr.evaluate(tuple)?),
            PreparedConstraintExpression::In(attr, values) => {
                let tuple_value = tuple
                    .get(attr)
                    .ok_or_else(|| ExpressionError::AttributeNotFound(attr.clone()))?;
                Ok(values.contains(tuple_value))
            }
            PreparedConstraintExpression::Like(attr, pattern_chars) => {
                let tuple_value = tuple
                    .get(attr)
                    .ok_or_else(|| ExpressionError::AttributeNotFound(attr.clone()))?;

                let text = match tuple_value {
                    ScalarValue::String(s) => s,
                    _ => {
                        return Err(ExpressionError::TypeMismatch(
                            "String".to_string(),
                            format!("{:?}", tuple_value.scalar_type()),
                        ));
                    }
                };

                // Use the shared helper from ConstraintExpression
                Ok(ConstraintExpression::matches_pattern_chars(
                    text,
                    pattern_chars,
                ))
            }
        }
    }

    /// Evaluates a comparison operation.
    fn evaluate_cmp(
        left_val: &ScalarValue,
        op: &CmpOp,
        right_val: &ScalarValue,
    ) -> Result<bool, ExpressionError> {
        match op {
            CmpOp::Eq => Ok(left_val == right_val),
            CmpOp::Ne => Ok(left_val != right_val),
            CmpOp::Lt => Ok(left_val < right_val),
            CmpOp::Le => Ok(left_val <= right_val),
            CmpOp::Gt => Ok(left_val > right_val),
            CmpOp::Ge => Ok(left_val >= right_val),
        }
    }
}

impl From<ConstraintExpression> for PreparedConstraintExpression {
    fn from(expr: ConstraintExpression) -> Self {
        match expr {
            ConstraintExpression::Cmp { left, op, right } => {
                PreparedConstraintExpression::Cmp { left, op, right }
            }
            ConstraintExpression::And(left, right) => PreparedConstraintExpression::And(
                Box::new((*left).into()),
                Box::new((*right).into()),
            ),
            ConstraintExpression::Or(left, right) => PreparedConstraintExpression::Or(
                Box::new((*left).into()),
                Box::new((*right).into()),
            ),
            ConstraintExpression::Not(expr) => {
                PreparedConstraintExpression::Not(Box::new((*expr).into()))
            }
            ConstraintExpression::In(attr, values) => {
                let value_set: HashSet<_> = values.into_iter().collect();
                PreparedConstraintExpression::In(attr, value_set)
            }
            ConstraintExpression::Like(attr, pattern) => {
                let pattern_chars: Vec<char> = pattern.chars().collect();
                PreparedConstraintExpression::Like(attr, pattern_chars)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;

    #[test]
    fn test_cmp_operations() {
        // Test all comparison operators that were previously uncovered
        let ops = vec![
            (CmpOp::Ne, 10i64, 20i64, true),
            (CmpOp::Ne, 10i64, 10i64, false),
            (CmpOp::Lt, 10i64, 20i64, true),
            (CmpOp::Lt, 20i64, 10i64, false),
            (CmpOp::Le, 10i64, 20i64, true),
            (CmpOp::Le, 10i64, 10i64, true),
            (CmpOp::Le, 20i64, 10i64, false),
            (CmpOp::Gt, 20i64, 10i64, true),
            (CmpOp::Gt, 10i64, 20i64, false),
            (CmpOp::Ge, 20i64, 10i64, true),
            (CmpOp::Ge, 10i64, 10i64, true),
            (CmpOp::Ge, 10i64, 20i64, false),
        ];

        for (op, left_val, right_val, expected) in ops {
            let expr = ConstraintExpression::Cmp {
                left: "val".to_string(),
                op,
                right: ValueOrRef::Value(ScalarValue::Int(right_val)),
            };

            let prepared = expr.prepare();
            let t = tuple! { val: left_val };

            assert_eq!(
                prepared.evaluate(&t).unwrap(),
                expected,
                "Failed for {:?}",
                op
            );
        }
    }

    #[test]
    fn test_logical_operations() {
        // Test OR
        let expr_or = ConstraintExpression::Or(
            Box::new(ConstraintExpression::Cmp {
                left: "a".to_string(),
                op: CmpOp::Eq,
                right: ValueOrRef::Value(ScalarValue::Int(1)),
            }),
            Box::new(ConstraintExpression::Cmp {
                left: "b".to_string(),
                op: CmpOp::Eq,
                right: ValueOrRef::Value(ScalarValue::Int(2)),
            }),
        );

        let prep_or = expr_or.prepare();
        if let PreparedConstraintExpression::Or(l, r) = &prep_or {
            assert!(matches!(**l, PreparedConstraintExpression::Cmp { .. }));
            assert!(matches!(**r, PreparedConstraintExpression::Cmp { .. }));
        } else {
            panic!("Expected PreparedConstraintExpression::Or");
        }

        // true || false = true
        assert!(prep_or.evaluate(&tuple! { a: 1i64, b: 0i64 }).unwrap());
        // false || true = true
        assert!(prep_or.evaluate(&tuple! { a: 0i64, b: 2i64 }).unwrap());
        // false || false = false
        assert!(!prep_or.evaluate(&tuple! { a: 0i64, b: 0i64 }).unwrap());

        // Test NOT
        let expr_not = ConstraintExpression::Not(Box::new(ConstraintExpression::Cmp {
            left: "a".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(1)),
        }));

        let prep_not = expr_not.prepare();
        if let PreparedConstraintExpression::Not(inner) = &prep_not {
            assert!(matches!(**inner, PreparedConstraintExpression::Cmp { .. }));
        } else {
            panic!("Expected PreparedConstraintExpression::Not");
        }

        assert!(!prep_not.evaluate(&tuple! { a: 1i64 }).unwrap());
        assert!(prep_not.evaluate(&tuple! { a: 2i64 }).unwrap());
    }

    #[test]
    fn test_like_type_mismatch() {
        let expr = ConstraintExpression::Like("val".to_string(), "A%".to_string());
        let prepared = expr.prepare();

        // Pass an integer instead of string
        let t = tuple! { val: 42i64 };
        let res = prepared.evaluate(&t);

        assert!(res.is_err());
        match res.unwrap_err() {
            ExpressionError::TypeMismatch(expected, got) => {
                assert_eq!(expected, "String");
                assert_eq!(got, "Int");
            }
            _ => panic!("Expected TypeMismatch error"),
        }
    }

    #[test]
    fn test_in_optimization() {
        // Create an IN expression with a large list
        let mut values = Vec::new();
        for i in 0..100 {
            values.push(ScalarValue::Int(i));
        }
        let expr = ConstraintExpression::In("id".to_string(), values);

        // Prepare it
        let prepared = expr.prepare();

        // Verify it was converted to PreparedConstraintExpression::In with a HashSet
        match &prepared {
            PreparedConstraintExpression::In(attr, set) => {
                assert_eq!(attr, "id");
                assert_eq!(set.len(), 100);
                assert!(set.contains(&ScalarValue::Int(50)));
            }
            _ => panic!("Expected PreparedConstraintExpression::In"),
        }

        // Verify evaluation
        let t1 = tuple! { id: 50i64 };
        assert!(prepared.evaluate(&t1).unwrap());

        let t2 = tuple! { id: 200i64 };
        assert!(!prepared.evaluate(&t2).unwrap());
    }

    #[test]
    fn test_like_optimization() {
        let expr = ConstraintExpression::Like("name".to_string(), "A%".to_string());

        // Prepare it
        let prepared = expr.prepare();

        // Verify it was converted to PreparedConstraintExpression::Like with Vec<char>
        match &prepared {
            PreparedConstraintExpression::Like(attr, chars) => {
                assert_eq!(attr, "name");
                assert_eq!(chars.len(), 2);
                assert_eq!(chars[0], 'A');
                assert_eq!(chars[1], '%');
            }
            _ => panic!("Expected PreparedConstraintExpression::Like"),
        }

        // Verify evaluation
        let t1 = tuple! { name: "Alice" };
        assert!(prepared.evaluate(&t1).unwrap());

        let t2 = tuple! { name: "Bob" };
        assert!(!prepared.evaluate(&t2).unwrap());
    }

    #[test]
    fn test_recursive_preparation() {
        let expr = ConstraintExpression::And(
            Box::new(ConstraintExpression::Cmp {
                left: "age".to_string(),
                op: CmpOp::Gt,
                right: ValueOrRef::Value(ScalarValue::Int(18)),
            }),
            Box::new(ConstraintExpression::In(
                "role".to_string(),
                vec![ScalarValue::String("admin".to_string())],
            )),
        );

        let prepared = expr.prepare();

        match &prepared {
            PreparedConstraintExpression::And(left, right) => {
                assert!(matches!(**left, PreparedConstraintExpression::Cmp { .. }));
                assert!(matches!(**right, PreparedConstraintExpression::In { .. }));
            }
            _ => panic!("Expected PreparedConstraintExpression::And"),
        }

        let valid = tuple! { age: 20i64, role: "admin" };
        assert!(prepared.evaluate(&valid).unwrap());

        let invalid = tuple! { age: 10i64, role: "admin" };
        assert!(!prepared.evaluate(&invalid).unwrap());
    }
}
