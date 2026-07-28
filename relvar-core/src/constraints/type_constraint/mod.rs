//! Type constraints for value domain restrictions.
//!
//! Type constraints define additional rules beyond basic type checking that
//! values must satisfy. These include range limits, enumerated values,
//! and string length requirements.
//!
//! # Constraint Types
//!
//! - [`TypeConstraint::Range`] - Value must be within min/max bounds
//! - [`TypeConstraint::Enum`] - Value must be one of specified options
//! - [`TypeConstraint::StringLength`] - String length must be within bounds
//!
//! # Example
//!
//! ```
//! use relvar_core::constraints::{TypeConstraint, AttributeConstraints};
//! use relvar_core::types::ScalarType;
//! use relvar_core::values::ScalarValue;
//!
//! // Age must be between 0 and 150
//! let age_constraint = TypeConstraint::Range {
//!     min: ScalarValue::Int(0),
//!     max: ScalarValue::Int(150),
//! };
//!
//! assert!(age_constraint.is_satisfied_by(&ScalarValue::Int(25)).unwrap());
//! assert!(!age_constraint.is_satisfied_by(&ScalarValue::Int(200)).unwrap());
//! ```

use crate::types::ScalarType;
use crate::values::ScalarValue;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur with type constraints.
#[derive(Debug, Error)]
pub enum TypeConstraintError {
    /// A value violates a constraint.
    #[error("Type constraint violation: value {0:?} does not satisfy constraint")]
    ConstraintViolation(ScalarValue),

    /// A value's type doesn't match what the constraint expects.
    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch {
        /// The expected type name.
        expected: String,
        /// The actual type name of the value.
        actual: String,
    },
}

/// A constraint that restricts the valid values for a scalar type.
///
/// Type constraints add domain restrictions beyond the basic type system.
/// They can enforce ranges, enumerated values, or string lengths.
///
/// # Examples
///
/// ```
/// use relvar_core::constraints::TypeConstraint;
/// use relvar_core::values::ScalarValue;
///
/// // Range constraint for percentages
/// let percentage = TypeConstraint::Range {
///     min: ScalarValue::Int(0),
///     max: ScalarValue::Int(100),
/// };
///
/// // Enum constraint for status values
/// let status = TypeConstraint::Enum {
///     allowed_values: vec![
///         ScalarValue::String("active".to_string()),
///         ScalarValue::String("inactive".to_string()),
///         ScalarValue::String("pending".to_string()),
///     ],
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypeConstraint {
    /// Value must be within an inclusive range.
    ///
    /// Both `min` and `max` bounds are included in the valid range.
    Range {
        /// The minimum allowed value (inclusive).
        min: ScalarValue,
        /// The maximum allowed value (inclusive).
        max: ScalarValue,
    },

    /// Value must be one of the specified allowed values.
    Enum {
        /// The list of valid values.
        allowed_values: Vec<ScalarValue>,
    },

    /// String length must be within the specified bounds.
    StringLength {
        /// Minimum string length (inclusive).
        min: usize,
        /// Maximum string length (inclusive).
        max: usize,
    },
}

impl TypeConstraint {
    /// Check if a value satisfies this constraint
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn is_satisfied_by(&self, value: &ScalarValue) -> Result<bool, TypeConstraintError> {
        match self {
            TypeConstraint::Range { min, max } => {
                // Check type compatibility
                if value.scalar_type() != min.scalar_type()
                    || value.scalar_type() != max.scalar_type()
                {
                    return Err(TypeConstraintError::TypeMismatch {
                        expected: min.scalar_type().name().to_string(),
                        actual: value.scalar_type().name().to_string(),
                    });
                }

                Ok(value >= min && value <= max)
            }
            TypeConstraint::Enum { allowed_values } => {
                if !allowed_values.is_empty() {
                    // Check type compatibility with first allowed value
                    if value.scalar_type() != allowed_values[0].scalar_type() {
                        return Err(TypeConstraintError::TypeMismatch {
                            expected: allowed_values[0].scalar_type().name().to_string(),
                            actual: value.scalar_type().name().to_string(),
                        });
                    }
                }

                Ok(allowed_values.contains(value))
            }
            TypeConstraint::StringLength { min, max } => {
                if let ScalarValue::String(s) = value {
                    let len = s.len();
                    Ok(len >= *min && len <= *max)
                } else {
                    Err(TypeConstraintError::TypeMismatch {
                        expected: "String".to_string(),
                        actual: value.scalar_type().name().to_string(),
                    })
                }
            }
        }
    }
}

/// Constraints on an attribute type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeConstraints {
    attribute_name: String,
    scalar_type: ScalarType,
    constraints: Vec<TypeConstraint>,
}

impl AttributeConstraints {
    /// Create new attribute constraints
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn new(attribute_name: String, scalar_type: ScalarType) -> Self {
        Self {
            attribute_name,
            scalar_type,
            constraints: Vec::new(),
        }
    }

    /// Add a constraint
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn with_constraint(mut self, constraint: TypeConstraint) -> Self {
        self.constraints.push(constraint);
        self
    }

    /// Get the attribute name
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn attribute_name(&self) -> &str {
        &self.attribute_name
    }

    /// Get the scalar type
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn scalar_type(&self) -> &ScalarType {
        &self.scalar_type
    }

    /// Get all constraints
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn constraints(&self) -> &[TypeConstraint] {
        &self.constraints
    }

    /// Check if a value satisfies all constraints
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn is_satisfied_by(&self, value: &ScalarValue) -> Result<bool, TypeConstraintError> {
        // Check base type
        if value.scalar_type() != self.scalar_type {
            return Err(TypeConstraintError::TypeMismatch {
                expected: self.scalar_type.name().to_string(),
                actual: value.scalar_type().name().to_string(),
            });
        }

        // Check all constraints
        for constraint in &self.constraints {
            if !constraint.is_satisfied_by(value)? {
                return Ok(false);
            }
        }

        Ok(true)
    }
}

#[cfg(test)]
#[cfg(test)]
mod tests;
