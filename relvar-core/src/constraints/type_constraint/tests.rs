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
