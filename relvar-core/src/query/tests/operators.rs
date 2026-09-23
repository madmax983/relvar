//! End-to-end tests for user-defined scalar operators in queries
//! (TTM RM Prescription 3).
//!
//! These exercise the full path: `Database::register_operator` ->
//! `ScalarExpression::Apply` inside a `ConstraintExpression::ScalarCmp`
//! predicate -> `Query::restrict` -> `Query::execute`, which resolves
//! operator names against the database's [`OperatorRegistry`].

use super::common::setup_db;
use crate::constraints::{CmpOp, ConstraintExpression, ExpressionError, ScalarExpression};
use crate::query::Query;
use crate::types::{OperatorError, OperatorSignature, ScalarType};
use crate::values::ScalarValue;
use crate::{query, tuple};

/// Registers `decade(age: Int) -> Int` (the decade of an age: 30 for 30..39).
fn register_decade(db: &mut crate::database::Database<crate::storage_engine::InMemoryEngine>) {
    db.register_operator(
        OperatorSignature {
            name: "decade".to_string(),
            param_types: vec![ScalarType::Int],
            return_type: ScalarType::Int,
        },
        |args| match args[0] {
            ScalarValue::Int(age) => Ok(ScalarValue::Int(age / 10 * 10)),
            _ => Err(OperatorError::EvaluationFailed {
                name: "decade".to_string(),
                reason: "expected Int".to_string(),
            }),
        },
    )
    .unwrap();
}

#[test]
fn operator_predicate_filters_tuples() {
    // USERS: Alice (age 30), Bob (age 25). decade(age) = 30 selects Alice.
    let mut db = setup_db();
    register_decade(&mut db);

    let predicate = ConstraintExpression::ScalarCmp {
        left: ScalarExpression::Apply {
            operator: "decade".to_string(),
            args: vec![ScalarExpression::Attribute("age".to_string())],
        },
        op: CmpOp::Eq,
        right: ScalarExpression::Value(ScalarValue::Int(30)),
    };
    let result = Query::scan("USERS")
        .restrict(predicate)
        .execute(&db)
        .unwrap();

    assert_eq!(result.cardinality(), 1);
    let names: Vec<String> = result
        .tuples()
        .map(|t| match t.get("name") {
            Some(ScalarValue::String(name)) => name.clone(),
            _ => String::new(),
        })
        .collect();
    assert_eq!(names, vec!["Alice".to_string()]);
}

#[test]
fn string_operator_in_predicate() {
    let mut db = setup_db();
    db.register_operator(
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

    // shout(name) = "BOB" selects Bob.
    let predicate = ConstraintExpression::ScalarCmp {
        left: ScalarExpression::Apply {
            operator: "shout".to_string(),
            args: vec![ScalarExpression::Attribute("name".to_string())],
        },
        op: CmpOp::Eq,
        right: ScalarExpression::Value(ScalarValue::String("BOB".to_string())),
    };
    let result = Query::scan("USERS")
        .restrict(predicate)
        .execute(&db)
        .unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn unregistered_operator_in_predicate_is_a_query_error() {
    let db = setup_db();
    let predicate = ConstraintExpression::ScalarCmp {
        left: ScalarExpression::Apply {
            operator: "nope".to_string(),
            args: vec![ScalarExpression::Attribute("age".to_string())],
        },
        op: CmpOp::Gt,
        right: ScalarExpression::Value(ScalarValue::Int(0)),
    };
    let err = Query::scan("USERS")
        .restrict(predicate)
        .execute(&db)
        .unwrap_err();
    assert!(
        matches!(
            err,
            crate::query::QueryError::Constraint(ExpressionError::Operator(_))
        ),
        "expected a constraint error wrapping an operator error, got {err:?}"
    );
}

#[test]
fn age_operator_on_possrep_date_type() {
    // The TTM showcase: a user-defined type (POSSREP Date) with a
    // user-defined operator `age_on(as_of, birthdate)`, used in a query
    // predicate like Tutorial D's `age(birthdate)`.
    use crate::database::Database;
    use crate::storage_engine::InMemoryEngine;
    use crate::types::{RelationType, TupleType};

    fn parse_date(value: &ScalarValue) -> Result<(i64, i64, i64), OperatorError> {
        let possrep = match value {
            ScalarValue::UserDefined { value, .. } => value.clone(),
            _ => {
                return Err(OperatorError::EvaluationFailed {
                    name: "age_on".to_string(),
                    reason: "expected Date".to_string(),
                });
            }
        };
        let text = match possrep.as_ref() {
            ScalarValue::String(s) => s.clone(),
            _ => {
                return Err(OperatorError::EvaluationFailed {
                    name: "age_on".to_string(),
                    reason: "expected Date possrep".to_string(),
                });
            }
        };
        let parts: Vec<i64> = text
            .split('-')
            .map(|p| {
                p.parse().map_err(|_| OperatorError::EvaluationFailed {
                    name: "age_on".to_string(),
                    reason: format!("bad date: {text}"),
                })
            })
            .collect::<Result<_, _>>()?;
        if parts.len() != 3 {
            return Err(OperatorError::EvaluationFailed {
                name: "age_on".to_string(),
                reason: format!("bad date: {text}"),
            });
        }
        Ok((parts[0], parts[1], parts[2]))
    }

    let date_type = ScalarType::user_defined("Date", ScalarType::String);
    let mut db = Database::new(InMemoryEngine::new());
    let heading = TupleType::new()
        .with_attribute("name", ScalarType::String)
        .with_attribute("birthdate", date_type.clone());
    db.create_relvar("PEOPLE", RelationType::new(heading))
        .unwrap();

    let insert_person = |db: &mut Database<InMemoryEngine>, name: &str, birth: &str| {
        let birth_value =
            ScalarValue::select(&date_type, ScalarValue::String(birth.to_string())).unwrap();
        db.insert(
            "PEOPLE",
            tuple! { name: name.to_string(), birthdate: birth_value },
        )
        .unwrap();
    };
    insert_person(&mut db, "Alice", "1990-05-17");
    insert_person(&mut db, "Bob", "2015-11-02");

    // age_on(as_of: Date, birth: Date) -> Int, full years elapsed.
    db.register_operator(
        OperatorSignature {
            name: "age_on".to_string(),
            param_types: vec![date_type.clone(), date_type.clone()],
            return_type: ScalarType::Int,
        },
        move |args| {
            let (ay, am, ad) = parse_date(&args[0])?;
            let (by, bm, bd) = parse_date(&args[1])?;
            let mut years = ay - by;
            if (am, ad) < (bm, bd) {
                years -= 1;
            }
            Ok(ScalarValue::Int(years))
        },
    )
    .unwrap();

    let as_of =
        ScalarValue::select(&date_type, ScalarValue::String("2026-09-23".to_string())).unwrap();

    // age_on(Date("2026-09-23"), birthdate) >= 30  ->  Alice only
    // (Alice: 36, Bob: 10).
    let predicate = ConstraintExpression::ScalarCmp {
        left: ScalarExpression::Apply {
            operator: "age_on".to_string(),
            args: vec![
                ScalarExpression::Value(as_of),
                ScalarExpression::Attribute("birthdate".to_string()),
            ],
        },
        op: CmpOp::Ge,
        right: ScalarExpression::Value(ScalarValue::Int(30)),
    };
    let result = query::Query::scan("PEOPLE")
        .restrict(predicate)
        .execute(&db)
        .unwrap();
    assert_eq!(result.cardinality(), 1);
    assert_eq!(
        result.tuples().next().unwrap().get("name"),
        Some(&ScalarValue::String("Alice".to_string()))
    );
}
