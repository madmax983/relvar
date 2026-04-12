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
/// # Example
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
    /// # Example
    ///
    /// ```ignore
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
    /// # Example
    ///
    /// ```ignore
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
    /// # Example
    ///
    /// ```ignore
    /// // Example
    /// ```
    pub fn with_constraint(mut self, constraint: TypeConstraint) -> Self {
        self.constraints.push(constraint);
        self
    }

    /// Get the attribute name
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Example
    /// ```
    pub fn attribute_name(&self) -> &str {
        &self.attribute_name
    }

    /// Get the scalar type
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Example
    /// ```
    pub fn scalar_type(&self) -> &ScalarType {
        &self.scalar_type
    }

    /// Get all constraints
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Example
    /// ```
    pub fn constraints(&self) -> &[TypeConstraint] {
        &self.constraints
    }

    /// Check if a value satisfies all constraints
    ///
    /// # Example
    ///
    /// ```ignore
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
mod tests {
    use super::*;

    #[test]
    fn test_range_constraint_int() {
        let constraint = TypeConstraint::Range {
            min: ScalarValue::Int(0),
            max: ScalarValue::Int(100),
        };

        assert!(constraint.is_satisfied_by(&ScalarValue::Int(50)).unwrap());
        assert!(constraint.is_satisfied_by(&ScalarValue::Int(0)).unwrap());
        assert!(constraint.is_satisfied_by(&ScalarValue::Int(100)).unwrap());
        assert!(!constraint.is_satisfied_by(&ScalarValue::Int(-1)).unwrap());
        assert!(!constraint.is_satisfied_by(&ScalarValue::Int(101)).unwrap());
    }

