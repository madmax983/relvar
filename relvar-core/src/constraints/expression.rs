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
use crate::types::{OperatorError, OperatorRegistry};
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

    /// An operator application was evaluated without an operator registry.
    ///
    /// `ScalarExpression::Apply` nodes can only be evaluated with
    /// [`ConstraintExpression::evaluate_with_operators`] (or
    /// [`ScalarExpression::evaluate_with`]), which resolves operator names
    /// against an [`OperatorRegistry`].
    #[error(
        "operator application '{0}' requires an operator registry; use evaluate_with_operators"
    )]
    OperatorRegistryRequired(String),

    /// Operator resolution or invocation failed.
    #[error(transparent)]
    Operator(#[from] OperatorError),
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

/// A scalar-valued expression: a literal, an attribute reference, or a
/// user-defined operator application.
///
/// `ScalarExpression` is the scalar level of the expression language;
/// [`ConstraintExpression`] is the boolean (predicate) level. Operator
/// applications compose: `Apply` nodes nest inside other `Apply` nodes'
/// argument lists, e.g. `upper(concat(first, last))`.
///
/// # Examples
///
/// ```
/// use relvar_core::constraints::ScalarExpression;
/// use relvar_core::values::ScalarValue;
/// use relvar_core::tuple;
///
/// let expr = ScalarExpression::Attribute("n".to_string());
/// assert_eq!(
///     expr.evaluate(&tuple! { n: 41i64 }).unwrap(),
///     ScalarValue::Int(41)
/// );
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ScalarExpression {
    /// A literal scalar value.
    Value(ScalarValue),
    /// A reference to an attribute by name.
    Attribute(String),
    /// Application of a user-defined scalar operator
    /// (TTM RM Prescription 3).
    ///
    /// The operator name is resolved against an [`OperatorRegistry`] at
    /// evaluation time (see [`evaluate_with`](Self::evaluate_with)), so
    /// expressions remain plain serializable data.
    Apply {
        /// Operator name, e.g. `"age"`.
        operator: String,
        /// Argument expressions, evaluated left to right.
        #[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]
        args: Vec<ScalarExpression>,
    },
}

impl ScalarExpression {
    /// Collects attribute names referenced by this expression into `attributes`.
    fn collect_attributes(&self, attributes: &mut HashSet<String>) {
        match self {
            ScalarExpression::Value(_) => {}
            ScalarExpression::Attribute(attr) => {
                attributes.insert(attr.clone());
            }
            ScalarExpression::Apply { args, .. } => {
                for arg in args {
                    arg.collect_attributes(attributes);
                }
            }
        }
    }

    /// Scans this expression tree to collect all referenced attribute names.
    pub fn referenced_attributes(&self) -> HashSet<String> {
        let mut attributes = HashSet::new();
        self.collect_attributes(&mut attributes);
        attributes
    }

    /// Evaluates this expression against a tuple, without an operator registry.
    ///
    /// Literals and attribute references evaluate normally; an [`Apply`](Self::Apply)
    /// node fails with [`ExpressionError::OperatorRegistryRequired`]. Use
    /// [`evaluate_with`](Self::evaluate_with) when the expression may
    /// contain operator applications.
    ///
    /// # Errors
    ///
    /// - [`ExpressionError::AttributeNotFound`] for missing attributes.
    /// - [`ExpressionError::OperatorRegistryRequired`] for `Apply` nodes.
    pub fn evaluate(&self, tuple: &Tuple) -> Result<ScalarValue, ExpressionError> {
        match self {
            ScalarExpression::Value(value) => Ok(value.clone()),
            ScalarExpression::Attribute(attr) => tuple
                .get(attr)
                .cloned()
                .ok_or_else(|| ExpressionError::AttributeNotFound(attr.clone())),
            ScalarExpression::Apply { operator, .. } => {
                Err(ExpressionError::OperatorRegistryRequired(operator.clone()))
            }
        }
    }

    /// Evaluates this expression against a tuple, resolving operator
    /// applications against `registry`.
    ///
    /// Arguments are evaluated left to right, then the operator is invoked
    /// with the resulting values (TTM RM Prescription 3(c) type discipline
    /// is enforced by the registry).
    ///
    /// # Errors
    ///
    /// - [`ExpressionError::AttributeNotFound`] for missing attributes.
    /// - [`ExpressionError::Operator`] if resolution or invocation fails.
    pub fn evaluate_with(
        &self,
        tuple: &Tuple,
        registry: &OperatorRegistry,
    ) -> Result<ScalarValue, ExpressionError> {
        match self {
            ScalarExpression::Value(value) => Ok(value.clone()),
            ScalarExpression::Attribute(attr) => tuple
                .get(attr)
                .cloned()
                .ok_or_else(|| ExpressionError::AttributeNotFound(attr.clone())),
            ScalarExpression::Apply { operator, args } => {
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    values.push(arg.evaluate_with(tuple, registry)?);
                }
                Ok(registry.invoke(operator, &values)?)
            }
        }
    }
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
/// - **Scalar comparisons**: comparisons over scalar expressions, enabling
///   user-defined operators in predicates, via `ScalarCmp`
///   (TTM RM Prescription 3)
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

    /// Comparison of two scalar expressions.
    ///
    /// This is the predicate-level hook for user-defined operators
    /// (TTM RM Prescription 3): either side may be an operator application,
    /// e.g. `age(birthdate) > 18` is
    /// `ScalarCmp { left: Apply { operator: "age", args: [Attribute("birthdate")] },
    /// op: Gt, right: Value(Int(18)) }`.
    ///
    /// Both sides must evaluate to values of the same type; otherwise
    /// evaluation fails with [`ExpressionError::TypeMismatch`].
    ScalarCmp {
        /// Left scalar expression
        left: ScalarExpression,
        /// Comparison operator
        op: CmpOp,
        /// Right scalar expression
        right: ScalarExpression,
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
            ConstraintExpression::ScalarCmp { left, right, .. } => {
                left.collect_attributes(attributes);
                right.collect_attributes(attributes);
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
    /// - The expression contains an operator application (see
    ///   [`evaluate_with_operators`](Self::evaluate_with_operators))
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
            ConstraintExpression::ScalarCmp { left, op, right } => {
                let left_val = left.evaluate(tuple)?;
                let right_val = right.evaluate(tuple)?;
                Self::evaluate_scalar_cmp(&left_val, op, &right_val)
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

    /// Evaluates this expression against a tuple, resolving user-defined
    /// operator applications against `registry` (TTM RM Prescription 3).
    ///
    /// This is the evaluation entry point for expressions that may contain
    /// [`ScalarExpression::Apply`] nodes. Expressions without operator
    /// applications evaluate exactly as [`evaluate`](Self::evaluate) does.
    ///
    /// # Errors
    ///
    /// Same as [`evaluate`](Self::evaluate), plus
    /// [`ExpressionError::Operator`] when operator resolution or invocation
    /// fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::constraints::{ConstraintExpression, CmpOp, ScalarExpression};
    /// use relvar_core::types::{OperatorRegistry, OperatorSignature, ScalarType};
    /// use relvar_core::values::ScalarValue;
    /// use relvar_core::tuple;
    ///
    /// let mut registry = OperatorRegistry::new();
    /// registry.register(
    ///     OperatorSignature {
    ///         name: "double".to_string(),
    ///         param_types: vec![ScalarType::Int],
    ///         return_type: ScalarType::Int,
    ///     },
    ///     |args| match args[0] {
    ///         ScalarValue::Int(n) => Ok(ScalarValue::Int(n * 2)),
    ///         _ => unreachable!("signature guarantees an Int argument"),
    ///     },
    /// ).unwrap();
    ///
    /// // double(n) > 40
    /// let expr = ConstraintExpression::ScalarCmp {
    ///     left: ScalarExpression::Apply {
    ///         operator: "double".to_string(),
    ///         args: vec![ScalarExpression::Attribute("n".to_string())],
    ///     },
    ///     op: CmpOp::Gt,
    ///     right: ScalarExpression::Value(ScalarValue::Int(40)),
    /// };
    /// assert!(expr.evaluate_with_operators(&tuple! { n: 21i64 }, &registry).unwrap());
    /// ```
    pub fn evaluate_with_operators(
        &self,
        tuple: &Tuple,
        registry: &OperatorRegistry,
    ) -> Result<bool, ExpressionError> {
        match self {
            ConstraintExpression::Cmp { left, op, right } => {
                Self::evaluate_cmp(tuple, left, op, right)
            }
            ConstraintExpression::ScalarCmp { left, op, right } => {
                let left_val = left.evaluate_with(tuple, registry)?;
                let right_val = right.evaluate_with(tuple, registry)?;
                Self::evaluate_scalar_cmp(&left_val, op, &right_val)
            }
            ConstraintExpression::And(left, right) => Ok(left
                .evaluate_with_operators(tuple, registry)?
                && right.evaluate_with_operators(tuple, registry)?),
            ConstraintExpression::Or(left, right) => Ok(left
                .evaluate_with_operators(tuple, registry)?
                || right.evaluate_with_operators(tuple, registry)?),
            ConstraintExpression::Not(expr) => Ok(!expr.evaluate_with_operators(tuple, registry)?),
            ConstraintExpression::In(attr, values) => Self::evaluate_in(tuple, attr, values),
            ConstraintExpression::Like(attr, pattern) => Self::evaluate_like(tuple, attr, pattern),
        }
    }

    /// Compares two evaluated scalar values with a comparison operator.
    ///
    /// Both values must be of the same type (TTM: comparisons are defined
    /// per type); mismatched types fail with [`ExpressionError::TypeMismatch`].
    /// The full [`ScalarType`](crate::types::ScalarType) is compared — not
    /// just the value discriminant — so two distinct user-defined types
    /// (which share a discriminant) never compare as the same type.
    fn evaluate_scalar_cmp(
        left_val: &ScalarValue,
        op: &CmpOp,
        right_val: &ScalarValue,
    ) -> Result<bool, ExpressionError> {
        if left_val.scalar_type() != right_val.scalar_type() {
            return Err(ExpressionError::TypeMismatch(
                format!("{:?}", left_val.scalar_type()),
                format!("{:?}", right_val.scalar_type()),
            ));
        }
        match op {
            CmpOp::Eq => Ok(left_val == right_val),
            CmpOp::Ne => Ok(left_val != right_val),
            CmpOp::Lt => Ok(left_val < right_val),
            CmpOp::Le => Ok(left_val <= right_val),
            CmpOp::Gt => Ok(left_val > right_val),
            CmpOp::Ge => Ok(left_val >= right_val),
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

        // Validate types are compatible for comparison.
        // The full scalar type is compared, not just the value
        // discriminant, so distinct user-defined types (which share a
        // discriminant) are never treated as the same type.
        if left.scalar_type() != right.scalar_type() {
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
    use crate::types::{OperatorError, OperatorRegistry, OperatorSignature, ScalarType};

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

    // ------------------------------------------------------------------
    // User-defined operators in expressions (TTM RM Prescription 3).
    // Written before the implementation (TDD): these fail to compile
    // until `ScalarExpression` and `ConstraintExpression::ScalarCmp` exist.
    // ------------------------------------------------------------------

    fn doubling_registry() -> OperatorRegistry {
        let mut registry = OperatorRegistry::new();
        registry
            .register(
                OperatorSignature {
                    name: "double".to_string(),
                    param_types: vec![ScalarType::Int],
                    return_type: ScalarType::Int,
                },
                |args| match args[0] {
                    ScalarValue::Int(n) => Ok(ScalarValue::Int(n * 2)),
                    _ => Err(OperatorError::EvaluationFailed {
                        name: "double".to_string(),
                        reason: "expected Int".to_string(),
                    }),
                },
            )
            .unwrap();
        registry
    }

    #[test]
    fn scalar_apply_evaluates_with_registry() {
        let registry = doubling_registry();
        let expr = ScalarExpression::Apply {
            operator: "double".to_string(),
            args: vec![ScalarExpression::Attribute("n".to_string())],
        };
        let tuple = tuple! { n: 21i64 };
        assert_eq!(
            expr.evaluate_with(&tuple, &registry).unwrap(),
            ScalarValue::Int(42)
        );
    }

    #[test]
    fn scalar_apply_without_registry_is_an_error() {
        // Plain `evaluate` cannot resolve operator names: the caller must
        // use `evaluate_with` / `evaluate_with_operators`.
        let expr = ScalarExpression::Apply {
            operator: "double".to_string(),
            args: vec![ScalarExpression::Value(ScalarValue::Int(1))],
        };
        let tuple = tuple! { n: 21i64 };
        let err = expr.evaluate(&tuple).unwrap_err();
        assert!(
            matches!(err, ExpressionError::OperatorRegistryRequired(_)),
            "expected OperatorRegistryRequired, got {err:?}"
        );
    }

    #[test]
    fn scalar_cmp_predicate_with_operator() {
        // double(n) > 40  <=>  n = 21  =>  true
        let registry = doubling_registry();
        let expr = ConstraintExpression::ScalarCmp {
            left: ScalarExpression::Apply {
                operator: "double".to_string(),
                args: vec![ScalarExpression::Attribute("n".to_string())],
            },
            op: CmpOp::Gt,
            right: ScalarExpression::Value(ScalarValue::Int(40)),
        };
        assert!(
            expr.evaluate_with_operators(&tuple! { n: 21i64 }, &registry)
                .unwrap()
        );
        assert!(
            !expr
                .evaluate_with_operators(&tuple! { n: 10i64 }, &registry)
                .unwrap()
        );
    }

    #[test]
    fn scalar_cmp_collects_referenced_attributes() {
        let expr = ConstraintExpression::ScalarCmp {
            left: ScalarExpression::Apply {
                operator: "double".to_string(),
                args: vec![
                    ScalarExpression::Attribute("n".to_string()),
                    ScalarExpression::Attribute("m".to_string()),
                ],
            },
            op: CmpOp::Gt,
            right: ScalarExpression::Attribute("limit".to_string()),
        };
        let attrs = expr.referenced_attributes();
        assert!(attrs.contains("n"));
        assert!(attrs.contains("m"));
        assert!(attrs.contains("limit"));
        assert_eq!(attrs.len(), 3);
    }

    #[test]
    fn nested_operator_application() {
        // double(double(n)) with n = 10 => 40
        let registry = doubling_registry();
        let expr = ScalarExpression::Apply {
            operator: "double".to_string(),
            args: vec![ScalarExpression::Apply {
                operator: "double".to_string(),
                args: vec![ScalarExpression::Attribute("n".to_string())],
            }],
        };
        assert_eq!(
            expr.evaluate_with(&tuple! { n: 10i64 }, &registry).unwrap(),
            ScalarValue::Int(40)
        );
    }

    #[test]
    fn scalar_cmp_type_mismatch_is_an_error() {
        // double(n) is Int; comparing Int > String must fail, not coerce.
        let registry = doubling_registry();
        let expr = ConstraintExpression::ScalarCmp {
            left: ScalarExpression::Apply {
                operator: "double".to_string(),
                args: vec![ScalarExpression::Attribute("n".to_string())],
            },
            op: CmpOp::Gt,
            right: ScalarExpression::Value(ScalarValue::String("x".to_string())),
        };
        let err = expr
            .evaluate_with_operators(&tuple! { n: 21i64 }, &registry)
            .unwrap_err();
        assert!(
            matches!(err, ExpressionError::TypeMismatch(_, _)),
            "expected TypeMismatch, got {err:?}"
        );
    }

    #[test]
    fn scalar_cmp_distinct_user_types_are_a_mismatch() {
        // Two differently-named user-defined types share a value
        // discriminant; comparing them must still fail as a type mismatch
        // (TTM: comparisons are defined per type).
        let date_type = ScalarType::user_defined("Date", ScalarType::String);
        let money_type = ScalarType::user_defined("Money", ScalarType::String);
        let date =
            ScalarValue::select(&date_type, ScalarValue::String("2020-01-01".to_string())).unwrap();
        let money = ScalarValue::select(&money_type, ScalarValue::String("2020-01-01".to_string()))
            .unwrap();

        let expr = ConstraintExpression::ScalarCmp {
            left: ScalarExpression::Value(date),
            op: CmpOp::Eq,
            right: ScalarExpression::Value(money),
        };
        let err = expr
            .evaluate_with_operators(&tuple! { n: 1i64 }, &OperatorRegistry::new())
            .unwrap_err();
        assert!(
            matches!(err, ExpressionError::TypeMismatch(_, _)),
            "expected TypeMismatch, got {err:?}"
        );
    }

    #[test]
    fn unknown_operator_in_expression_is_an_error() {
        let registry = OperatorRegistry::new();
        let expr = ConstraintExpression::ScalarCmp {
            left: ScalarExpression::Apply {
                operator: "missing".to_string(),
                args: vec![ScalarExpression::Attribute("n".to_string())],
            },
            op: CmpOp::Gt,
            right: ScalarExpression::Value(ScalarValue::Int(1)),
        };
        let err = expr
            .evaluate_with_operators(&tuple! { n: 1i64 }, &registry)
            .unwrap_err();
        assert!(
            matches!(err, ExpressionError::Operator(_)),
            "expected Operator error, got {err:?}"
        );
    }

    #[test]
    fn prepared_scalar_cmp_evaluates_with_registry() {
        let registry = doubling_registry();
        let expr = ConstraintExpression::ScalarCmp {
            left: ScalarExpression::Apply {
                operator: "double".to_string(),
                args: vec![ScalarExpression::Attribute("n".to_string())],
            },
            op: CmpOp::Gt,
            right: ScalarExpression::Value(ScalarValue::Int(40)),
        };
        let prepared = expr.prepare();
        assert!(
            prepared
                .evaluate_with_operators(&tuple! { n: 21i64 }, &registry)
                .unwrap()
        );
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
