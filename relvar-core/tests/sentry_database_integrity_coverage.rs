use relvar_core::constraints::{
    AttributeConstraints, CheckConstraint, CheckConstraints, CmpOp, ConstraintExpression,
    ForeignKey, ForeignKeyConstraints, KeyConstraints, PrimaryKey, TypeConstraint, ValueOrRef,
};
use relvar_core::database::Database;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::ScalarValue;

#[test]
fn test_set_key_constraints_coverage() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    db.create_relvar("TEST", rel_type).unwrap();

    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    let constraints = KeyConstraints::new().with_primary_key(pk);

    let res = db.set_key_constraints("TEST", constraints);
    assert!(res.is_ok());
}

#[test]
fn test_set_foreign_key_constraints_coverage() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    let dept_type = RelationType::new(TupleType::new().with_attribute("dept_id", ScalarType::Int));
    db.create_relvar("DEPT", dept_type).unwrap();

    let emp_type = RelationType::new(TupleType::new().with_attribute("dept_id", ScalarType::Int));
    db.create_relvar("EMP", emp_type).unwrap();

    let pk = PrimaryKey::new(vec!["dept_id".to_string()]).unwrap();
    let pk_constraints = KeyConstraints::new().with_primary_key(pk);
    db.set_key_constraints("DEPT", pk_constraints).unwrap();

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
fn test_set_type_constraints_coverage() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(TupleType::new().with_attribute("name", ScalarType::String));
    db.create_relvar("TEST", rel_type).unwrap();

    let constraints = AttributeConstraints::new("name".to_string(), ScalarType::String)
        .with_constraint(TypeConstraint::StringLength { min: 1, max: 10 });
    let res = db.set_type_constraints("TEST", "name", constraints);
    assert!(res.is_ok());
}

#[test]
fn test_set_check_constraints_coverage() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(TupleType::new().with_attribute("age", ScalarType::Int));
    db.create_relvar("TEST", rel_type).unwrap();

    let constraints = CheckConstraints::new().with_constraint(CheckConstraint::new(
        "positive_age",
        "Age must be positive",
        ConstraintExpression::Cmp {
            left: "age".to_string(),
            op: CmpOp::Gt,
            right: ValueOrRef::Value(ScalarValue::Int(0)),
        },
    ));

    let res = db.set_check_constraints("TEST", constraints);
    assert!(res.is_ok());
}