    #[test]
    fn test_range_constraint_type_mismatch() {
        let constraint = TypeConstraint::Range {
            min: ScalarValue::Int(0),
            max: ScalarValue::Int(100),
        };

        let result = constraint.is_satisfied_by(&ScalarValue::String("test".to_string()));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TypeConstraintError::TypeMismatch { .. }
        ));
    }

    #[test]
    fn test_enum_constraint() {
        let constraint = TypeConstraint::Enum {
            allowed_values: vec![
                ScalarValue::String("red".to_string()),
                ScalarValue::String("green".to_string()),
                ScalarValue::String("blue".to_string()),
            ],
        };

        assert!(
            constraint
                .is_satisfied_by(&ScalarValue::String("red".to_string()))
                .unwrap()
        );
        assert!(
            constraint
                .is_satisfied_by(&ScalarValue::String("green".to_string()))
                .unwrap()
        );
        assert!(
            !constraint
                .is_satisfied_by(&ScalarValue::String("yellow".to_string()))
                .unwrap()
        );
    }

    #[test]
    fn test_string_length_constraint() {
        let constraint = TypeConstraint::StringLength { min: 3, max: 10 };

        assert!(
            constraint
                .is_satisfied_by(&ScalarValue::String("test".to_string()))
                .unwrap()
        );
        assert!(
            constraint
                .is_satisfied_by(&ScalarValue::String("abc".to_string()))
                .unwrap()
        );
        assert!(
            constraint
                .is_satisfied_by(&ScalarValue::String("1234567890".to_string()))
                .unwrap()
        );
        assert!(
            !constraint
                .is_satisfied_by(&ScalarValue::String("ab".to_string()))
                .unwrap()
        );
        assert!(
            !constraint
                .is_satisfied_by(&ScalarValue::String("12345678901".to_string()))
                .unwrap()
        );
    }

    #[test]
    fn test_positive_int_replacement() {
        // Replacement for PositiveInt: Range { min: 1, max: MAX }
        let constraint = TypeConstraint::Range {
            min: ScalarValue::Int(1),
            max: ScalarValue::Int(i64::MAX),
        };

        assert!(constraint.is_satisfied_by(&ScalarValue::Int(1)).unwrap());
        assert!(constraint.is_satisfied_by(&ScalarValue::Int(100)).unwrap());
        assert!(!constraint.is_satisfied_by(&ScalarValue::Int(0)).unwrap());
        assert!(!constraint.is_satisfied_by(&ScalarValue::Int(-1)).unwrap());
    }

    #[test]
    fn test_non_negative_int_replacement() {
        // Replacement for NonNegativeInt: Range { min: 0, max: MAX }
        let constraint = TypeConstraint::Range {
            min: ScalarValue::Int(0),
            max: ScalarValue::Int(i64::MAX),
        };

        assert!(constraint.is_satisfied_by(&ScalarValue::Int(0)).unwrap());
        assert!(constraint.is_satisfied_by(&ScalarValue::Int(1)).unwrap());
        assert!(constraint.is_satisfied_by(&ScalarValue::Int(100)).unwrap());
        assert!(!constraint.is_satisfied_by(&ScalarValue::Int(-1)).unwrap());
    }

    #[test]
    fn test_attribute_constraints() {
        let age_constraints = AttributeConstraints::new("age".to_string(), ScalarType::Int)
            .with_constraint(TypeConstraint::Range {
                min: ScalarValue::Int(0),
                max: ScalarValue::Int(150),
            })
            // Use Range instead of NonNegativeInt
            .with_constraint(TypeConstraint::Range {
                min: ScalarValue::Int(0),
                max: ScalarValue::Int(i64::MAX),
            });

        assert!(
            age_constraints
                .is_satisfied_by(&ScalarValue::Int(25))
                .unwrap()
        );
        assert!(
            age_constraints
                .is_satisfied_by(&ScalarValue::Int(0))
                .unwrap()
        );
        assert!(
            !age_constraints
                .is_satisfied_by(&ScalarValue::Int(-1))
                .unwrap()
        );
        assert!(
            !age_constraints
                .is_satisfied_by(&ScalarValue::Int(200))
                .unwrap()
        );
    }

    #[test]
    fn test_attribute_constraints_type_mismatch() {
        let age_constraints = AttributeConstraints::new("age".to_string(), ScalarType::Int);

        let result = age_constraints.is_satisfied_by(&ScalarValue::String("25".to_string()));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TypeConstraintError::TypeMismatch { .. }
        ));
    }

    #[test]
    fn test_multiple_constraints_all_must_pass() {
        let password_constraints =
            AttributeConstraints::new("password".to_string(), ScalarType::String)
                .with_constraint(TypeConstraint::StringLength { min: 8, max: 100 });

        assert!(
            password_constraints
                .is_satisfied_by(&ScalarValue::String("password123".to_string()))
                .unwrap()
        );
        assert!(
            !password_constraints
                .is_satisfied_by(&ScalarValue::String("pass".to_string()))
                .unwrap()
        );
    }

    #[test]
    fn test_enum_empty() {
        let constraint = TypeConstraint::Enum {
            allowed_values: vec![],
        };

        assert!(!constraint.is_satisfied_by(&ScalarValue::Int(1)).unwrap());
    }

    #[test]
    fn test_range_constraint_min_equals_max() {
        let constraint = TypeConstraint::Range {
            min: ScalarValue::Int(5),
            max: ScalarValue::Int(5),
        };

        // Exactly 5 should pass
        assert!(constraint.is_satisfied_by(&ScalarValue::Int(5)).unwrap());

        // Other values should fail
        assert!(!constraint.is_satisfied_by(&ScalarValue::Int(4)).unwrap());
        assert!(!constraint.is_satisfied_by(&ScalarValue::Int(6)).unwrap());
    }

    #[test]
    fn test_range_constraint_float() {
        let constraint = TypeConstraint::Range {
            min: ScalarValue::Float(0.0),
            max: ScalarValue::Float(1.0),
        };

        assert!(
            constraint
                .is_satisfied_by(&ScalarValue::Float(0.0))
                .unwrap()
        );
        assert!(
            constraint
                .is_satisfied_by(&ScalarValue::Float(0.5))
                .unwrap()
        );
        assert!(
            constraint
                .is_satisfied_by(&ScalarValue::Float(1.0))
                .unwrap()
        );
        assert!(
            !constraint
                .is_satisfied_by(&ScalarValue::Float(-0.1))
                .unwrap()
        );
        assert!(
            !constraint
                .is_satisfied_by(&ScalarValue::Float(1.1))
                .unwrap()
        );
    }

    #[test]
    fn test_string_length_min_equals_max() {
        let constraint = TypeConstraint::StringLength { min: 5, max: 5 };

        // Exactly 5 chars should pass
        assert!(
            constraint
                .is_satisfied_by(&ScalarValue::String("hello".to_string()))
                .unwrap()
        );

        // Other lengths should fail
        assert!(
            !constraint
                .is_satisfied_by(&ScalarValue::String("hi".to_string()))
                .unwrap()
        );
        assert!(
            !constraint
                .is_satisfied_by(&ScalarValue::String("hello!".to_string()))
                .unwrap()
        );
    }

    #[test]
    fn test_string_length_empty_string() {
        let constraint = TypeConstraint::StringLength { min: 1, max: 10 };

        // Empty string should fail
        assert!(
            !constraint
                .is_satisfied_by(&ScalarValue::String("".to_string()))
                .unwrap()
        );
    }

    #[test]
    fn test_string_length_zero_min() {
        let constraint = TypeConstraint::StringLength { min: 0, max: 5 };

        // Empty string should pass with min=0
        assert!(
            constraint
                .is_satisfied_by(&ScalarValue::String("".to_string()))
                .unwrap()
        );
    }

    #[test]
    fn test_enum_constraint_with_different_types() {
        let constraint = TypeConstraint::Enum {
            allowed_values: vec![
                ScalarValue::Int(1),
                ScalarValue::Int(2),
                ScalarValue::Int(3),
            ],
        };

        // Wrong type should return error (type mismatch)
        let result = constraint.is_satisfied_by(&ScalarValue::String("1".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_range_constraint_string() {
        let constraint = TypeConstraint::Range {
            min: ScalarValue::String("a".to_string()),
            max: ScalarValue::String("z".to_string()),
        };

        assert!(
            constraint
                .is_satisfied_by(&ScalarValue::String("m".to_string()))
                .unwrap()
        );
        assert!(
            constraint
                .is_satisfied_by(&ScalarValue::String("a".to_string()))
                .unwrap()
        );
        assert!(
            constraint
                .is_satisfied_by(&ScalarValue::String("z".to_string()))
                .unwrap()
        );
    }

    #[test]
    fn test_attribute_constraints_multiple_failures() {
        let constraints = AttributeConstraints::new("age".to_string(), ScalarType::Int)
            // Use Range instead of PositiveInt
            .with_constraint(TypeConstraint::Range {
                min: ScalarValue::Int(1),
                max: ScalarValue::Int(i64::MAX),
            })
            .with_constraint(TypeConstraint::Range {
                min: ScalarValue::Int(1),
                max: ScalarValue::Int(120),
            });

        // Value that fails multiple constraints should return false
        let result = constraints.is_satisfied_by(&ScalarValue::Int(-5));
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}
