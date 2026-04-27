use relvar_core::constraints::{
    AttributeConstraints, CheckConstraints, ForeignKey, ForeignKeyConstraints, KeyConstraints,
    PrimaryKey,
};
use relvar_core::database::Database;
use relvar_core::error::DatabaseError;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};

fn setup() -> Database<InMemoryEngine> {
    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("val", ScalarType::Int),
    );
    db.create_relvar("TEST", rel_type).unwrap();
    db.insert("TEST", tuple! { id: 1i64, val: 10i64 }).unwrap();
    db.insert("TEST", tuple! { id: 2i64, val: 20i64 }).unwrap();
    db
}

#[test]
fn test_database_set_key_constraints_fails() {
    let mut db = setup();

    db.insert("TEST", tuple! { id: 3i64, val: 10i64 }).unwrap();

    // Try to set primary key on val, which has duplicates (10i64)
    let pk = relvar_core::constraints::PrimaryKey::new(vec!["val".to_string()]).unwrap();
    let constraints = KeyConstraints::new().with_primary_key(pk);

    let result = db.set_key_constraints("TEST", constraints);
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_set_foreign_key_constraints_fails() {
    let mut db = setup();

    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("test_id", ScalarType::Int),
    );
    db.create_relvar("CHILD", child_type).unwrap();

    // Insert an orphan record
    db.insert("CHILD", tuple! { child_id: 100i64, test_id: 999i64 })
        .unwrap();

    let fk = relvar_core::constraints::ForeignKey::new(
        vec!["test_id".to_string()],
        "TEST".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let constraints = ForeignKeyConstraints::new().with_foreign_key(fk);

    let result = db.set_foreign_key_constraints("CHILD", constraints);
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_set_type_constraints_fails() {
    let mut db = setup();

    let type_cons = relvar_core::constraints::TypeConstraint::Range {
        min: relvar_core::values::ScalarValue::Int(0),
        max: relvar_core::values::ScalarValue::Int(15),
    };

    let constraints =
        AttributeConstraints::new("val".to_string(), ScalarType::Int).with_constraint(type_cons);

    // Should fail because one of the tuples has val=20
    let result = db.set_type_constraints("TEST", "val", constraints);
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_set_check_constraints_fails() {
    let mut db = setup();

    let check_expr = relvar_core::constraints::ConstraintExpression::Cmp {
        left: "val".to_string(),
        op: relvar_core::constraints::CmpOp::Lt,
        right: relvar_core::constraints::ValueOrRef::Value(relvar_core::values::ScalarValue::Int(
            15,
        )),
    };

    let constraints =
        CheckConstraints::new().with_constraint(relvar_core::constraints::CheckConstraint::new(
            "val_lt_15".to_string(),
            "must be less than 15".to_string(),
            check_expr,
        ));

    // Should fail because one tuple has val=20
    let result = db.set_check_constraints("TEST", constraints);
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_set_key_constraints_nonexistent_fails() {
    let mut db = setup();
    let constraints = KeyConstraints::new();
    let result = db.set_key_constraints("NONEXISTENT", constraints);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            relvar_core::constraints::ConstraintManagerError::RelationNotFound(_)
        ))
    ));
}

#[test]
fn test_database_set_foreign_key_constraints_nonexistent_fails() {
    let mut db = setup();
    let constraints = ForeignKeyConstraints::new();
    let result = db.set_foreign_key_constraints("NONEXISTENT", constraints);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            relvar_core::constraints::ConstraintManagerError::RelationNotFound(_)
        ))
    ));
}

#[test]
fn test_database_set_type_constraints_nonexistent_fails() {
    let mut db = setup();
    let constraints = AttributeConstraints::new("val".to_string(), ScalarType::Int);
    let result = db.set_type_constraints("NONEXISTENT", "val", constraints);
    assert!(result.is_err());
    // The type constraint internally triggers a fetch which eventually resolves through map_err returning just Constraint(_)
    assert!(matches!(result, Err(DatabaseError::Constraint(_))));
}

