use super::common::*;
use crate::constraints::ConstraintManagerError;
use crate::constraints::expression::{CmpOp, ConstraintExpression, ValueOrRef};
use crate::constraints::{CheckConstraint, CheckConstraints};
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::InMemoryEngine;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::ScalarValue;

#[test]
fn test_primary_key_constraint() {
    use crate::constraints::{KeyConstraints, PrimaryKey};

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    // Add primary key constraint
    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    let constraints = KeyConstraints::new().with_primary_key(pk);
    db.set_key_constraints("TEST", constraints).unwrap();

    // First insert should succeed
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Duplicate primary key should fail
    let result = db.insert("TEST", tuple! { id: 1i64, name: "Bob" });
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
}

#[test]
fn test_candidate_key_constraint() {
    use crate::constraints::{CandidateKey, KeyConstraints};

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    // Add candidate key constraint on name
    let ck = CandidateKey::new(vec!["name".to_string()]).unwrap();
    let constraints = KeyConstraints::new().with_candidate_key(ck);
    db.set_key_constraints("TEST", constraints).unwrap();

    // First insert should succeed
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Duplicate candidate key should fail
    let result = db.insert("TEST", tuple! { id: 2i64, name: "Alice" });
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::CandidateKeyViolation
        ))
    ));
}

#[test]
fn test_foreign_key_constraint() {
    use crate::constraints::{ForeignKey, ForeignKeyConstraints};

    let mut db = setup_parent_child_db();

    // Insert parent record
    db.insert("PARENT", tuple! { id: 10i64, name: "Engineering" })
        .unwrap();

    // Add foreign key constraint
    let fk = ForeignKey::new(
        vec!["parent_id".to_string()],
        "PARENT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
    db.set_foreign_key_constraints("CHILD", fk_constraints)
        .unwrap();

    // Insert with valid foreign key should succeed
    db.insert("CHILD", tuple! { child_id: 1i64, parent_id: 10i64 })
        .unwrap();

    // Insert with invalid foreign key should fail
    let result = db.insert("CHILD", tuple! { child_id: 2i64, parent_id: 99i64 });
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
}

#[test]
fn test_type_constraint_range() {
    use crate::constraints::AttributeConstraints;
    use crate::constraints::type_constraint::TypeConstraint;
    use crate::values::ScalarValue;

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    // Add range constraint on id (1..100)
    let range_constraint = TypeConstraint::Range {
        min: ScalarValue::Int(1),
        max: ScalarValue::Int(100),
    };
    let attr_constraints = AttributeConstraints::new("id".to_string(), ScalarType::Int)
        .with_constraint(range_constraint);
    db.set_type_constraints("TEST", "id", attr_constraints)
        .unwrap();

    // Valid value should succeed
    db.insert("TEST", tuple! { id: 50i64, name: "Alice" })
        .unwrap();

    // Value below min should fail
    let result = db.insert("TEST", tuple! { id: 0i64, name: "Bob" });
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::TypeConstraintViolation(_)
        ))
    ));

    // Value above max should fail
    let result = db.insert("TEST", tuple! { id: 101i64, name: "Charlie" });
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::TypeConstraintViolation(_)
        ))
    ));
}

#[test]
fn test_type_constraint_positive_int() {
    use crate::constraints::AttributeConstraints;
    use crate::constraints::type_constraint::TypeConstraint;

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    // Add positive int constraint
    let attr_constraints = AttributeConstraints::new("id".to_string(), ScalarType::Int)
        .with_constraint(TypeConstraint::Range {
            min: ScalarValue::Int(1),
            max: ScalarValue::Int(i64::MAX),
        });
    db.set_type_constraints("TEST", "id", attr_constraints)
        .unwrap();

    // Positive value should succeed
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Zero should fail
    let result = db.insert("TEST", tuple! { id: 0i64, name: "Bob" });
    assert!(result.is_err());

    // Negative should fail
    let result = db.insert("TEST", tuple! { id: -1i64, name: "Charlie" });
    assert!(result.is_err());
}

