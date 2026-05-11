use relvar_core::constraints::{
    AttributeConstraints, CheckConstraint, CheckConstraints, CmpOp, ConstraintExpression,
    TypeConstraint, ValueOrRef,
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
fn test_database_update_bulk_validation_fails_check_constraint() {
    let mut db = setup();

    let check = CheckConstraint::new(
        "val_positive".to_string(),
        "val must be positive".to_string(),
        ConstraintExpression::Cmp {
            left: "val".to_string(),
            op: CmpOp::Gt,
            right: ValueOrRef::Value(relvar_core::values::ScalarValue::Int(0)),
        },
    );
    let checks = CheckConstraints::new().with_constraint(check);
    db.set_check_constraints("TEST", checks).unwrap();

    // This will update 'val' to -5, which violates the check constraint.
    // It should be caught in `validate_relation_constraints` (bulk content validation).
    let result = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |t| {
            let mut map = std::collections::BTreeMap::new();
            map.insert("id".to_string(), relvar_core::values::ScalarValue::Int(1));
            map.insert("val".to_string(), relvar_core::values::ScalarValue::Int(-5));
            relvar_core::values::Tuple::new(std::sync::Arc::new(t.tuple_type().clone()), map)
                .unwrap()
        },
    );

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            relvar_core::constraints::ConstraintManagerError::CheckConstraintViolation(_)
        ))
    ));
}

#[test]
fn test_database_update_parent_violates_child_foreign_key() {
    let mut db = setup();

    // Create child table
    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("parent_id", ScalarType::Int),
    );
    db.create_relvar("CHILD", child_type).unwrap();

    let fk = relvar_core::constraints::ForeignKey::new(
        vec!["parent_id".to_string()],
        "TEST".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let fk_constraints =
        relvar_core::constraints::ForeignKeyConstraints::new().with_foreign_key(fk);
    db.set_foreign_key_constraints("CHILD", fk_constraints)
        .unwrap();

    db.insert("CHILD", tuple! { child_id: 10i64, parent_id: 1i64 })
        .unwrap();

    // Update the parent's id, causing the child's reference to become invalid.
    let result = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |t| {
            let mut map = std::collections::BTreeMap::new();
            map.insert("id".to_string(), relvar_core::values::ScalarValue::Int(2));
            map.insert("val".to_string(), relvar_core::values::ScalarValue::Int(10));
            relvar_core::values::Tuple::new(std::sync::Arc::new(t.tuple_type().clone()), map)
                .unwrap()
        },
    );

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            relvar_core::constraints::ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
}

#[test]
fn test_database_delete_parent_violates_child_foreign_key() {
    let mut db = setup();

    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("parent_id", ScalarType::Int),
    );
    db.create_relvar("CHILD", child_type).unwrap();

    let fk = relvar_core::constraints::ForeignKey::new(
        vec!["parent_id".to_string()],
        "TEST".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let fk_constraints =
        relvar_core::constraints::ForeignKeyConstraints::new().with_foreign_key(fk);
    db.set_foreign_key_constraints("CHILD", fk_constraints)
        .unwrap();

    db.insert("CHILD", tuple! { child_id: 10i64, parent_id: 1i64 })
        .unwrap();

    // Delete the parent, causing the child's reference to become invalid.
    let result = db.delete("TEST", |t| t.get_typed::<i64>("id").unwrap() == 1);

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            relvar_core::constraints::ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
}

#[test]
fn test_database_delete_and_update_coverage() {
    use relvar_core::database::Database;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    let mut db = Database::new(InMemoryEngine::new());
    let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    db.create_relvar("USERS", RelationType::new(heading))
        .unwrap();
    db.insert("USERS", tuple! { id: 1i64 }).unwrap();
    db.insert("USERS", tuple! { id: 2i64 }).unwrap();

    let deleted_count = db
        .delete("USERS", |t| t.get_typed::<i64>("id").unwrap() == 1)
        .unwrap();
    assert_eq!(deleted_count, 1);

    let updated_count = db
        .update(
            "USERS",
            |t| t.get_typed::<i64>("id").unwrap() == 2,
            |t| {
                let mut new_t = t.clone();
                let _ = new_t.set("id".to_string(), relvar_core::values::ScalarValue::Int(3));
                new_t
            },
        )
        .unwrap();
    assert_eq!(updated_count, 1);
}