#[test]
fn test_database_set_check_constraints_nonexistent_fails() {
    let mut db = setup();
    let constraints = CheckConstraints::new();
    let result = db.set_check_constraints("NONEXISTENT", constraints);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            relvar_core::constraints::ConstraintManagerError::RelationNotFound(_)
        ))
    ));
}

#[test]
fn test_database_set_key_constraints_success() {
    let mut db = setup();
    let pk = relvar_core::constraints::PrimaryKey::new(vec!["id".to_string()]).unwrap();
    let constraints = KeyConstraints::new().with_primary_key(pk);
    let result = db.set_key_constraints("TEST", constraints);
    assert!(result.is_ok());
}

#[test]
fn test_database_set_foreign_key_constraints_success() {
    let mut db = setup();

    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("test_id", ScalarType::Int),
    );
    db.create_relvar("CHILD", child_type).unwrap();

    db.insert("CHILD", tuple! { child_id: 100i64, test_id: 1i64 })
        .unwrap();

    let fk = relvar_core::constraints::ForeignKey::new(
        vec!["test_id".to_string()],
        "TEST".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let constraints = ForeignKeyConstraints::new().with_foreign_key(fk);

    let result = db.set_foreign_key_constraints("CHILD", constraints);
    assert!(result.is_ok());
}

#[test]
fn test_database_set_type_constraints_success() {
    let mut db = setup();

    let type_cons = relvar_core::constraints::TypeConstraint::Range {
        min: relvar_core::values::ScalarValue::Int(0),
        max: relvar_core::values::ScalarValue::Int(50),
    };

    let constraints =
        AttributeConstraints::new("val".to_string(), ScalarType::Int).with_constraint(type_cons);

    let result = db.set_type_constraints("TEST", "val", constraints);
    assert!(result.is_ok());
}

#[test]
fn test_database_set_check_constraints_success() {
    let mut db = setup();

    let check_expr = relvar_core::constraints::ConstraintExpression::Cmp {
        left: "val".to_string(),
        op: relvar_core::constraints::CmpOp::Gt,
        right: relvar_core::constraints::ValueOrRef::Value(relvar_core::values::ScalarValue::Int(
            5,
        )),
    };

    let constraints =
        CheckConstraints::new().with_constraint(relvar_core::constraints::CheckConstraint::new(
            "val_gt_5".to_string(),
            "must be greater than 5".to_string(),
            check_expr,
        ));

    let result = db.set_check_constraints("TEST", constraints);
    assert!(result.is_ok());
}