#[test]
fn test_delete_with_foreign_key_constraint() {
    use crate::constraints::{ForeignKey, ForeignKeyConstraints};

    let mut db = setup_parent_child_db();

    let fk = ForeignKey::new(
        vec!["parent_id".to_string()],
        "PARENT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
    db.set_foreign_key_constraints("CHILD", fk_constraints)
        .unwrap();

    // Insert parent and child
    db.insert("PARENT", tuple! { id: 1i64, name: "Parent1" })
        .unwrap();
    db.insert("CHILD", tuple! { child_id: 10i64, parent_id: 1i64 })
        .unwrap();

    // Deleting parent should fail due to foreign key
    let result = db.delete("PARENT", |_| true);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
}

#[test]
fn test_update_with_primary_key_violation() {
    use crate::constraints::{KeyConstraints, PrimaryKey};

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    let constraints = KeyConstraints::new().with_primary_key(pk);
    db.set_key_constraints("TEST", constraints).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

    // Update that would create duplicate primary key should fail
    let result = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 2,
        |_| {
            tuple! { id: 1i64, name: "Bob" }
        },
    );
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
}

#[test]
fn test_set_type_constraints() {
    use crate::constraints::AttributeConstraints;
    use crate::constraints::type_constraint::TypeConstraint;
    use crate::values::ScalarValue;

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    // Create attribute constraints
    let range_constraint = TypeConstraint::Range {
        min: ScalarValue::Int(1),
        max: ScalarValue::Int(100),
    };

    let attr_constraints = AttributeConstraints::new("id".to_string(), ScalarType::Int)
        .with_constraint(range_constraint);

    db.set_type_constraints("TEST", "id", attr_constraints)
        .unwrap();

    // Valid value should succeed
    db.insert("TEST", tuple! { id: 50i64, name: "Alice" })
        .unwrap();

    // Invalid value should fail
    let result = db.insert("TEST", tuple! { id: 200i64, name: "Bob" });
    assert!(result.is_err());
}

#[test]
fn test_set_foreign_key_constraints_with_existing_valid_data() {
    use crate::constraints::{ForeignKey, ForeignKeyConstraints};

    let mut db = setup_parent_child_db();

    // Insert data BEFORE setting FK
    db.insert("PARENT", tuple! { id: 1i64, name: "Parent1" })
        .unwrap();
    db.insert("CHILD", tuple! { child_id: 100i64, parent_id: 1i64 })
        .unwrap();

    // Set FK constraints
    let fk = ForeignKey::new(
        vec!["parent_id".to_string()],
        "PARENT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);

    // Should succeed as data is valid
    db.set_foreign_key_constraints("CHILD", fk_constraints)
        .unwrap();
}

#[test]
fn test_set_foreign_key_constraints_with_existing_invalid_data() {
    use crate::constraints::{ForeignKey, ForeignKeyConstraints};

    let mut db = setup_parent_child_db();

    // Insert data BEFORE setting FK
    db.insert("PARENT", tuple! { id: 1i64, name: "Parent1" })
        .unwrap();
    // Insert ORPHAN child
    db.insert("CHILD", tuple! { child_id: 100i64, parent_id: 99i64 })
        .unwrap();

    // Set FK constraints
    let fk = ForeignKey::new(
        vec!["parent_id".to_string()],
        "PARENT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);

    // Should fail
    let result = db.set_foreign_key_constraints("CHILD", fk_constraints);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
}

#[test]
fn test_delete_referenced_parent_success() {
    use crate::constraints::{ForeignKey, ForeignKeyConstraints};

    let mut db = setup_parent_child_db();

    let fk = ForeignKey::new(
        vec!["parent_id".to_string()],
        "PARENT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    db.set_foreign_key_constraints("CHILD", ForeignKeyConstraints::new().with_foreign_key(fk))
        .unwrap();

    // Insert parents
    db.insert("PARENT", tuple! { id: 1i64, name: "Parent1" })
        .unwrap();
    db.insert("PARENT", tuple! { id: 2i64, name: "Parent2" })
        .unwrap();

    // Insert child referencing Parent1
    db.insert("CHILD", tuple! { child_id: 100i64, parent_id: 1i64 })
        .unwrap();

    // Delete Parent2 (not referenced) - should succeed
    let count = db
        .delete("PARENT", |t| t.get_typed::<i64>("id").unwrap() == 2)
        .unwrap();
    assert_eq!(count, 1);

    // Verify Parent2 is gone
    let result = db.query("PARENT").unwrap();
    assert_eq!(result.cardinality(), 1);
    assert_eq!(
        result
            .tuples()
            .next()
            .unwrap()
            .get_typed::<String>("name")
            .unwrap(),
        "Parent1"
    );
}

#[test]
fn test_update_with_candidate_key_violation() {
    use crate::constraints::{CandidateKey, KeyConstraints};

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    let ck = CandidateKey::new(vec!["name".to_string()]).unwrap();
    db.set_key_constraints("TEST", KeyConstraints::new().with_candidate_key(ck))
        .unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

    // Try to update to create duplicate candidate key
    let result = db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 2,
        |_| {
            tuple! { id: 2i64, name: "Alice" }
        },
    );

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::CandidateKeyViolation
        ))
    ));
}

