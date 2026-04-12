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
//! use relvar_core::{ConstraintExpression, CmpOp, ValueOrRef};
//! use relvar_core::values::ScalarValue;
//! use relvar_core::tuple;
//!
//! // Create a constraint: salary > 0
//! let expr = ConstraintExpression::Cmp {
//!     left: "salary".to_string(),
//!     op: CmpOp::Gt,
//!     right: ValueOrRef::Value(ScalarValue::Int(0)),
//! };
//!
//! let valid_tuple = tuple! { salary: 50000i64 };
//! assert!(expr.evaluate(&valid_tuple).unwrap());
//!
//! let invalid_tuple = tuple! { salary: -100i64 };
//! assert!(!expr.evaluate(&invalid_tuple).unwrap());
//! ```

use super::prepared::PreparedConstraintExpression;
use crate::values::{ScalarValue, Tuple};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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
/// - **Comparisons**: Eq, Ne, Lt, Le, Gt, Ge via `Cmp`
/// - **Logical**: And, Or, Not
/// - **Set membership**: In
/// - **Pattern matching**: Like
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConstraintExpression {
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
    And(Box<ConstraintExpression>, Box<ConstraintExpression>),
    /// Logical OR: at least one expression must be true
    Or(Box<ConstraintExpression>, Box<ConstraintExpression>),
    /// Logical NOT: inverts the expression
    Not(Box<ConstraintExpression>),

    /// Set membership: attribute IN (value1, value2, ...)
    In(String, Vec<ScalarValue>),

    /// Pattern matching: attribute LIKE pattern
    ///
    /// # Wildcards
    /// - `%`: Matches any sequence of characters (including empty).
    /// - `_`: Matches exactly one character.
    ///
    /// # Complexity
    /// Uses a dynamic programming approach with **O(N*M)** time complexity and **O(M)** space complexity,
    /// where N is the string length and M is the pattern length.
    ///
    /// # Case Sensitivity
    /// The matching is **case-sensitive**. "A" does not match "a".
    ///
    /// # Example
    /// ```
    /// use relvar_core::ConstraintExpression;
    /// use relvar_core::tuple;
    ///
    /// // Matches "data_2024.csv", "data_final.csv", etc.
    /// let expr = ConstraintExpression::Like("filename".to_string(), "data_%.csv".to_string());
    /// assert!(expr.evaluate(&tuple! { filename: "data_2024.csv" }).unwrap());
    ///
    /// // Case-sensitive matching
    /// let case_expr = ConstraintExpression::Like("code".to_string(), "ABC".to_string());
    /// assert!(case_expr.evaluate(&tuple! { code: "ABC" }).unwrap());
    /// assert!(!case_expr.evaluate(&tuple! { code: "abc" }).unwrap());
    /// ```
    Like(String, String),
}

impl ConstraintExpression {
    /// Scans the expression tree to collect all referenced attribute names.
    ///
    /// This includes:
    /// - Attributes on the left side of comparisons
    /// - Attributes referenced via `ValueOrRef::Attribute` on the right side
    /// - Attributes used in `IN` and `LIKE` expressions
    /// - Attributes in nested sub-expressions (AND, OR, NOT)
    ///
    /// # Example
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn referenced_attributes(&self) -> HashSet<String> {
        let mut attributes = HashSet::new();
        self.collect_attributes(&mut attributes);
        attributes
    }

    fn collect_attributes(&self, attributes: &mut HashSet<String>) {
        match self {
            ConstraintExpression::Cmp { left, right, .. } => {
                attributes.insert(left.clone());
                if let ValueOrRef::Attribute(attr) = right {
                    attributes.insert(attr.clone());
                }
            }
            ConstraintExpression::And(l, r) | ConstraintExpression::Or(l, r) => {
                l.collect_attributes(attributes);
                r.collect_attributes(attributes);
            }
            ConstraintExpression::Not(expr) => {
                expr.collect_attributes(attributes);
            }
            ConstraintExpression::In(attr, _) => {
                attributes.insert(attr.clone());
            }
            ConstraintExpression::Like(attr, _) => {
                attributes.insert(attr.clone());
            }
        }
    }

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
    /// use relvar_core::{ConstraintExpression, CmpOp, ValueOrRef};
    /// use relvar_core::values::ScalarValue;
    /// use relvar_core::tuple;
    ///
    /// let expr = ConstraintExpression::Cmp {
    ///     left: "age".to_string(),
    ///     op: CmpOp::Gt,
    ///     right: ValueOrRef::Value(ScalarValue::Int(0)),
    /// };
    ///
    /// let tuple = tuple! { age: 30i64 };
    /// assert!(expr.evaluate(&tuple).unwrap());
    /// ```
    pub fn evaluate(&self, tuple: &Tuple) -> Result<bool, ExpressionError> {
        match self {
            ConstraintExpression::Cmp { left, op, right } => {
                Self::evaluate_cmp(tuple, left, op, right)
            }
            ConstraintExpression::And(left, right) => {
                Ok(left.evaluate(tuple)? && right.evaluate(tuple)?)
            }
            ConstraintExpression::Or(left, right) => {
                Ok(left.evaluate(tuple)? || right.evaluate(tuple)?)
            }
            ConstraintExpression::Not(expr) => Ok(!expr.evaluate(tuple)?),
            ConstraintExpression::In(attr, values) => Self::evaluate_in(tuple, attr, values),
            ConstraintExpression::Like(attr, pattern) => Self::evaluate_like(tuple, attr, pattern),
        }
    }

