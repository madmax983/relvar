//! Tests for user-defined scalar operators (TTM RM Prescription 3).
//!
//! These tests were written before the implementation (TDD): they fail to
//! compile until `OperatorRegistry`, `OperatorSignature` and `OperatorError`
//! exist with the specified API.

use super::*;
use crate::values::ScalarValue;

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

#[test]
fn register_and_invoke_int_operator() {
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

    let result = registry.invoke("double", &[ScalarValue::Int(21)]).unwrap();
    assert_eq!(result, ScalarValue::Int(42));
}

#[test]
fn register_and_invoke_string_helper() {
    let mut registry = OperatorRegistry::new();
    registry
        .register(
            OperatorSignature {
                name: "shout".to_string(),
                param_types: vec![ScalarType::String],
                return_type: ScalarType::String,
            },
            |args| match &args[0] {
                ScalarValue::String(s) => Ok(ScalarValue::String(s.to_uppercase())),
                _ => Err(OperatorError::EvaluationFailed {
                    name: "shout".to_string(),
                    reason: "expected String".to_string(),
                }),
            },
        )
        .unwrap();

    let result = registry
        .invoke("shout", &[ScalarValue::String("hello".to_string())])
        .unwrap();
    assert_eq!(result, ScalarValue::String("HELLO".to_string()));
}

#[test]
fn duplicate_registration_is_rejected() {
    let mut registry = OperatorRegistry::new();
    let signature = OperatorSignature {
        name: "double".to_string(),
        param_types: vec![ScalarType::Int],
        return_type: ScalarType::Int,
    };
    registry
        .register(signature.clone(), |args| Ok(args[0].clone()))
        .unwrap();

    let err = registry
        .register(signature, |args| Ok(args[0].clone()))
        .unwrap_err();
    assert!(
        matches!(err, OperatorError::AlreadyRegistered { .. }),
        "expected AlreadyRegistered, got {err:?}"
    );
}

#[test]
fn unregister_removes_operator() {
    let mut registry = OperatorRegistry::new();
    registry
        .register(
            OperatorSignature {
                name: "double".to_string(),
                param_types: vec![ScalarType::Int],
                return_type: ScalarType::Int,
            },
            |args| Ok(args[0].clone()),
        )
        .unwrap();
    assert!(registry.contains("double", &[ScalarType::Int]));

    registry.unregister("double", &[ScalarType::Int]).unwrap();
    assert!(!registry.contains("double", &[ScalarType::Int]));

    let err = registry
        .invoke("double", &[ScalarValue::Int(1)])
        .unwrap_err();
    assert!(matches!(err, OperatorError::UnknownOperator { .. }));
}

// ---------------------------------------------------------------------------
// Invocation checks (TTM P3(b): declared result type, P3(c): declared params)
// ---------------------------------------------------------------------------

#[test]
fn unknown_operator_errors() {
    let registry = OperatorRegistry::new();
    let err = registry.invoke("nope", &[ScalarValue::Int(1)]).unwrap_err();
    assert!(matches!(err, OperatorError::UnknownOperator { .. }));
}

#[test]
fn arity_mismatch_errors() {
    let mut registry = OperatorRegistry::new();
    registry
        .register(
            OperatorSignature {
                name: "add".to_string(),
                param_types: vec![ScalarType::Int, ScalarType::Int],
                return_type: ScalarType::Int,
            },
            |args| {
                let (a, b) = match (&args[0], &args[1]) {
                    (ScalarValue::Int(a), ScalarValue::Int(b)) => (a, b),
                    _ => {
                        return Err(OperatorError::EvaluationFailed {
                            name: "add".to_string(),
                            reason: "expected Int, Int".to_string(),
                        });
                    }
                };
                Ok(ScalarValue::Int(a + b))
            },
        )
        .unwrap();

    let err = registry.invoke("add", &[ScalarValue::Int(1)]).unwrap_err();
    assert!(
        matches!(err, OperatorError::ArityMismatch { .. }),
        "expected ArityMismatch, got {err:?}"
    );
}

