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
//! use relvar_core::constraints::check::{CheckConstraint, CheckPredicate};
//! use relvar_core::constraints::expression::{ConstraintExpression, ValueOrRef};
//! use relvar_core::values::ScalarValue;
//! use relvar_core::tuple;
//!
//! // Create a CHECK constraint using an expression
//! let constraint = CheckConstraint::from_expression(
//!     "positive_salary",
//!     "Salary must be positive",
//!     ConstraintExpression::Gt(
//!         "salary".to_string(),
//!         ValueOrRef::Value(ScalarValue::Int(0)),
//!     ),
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
use serde::{Deserialize, Serialize, Serializer};
use std::sync::Arc;
use thiserror::Error;

/// Errors that can occur during CHECK constraint evaluation.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
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

/// Predicate type for CHECK constraints.
///
/// Supports both closure-based (flexible, not serializable) and
/// expression-based (serializable, persistent) predicates.
#[derive(Clone, Serialize, Deserialize)]
pub enum CheckPredicate {
    /// Closure-based predicate (not serializable).
    ///
    /// Use for complex business logic that can't be expressed in the DSL.
    /// Must be re-registered on database restart.
    #[serde(skip)]
    Dynamic(
        #[serde(skip)]
        #[allow(clippy::type_complexity)]
        Arc<dyn Fn(&Tuple) -> bool + Send + Sync>,
    ),

    /// Expression-based predicate (serializable).
    ///
    /// Can be persisted to storage and loaded on restart.
    Expression(Box<ConstraintExpression>),
}

impl std::fmt::Debug for CheckPredicate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckPredicate::Dynamic(_) => write!(f, "Dynamic(<closure>)"),
            CheckPredicate::Expression(expr) => write!(f, "Expression({:?})", expr),
        }
    }
}

/// A CHECK constraint that must be satisfied by every tuple in a relvar.
///
/// CHECK constraints are tuple-level constraints that enforce business rules
/// on individual tuples. They are checked on INSERT and UPDATE operations.
///
/// # Example
///
/// ```
/// use relvar_core::constraints::check::CheckConstraint;
/// use relvar_core::tuple;
///
/// let constraint = CheckConstraint::from_closure(
///     "positive_salary",
///     "Salary must be positive",
///     |tuple| {
///         tuple.get("salary")
///             .map(|v| matches!(v, relvar_core::values::ScalarValue::Int(n) if *n > 0))
///             .unwrap_or(false)
///     }
/// );
///
/// let valid = tuple! { name: "Alice", salary: 50000i64 };
/// assert!(constraint.is_satisfied_by(&valid).unwrap());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckConstraint {
    /// Constraint name (unique identifier).
    name: String,
    /// The predicate to evaluate.
    predicate: CheckPredicate,
    /// Human-readable description.
    description: String,
}

impl CheckConstraint {
    /// Creates a CHECK constraint from a closure.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::constraints::check::CheckConstraint;
    /// use relvar_core::values::ScalarValue;
    ///
    /// let constraint = CheckConstraint::from_closure(
    ///     "positive_balance",
    ///     "Balance must be non-negative",
    ///     |tuple| {
    ///         tuple.get("balance")
    ///             .map(|v| matches!(v, ScalarValue::Int(n) if *n >= 0))
    ///             .unwrap_or(false)
    ///     }
    /// );
    /// ```
    pub fn from_closure<F>(name: impl Into<String>, description: impl Into<String>, f: F) -> Self
    where
        F: Fn(&Tuple) -> bool + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            predicate: CheckPredicate::Dynamic(Arc::new(f)),
            description: description.into(),
        }
    }

    /// Creates a CHECK constraint from an expression.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::constraints::check::CheckConstraint;
    /// use relvar_core::constraints::expression::{ConstraintExpression, ValueOrRef};
    /// use relvar_core::values::ScalarValue;
    ///
    /// let constraint = CheckConstraint::from_expression(
    ///     "valid_age",
    ///     "Age must be between 0 and 150",
    ///     ConstraintExpression::Between(
    ///         "age".to_string(),
    ///         ScalarValue::Int(0),
    ///         ScalarValue::Int(150),
    ///     ),
    /// );
    /// ```
    pub fn from_expression(
        name: impl Into<String>,
        description: impl Into<String>,
        expr: ConstraintExpression,
    ) -> Self {
        Self {
            name: name.into(),
            predicate: CheckPredicate::Expression(Box::new(expr)),
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

    /// Returns true if this constraint is serializable (expression-based).
    pub fn is_serializable(&self) -> bool {
        matches!(self.predicate, CheckPredicate::Expression(_))
    }

    /// Checks if this constraint is satisfied by the given tuple.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the constraint evaluation fails.
    pub fn is_satisfied_by(&self, tuple: &Tuple) -> Result<bool, CheckConstraintError> {
        match &self.predicate {
            CheckPredicate::Dynamic(f) => Ok(f(tuple)),
            CheckPredicate::Expression(expr) => (**expr)
                .evaluate(tuple)
                .map_err(|e| CheckConstraintError::EvaluationError(e.to_string())),
        }
    }
}

/// A collection of CHECK constraints for a relvar.
///
/// This manages multiple CHECK constraints and provides methods to check
/// if all constraints are satisfied by a tuple.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CheckConstraints {
    constraints: Vec<CheckConstraint>,
}

