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
    And(
        #[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]
        Box<ConstraintExpression>,
        #[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]
        Box<ConstraintExpression>,
    ),
    /// Logical OR: at least one expression must be true
    Or(
        #[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]
        Box<ConstraintExpression>,
        #[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]
        Box<ConstraintExpression>,
    ),
    /// Logical NOT: inverts the expression
    Not(
        #[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]
        Box<ConstraintExpression>,
    ),

    /// Set membership: attribute IN (value1, value2, ...)
    In(String, std::collections::HashSet<ScalarValue>),

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
    /// # Examples
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
    /// # Examples
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
    /// # Examples
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
        values: &std::collections::HashSet<ScalarValue>,
    ) -> Result<bool, ExpressionError> {
        let tuple_value = tuple
            .get(attr)
            .ok_or_else(|| ExpressionError::AttributeNotFound(attr.to_string()))?;

        Ok(values.contains(tuple_value))
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
    /// # Examples
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
mod tests;
