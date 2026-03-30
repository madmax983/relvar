use relvar_core::constraints::{
    AttributeConstraints, CandidateKey, CheckConstraint, CheckConstraints, CmpOp,
    ConstraintExpression, KeyConstraints, TypeConstraint, ValueOrRef,
};
use relvar_core::database::Database;
use relvar_core::error::DatabaseError;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::ScalarValue;

fn setup() -> Database<InMemoryEngine> {
    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("val", ScalarType::Int),
    );
    db.create_relvar("TEST", rel_type).unwrap();
    db.insert("TEST", tuple! { id: 1i64, val: 10i64 }).unwrap();
    db
}

#[test]
fn test_database_delete_foreign_key_violation() {
    let mut db = setup();

    // Create child table
    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("test_id", ScalarType::Int),
    );
    db.create_relvar("CHILD", child_type).unwrap();

    let fk = relvar_core::constraints::ForeignKey::new(
        vec!["test_id".to_string()],
        "TEST".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let fk_constraints =
        relvar_core::constraints::ForeignKeyConstraints::new().with_foreign_key(fk);
    db.set_foreign_key_constraints("CHILD", fk_constraints)
        .unwrap();

    db.insert("CHILD", tuple! { child_id: 100i64, test_id: 1i64 })
        .unwrap();

    // This delete should fail due to foreign key constraint
    let result = db.delete("TEST", |t| t.get_typed::<i64>("id").unwrap() == 1);
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_update_type_constraint_violation() {
    let mut db = setup();

    let type_cons = TypeConstraint::Range {
        min: ScalarValue::Int(0),
        max: ScalarValue::Int(100),
    };
    db.set_type_constraints(
        "TEST",
        "val",
        AttributeConstraints::new("val".to_string(), ScalarType::Int).with_constraint(type_cons),
    )
    .unwrap();

    // Update should fail due to type constraint violation
    let result = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |_t| tuple! { id: 1i64, val: 200i64 },
    );

    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_insert_check_constraint_violation() {
    let mut db = setup();

    let check_expr = ConstraintExpression::Cmp {
        left: "val".to_string(),
        op: CmpOp::Gt,
        right: ValueOrRef::Value(ScalarValue::Int(0)),
    };
    let checks = CheckConstraints::new().with_constraint(CheckConstraint::new(
        "val_positive".to_string(),
        "must be positive".to_string(),
        check_expr,
    ));
    db.set_check_constraints("TEST", checks).unwrap();

    // Insert should fail due to check constraint
    let result = db.insert("TEST", tuple! { id: 2i64, val: -10i64 });
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_insert_type_constraint_violation() {
    let mut db = setup();

    let type_cons = TypeConstraint::Range {
        min: ScalarValue::Int(0),
        max: ScalarValue::Int(100),
    };
    db.set_type_constraints(
        "TEST",
        "val",
        AttributeConstraints::new("val".to_string(), ScalarType::Int).with_constraint(type_cons),
    )
    .unwrap();

    // Insert should fail due to type constraint violation
    let result = db.insert("TEST", tuple! { id: 2i64, val: 200i64 });

    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_insert_duplicate_primary_key() {
    let mut db = setup();

    let pk = relvar_core::constraints::PrimaryKey::new(vec!["id".to_string()]).unwrap();
    let key_constraints = relvar_core::constraints::KeyConstraints::new().with_primary_key(pk);
    db.set_key_constraints("TEST", key_constraints).unwrap();

    // Insert should fail due to duplicate primary key
    let result = db.insert("TEST", tuple! { id: 1i64, val: 20i64 });
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_update_referencing_foreign_keys_violation() {
    let mut db = setup();

    // Create parent table (TEST is already created)

    // Create child table
    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("test_id", ScalarType::Int),
    );
    db.create_relvar("CHILD", child_type).unwrap();

    let fk = relvar_core::constraints::ForeignKey::new(
        vec!["test_id".to_string()],
        "TEST".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let fk_constraints =
        relvar_core::constraints::ForeignKeyConstraints::new().with_foreign_key(fk);
    db.set_foreign_key_constraints("CHILD", fk_constraints)
        .unwrap();

    db.insert("CHILD", tuple! { child_id: 100i64, test_id: 1i64 })
        .unwrap();

    // This update should fail due to foreign key constraint (updating the PK of the parent which is referenced)
    let result = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |_t| tuple! { id: 2i64, val: 10i64 },
    );
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_update_key_constraint_violation() {
    let mut db = setup();

    let pk = relvar_core::constraints::PrimaryKey::new(vec!["id".to_string()]).unwrap();
    let key_constraints = relvar_core::constraints::KeyConstraints::new().with_primary_key(pk);
    db.set_key_constraints("TEST", key_constraints).unwrap();

    db.insert("TEST", tuple! { id: 2i64, val: 20i64 }).unwrap();

    // Update should fail due to duplicate primary key
    let result = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 2,
        |_t| tuple! { id: 1i64, val: 20i64 }, // duplicate id 1
    );
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_update_check_constraint_violation() {
    let mut db = setup();

    let check_expr = ConstraintExpression::Cmp {
        left: "val".to_string(),
        op: CmpOp::Gt,
        right: ValueOrRef::Value(ScalarValue::Int(0)),
    };
    let checks = CheckConstraints::new().with_constraint(CheckConstraint::new(
        "val_positive".to_string(),
        "must be positive".to_string(),
        check_expr,
    ));
    db.set_check_constraints("TEST", checks).unwrap();

    // Update should fail due to check constraint
    let result = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |_t| tuple! { id: 1i64, val: -10i64 },
    );
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_insert_foreign_key_violation() {
    let mut db = setup();

    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("test_id", ScalarType::Int),
    );
    db.create_relvar("CHILD", child_type).unwrap();

    let fk = relvar_core::constraints::ForeignKey::new(
        vec!["test_id".to_string()],
        "TEST".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let fk_constraints =
        relvar_core::constraints::ForeignKeyConstraints::new().with_foreign_key(fk);
    db.set_foreign_key_constraints("CHILD", fk_constraints)
        .unwrap();

    // Insert should fail due to missing parent key (id = 999)
    let result = db.insert("CHILD", tuple! { child_id: 100i64, test_id: 999i64 });
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_update_foreign_key_violation() {
    let mut db = setup();

    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("test_id", ScalarType::Int),
    );
    db.create_relvar("CHILD", child_type).unwrap();

    let fk = relvar_core::constraints::ForeignKey::new(
        vec!["test_id".to_string()],
        "TEST".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let fk_constraints =
        relvar_core::constraints::ForeignKeyConstraints::new().with_foreign_key(fk);
    db.set_foreign_key_constraints("CHILD", fk_constraints)
        .unwrap();

    db.insert("CHILD", tuple! { child_id: 100i64, test_id: 1i64 })
        .unwrap();

    // Update should fail due to missing parent key (id = 999)
    let result = db.update(
        "CHILD",
        |t| t.get_typed::<i64>("child_id").unwrap() == 100,
        |_t| tuple! { child_id: 100i64, test_id: 999i64 },
    );
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_delete_successful() {
    let mut db = setup();
    let count = db.delete("TEST", |t| t.get_typed::<i64>("id").unwrap() == 1).unwrap();
    assert_eq!(count, 1);

    // Deleting again should be 0
    let count2 = db.delete("TEST", |t| t.get_typed::<i64>("id").unwrap() == 1).unwrap();
    assert_eq!(count2, 0);
}

#[test]
fn test_database_update_successful() {
    let mut db = setup();
    let count = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |_t| tuple! { id: 1i64, val: 55i64 },
    )
    .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn test_database_insert_candidate_key_violation() {
    let mut db = setup();

    let ck = CandidateKey::new(vec!["val".to_string()]).unwrap();
    let key_constraints = KeyConstraints::new().with_candidate_key(ck);
    db.set_key_constraints("TEST", key_constraints).unwrap();

    // Insert should fail due to duplicate candidate key
    let result = db.insert("TEST", tuple! { id: 2i64, val: 10i64 });
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_update_candidate_key_violation() {
    let mut db = setup();

    let ck = CandidateKey::new(vec!["val".to_string()]).unwrap();
    let key_constraints = KeyConstraints::new().with_candidate_key(ck);
    db.set_key_constraints("TEST", key_constraints).unwrap();

    db.insert("TEST", tuple! { id: 2i64, val: 20i64 }).unwrap();

    // Update should fail due to duplicate candidate key
    let result = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 2,
        |_t| tuple! { id: 2i64, val: 10i64 }, // duplicate val 10
    );
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}
