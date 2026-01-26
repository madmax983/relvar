use crate::types::ScalarType;
use crate::values::ScalarValue;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TypeConstraintError {
    #[error("Type constraint violation: value {0:?} does not satisfy constraint")]
    ConstraintViolation(ScalarValue),
    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },
}

/// A constraint on a scalar type
#[derive(Serialize, Deserialize)]
pub enum TypeConstraint {
    /// Value must be within a range (inclusive)
    Range {
        min: ScalarValue,
        max: ScalarValue,
    },
    /// Value must be one of the specified values
    Enum { allowed_values: Vec<ScalarValue> },
    /// String must match a pattern (simplified - just length for now)
    StringLength { min: usize, max: usize },
    /// Integer must be positive
    PositiveInt,
    /// Integer must be non-negative
    NonNegativeInt,
    /// Custom validation function (not serializable, for in-memory use only)
    #[serde(skip)]
    Custom {
        #[serde(skip)]
        validator: Option<Box<dyn Fn(&ScalarValue) -> bool + Send + Sync>>,
        description: String,
    },
}

// Manual Clone implementation since Custom variant contains a function
impl Clone for TypeConstraint {
    fn clone(&self) -> Self {
        match self {
            TypeConstraint::Range { min, max } => TypeConstraint::Range {
                min: min.clone(),
                max: max.clone(),
            },
            TypeConstraint::Enum { allowed_values } => TypeConstraint::Enum {
                allowed_values: allowed_values.clone(),
            },
            TypeConstraint::StringLength { min, max } => TypeConstraint::StringLength {
                min: *min,
                max: *max,
            },
            TypeConstraint::PositiveInt => TypeConstraint::PositiveInt,
            TypeConstraint::NonNegativeInt => TypeConstraint::NonNegativeInt,
            TypeConstraint::Custom {
                validator: _,
                description,
            } => TypeConstraint::Custom {
                validator: None, // Can't clone functions
                description: description.clone(),
            },
        }
    }
}

// Manual Debug implementation since Custom variant contains a function
impl std::fmt::Debug for TypeConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeConstraint::Range { min, max } => f
                .debug_struct("Range")
                .field("min", min)
                .field("max", max)
                .finish(),
            TypeConstraint::Enum { allowed_values } => f
                .debug_struct("Enum")
                .field("allowed_values", allowed_values)
                .finish(),
            TypeConstraint::StringLength { min, max } => f
                .debug_struct("StringLength")
                .field("min", min)
                .field("max", max)
                .finish(),
            TypeConstraint::PositiveInt => f.debug_tuple("PositiveInt").finish(),
            TypeConstraint::NonNegativeInt => f.debug_tuple("NonNegativeInt").finish(),
            TypeConstraint::Custom {
                validator,
                description,
            } => f
                .debug_struct("Custom")
                .field("validator", &validator.is_some())
                .field("description", description)
                .finish(),
        }
    }
}