#[test]
fn test_set_check_constraints() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("salary", ScalarType::Int),
    );

    db.create_relvar("EMPLOYEES", rel_type).unwrap();

    // Create CHECK constraint: salary must be positive
    let constraints = CheckConstraints::new().with_constraint(CheckConstraint::new(
        "positive_salary",
        "Salary must be positive",
        ConstraintExpression::Cmp {
            left: "salary".to_string(),
            op: CmpOp::Gt,
            right: ValueOrRef::Value(ScalarValue::Int(0)),
        },
    ));

    db.set_check_constraints("EMPLOYEES", constraints).unwrap();
}

#[test]
fn test_set_check_constraint_validates_existing_data() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("salary", ScalarType::Int),
    );

    db.create_relvar("EMPLOYEES", rel_type).unwrap();

    // Insert tuple with negative salary
    db.insert("EMPLOYEES", tuple! { id: 1i64, salary: -100i64 })
        .unwrap();

    // Try to add CHECK constraint - should fail because existing data violates it
    let constraints = CheckConstraints::new().with_constraint(CheckConstraint::new(
        "positive_salary",
        "Salary must be positive",
        ConstraintExpression::Cmp {
            left: "salary".to_string(),
            op: CmpOp::Gt,
            right: ValueOrRef::Value(ScalarValue::Int(0)),
        },
    ));

    let result = db.set_check_constraints("EMPLOYEES", constraints);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::CheckConstraintViolation(_)
        ))
    ));
}

#[test]
fn test_insert_enforces_check_constraints() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("salary", ScalarType::Int),
    );

    db.create_relvar("EMPLOYEES", rel_type).unwrap();

    // Add CHECK constraint: salary must be positive
    let constraints = CheckConstraints::new().with_constraint(CheckConstraint::new(
        "positive_salary",
        "Salary must be positive",
        ConstraintExpression::Cmp {
            left: "salary".to_string(),
            op: CmpOp::Gt,
            right: ValueOrRef::Value(ScalarValue::Int(0)),
        },
    ));
    db.set_check_constraints("EMPLOYEES", constraints).unwrap();

    // Insert with positive salary should succeed
    let result = db.insert("EMPLOYEES", tuple! { id: 1i64, salary: 50000i64 });
    assert!(result.is_ok());

    // Insert with negative salary should fail
    let result = db.insert("EMPLOYEES", tuple! { id: 2i64, salary: -100i64 });
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::CheckConstraintViolation(_)
        ))
    ));
}

#[test]
fn test_insert_satisfies_check_constraints() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("age", ScalarType::Int),
    );

    db.create_relvar("PERSONS", rel_type).unwrap();

    // Add complex CHECK constraint: age between 0 and 150
    let constraints = CheckConstraints::new().with_constraint(CheckConstraint::new(
        "valid_age",
        "Age must be between 0 and 150",
        ConstraintExpression::And(
            Box::new(ConstraintExpression::Cmp {
                left: "age".to_string(),
                op: CmpOp::Gt,
                right: ValueOrRef::Value(ScalarValue::Int(0)),
            }),
            Box::new(ConstraintExpression::Cmp {
                left: "age".to_string(),
                op: CmpOp::Lt,
                right: ValueOrRef::Value(ScalarValue::Int(150)),
            }),
        ),
    ));
    db.set_check_constraints("PERSONS", constraints).unwrap();

    // Insert with valid age should succeed
    let result = db.insert("PERSONS", tuple! { id: 1i64, age: 30i64 });
    assert!(result.is_ok());

    // Insert with invalid age (too low) should fail
    let result = db.insert("PERSONS", tuple! { id: 2i64, age: -5i64 });
    assert!(result.is_err());

    // Insert with invalid age (too high) should fail
    let result = db.insert("PERSONS", tuple! { id: 3i64, age: 200i64 });
    assert!(result.is_err());
}

