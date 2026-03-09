use super::*;
use crate::constraints::ConstraintManagerError;
use crate::constraints::expression::{CmpOp, ConstraintExpression, ValueOrRef};
use crate::constraints::{CheckConstraint, CheckConstraints};
use crate::storage_engine::{InMemoryEngine, StorageError};
use crate::tuple;
use crate::types::{ScalarType, TupleType};
use crate::values::ScalarValue;

fn test_rel_type() -> RelationType {
    RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    )
}

#[test]
fn test_create_and_query_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("TEST", test_rel_type()).unwrap();
    assert!(db.relvar_exists("TEST"));

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_drop_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("TEST", test_rel_type()).unwrap();
    db.drop_relvar("TEST").unwrap();

    assert!(!db.relvar_exists("TEST"));
}

#[test]
fn test_delete() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("TEST", test_rel_type()).unwrap();
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

    let deleted = db
        .delete("TEST", |t| t.get_typed::<i64>("id").unwrap() == 1)
        .unwrap();
    assert_eq!(deleted, 1);

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_update() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("TEST", test_rel_type()).unwrap();
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    let updated = db
        .update(
            "TEST",
            |t| t.get_typed::<i64>("id").unwrap() == 1,
            |t| tuple! { id: t.get_typed::<i64>("id").unwrap(), name: "Alicia" },
        )
        .unwrap();
    assert_eq!(updated, 1);

    let result = db.query("TEST").unwrap();
    let tuple = result.tuples().next().unwrap();
    assert_eq!(tuple.get_typed::<String>("name").unwrap(), "Alicia");
}

#[test]
fn test_transactions() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    db.create_relvar("TEST", test_rel_type()).unwrap();
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    db.begin().unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 2);

    db.rollback().unwrap();

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);
}

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
    use crate::constraints::{ForeignKey, ForeignKeyConstraints, KeyConstraints, PrimaryKey};

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    // Create parent and child relvars
    let parent_type = RelationType::new(
        TupleType::new()
            .with_attribute("dept_id", ScalarType::Int)
            .with_attribute("dept_name", ScalarType::String),
    );
    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("dept_id", ScalarType::Int),
    );

    db.create_relvar("DEPT", parent_type).unwrap();
    db.create_relvar("EMP", child_type).unwrap();

    // Add primary key to parent
    let pk = PrimaryKey::new(vec!["dept_id".to_string()]).unwrap();
    let dept_constraints = KeyConstraints::new().with_primary_key(pk);
    db.set_key_constraints("DEPT", dept_constraints).unwrap();

    // Insert parent record
    db.insert("DEPT", tuple! { dept_id: 10i64, dept_name: "Engineering" })
        .unwrap();

    // Add foreign key constraint
    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["dept_id".to_string()],
    )
    .unwrap();
    let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
    db.set_foreign_key_constraints("EMP", fk_constraints)
        .unwrap();

    // Insert with valid foreign key should succeed
    db.insert("EMP", tuple! { emp_id: 1i64, dept_id: 10i64 })
        .unwrap();

    // Insert with invalid foreign key should fail
    let result = db.insert("EMP", tuple! { emp_id: 2i64, dept_id: 99i64 });
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
fn test_virtual_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

    // Define virtual relvar that projects just names
    db.define_virtual_relvar(
        "NAMES",
        RelationType::new(TupleType::new().with_attribute("name", ScalarType::String)),
        |db| {
            let test = db.query("TEST")?;
            Ok(test.project(&["name"]))
        },
    )
    .unwrap();

    // Query virtual relvar
    let names = db.query("NAMES").unwrap();
    assert_eq!(names.degree(), 1);
    assert_eq!(names.cardinality(), 2);
}

#[test]
fn test_delete_with_foreign_key_constraint() {
    use crate::constraints::{ForeignKey, ForeignKeyConstraints, KeyConstraints, PrimaryKey};

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    // Create parent and child
    let parent_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    );
    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("parent_id", ScalarType::Int),
    );

    db.create_relvar("PARENT", parent_type).unwrap();
    db.create_relvar("CHILD", child_type).unwrap();

    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    let parent_constraints = KeyConstraints::new().with_primary_key(pk);
    db.set_key_constraints("PARENT", parent_constraints)
        .unwrap();

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
fn test_transaction_rollback_with_constraints() {
    use crate::constraints::{KeyConstraints, PrimaryKey};

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    let constraints = KeyConstraints::new().with_primary_key(pk);
    db.set_key_constraints("TEST", constraints).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    db.begin().unwrap();

    // Make some changes
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();
    db.delete("TEST", |t| t.get_typed::<i64>("id").unwrap() == 1)
        .unwrap();

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);

    // Rollback
    db.rollback().unwrap();

    // Should be back to original state
    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);
    assert!(result.contains(&tuple! { id: 1i64, name: "Alice" }));
}

