//! Constraint expression DSL for serializable predicates.
//!
//! This module provides a declarative expression language for defining
//! constraints that can be persisted to storage and evaluated at runtime.
//!
//! # TTM Compliance
//!
//! Implements TTM RM Prescription 9 (general integrity constraints)
//! with serializable predicates that can be stored in the system catalog.
//!
//! # Example
//!
//! ```
//! use relvar_core::constraints::expression::{ConstraintExpression, ValueOrRef};
//! use relvar_core::values::ScalarValue;
//! use relvar_core::tuple;
//!
//! // Create a constraint: salary > 0
//! let expr = ConstraintExpression::Gt(
//!     "salary".to_string(),
//!     ValueOrRef::Value(ScalarValue::Int(0)),
//! );
//!
//! let valid_tuple = tuple! { salary: 50000i64 };
//! assert!(expr.evaluate(&valid_tuple).unwrap());
//!
//! let invalid_tuple = tuple! { salary: -100i64 };
//! assert!(!expr.evaluate(&invalid_tuple).unwrap());
//! ```

use crate::values::{ScalarValue, Tuple};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during constraint expression evaluation.
#[derive(Debug, Error)]
pub enum ExpressionError {
    /// Attribute referenced in expression not found in tuple.
    #[error("Attribute '{0}' not found in tuple")]
    AttributeNotFound(String),

    /// Type mismatch during comparison.
    #[error("Type mismatch in expression: cannot compare {0:?} with {1:?}")]
    TypeMismatch(String, String),

    /// Invalid comparison operation for the given types.
    #[error("Invalid comparison: {0}")]
    InvalidComparison(String),
}

/// Represents a value or attribute reference in a constraint expression.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ValueOrRef {
    /// A literal scalar value.
    Value(ScalarValue),
    /// A reference to an attribute by name.
    Attribute(String),
}

/// Comparison operators for constraint expressions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CmpOp {
    /// Equal (=)
    Eq,
    /// Not equal (≠)
    Ne,
    /// Less than (<)
    Lt,
    /// Less than or equal (≤)
    Le,
    /// Greater than (>)
    Gt,
    /// Greater than or equal (≥)
    Ge,
}

/// Constraint expression DSL for serializable predicates.
///
/// This enum represents a tree of expressions that can be evaluated
/// against a tuple to produce a boolean result. Expressions can be
/// serialized for storage in the system catalog.
///
/// # Supported Operations
///
/// - **Comparisons**: Eq, Ne, Lt, Le, Gt, Ge
/// - **Logical**: And, Or, Not
/// - **Attribute comparison**: Compare two attributes
/// - **Range**: Between
/// - **Set membership**: In
/// - **Pattern matching**: Like
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConstraintExpression {
    /// Equality comparison: attribute = value
    Eq(String, ValueOrRef),
    /// Inequality comparison: attribute ≠ value
    Ne(String, ValueOrRef),
    /// Less than: attribute < value
    Lt(String, ValueOrRef),
    /// Less than or equal: attribute ≤ value
    Le(String, ValueOrRef),
    /// Greater than: attribute > value
    Gt(String, ValueOrRef),
    /// Greater than or equal: attribute ≥ value
    Ge(String, ValueOrRef),

    /// Logical AND: both expressions must be true
    And(Box<ConstraintExpression>, Box<ConstraintExpression>),
    /// Logical OR: at least one expression must be true
    Or(Box<ConstraintExpression>, Box<ConstraintExpression>),
    /// Logical NOT: inverts the expression
    Not(Box<ConstraintExpression>),

    /// Attribute-to-attribute comparison: left_attr op right_attr
    AttrCmp {
        /// Left attribute name
        left: String,
        /// Comparison operator
        op: CmpOp,
        /// Right attribute name
        right: String,
    },

    /// Range check: value BETWEEN min AND max (inclusive)
    Between(String, ScalarValue, ScalarValue),

    /// Set membership: attribute IN (value1, value2, ...)
    In(String, Vec<ScalarValue>),

    /// Pattern matching: attribute LIKE pattern
    /// (Simple SQL-style patterns: % for any chars, _ for single char)
    Like(String, String),
}