#[test]
fn test_update_constraint_violation_in_loop() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("salary", ScalarType::Int),
    );

    db.create_relvar("EMPLOYEES", rel_type).unwrap();

    // CHECK constraint: salary > 0
    let constraints = CheckConstraints::new().with_constraint(CheckConstraint::new(
        "positive_salary",
        "Salary must be positive",
        ConstraintExpression::Cmp {
            left: "salary".to_string(),
            op: CmpOp::Gt,
            right: ValueOrRef::Value(ScalarValue::Int(0)),
        },
    ));
    db.set_check_constraints("EMPLOYEES", constraints).unwrap();

    db.insert("EMPLOYEES", tuple! { id: 1i64, salary: 50000i64 })
        .unwrap();

    // Update to set salary to -100 (violation)
    let result = db.update(
        "EMPLOYEES",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |_t| tuple! { id: 1i64, salary: -100i64 },
    );

    // Should fail due to constraint violation in the try_for_each loop
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::CheckConstraintViolation(_)
        ))
    ));
}

#[test]
fn test_insert_with_key_violation() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    use crate::constraints::{KeyConstraints, PrimaryKey};
    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    let constraints = KeyConstraints::new().with_primary_key(pk);
    db.set_key_constraints("TEST", constraints).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    let result = db.validate_insert("TEST", &tuple! { id: 1i64, name: "Bob" });

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
}

#[test]
fn test_update_returns_type_constraint_violation() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("age", ScalarType::Int),
    );

    db.create_relvar("PERSONS", rel_type).unwrap();

    use crate::constraints::{AttributeConstraints, TypeConstraint};
    let attr_constraints = AttributeConstraints::new("age".to_string(), ScalarType::Int)
        .with_constraint(TypeConstraint::Range {
            min: ScalarValue::Int(0),
            max: ScalarValue::Int(150),
        });
    db.set_type_constraints("PERSONS", "age", attr_constraints)
        .unwrap();

    db.insert("PERSONS", tuple! { id: 1i64, age: 30i64 })
        .unwrap();

    let result = db.update(
        "PERSONS",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |_| tuple! { id: 1i64, age: -5i64 },
    );

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::TypeConstraintViolation(_)
        ))
    ));
}

#[test]
fn test_update_returns_foreign_key_violation() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("DEPT", test_rel_type()).unwrap();
    db.insert("DEPT", tuple! { id: 1i64, name: "Engineering" })
        .unwrap();

    let emp_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("dept_id", ScalarType::Int),
    );
    db.create_relvar("EMP", emp_type).unwrap();
    db.insert("EMP", tuple! { id: 100i64, dept_id: 1i64 })
        .unwrap();

    use crate::constraints::{ForeignKey, ForeignKeyConstraints};
    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
    db.set_foreign_key_constraints("EMP", constraints).unwrap();

    // Try to update dept_id to non-existent department
    let result = db.update(
        "EMP",
        |t| t.get_typed::<i64>("id").unwrap() == 100,
        |_| tuple! { id: 100i64, dept_id: 99i64 },
    );

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
}

#[test]
fn test_delete_returns_foreign_key_violation() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("DEPT", test_rel_type()).unwrap();
    db.insert("DEPT", tuple! { id: 1i64, name: "Engineering" })
        .unwrap();

    let emp_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("dept_id", ScalarType::Int),
    );
    db.create_relvar("EMP", emp_type).unwrap();
    db.insert("EMP", tuple! { id: 100i64, dept_id: 1i64 })
        .unwrap();

    use crate::constraints::{ForeignKey, ForeignKeyConstraints};
    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    let constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
    db.set_foreign_key_constraints("EMP", constraints).unwrap();

    // Try to delete the department which is still referenced
    let result = db.delete("DEPT", |t| t.get_typed::<i64>("id").unwrap() == 1);

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
}