#[test]
fn test_get_constraints() {
    let mut db = Database::new(InMemoryEngine::new());

    // Key constraints
    let type1 = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    db.create_relvar("TEST_KEY", type1.clone()).unwrap();
    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    db.set_key_constraints("TEST_KEY", KeyConstraints::new().with_primary_key(pk))
        .unwrap();
    let kc = db.get_key_constraints("TEST_KEY").unwrap();
    assert!(kc.primary_key().is_some());

    // FK constraints
    let type2 = RelationType::new(TupleType::new().with_attribute("ref_id", ScalarType::Int));
    db.create_relvar("TEST_FK", type2.clone()).unwrap();
    let fk = ForeignKey::new(
        vec!["ref_id".to_string()],
        "TEST_KEY".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    db.set_foreign_key_constraints("TEST_FK", ForeignKeyConstraints::new().with_foreign_key(fk))
        .unwrap();
    let fkc = db.get_foreign_key_constraints("TEST_FK").unwrap();
    assert_eq!(fkc.foreign_keys().len(), 1);
}

#[test]
fn test_database_integrity_getters() {
    let mut db = Database::new(InMemoryEngine::new());

    // Setup tables
    let type1 = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    let type2 = RelationType::new(TupleType::new().with_attribute("ref_id", ScalarType::Int));
    db.create_relvar("A", type1).unwrap();
    db.create_relvar("B", type2).unwrap();

    // Set up PK
    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    db.set_key_constraints("A", KeyConstraints::new().with_primary_key(pk))
        .unwrap();

    // Set up FK
    let fk = ForeignKey::new(
        vec!["ref_id".to_string()],
        "A".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
    db.set_foreign_key_constraints("B", constraints).unwrap();

    // Test getters
    let current_fks = db.get_foreign_key_constraints("B").unwrap();
    assert_eq!(current_fks.foreign_keys().len(), 1);

    let current_pks = db.get_key_constraints("A").unwrap();
    assert!(current_pks.primary_key().is_some());

    // Test non-existent
    assert!(db.get_key_constraints("NONEXISTENT").is_none());
    assert!(db.get_foreign_key_constraints("NONEXISTENT").is_none());
    assert!(db.get_foreign_key_constraints("A").is_none());
}

#[test]
fn test_database_integrity_type_and_check() {
    let mut db = Database::new(InMemoryEngine::new());

    // Setup tables
    let type1 = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("age", ScalarType::Int),
    );
    db.create_relvar("PERSON", type1).unwrap();

    // Check type constraints (note: there are no getters for these in db, but they exist on constraints)
    // we just want coverage for the setters
    let attr_constraints = AttributeConstraints::new("age".to_string(), ScalarType::Int)
        .with_constraint(relvar_core::constraints::TypeConstraint::Range {
            min: relvar_core::values::ScalarValue::Int(0),
            max: relvar_core::values::ScalarValue::Int(150),
        });
    db.set_type_constraints("PERSON", "age", attr_constraints)
        .unwrap();

    let checks =
        CheckConstraints::new().with_constraint(relvar_core::constraints::CheckConstraint::new(
            "valid_age".to_string(),
            "Age must be >= 18".to_string(),
            relvar_core::constraints::ConstraintExpression::Cmp {
                left: "age".to_string(),
                op: relvar_core::constraints::CmpOp::Gt,
                right: relvar_core::constraints::ValueOrRef::Value(
                    relvar_core::values::ScalarValue::Int(17),
                ),
            },
        ));
    db.set_check_constraints("PERSON", checks).unwrap();
}

// Data.rs Integrity get/set testing coverage uncovered lines: 142, 143, 144, 178, 179, 180, 181, 223

#[test]
fn test_database_integrity_setters_coverage() {
    use relvar_core::database::Database;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    db.create_relvar("TEST", rel_type.clone()).unwrap();

    let fk_constraints = relvar_core::constraints::ForeignKeyConstraints::new();
    db.set_foreign_key_constraints("TEST", fk_constraints)
        .unwrap();

    let attr_constraints =
        relvar_core::constraints::AttributeConstraints::new("id".to_string(), ScalarType::Int);
    db.set_type_constraints("TEST", "id", attr_constraints)
        .unwrap();

    let check_constraints = relvar_core::constraints::CheckConstraints::new();
    db.set_check_constraints("TEST", check_constraints).unwrap();
}

#[test]
fn test_database_set_key_constraints_coverage() {
    use relvar_core::constraints::{KeyConstraints, PrimaryKey};
    use relvar_core::database::Database;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    db.create_relvar("TEST", rel_type).unwrap();

    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    let constraints = KeyConstraints::new().with_primary_key(pk);

    db.set_key_constraints("TEST", constraints).unwrap();
}

#[test]
fn test_database_set_foreign_key_constraints_coverage() {
    use relvar_core::constraints::{ForeignKey, ForeignKeyConstraints};
    use relvar_core::database::Database;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

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

    db.set_foreign_key_constraints("EMP", constraints).unwrap();
}

#[test]
fn test_database_set_type_constraints_coverage() {
    use relvar_core::constraints::{AttributeConstraints, TypeConstraint};
    use relvar_core::database::Database;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::ScalarValue;

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
}

#[test]
fn test_database_set_check_constraints_coverage() {
    use relvar_core::constraints::{
        CheckConstraint, CheckConstraints, CmpOp, ConstraintExpression, ValueOrRef,
    };
    use relvar_core::database::Database;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::ScalarValue;

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

    db.set_check_constraints("PEOPLE", constraints).unwrap();
}

#[test]
fn test_database_set_constraints_nonexistent_relation() {
    use relvar_core::constraints::{KeyConstraints, PrimaryKey};
    use relvar_core::database::Database;
    use relvar_core::storage_engine::InMemoryEngine;

    let mut db = Database::new(InMemoryEngine::new());

    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    let constraints = KeyConstraints::new().with_primary_key(pk);

    assert!(db.set_key_constraints("TEST", constraints).is_err());
}

#[test]
fn test_database_set_type_constraints_attribute_not_found() {
    use relvar_core::constraints::{AttributeConstraints, TypeConstraint};
    use relvar_core::database::Database;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::ScalarValue;

    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    db.create_relvar("TEST", rel_type).unwrap();

    let attr_constraints = AttributeConstraints::new("count".to_string(), ScalarType::Int)
        .with_constraint(TypeConstraint::Range {
            min: ScalarValue::Int(1),
            max: ScalarValue::Int(10),
        });

    assert!(
        db.set_type_constraints("TEST", "count", attr_constraints)
            .is_err()
    );
}

#[test]
fn test_constraint_manager_foreign_key_violation_error_path() {
    use relvar_core::constraints::{ForeignKey, ForeignKeyConstraints};
    use relvar_core::database::Database;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    let mut db = Database::new(InMemoryEngine::new());

    let dept_type = RelationType::new(TupleType::new().with_attribute("dept_id", ScalarType::Int));
    db.create_relvar("DEPT", dept_type).unwrap();
    db.insert("DEPT", tuple! { dept_id: 1i64 }).unwrap();

    let emp_type = RelationType::new(
        TupleType::new()
            .with_attribute("dept_id", ScalarType::Int)
            .with_attribute("id", ScalarType::Int),
    );
    db.create_relvar("EMP", emp_type).unwrap();
    db.insert("EMP", tuple! { id: 10i64, dept_id: 1i64 })
        .unwrap();

    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["missing_in_dept".to_string()],
    )
    .unwrap();
    let constraints = ForeignKeyConstraints::new().with_foreign_key(fk);

    assert!(db.set_foreign_key_constraints("EMP", constraints).is_err());
}

#[test]
fn test_constraint_manager_validate_referencing_fks_error() {
    use relvar_core::constraints::{ForeignKey, ForeignKeyConstraints};
    use relvar_core::database::Database;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    let mut db = Database::new(InMemoryEngine::new());

    let dept_type = RelationType::new(TupleType::new().with_attribute("dept_id", ScalarType::Int));
    db.create_relvar("DEPT", dept_type).unwrap();
    db.insert("DEPT", tuple! { dept_id: 1i64 }).unwrap();

    let emp_type = RelationType::new(
        TupleType::new()
            .with_attribute("dept_id", ScalarType::Int)
            .with_attribute("id", ScalarType::Int),
    );
    db.create_relvar("EMP", emp_type).unwrap();

    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["dept_id".to_string()],
    )
    .unwrap();
    let constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
    db.set_foreign_key_constraints("EMP", constraints).unwrap();

    db.insert("EMP", tuple! { id: 10i64, dept_id: 1i64 })
        .unwrap();

    let res = db.delete("DEPT", |_| true);
    assert!(res.is_err());
    if let Err(e) = res {
        let err_str = e.to_string();
        assert!(
            err_str.contains("EMP"),
            "Should contain referencing relation name"
        );
    }
}