impl ConstraintExpression {
    /// Evaluates this expression against a tuple.
    ///
    /// Returns `true` if the tuple satisfies the constraint,
    /// `false` otherwise.
    ///
    /// # Errors
    ///
    /// Returns `Err` if:
    /// - A referenced attribute is not found in the tuple
    /// - A type mismatch occurs during comparison
    /// - An invalid comparison is attempted
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::constraints::expression::{ConstraintExpression, ValueOrRef};
    /// use relvar_core::values::ScalarValue;
    /// use relvar_core::tuple;
    ///
    /// let expr = ConstraintExpression::Gt(
    ///     "age".to_string(),
    ///     ValueOrRef::Value(ScalarValue::Int(0)),
    /// );
    ///
    /// let tuple = tuple! { age: 30i64 };
    /// assert!(expr.evaluate(&tuple).unwrap());
    /// ```
    pub fn evaluate(&self, tuple: &Tuple) -> Result<bool, ExpressionError> {
        match self {
            ConstraintExpression::Eq(attr, value_or_ref) => {
                let tuple_value = tuple
                    .get(attr)
                    .ok_or_else(|| ExpressionError::AttributeNotFound(attr.clone()))?;
                let compare_value = Self::resolve_value_or_ref(tuple, value_or_ref)?;
                Ok(tuple_value == compare_value)
            }
            ConstraintExpression::Ne(attr, value_or_ref) => {
                let tuple_value = tuple
                    .get(attr)
                    .ok_or_else(|| ExpressionError::AttributeNotFound(attr.clone()))?;
                let compare_value = Self::resolve_value_or_ref(tuple, value_or_ref)?;
                Ok(tuple_value != compare_value)
            }
            ConstraintExpression::Lt(attr, value_or_ref) => {
                let tuple_value = tuple
                    .get(attr)
                    .ok_or_else(|| ExpressionError::AttributeNotFound(attr.clone()))?;
                let compare_value = Self::resolve_value_or_ref(tuple, value_or_ref)?;
                Ok(tuple_value < compare_value)
            }
            ConstraintExpression::Le(attr, value_or_ref) => {
                let tuple_value = tuple
                    .get(attr)
                    .ok_or_else(|| ExpressionError::AttributeNotFound(attr.clone()))?;
                let compare_value = Self::resolve_value_or_ref(tuple, value_or_ref)?;
                Ok(tuple_value <= compare_value)
            }
            ConstraintExpression::Gt(attr, value_or_ref) => {
                let tuple_value = tuple
                    .get(attr)
                    .ok_or_else(|| ExpressionError::AttributeNotFound(attr.clone()))?;
                let compare_value = Self::resolve_value_or_ref(tuple, value_or_ref)?;
                Ok(tuple_value > compare_value)
            }
            ConstraintExpression::Ge(attr, value_or_ref) => {
                let tuple_value = tuple
                    .get(attr)
                    .ok_or_else(|| ExpressionError::AttributeNotFound(attr.clone()))?;
                let compare_value = Self::resolve_value_or_ref(tuple, value_or_ref)?;
                Ok(tuple_value >= compare_value)
            }
            ConstraintExpression::And(left, right) => {
                Ok(left.evaluate(tuple)? && right.evaluate(tuple)?)
            }
            ConstraintExpression::Or(left, right) => {
                Ok(left.evaluate(tuple)? || right.evaluate(tuple)?)
            }
            ConstraintExpression::Not(expr) => Ok(!expr.evaluate(tuple)?),
            ConstraintExpression::AttrCmp { left, op, right } => {
                let left_value = tuple
                    .get(left)
                    .ok_or_else(|| ExpressionError::AttributeNotFound(left.clone()))?;
                let right_value = tuple
                    .get(right)
                    .ok_or_else(|| ExpressionError::AttributeNotFound(right.clone()))?;

                match op {
                    CmpOp::Eq => Ok(left_value == right_value),
                    CmpOp::Ne => Ok(left_value != right_value),
                    CmpOp::Lt => Ok(left_value < right_value),
                    CmpOp::Le => Ok(left_value <= right_value),
                    CmpOp::Gt => Ok(left_value > right_value),
                    CmpOp::Ge => Ok(left_value >= right_value),
                }
            }
            ConstraintExpression::Between(attr, min, max) => {
                let tuple_value = tuple
                    .get(attr)
                    .ok_or_else(|| ExpressionError::AttributeNotFound(attr.clone()))?;
                Ok(tuple_value >= min && tuple_value <= max)
            }
            ConstraintExpression::In(attr, values) => {
                let tuple_value = tuple
                    .get(attr)
                    .ok_or_else(|| ExpressionError::AttributeNotFound(attr.clone()))?;
                Ok(values.contains(tuple_value))
            }
            ConstraintExpression::Like(attr, pattern) => {
                let tuple_value = tuple
                    .get(attr)
                    .ok_or_else(|| ExpressionError::AttributeNotFound(attr.clone()))?;

                // Extract string value
                let string_value = match tuple_value {
                    ScalarValue::String(s) => s,
                    _ => {
                        return Err(ExpressionError::TypeMismatch(
                            "String".to_string(),
                            format!("{:?}", tuple_value.scalar_type()),
                        ));
                    }
                };

                // Simple LIKE pattern matching: % = any chars, _ = single char
                Ok(Self::matches_pattern(string_value, pattern))
            }
        }
    }