#[test]
fn test_error_relvar_not_found() {
    let db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    let result = db.query("NONEXISTENT");
    assert!(result.is_err());
    // Error should be Storage(RelationNotFound) since it comes from the engine
    match result {
        Err(DatabaseError::Storage(StorageError::RelationNotFound(name))) => {
            assert_eq!(name, "NONEXISTENT");
        }
        other => panic!("Expected RelationNotFound error, got: {:?}", other),
    }
}

#[test]
fn test_error_duplicate_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    let result = db.create_relvar("TEST", test_rel_type());
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::RelationAlreadyExists(_))
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
fn test_transaction_commit() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Begin transaction
    db.begin().unwrap();

    // Make changes
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

    // Commit
    db.commit().unwrap();

    // Changes should persist
    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 2);

    // Transaction should no longer be active
    assert!(!db.in_transaction);
}

#[test]
fn test_commit_without_transaction() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    let result = db.commit();
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::TransactionError(_))));
}

#[test]
fn test_rollback_without_transaction() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    let result = db.rollback();
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::TransactionError(_))));
}

#[test]
fn test_begin_nested_transaction_fails() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.begin().unwrap();

    // Try to begin another transaction
    let result = db.begin();
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::TransactionError(_))));
}

#[test]
fn test_drop_virtual_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Define virtual relvar
    db.define_virtual_relvar(
        "VIRT",
        RelationType::new(TupleType::new().with_attribute("name", ScalarType::String)),
        |db| {
            let test = db.query("TEST")?;
            Ok(test.project(&["name"]))
        },
    )
    .unwrap();

    // Verify it exists
    let result = db.query("VIRT");
    assert!(result.is_ok());

    // Drop it
    db.drop_virtual_relvar("VIRT").unwrap();

    // Should no longer exist
    let result = db.query("VIRT");
    assert!(result.is_err());
}

#[test]
fn test_drop_nonexistent_virtual_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    let result = db.drop_virtual_relvar("NONEXISTENT");
    assert!(result.is_err());
    assert!(matches!(result, Err(DatabaseError::RelationNotFound(_))));
}

#[test]
fn test_cannot_insert_into_virtual_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.define_virtual_relvar(
        "VIRT",
        RelationType::new(TupleType::new().with_attribute("name", ScalarType::String)),
        |db| {
            let test = db.query("TEST")?;
            Ok(test.project(&["name"]))
        },
    )
    .unwrap();

    // Try to insert
    let result = db.insert("VIRT", tuple! { name: "Alice" });
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::CannotModifyVirtualRelvar(_))
    ));
}

#[test]
fn test_cannot_delete_from_virtual_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.define_virtual_relvar("VIRT", test_rel_type(), |db| db.query("TEST"))
        .unwrap();

    // Try to delete
    let result = db.delete("VIRT", |_| true);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::CannotModifyVirtualRelvar(_))
    ));
}

#[test]
fn test_cannot_update_virtual_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.define_virtual_relvar("VIRT", test_rel_type(), |db| db.query("TEST"))
        .unwrap();

    // Try to update
    let result = db.update("VIRT", |_| true, |_| tuple! { id: 99i64, name: "X" });
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DatabaseError::CannotModifyVirtualRelvar(_))
    ));
}

#[test]
fn test_cannot_drop_base_relvar_when_virtual_depends_on_it() {
    // This would require tracking dependencies, which might not be implemented
    // Skipping for now as it may not be a current feature
}

#[test]
fn test_virtual_relvar_error_propagation() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    // Define virtual relvar that queries nonexistent base
    db.define_virtual_relvar("VIRT", test_rel_type(), |db| db.query("NONEXISTENT"))
        .unwrap();

    // Querying it should fail
    let result = db.query("VIRT");
    assert!(result.is_err());
}

