use super::common::*;
use crate::constraints::ConstraintManagerError;
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::InMemoryEngine;
use crate::tuple;

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
fn test_bulk_insert_inserts_all_tuples() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    let tuples = vec![
        tuple! { id: 1i64, name: "Alice" },
        tuple! { id: 2i64, name: "Bob" },
        tuple! { id: 3i64, name: "Charlie" },
    ];
    db.bulk_insert("TEST", tuples).unwrap();

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 3);
    assert!(result.contains(&tuple! { id: 2i64, name: "Bob" }));
}

#[test]
fn test_bulk_insert_empty_batch_is_noop() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    db.bulk_insert("TEST", vec![]).unwrap();

    let result = db.query("TEST").unwrap();
    assert_eq!(result.cardinality(), 0);
}

#[test]
fn test_bulk_insert_missing_relation_errors() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    let result = db.bulk_insert("MISSING", vec![tuple! { id: 1i64, name: "Alice" }]);
    assert!(matches!(result, Err(DatabaseError::RelationNotFound(_))));
}

#[test]
fn test_bulk_insert_rejects_virtual_relvar() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();
    db.define_virtual_relvar("VTEST", test_rel_type(), |db_exec| db_exec.query("TEST"))
        .unwrap();

    let result = db.bulk_insert("VTEST", vec![tuple! { id: 1i64, name: "Alice" }]);
    assert!(matches!(
        result,
        Err(DatabaseError::CannotModifyVirtualRelvar(_))
    ));
}

#[test]
fn test_bulk_insert_enforces_primary_key_against_existing_data() {
    use crate::constraints::{KeyConstraints, PrimaryKey};

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    db.set_key_constraints("TEST", KeyConstraints::new().with_primary_key(pk))
        .unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Batch contains a tuple duplicating the existing primary key.
    let result = db.bulk_insert(
        "TEST",
        vec![
            tuple! { id: 2i64, name: "Bob" },
            tuple! { id: 1i64, name: "Duplicate" },
        ],
    );
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::PrimaryKeyViolation
        ))
    ));

    // Atomic: nothing from the batch was written.
    assert_eq!(db.query("TEST").unwrap().cardinality(), 1);
}

#[test]
fn test_bulk_insert_rejects_duplicates_within_batch() {
    use crate::constraints::{KeyConstraints, PrimaryKey};

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    db.set_key_constraints("TEST", KeyConstraints::new().with_primary_key(pk))
        .unwrap();

    // Two tuples in the same batch share a primary key value.
    let result = db.bulk_insert(
        "TEST",
        vec![
            tuple! { id: 5i64, name: "Alice" },
            tuple! { id: 5i64, name: "Bob" },
        ],
    );
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::PrimaryKeyViolation
        ))
    ));

    // Atomic: nothing was written.
    assert_eq!(db.query("TEST").unwrap().cardinality(), 0);
}

#[test]
fn test_bulk_insert_enforces_candidate_key() {
    use crate::constraints::{CandidateKey, KeyConstraints};

    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    let ck = CandidateKey::new(vec!["name".to_string()]).unwrap();
    db.set_key_constraints("TEST", KeyConstraints::new().with_candidate_key(ck))
        .unwrap();

    let result = db.bulk_insert(
        "TEST",
        vec![
            tuple! { id: 1i64, name: "Alice" },
            tuple! { id: 2i64, name: "Alice" },
        ],
    );
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::CandidateKeyViolation
        ))
    ));
    assert_eq!(db.query("TEST").unwrap().cardinality(), 0);
}

#[test]
fn test_bulk_insert_validates_tuple_types() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    // Second tuple has the wrong type for `id`.
    let result = db.bulk_insert(
        "TEST",
        vec![
            tuple! { id: 1i64, name: "Alice" },
            tuple! { id: "not-an-int", name: "Bob" },
        ],
    );
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::TupleMismatch
        ))
    ));

    // Atomic: the valid tuple was not written either.
    assert_eq!(db.query("TEST").unwrap().cardinality(), 0);
}

#[test]
fn test_bulk_insert_validates_foreign_keys() {
    use crate::constraints::{ForeignKey, ForeignKeyConstraints};

    let mut db = setup_parent_child_db();

    db.insert("PARENT", tuple! { id: 10i64, name: "Engineering" })
        .unwrap();

    let fk = ForeignKey::new(
        vec!["parent_id".to_string()],
        "PARENT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();
    db.set_foreign_key_constraints("CHILD", ForeignKeyConstraints::new().with_foreign_key(fk))
        .unwrap();

    // Second tuple references a nonexistent parent.
    let result = db.bulk_insert(
        "CHILD",
        vec![
            tuple! { child_id: 1i64, parent_id: 10i64 },
            tuple! { child_id: 2i64, parent_id: 99i64 },
        ],
    );
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
    assert_eq!(db.query("CHILD").unwrap().cardinality(), 0);
}

#[test]
fn test_insert_without_key_constraints_succeeds() {
    // Regression guard for #40: inserting into a relation with no key
    // constraints must not require a full relation load to succeed.
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    for i in 0..50 {
        db.insert(
            "TEST",
            tuple! { id: i as i64, name: format!("Employee_{}", i) },
        )
        .unwrap();
    }

    assert_eq!(db.query("TEST").unwrap().cardinality(), 50);
}