    /// Simple SQL LIKE pattern matching.
    /// % matches any sequence of characters
    /// _ matches any single character
    fn matches_pattern(text: &str, pattern: &str) -> bool {
        let text_chars: Vec<char> = text.chars().collect();
        let pattern_chars: Vec<char> = pattern.chars().collect();

        Self::matches_pattern_recursive(&text_chars, &pattern_chars, 0, 0)
    }

    fn matches_pattern_recursive(
        text: &[char],
        pattern: &[char],
        text_idx: usize,
        pattern_idx: usize,
    ) -> bool {
        // Base cases
        if pattern_idx >= pattern.len() {
            return text_idx >= text.len();
        }

        if text_idx >= text.len() {
            // Remaining pattern must be all %
            return pattern[pattern_idx..].iter().all(|&c| c == '%');
        }

        match pattern[pattern_idx] {
            '%' => {
                // Try matching zero or more characters
                Self::matches_pattern_recursive(text, pattern, text_idx, pattern_idx + 1)
                    || Self::matches_pattern_recursive(text, pattern, text_idx + 1, pattern_idx)
            }
            '_' => {
                // Match exactly one character
                Self::matches_pattern_recursive(text, pattern, text_idx + 1, pattern_idx + 1)
            }
            c => {
                // Match literal character
                if text[text_idx] == c {
                    Self::matches_pattern_recursive(text, pattern, text_idx + 1, pattern_idx + 1)
                } else {
                    false
                }
            }
        }
    }