impl Serialize for CheckConstraints {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        // Filter out non-serializable constraints (Dynamic predicates)
        // We only persist constraints defined via expressions
        let serializable: Vec<&CheckConstraint> = self
            .constraints
            .iter()
            .filter(|c| c.is_serializable())
            .collect();

        let mut state = serializer.serialize_struct("CheckConstraints", 1)?;
        state.serialize_field("constraints", &serializable)?;
        state.end()
    }
}

impl CheckConstraints {
    /// Creates a new empty collection of CHECK constraints.
    pub fn new() -> Self {
        Self {
            constraints: Vec::new(),
        }
    }

    /// Adds a constraint to the collection.
    pub fn with_constraint(mut self, constraint: CheckConstraint) -> Self {
        self.constraints.push(constraint);
        self
    }

    /// Checks if all constraints are satisfied by the given tuple.
    ///
    /// Returns `Ok(true)` if all constraints are satisfied.
    ///
    /// # Errors
    ///
    /// Returns `Err` with the first violated constraint.
    pub fn are_all_satisfied_by(&self, tuple: &Tuple) -> Result<bool, CheckConstraintError> {
        for constraint in &self.constraints {
            if !constraint.is_satisfied_by(tuple)? {
                return Err(CheckConstraintError::Violation {
                    constraint_name: constraint.name.clone(),
                    description: constraint.description.clone(),
                });
            }
        }
        Ok(true)
    }

    /// Returns only the serializable (expression-based) constraints.
    ///
    /// Useful for persistence - closure-based constraints cannot be serialized.
    pub fn serializable_constraints(&self) -> Vec<&CheckConstraint> {
        self.constraints
            .iter()
            .filter(|c| c.is_serializable())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::expression::{ConstraintExpression, ValueOrRef};
    use crate::tuple;
    use crate::values::ScalarValue;

    #[test]
    fn test_check_constraint_from_closure() {
        let constraint =
            CheckConstraint::from_closure("positive_salary", "Salary must be positive", |tuple| {
                tuple
                    .get("salary")
                    .map(|v| matches!(v, ScalarValue::Int(n) if *n > 0))
                    .unwrap_or(false)
            });

        assert_eq!(constraint.name(), "positive_salary");
        assert!(!constraint.is_serializable());
    }

    #[test]
    fn test_check_constraint_from_expression() {
        let constraint = CheckConstraint::from_expression(
            "positive_salary",
            "Salary must be positive",
            ConstraintExpression::Gt("salary".to_string(), ValueOrRef::Value(ScalarValue::Int(0))),
        );

        assert_eq!(constraint.name(), "positive_salary");
        assert!(constraint.is_serializable());
    }

    #[test]
    fn test_check_constraint_dynamic_satisfied() {
        let constraint =
            CheckConstraint::from_closure("positive_salary", "Salary must be positive", |tuple| {
                tuple
                    .get("salary")
                    .map(|v| matches!(v, ScalarValue::Int(n) if *n > 0))
                    .unwrap_or(false)
            });

        let good_tuple = tuple! { name: "Alice", salary: 50000i64 };
        assert!(constraint.is_satisfied_by(&good_tuple).unwrap());

        let bad_tuple = tuple! { name: "Bob", salary: -100i64 };
        assert!(!constraint.is_satisfied_by(&bad_tuple).unwrap());
    }

    #[test]
    fn test_check_constraint_expression_satisfied() {
        let constraint = CheckConstraint::from_expression(
            "positive_salary",
            "Salary must be positive",
            ConstraintExpression::Gt("salary".to_string(), ValueOrRef::Value(ScalarValue::Int(0))),
        );

        let good_tuple = tuple! { salary: 50000i64 };
        assert!(constraint.is_satisfied_by(&good_tuple).unwrap());

        let bad_tuple = tuple! { salary: -100i64 };
        assert!(!constraint.is_satisfied_by(&bad_tuple).unwrap());
    }

    #[test]
    fn test_check_constraints_all_satisfied() {
        let constraints = CheckConstraints::new()
            .with_constraint(CheckConstraint::from_closure("c1", "d1", |_| true))
            .with_constraint(CheckConstraint::from_expression(
                "c2",
                "d2",
                ConstraintExpression::Gt("x".to_string(), ValueOrRef::Value(ScalarValue::Int(0))),
            ));

        let tuple = tuple! { x: 10i64 };
        assert!(constraints.are_all_satisfied_by(&tuple).unwrap());
    }

    #[test]
    fn test_check_constraints_violation_returns_error() {
        let constraints = CheckConstraints::new().with_constraint(CheckConstraint::from_closure(
            "must_fail",
            "always fails",
            |_| false,
        ));

        let tuple = tuple! { id: 1i64 };
        let result = constraints.are_all_satisfied_by(&tuple);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_constraints_serializable_only() {
        let constraints = CheckConstraints::new()
            .with_constraint(CheckConstraint::from_closure("c1", "d1", |_| true))
            .with_constraint(CheckConstraint::from_expression(
                "c2",
                "d2",
                ConstraintExpression::Gt("x".to_string(), ValueOrRef::Value(ScalarValue::Int(0))),
            ));

        let serializable = constraints.serializable_constraints();
        assert_eq!(serializable.len(), 1);
        assert_eq!(serializable[0].name(), "c2");
    }
}
