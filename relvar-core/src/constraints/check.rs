//! CHECK constraints for tuple-level integrity.
//!
//! This module implements CHECK constraints per TTM RM Prescription 9.
//! CHECK constraints are predicates that must be satisfied by every tuple
//! in a relvar.
//!
//! # TTM Compliance
//!
//! - TTM RM Prescription 9: General integrity constraints
//! - Predicates operate on tuple values (no NULL involvement)
//! - Constraints are declarative, not procedural
//!
//! # Example
//!
//! ```
//! use relvar_core::constraints::check::CheckConstraint;
//! use relvar_core::constraints::expression::{ConstraintExpression, CmpOp, ValueOrRef};
//! use relvar_core::values::ScalarValue;
//! use relvar_core::tuple;
//!
//! // Create a CHECK constraint using an expression
//! let constraint = CheckConstraint::new(
//!     "positive_salary",
//!     "Salary must be positive",
//!     ConstraintExpression::Cmp {
//!         left: "salary".to_string(),
//!         op: CmpOp::Gt,
//!         right: ValueOrRef::Value(ScalarValue::Int(0)),
//!     },
//! );
//!
//! let valid_tuple = tuple! { salary: 50000i64 };
//! assert!(constraint.is_satisfied_by(&valid_tuple).unwrap());
//!
//! let invalid_tuple = tuple! { salary: -100i64 };
//! assert!(!constraint.is_satisfied_by(&invalid_tuple).unwrap());
//! ```

use crate::constraints::expression::ConstraintExpression;
use crate::values::Tuple;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during CHECK constraint evaluation.
#[derive(Debug, Error)]
pub enum CheckConstraintError {
    /// The constraint was violated.
    #[error("CHECK constraint '{constraint_name}' violated: {description}")]
    Violation {
        /// Name of the violated constraint.
        constraint_name: String,
        /// Human-readable description of what the constraint requires.
        description: String,
    },

    /// An error occurred during expression evaluation.
    #[error("Error evaluating constraint expression: {0}")]
    EvaluationError(String),
}

/// A CHECK constraint that must be satisfied by every tuple in a relvar.
///
/// CHECK constraints are tuple-level constraints that enforce business rules
/// on individual tuples. They are checked on INSERT and UPDATE operations.
///
/// Constraints are defined using declarative expressions ([`ConstraintExpression`]),
/// ensuring they can be serialized and persisted in the system catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckConstraint {
    /// Constraint name (unique identifier).
    name: String,
    /// The expression to evaluate.
    expression: ConstraintExpression,
    /// Human-readable description.
    description: String,
}

impl CheckConstraint {
    /// Creates a new CHECK constraint from an expression.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::constraints::check::CheckConstraint;
    /// use relvar_core::constraints::expression::{ConstraintExpression, CmpOp, ValueOrRef};
    /// use relvar_core::values::ScalarValue;
    ///
    /// let constraint = CheckConstraint::new(
    ///     "valid_age",
    ///     "Age must be between 0 and 150",
    ///     ConstraintExpression::And(
    ///         Box::new(ConstraintExpression::Cmp {
    ///             left: "age".to_string(),
    ///             op: CmpOp::Ge,
    ///             right: ValueOrRef::Value(ScalarValue::Int(0)),
    ///         }),
    ///         Box::new(ConstraintExpression::Cmp {
    ///             left: "age".to_string(),
    ///             op: CmpOp::Le,
    ///             right: ValueOrRef::Value(ScalarValue::Int(150)),
    ///         }),
    ///     ),
    /// );
    /// ```
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        expression: ConstraintExpression,
    ) -> Self {
        Self {
            name: name.into(),
            expression,
            description: description.into(),
        }
    }

    /// Returns the constraint name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the constraint description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Returns the constraint expression.
    pub fn expression(&self) -> &ConstraintExpression {
        &self.expression
    }

    /// Checks if this constraint is satisfied by the given tuple.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the constraint evaluation fails (e.g. type mismatch).
    pub fn is_satisfied_by(&self, tuple: &Tuple) -> Result<bool, CheckConstraintError> {
        self.expression
            .evaluate(tuple)
            .map_err(|e| CheckConstraintError::EvaluationError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::expression::{CmpOp, ConstraintExpression, ValueOrRef};
    use crate::tuple;
    use crate::values::ScalarValue;

    #[test]
    fn test_check_constraint_creation() {
        let constraint = CheckConstraint::new(
            "positive_salary",
            "Salary must be positive",
            ConstraintExpression::Cmp {
                left: "salary".to_string(),
                op: CmpOp::Gt,
                right: ValueOrRef::Value(ScalarValue::Int(0)),
            },
        );

        assert_eq!(constraint.name(), "positive_salary");
        assert_eq!(constraint.description(), "Salary must be positive");
    }

    #[test]
    fn test_check_constraint_satisfied() {
        let constraint = CheckConstraint::new(
            "positive_salary",
            "Salary must be positive",
            ConstraintExpression::Cmp {
                left: "salary".to_string(),
                op: CmpOp::Gt,
                right: ValueOrRef::Value(ScalarValue::Int(0)),
            },
        );

        let good_tuple = tuple! { salary: 50000i64 };
        assert!(constraint.is_satisfied_by(&good_tuple).unwrap());

        let bad_tuple = tuple! { salary: -100i64 };
        assert!(!constraint.is_satisfied_by(&bad_tuple).unwrap());
    }

    #[test]
    fn test_check_constraints_list_validation() {
        let constraints = vec![
            CheckConstraint::new(
                "c1",
                "x > 0",
                ConstraintExpression::Cmp {
                    left: "x".to_string(),
                    op: CmpOp::Gt,
                    right: ValueOrRef::Value(ScalarValue::Int(0)),
                },
            ),
            CheckConstraint::new(
                "c2",
                "x < 100",
                ConstraintExpression::Cmp {
                    left: "x".to_string(),
                    op: CmpOp::Lt,
                    right: ValueOrRef::Value(ScalarValue::Int(100)),
                },
            ),
        ];

        let tuple = tuple! { x: 10i64 };
        for constraint in &constraints {
            assert!(constraint.is_satisfied_by(&tuple).unwrap());
        }
    }

    #[test]
    fn test_check_constraints_list_violation() {
        let constraints = vec![CheckConstraint::new(
            "must_fail",
            "always fails",
            ConstraintExpression::Cmp {
                left: "id".to_string(),
                op: CmpOp::Lt,
                right: ValueOrRef::Value(ScalarValue::Int(0)),
            },
        )];

        let tuple = tuple! { id: 1i64 };
        let mut violated = false;
        for constraint in &constraints {
            if !constraint.is_satisfied_by(&tuple).unwrap() {
                violated = true;
                break;
            }
        }
        assert!(violated);
    }

    #[test]
    fn test_serialization() {
        let constraint = CheckConstraint::new(
            "test_const",
            "desc",
            ConstraintExpression::Cmp {
                left: "a".to_string(),
                op: CmpOp::Eq,
                right: ValueOrRef::Value(ScalarValue::Int(1)),
            },
        );

        let serialized = serde_json::to_string(&constraint).unwrap();
        let deserialized: CheckConstraint = serde_json::from_str(&serialized).unwrap();

        assert_eq!(constraint.name(), deserialized.name());
        assert_eq!(constraint.description(), deserialized.description());

        let tuple = tuple! { a: 1i64 };
        assert!(deserialized.is_satisfied_by(&tuple).unwrap());
    }
}