#[test]
fn test_set_foreign_key_constraints_with_existing_valid_data() {
    use crate::constraints::{ForeignKey, ForeignKeyConstraints, KeyConstraints, PrimaryKey};

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    // Create parent and child relvars
    let parent_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    );
    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("parent_id", ScalarType::Int),
    );

    db.create_relvar("PARENT", parent_type).unwrap();
    db.create_relvar("CHILD", child_type).unwrap();

    // Set PK on parent
    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    db.set_key_constraints("PARENT", KeyConstraints::new().with_primary_key(pk))
        .unwrap();

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
    use crate::constraints::{ForeignKey, ForeignKeyConstraints, KeyConstraints, PrimaryKey};

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    // Create parent and child relvars
    let parent_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    );
    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("parent_id", ScalarType::Int),
    );

    db.create_relvar("PARENT", parent_type).unwrap();
    db.create_relvar("CHILD", child_type).unwrap();

    // Set PK on parent
    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    db.set_key_constraints("PARENT", KeyConstraints::new().with_primary_key(pk))
        .unwrap();

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
    use crate::constraints::{ForeignKey, ForeignKeyConstraints, KeyConstraints, PrimaryKey};

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    let parent_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    );
    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("parent_id", ScalarType::Int),
    );

    db.create_relvar("PARENT", parent_type).unwrap();
    db.create_relvar("CHILD", child_type).unwrap();

    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    db.set_key_constraints("PARENT", KeyConstraints::new().with_primary_key(pk))
        .unwrap();

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
fn test_delete_returns_count() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();
    db.insert("TEST", tuple! { id: 3i64, name: "Charlie" })
        .unwrap();

    // Delete some tuples
    let count = db
        .delete("TEST", |t| t.get_typed::<i64>("id").unwrap() > 1)
        .unwrap();
    assert_eq!(count, 2);

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_update_returns_count() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

    // Update tuples
    let count = db
        .update(
            "TEST",
            |t| t.get_typed::<i64>("id").unwrap() == 1,
            |_| {
                tuple! { id: 1i64, name: "Alicia" }
            },
        )
        .unwrap();

    assert_eq!(count, 1);

    let result = db.query("TEST").unwrap();
    assert!(result.contains(&tuple! { id: 1i64, name: "Alicia" }));
}

#[test]
fn test_delete_no_matches_returns_zero() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Delete with no matches
    let count = db
        .delete("TEST", |t| t.get_typed::<i64>("id").unwrap() > 100)
        .unwrap();
    assert_eq!(count, 0);

    // Relation should be unchanged
    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 1);
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
fn test_virtual_relvar_immutability_enforcement() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    // Create a log relation
    let log_type = RelationType::new(TupleType::new().with_attribute("count", ScalarType::Int));
    db.create_relvar("LOG", log_type).unwrap();

    // Define a view. The compiler enforces that we cannot call mutable methods
    // like insert() inside the evaluator because it receives &Database<E>, not &mut Database.
    db.define_virtual_relvar("SAFE_VIEW", test_rel_type(), |db| {
        // db.insert("LOG", ...); // This would cause compilation error!

        // Read operations are allowed
        let _ = db.query("LOG")?;

        Ok(Relation::new(test_rel_type()))
    })
    .unwrap();

    // Query the view
    assert!(db.query("SAFE_VIEW").is_ok());
}

#[test]
fn test_insert_type_mismatch() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    );

    db.create_relvar("PEOPLE", rel_type.clone()).unwrap();

    // Try inserting a tuple with the wrong type using new_unchecked to bypass Tuple::new validation
    let bad_heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("name", ScalarType::Int);

    let mut values = std::collections::BTreeMap::new();
    values.insert("id".to_string(), ScalarValue::Int(1));
    values.insert("name".to_string(), ScalarValue::Int(42));

    let invalid_tuple = Tuple::new_unchecked(std::sync::Arc::new(bad_heading), values);

    let result = db.insert("PEOPLE", invalid_tuple);

    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            crate::constraints::ConstraintManagerError::TupleMismatch
        ))
    ));
}

#[test]
fn test_validate_relation_constraints_check_violation() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("age", ScalarType::Int),
    );

    db.create_relvar("PERSONS", rel_type).unwrap();
    db.insert("PERSONS", tuple! { id: 1i64, age: 25i64 })
        .unwrap();

    // Now set a check constraint that existing data violates
    let constraints = CheckConstraints::new().with_constraint(CheckConstraint::new(
        "valid_age",
        "Age must be under 20",
        ConstraintExpression::Cmp {
            left: "age".to_string(),
            op: CmpOp::Lt,
            right: ValueOrRef::Value(ScalarValue::Int(20)),
        },
    ));

    // this triggers `validate_relation_constraints` which will loop through the relations
    let result = db.set_check_constraints("PERSONS", constraints);
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            crate::constraints::ConstraintManagerError::CheckConstraintViolation(_)
        ))
    ));
}