#[test]
fn argument_type_mismatch_errors() {
    let mut registry = OperatorRegistry::new();
    registry
        .register(
            OperatorSignature {
                name: "double".to_string(),
                param_types: vec![ScalarType::Int],
                return_type: ScalarType::Int,
            },
            |args| Ok(args[0].clone()),
        )
        .unwrap();

    // Same name and arity, wrong argument type: no overload matches.
    let err = registry
        .invoke("double", &[ScalarValue::String("x".to_string())])
        .unwrap_err();
    assert!(
        matches!(err, OperatorError::ArgumentTypeMismatch { .. }),
        "expected ArgumentTypeMismatch, got {err:?}"
    );
}

#[test]
fn result_type_is_checked_against_signature() {
    let mut registry = OperatorRegistry::new();
    // A buggy implementation claims to return Int but returns String.
    registry
        .register(
            OperatorSignature {
                name: "broken".to_string(),
                param_types: vec![ScalarType::Int],
                return_type: ScalarType::Int,
            },
            |_| Ok(ScalarValue::String("oops".to_string())),
        )
        .unwrap();

    let err = registry
        .invoke("broken", &[ScalarValue::Int(1)])
        .unwrap_err();
    assert!(
        matches!(err, OperatorError::ResultTypeMismatch { .. }),
        "expected ResultTypeMismatch, got {err:?}"
    );
}

#[test]
fn operator_on_user_defined_type() {
    // TTM story: a user-defined type (POSSREP) with a user-defined operator.
    let date_type = ScalarType::user_defined("Date", ScalarType::String);

    let mut registry = OperatorRegistry::new();
    registry
        .register(
            OperatorSignature {
                name: "year_of".to_string(),
                param_types: vec![date_type.clone()],
                return_type: ScalarType::Int,
            },
            |args| match &args[0] {
                ScalarValue::UserDefined { value, .. } => match value.as_ref() {
                    ScalarValue::String(s) => {
                        let year: i64 =
                            s[0..4]
                                .parse()
                                .map_err(|_| OperatorError::EvaluationFailed {
                                    name: "year_of".to_string(),
                                    reason: format!("bad date: {s}"),
                                })?;
                        Ok(ScalarValue::Int(year))
                    }
                    _ => Err(OperatorError::EvaluationFailed {
                        name: "year_of".to_string(),
                        reason: "expected Date possrep".to_string(),
                    }),
                },
                _ => Err(OperatorError::EvaluationFailed {
                    name: "year_of".to_string(),
                    reason: "expected Date".to_string(),
                }),
            },
        )
        .unwrap();

    let date =
        ScalarValue::select(&date_type, ScalarValue::String("1990-05-17".to_string())).unwrap();
    let result = registry.invoke("year_of", &[date]).unwrap();
    assert_eq!(result, ScalarValue::Int(1990));
}

// ---------------------------------------------------------------------------
// Overloading by parameter types
// ---------------------------------------------------------------------------

#[test]
fn overloads_resolve_by_parameter_types() {
    let mut registry = OperatorRegistry::new();
    registry
        .register(
            OperatorSignature {
                name: "describe".to_string(),
                param_types: vec![ScalarType::Int],
                return_type: ScalarType::String,
            },
            |_| Ok(ScalarValue::String("an int".to_string())),
        )
        .unwrap();
    registry
        .register(
            OperatorSignature {
                name: "describe".to_string(),
                param_types: vec![ScalarType::String],
                return_type: ScalarType::String,
            },
            |_| Ok(ScalarValue::String("a string".to_string())),
        )
        .unwrap();

    assert_eq!(
        registry.invoke("describe", &[ScalarValue::Int(1)]).unwrap(),
        ScalarValue::String("an int".to_string())
    );
    assert_eq!(
        registry
            .invoke("describe", &[ScalarValue::String("x".to_string())])
            .unwrap(),
        ScalarValue::String("a string".to_string())
    );
}
