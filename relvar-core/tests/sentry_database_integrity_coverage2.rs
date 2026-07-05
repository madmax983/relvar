use relvar_core::constraints::AttributeConstraints;
use relvar_core::constraints::TypeConstraint;
use relvar_core::constraints::{
    CheckConstraint, CheckConstraints, CmpOp, ConstraintExpression, ValueOrRef,
};
use relvar_core::constraints::{ForeignKey, ForeignKeyConstraints};
use relvar_core::database::Database;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::ScalarValue;

#[test]
fn test_set_foreign_key_constraints() {
    let mut db = Database::new(InMemoryEngine::new());

    let dept_type = RelationType::new(TupleType::new().with_attribute("dept_id", ScalarType::Int));
    db.create_relvar("DEPT", dept_type).unwrap();

    let emp_type = RelationType::new(TupleType::new().with_attribute("dept_id", ScalarType::Int));
    db.create_relvar("EMP", emp_type).unwrap();

    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["dept_id".to_string()],
    )
    .unwrap();
    let constraints = ForeignKeyConstraints::new().with_foreign_key(fk);

    let res = db.set_foreign_key_constraints("EMP", constraints);
    assert!(res.is_ok());
}

#[test]
fn test_set_type_constraints() {
    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(TupleType::new().with_attribute("count", ScalarType::Int));
    db.create_relvar("TEST", rel_type).unwrap();

    let attr_constraints = AttributeConstraints::new("count".to_string(), ScalarType::Int)
        .with_constraint(TypeConstraint::Range {
            min: ScalarValue::Int(1),
            max: ScalarValue::Int(i64::MAX),
        });

    let res = db.set_type_constraints("TEST", "count", attr_constraints);
    assert!(res.is_ok());
}

#[test]
fn test_set_check_constraints() {
    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(TupleType::new().with_attribute("age", ScalarType::Int));
    db.create_relvar("PEOPLE", rel_type).unwrap();

    let constraints = CheckConstraints::new().with_constraint(CheckConstraint::new(
        "valid_age",
        "Age must be non-negative",
        ConstraintExpression::Cmp {
            left: "age".to_string(),
            op: CmpOp::Gt,
            right: ValueOrRef::Value(ScalarValue::Int(-1)),
        },
    ));

    let res = db.set_check_constraints("PEOPLE", constraints);
    assert!(res.is_ok());
}
