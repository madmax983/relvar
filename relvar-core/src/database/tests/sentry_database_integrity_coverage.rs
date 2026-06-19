use crate::constraints::{AttributeConstraints, TypeConstraint};
use crate::constraints::{
    CheckConstraint, CheckConstraints, CmpOp, ConstraintExpression, ValueOrRef,
};
use crate::constraints::{ForeignKey, ForeignKeyConstraints};
use crate::constraints::{KeyConstraints, PrimaryKey};
use crate::database::Database;
use crate::storage_engine::InMemoryEngine;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::ScalarValue;

#[test]
fn test_database_integrity_getters() {
    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    db.create_relvar("TEST", rel_type.clone()).unwrap();

    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    db.set_key_constraints("TEST", KeyConstraints::new().with_primary_key(pk))
        .unwrap();

    let constraints = db.get_key_constraints("TEST").unwrap();
    assert!(constraints.primary_key().is_some());
    assert_eq!(
        constraints.primary_key().unwrap().attributes(),
        &vec!["id".to_string()]
    );
    assert!(db.get_key_constraints("NONEXISTENT").is_none());
}

#[test]
fn test_database_integrity_fk_getter() {
    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    let child_type = RelationType::new(TupleType::new().with_attribute("ref_id", ScalarType::Int));
    db.create_relvar("A", rel_type).unwrap();
    db.create_relvar("B", child_type).unwrap();

    let fk = ForeignKey::new(
        vec!["ref_id".to_string()],
        "A".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
    db.set_foreign_key_constraints("B", constraints).unwrap();

    let current_fks = db.get_foreign_key_constraints("B").unwrap();
    assert_eq!(current_fks.foreign_keys().len(), 1);
    assert!(db.get_foreign_key_constraints("NONEXISTENT").is_none());
}

#[test]
fn test_database_integrity_setters_coverage() {
    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(TupleType::new().with_attribute("count", ScalarType::Int));
    db.create_relvar("TEST", rel_type).unwrap();

    let attr_constraints = AttributeConstraints::new("count".to_string(), ScalarType::Int)
        .with_constraint(TypeConstraint::Range {
            min: ScalarValue::Int(1),
            max: ScalarValue::Int(i64::MAX),
        });
    db.set_type_constraints("TEST", "count", attr_constraints)
        .unwrap();

    let checks = CheckConstraints::new().with_constraint(CheckConstraint::new(
        "valid_count",
        "Count must be > 0",
        ConstraintExpression::Cmp {
            left: "count".to_string(),
            op: CmpOp::Gt,
            right: ValueOrRef::Value(ScalarValue::Int(0)),
        },
    ));

    db.set_check_constraints("TEST", checks.clone()).unwrap();
}

#[test]
fn test_database_schema_drop_and_get() {
    let mut db = Database::new(InMemoryEngine::new());
    let err = db.drop_relvar("DOES_NOT_EXIST").unwrap_err();
    assert!(matches!(err, crate::error::DatabaseError::Storage(_)));
    let err2 = db.get_relvar_type("DOES_NOT_EXIST").unwrap_err();
    assert!(matches!(err2, crate::error::DatabaseError::Storage(_)));
}