impl TypeConstraint {
    /// Check if a value satisfies this constraint
    pub fn is_satisfied_by(&self, value: &ScalarValue) -> Result<bool, TypeConstraintError> {
        match self {
            TypeConstraint::Range { min, max } => {
                // Check type compatibility
                if value.scalar_type() != min.scalar_type()
                    || value.scalar_type() != max.scalar_type()
                {
                    return Err(TypeConstraintError::TypeMismatch {
                        expected: min.scalar_type().name(),
                        actual: value.scalar_type().name(),
                    });
                }

                Ok(value >= min && value <= max)
            }
            TypeConstraint::Enum { allowed_values } => {
                if !allowed_values.is_empty() {
                    // Check type compatibility with first allowed value
                    if value.scalar_type() != allowed_values[0].scalar_type() {
                        return Err(TypeConstraintError::TypeMismatch {
                            expected: allowed_values[0].scalar_type().name(),
                            actual: value.scalar_type().name(),
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
                        actual: value.scalar_type().name(),
                    })
                }
            }
            TypeConstraint::PositiveInt => {
                if let ScalarValue::Int(n) = value {
                    Ok(*n > 0)
                } else {
                    Err(TypeConstraintError::TypeMismatch {
                        expected: "Int".to_string(),
                        actual: value.scalar_type().name(),
                    })
                }
            }
            TypeConstraint::NonNegativeInt => {
                if let ScalarValue::Int(n) = value {
                    Ok(*n >= 0)
                } else {
                    Err(TypeConstraintError::TypeMismatch {
                        expected: "Int".to_string(),
                        actual: value.scalar_type().name(),
                    })
                }
            }
            TypeConstraint::Custom {
                validator,
                description: _,
            } => {
                if let Some(f) = validator {
                    Ok(f(value))
                } else {
                    Ok(true) // Skip validation if no function (e.g., after deserialization)
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
    pub fn new(attribute_name: String, scalar_type: ScalarType) -> Self {
        Self {
            attribute_name,
            scalar_type,
            constraints: Vec::new(),
        }
    }

    /// Add a constraint
    pub fn with_constraint(mut self, constraint: TypeConstraint) -> Self {
        self.constraints.push(constraint);
        self
    }

    /// Get the attribute name
    pub fn attribute_name(&self) -> &str {
        &self.attribute_name
    }

    /// Get the scalar type
    pub fn scalar_type(&self) -> &ScalarType {
        &self.scalar_type
    }

    /// Check if a value satisfies all constraints
    pub fn is_satisfied_by(&self, value: &ScalarValue) -> Result<bool, TypeConstraintError> {
        // Check base type
        if value.scalar_type() != self.scalar_type {
            return Err(TypeConstraintError::TypeMismatch {
                expected: self.scalar_type.name(),
                actual: value.scalar_type().name(),
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

        assert!(constraint
            .is_satisfied_by(&ScalarValue::String("red".to_string()))
            .unwrap());
        assert!(constraint
            .is_satisfied_by(&ScalarValue::String("green".to_string()))
            .unwrap());
        assert!(!constraint
            .is_satisfied_by(&ScalarValue::String("yellow".to_string()))
            .unwrap());
    }

    #[test]
    fn test_string_length_constraint() {
        let constraint = TypeConstraint::StringLength { min: 3, max: 10 };

        assert!(constraint
            .is_satisfied_by(&ScalarValue::String("test".to_string()))
            .unwrap());
        assert!(constraint
            .is_satisfied_by(&ScalarValue::String("abc".to_string()))
            .unwrap());
        assert!(constraint
            .is_satisfied_by(&ScalarValue::String("1234567890".to_string()))
            .unwrap());
        assert!(!constraint
            .is_satisfied_by(&ScalarValue::String("ab".to_string()))
            .unwrap());
        assert!(!constraint
            .is_satisfied_by(&ScalarValue::String("12345678901".to_string()))
            .unwrap());
    }

    #[test]
    fn test_positive_int_constraint() {
        let constraint = TypeConstraint::PositiveInt;

        assert!(constraint.is_satisfied_by(&ScalarValue::Int(1)).unwrap());
        assert!(constraint.is_satisfied_by(&ScalarValue::Int(100)).unwrap());
        assert!(!constraint.is_satisfied_by(&ScalarValue::Int(0)).unwrap());
        assert!(!constraint.is_satisfied_by(&ScalarValue::Int(-1)).unwrap());
    }

    #[test]
    fn test_non_negative_int_constraint() {
        let constraint = TypeConstraint::NonNegativeInt;

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
            .with_constraint(TypeConstraint::NonNegativeInt);

        assert!(age_constraints.is_satisfied_by(&ScalarValue::Int(25)).unwrap());
        assert!(age_constraints.is_satisfied_by(&ScalarValue::Int(0)).unwrap());
        assert!(!age_constraints.is_satisfied_by(&ScalarValue::Int(-1)).unwrap());
        assert!(!age_constraints.is_satisfied_by(&ScalarValue::Int(200)).unwrap());
    }

    #[test]
    fn test_attribute_constraints_type_mismatch() {
        let age_constraints =
            AttributeConstraints::new("age".to_string(), ScalarType::Int);

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

        assert!(password_constraints
            .is_satisfied_by(&ScalarValue::String("password123".to_string()))
            .unwrap());
        assert!(!password_constraints
            .is_satisfied_by(&ScalarValue::String("pass".to_string()))
            .unwrap());
    }

    #[test]
    fn test_custom_constraint() {
        let constraint = TypeConstraint::Custom {
            validator: Some(Box::new(|v| {
                if let ScalarValue::Int(n) = v {
                    n % 2 == 0 // Even numbers only
                } else {
                    false
                }
            })),
            description: "Must be even".to_string(),
        };

        assert!(constraint.is_satisfied_by(&ScalarValue::Int(2)).unwrap());
        assert!(constraint.is_satisfied_by(&ScalarValue::Int(100)).unwrap());
        assert!(!constraint.is_satisfied_by(&ScalarValue::Int(1)).unwrap());
        assert!(!constraint.is_satisfied_by(&ScalarValue::Int(99)).unwrap());
    }

    #[test]
    fn test_enum_empty() {
        let constraint = TypeConstraint::Enum {
            allowed_values: vec![],
        };

        assert!(!constraint.is_satisfied_by(&ScalarValue::Int(1)).unwrap());
    }
}