    fn evaluate_cmp(
        tuple: &Tuple,
        left: &str,
        op: &CmpOp,
        right: &ValueOrRef,
    ) -> Result<bool, ExpressionError> {
        let (left_val, right_val) = Self::get_comparison_operands(tuple, left, right)?;
        match op {
            CmpOp::Eq => Ok(left_val == right_val),
            CmpOp::Ne => Ok(left_val != right_val),
            CmpOp::Lt => Ok(left_val < right_val),
            CmpOp::Le => Ok(left_val <= right_val),
            CmpOp::Gt => Ok(left_val > right_val),
            CmpOp::Ge => Ok(left_val >= right_val),
        }
    }

    fn evaluate_in(
        tuple: &Tuple,
        attr: &str,
        values: &[ScalarValue],
    ) -> Result<bool, ExpressionError> {
        let tuple_value = tuple
            .get(attr)
            .ok_or_else(|| ExpressionError::AttributeNotFound(attr.to_string()))?;

        // Use HashSet for O(1) lookup instead of Vec::contains O(N)
        // For small lists (< 10 items), Vec is actually faster due to cache locality
        if values.len() < 10 {
            Ok(values.contains(tuple_value))
        } else {
            use std::collections::HashSet;
            let value_set: HashSet<_> = values.iter().collect();
            Ok(value_set.contains(tuple_value))
        }
    }

    fn evaluate_like(tuple: &Tuple, attr: &str, pattern: &str) -> Result<bool, ExpressionError> {
        let tuple_value = tuple
            .get(attr)
            .ok_or_else(|| ExpressionError::AttributeNotFound(attr.to_string()))?;

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

    /// Simple SQL LIKE pattern matching using dynamic programming.
    ///
    /// Uses an iterative DP approach to avoid stack overflow with patterns
    /// containing many wildcards (% and _).
    ///
    /// % matches any sequence of characters (including empty)
    /// _ matches exactly one character
    ///
    /// Time complexity: O(n*m) where n = text length, m = pattern length
    /// Space complexity: O(m) optimized (was O(n*m))
    fn matches_pattern(text: &str, pattern: &str) -> bool {
        // Collect pattern chars for random access (usually small)
        let pattern_chars: Vec<char> = pattern.chars().collect();
        Self::matches_pattern_chars(text, &pattern_chars)
    }

    pub(crate) fn matches_pattern_chars(text: &str, pattern_chars: &[char]) -> bool {
        let pattern_len = pattern_chars.len();

        // DP state: only need previous row and current row
        // dp[j] stores whether pattern[0..j] matches current text prefix
        let mut prev_dp = vec![false; pattern_len + 1];
        let mut curr_dp = vec![false; pattern_len + 1];

        // Base case: empty text matches empty pattern
        prev_dp[0] = true;

        // Handle patterns that start with % (can match empty text)
        for j in 1..=pattern_len {
            if pattern_chars[j - 1] == '%' {
                prev_dp[j] = prev_dp[j - 1];
            } else {
                prev_dp[j] = false;
            }
        }

        // Iterate through text characters directly (avoids O(N) allocation)
        for text_char in text.chars() {
            // New row starts with false (non-empty text doesn't match empty pattern)
            curr_dp[0] = false;

            for j in 1..=pattern_len {
                match pattern_chars[j - 1] {
                    '%' => {
                        // % matches zero chars (curr_dp[j-1]) or one/more (prev_dp[j])
                        // Note: In full DP, this was dp[i][j] = dp[i][j-1] || dp[i-1][j]
                        // Here: curr_dp[j] = curr_dp[j-1] || prev_dp[j]
                        curr_dp[j] = curr_dp[j - 1] || prev_dp[j];
                    }
                    '_' => {
                        // _ matches exactly one char
                        // dp[i][j] = dp[i-1][j-1]
                        curr_dp[j] = prev_dp[j - 1];
                    }
                    c => {
                        // Literal char match
                        // dp[i][j] = dp[i-1][j-1] && char_match
                        curr_dp[j] = prev_dp[j - 1] && text_char == c;
                    }
                }
            }
            // Move current row to previous for next iteration
            prev_dp.copy_from_slice(&curr_dp);
        }

        prev_dp[pattern_len]
    }

    /// Prepares the constraint expression for efficient repeated evaluation.
    ///
    /// Optimizations:
    /// - `IN`: Converts `Vec` to `HashSet` for O(1) lookup.
    /// - `LIKE`: Pre-parses pattern string to `Vec<char>`.
    ///
    /// # Example
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn prepare(&self) -> PreparedConstraintExpression {
        PreparedConstraintExpression::from(self.clone())
    }

    /// Helper to resolve a ValueOrRef to a concrete ScalarValue from the tuple.
    pub(crate) fn resolve_value_or_ref<'a>(
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

    /// Helper to get and validate comparison operands.
    ///
    /// Fetches both operands and ensures they are of compatible types
    /// before performing comparison operations.
    pub(crate) fn get_comparison_operands<'a>(
        tuple: &'a Tuple,
        attr: &str,
        value_or_ref: &'a ValueOrRef,
    ) -> Result<(&'a ScalarValue, &'a ScalarValue), ExpressionError> {
        let left = tuple
            .get(attr)
            .ok_or_else(|| ExpressionError::AttributeNotFound(attr.to_string()))?;
        let right = Self::resolve_value_or_ref(tuple, value_or_ref)?;

        // Validate types are compatible for comparison
        // Using discriminant to check if they are the same enum variant
        if std::mem::discriminant(left) != std::mem::discriminant(right) {
            return Err(ExpressionError::TypeMismatch(
                format!("{:?}", left.scalar_type()),
                format!("{:?}", right.scalar_type()),
            ));
        }

        Ok((left, right))
    }
}

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