#[test]
fn test_database_insert_type_constraint_violation_detail() {
    use relvar_core::constraints::{AttributeConstraints, TypeConstraint};
    use relvar_core::database::Database;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::ScalarValue;

    let mut db = Database::new(InMemoryEngine::new());
    let heading = TupleType::new().with_attribute("count", ScalarType::Int);
    db.create_relvar("TEST", RelationType::new(heading))
        .unwrap();

    let attr_constraints = AttributeConstraints::new("count".to_string(), ScalarType::Int)
        .with_constraint(TypeConstraint::Range {
            min: ScalarValue::Int(1),
            max: ScalarValue::Int(10),
        });
    db.set_type_constraints("TEST", "count", attr_constraints)
        .unwrap();

    let attr_constraints_bad = AttributeConstraints::new("count".to_string(), ScalarType::Int)
        .with_constraint(TypeConstraint::Range {
            min: ScalarValue::String("1".to_string()),
            max: ScalarValue::Int(10),
        });

    let mut db_bad = Database::new(InMemoryEngine::new());
    db_bad
        .create_relvar(
            "TEST_BAD",
            RelationType::new(TupleType::new().with_attribute("count", ScalarType::Int)),
        )
        .unwrap();
    db_bad
        .set_type_constraints("TEST_BAD", "count", attr_constraints_bad)
        .unwrap();

    assert!(db_bad.insert("TEST_BAD", tuple! { count: 5i64 }).is_err());
}

#[test]
fn test_database_update_key_constraint_violation_uncovered2() {
    let mut db = setup();

    db.insert("TEST", tuple! { id: 2i64, val: 20i64 }).unwrap();

    let pk = relvar_core::constraints::PrimaryKey::new(vec!["id".to_string()]).unwrap();
    let key_cons = relvar_core::constraints::KeyConstraints::new().with_primary_key(pk);
    db.set_key_constraints("TEST", key_cons).unwrap();

    // Update id 2 to id 1, which violates the key constraint
    let result = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 2,
        |_t| tuple! { id: 1i64, val: 20i64 },
    );
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_delete_virtual_relvar_fails_uncovered2() {
    let mut db = setup();
    let rel_type = db.get_relvar_type("TEST").unwrap();
    db.define_virtual_relvar("VIRT", rel_type, |_db| {
        Ok(relvar_core::values::Relation::new(
            relvar_core::types::RelationType::new(relvar_core::types::TupleType::new()),
        ))
    })
    .unwrap();

    let result = db.delete("VIRT", |_| true);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::CannotModifyVirtualRelvar(_))
    ));
}

#[test]
fn test_database_update_key_constraint_bulk_validation() {
    let mut db = setup();

    db.insert("TEST", tuple! { id: 2i64, val: 20i64 }).unwrap();
    db.insert("TEST", tuple! { id: 3i64, val: 30i64 }).unwrap();

    let pk = relvar_core::constraints::PrimaryKey::new(vec!["val".to_string()]).unwrap();
    let key_cons = relvar_core::constraints::KeyConstraints::new().with_primary_key(pk);
    db.set_key_constraints("TEST", key_cons).unwrap();

    // Update id 2 and id 3 to have the same val, which violates the key constraint
    let result = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() >= 2,
        |t| tuple! { id: t.get_typed::<i64>("id").unwrap(), val: 100i64 },
    );
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_update_parent_violates_referencing_foreign_key() {
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

    // Update parent TEST.id to 2 should fail because CHILD has test_id = 1
    let result = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |_t| tuple! { id: 2i64, val: 10i64 },
    );
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}