    /// Helper to resolve a ValueOrRef to a concrete ScalarValue from the tuple.
    fn resolve_value_or_ref<'a>(
        tuple: &'a Tuple,
        value_or_ref: &'a ValueOrRef,
    ) -> Result<&'a ScalarValue, ExpressionError> {
        match value_or_ref {
            ValueOrRef::Value(v) => Ok(v),
            ValueOrRef::Attribute(attr) => tuple
                .get(attr)
                .ok_or_else(|| ExpressionError::AttributeNotFound(attr.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;

    #[test]
    fn test_constraint_expression_eq() {
        let expr = ConstraintExpression::Eq(
            "salary".to_string(),
            ValueOrRef::Value(ScalarValue::Int(50000)),
        );

        let tuple = tuple! { salary: 50000i64 };
        assert!(expr.evaluate(&tuple).unwrap());

        let tuple_bad = tuple! { salary: 30000i64 };
        assert!(!expr.evaluate(&tuple_bad).unwrap());
    }

    #[test]
    fn test_constraint_expression_ne() {
        let expr = ConstraintExpression::Ne(
            "status".to_string(),
            ValueOrRef::Value(ScalarValue::String("inactive".to_string())),
        );

        let tuple = tuple! { status: "active" };
        assert!(expr.evaluate(&tuple).unwrap());

        let tuple_bad = tuple! { status: "inactive" };
        assert!(!expr.evaluate(&tuple_bad).unwrap());
    }

    #[test]
    fn test_constraint_expression_lt() {
        let expr =
            ConstraintExpression::Lt("age".to_string(), ValueOrRef::Value(ScalarValue::Int(65)));

        let tuple = tuple! { age: 30i64 };
        assert!(expr.evaluate(&tuple).unwrap());

        let tuple_bad = tuple! { age: 70i64 };
        assert!(!expr.evaluate(&tuple_bad).unwrap());

        let tuple_equal = tuple! { age: 65i64 };
        assert!(!expr.evaluate(&tuple_equal).unwrap());
    }

    #[test]
    fn test_constraint_expression_le() {
        let expr = ConstraintExpression::Le(
            "score".to_string(),
            ValueOrRef::Value(ScalarValue::Int(100)),
        );

        let tuple = tuple! { score: 95i64 };
        assert!(expr.evaluate(&tuple).unwrap());

        let tuple_equal = tuple! { score: 100i64 };
        assert!(expr.evaluate(&tuple_equal).unwrap());

        let tuple_bad = tuple! { score: 105i64 };
        assert!(!expr.evaluate(&tuple_bad).unwrap());
    }

    #[test]
    fn test_constraint_expression_gt() {
        let expr =
            ConstraintExpression::Gt("salary".to_string(), ValueOrRef::Value(ScalarValue::Int(0)));

        let tuple = tuple! { salary: 50000i64 };
        assert!(expr.evaluate(&tuple).unwrap());

        let tuple_bad = tuple! { salary: -100i64 };
        assert!(!expr.evaluate(&tuple_bad).unwrap());

        let tuple_equal = tuple! { salary: 0i64 };
        assert!(!expr.evaluate(&tuple_equal).unwrap());
    }

    #[test]
    fn test_constraint_expression_ge() {
        let expr = ConstraintExpression::Ge(
            "balance".to_string(),
            ValueOrRef::Value(ScalarValue::Int(0)),
        );

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
            Box::new(ConstraintExpression::Gt(
                "age".to_string(),
                ValueOrRef::Value(ScalarValue::Int(0)),
            )),
            Box::new(ConstraintExpression::Lt(
                "age".to_string(),
                ValueOrRef::Value(ScalarValue::Int(150)),
            )),
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
            Box::new(ConstraintExpression::Eq(
                "status".to_string(),
                ValueOrRef::Value(ScalarValue::String("active".to_string())),
            )),
            Box::new(ConstraintExpression::Eq(
                "status".to_string(),
                ValueOrRef::Value(ScalarValue::String("pending".to_string())),
            )),
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
        let expr = ConstraintExpression::Not(Box::new(ConstraintExpression::Eq(
            "deleted".to_string(),
            ValueOrRef::Value(ScalarValue::Bool(true)),
        )));

        let not_deleted = tuple! { deleted: false };
        assert!(expr.evaluate(&not_deleted).unwrap());

        let deleted = tuple! { deleted: true };
        assert!(!expr.evaluate(&deleted).unwrap());
    }

    #[test]
    fn test_attr_comparison_ge() {
        let expr = ConstraintExpression::AttrCmp {
            left: "end_date".to_string(),
            op: CmpOp::Ge,
            right: "start_date".to_string(),
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
        let expr = ConstraintExpression::AttrCmp {
            left: "password".to_string(),
            op: CmpOp::Eq,
            right: "password_confirmation".to_string(),
        };

        let valid = tuple! { password: "secret123", password_confirmation: "secret123" };
        assert!(expr.evaluate(&valid).unwrap());

        let invalid = tuple! { password: "secret123", password_confirmation: "different" };
        assert!(!expr.evaluate(&invalid).unwrap());
    }

    #[test]
    fn test_attr_comparison_lt() {
        let expr = ConstraintExpression::AttrCmp {
            left: "min_value".to_string(),
            op: CmpOp::Lt,
            right: "max_value".to_string(),
        };

        let valid = tuple! { min_value: 10i64, max_value: 100i64 };
        assert!(expr.evaluate(&valid).unwrap());

        let invalid = tuple! { min_value: 100i64, max_value: 10i64 };
        assert!(!expr.evaluate(&invalid).unwrap());

        let equal = tuple! { min_value: 50i64, max_value: 50i64 };
        assert!(!expr.evaluate(&equal).unwrap());
    }

    #[test]
    fn test_between_inclusive() {
        let expr = ConstraintExpression::Between(
            "age".to_string(),
            ScalarValue::Int(18),
            ScalarValue::Int(65),
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
            ],
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
            ],
        );

        let valid = tuple! { priority: 2i64 };
        assert!(expr.evaluate(&valid).unwrap());

        let invalid = tuple! { priority: 5i64 };
        assert!(!expr.evaluate(&invalid).unwrap());
    }

    #[test]
    fn test_expression_serialization_simple() {
        let expr =
            ConstraintExpression::Gt("age".to_string(), ValueOrRef::Value(ScalarValue::Int(0)));

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
            Box::new(ConstraintExpression::Gt(
                "age".to_string(),
                ValueOrRef::Value(ScalarValue::Int(0)),
            )),
            Box::new(ConstraintExpression::Lt(
                "age".to_string(),
                ValueOrRef::Value(ScalarValue::Int(150)),
            )),
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
        let expr = ConstraintExpression::AttrCmp {
            left: "end_date".to_string(),
            op: CmpOp::Ge,
            right: "start_date".to_string(),
        };

        let json = serde_json::to_string(&expr).unwrap();
        let restored: ConstraintExpression = serde_json::from_str(&json).unwrap();
        assert_eq!(expr, restored);
    }

    #[test]
    fn test_expression_serialization_between() {
        let expr = ConstraintExpression::Between(
            "score".to_string(),
            ScalarValue::Int(0),
            ScalarValue::Int(100),
        );

        let json = serde_json::to_string(&expr).unwrap();
        let restored: ConstraintExpression = serde_json::from_str(&json).unwrap();
        assert_eq!(expr, restored);
    }
}
