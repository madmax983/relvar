use super::common::*;
use crate::collections::HashSet;
use crate::constraints::ConstraintManagerError;
use crate::constraints::{CandidateKey, DatabaseAssertion, KeyConstraints, PrimaryKey};
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::InMemoryEngine;
use crate::tuple;
use crate::values::ScalarValue;

fn pk_on_id() -> KeyConstraints {
    KeyConstraints::new().with_primary_key(PrimaryKey::new(vec!["id".to_string()]).unwrap())
}

/// Direct index probe: does the key index hold `values` for the key?
fn index_contains(
    db: &Database<InMemoryEngine>,
    relation: &str,
    attrs: &[&str],
    values: Vec<ScalarValue>,
) -> bool {
    let key = (
        relation.to_string(),
        attrs.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    );
    db.key_index
        .get(&key)
        .is_some_and(|set| set.contains(&values))
}

fn setup_keyed_db() -> Database<InMemoryEngine> {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();
    db.set_key_constraints("TEST", pk_on_id()).unwrap();
    db
}

#[test]
fn test_key_index_populated_on_insert() {
    let mut db = setup_keyed_db();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    assert!(index_contains(
        &db,
        "TEST",
        &["id"],
        vec![ScalarValue::Int(1)]
    ));
    assert!(!index_contains(
        &db,
        "TEST",
        &["id"],
        vec![ScalarValue::Int(2)]
    ));
}

#[test]
fn test_key_index_rejects_duplicate_key() {
    let mut db = setup_keyed_db();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    let result = db.insert("TEST", tuple! { id: 1i64, name: "Bob" });
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
    assert_eq!(db.query("TEST").unwrap().cardinality(), 1);
}

#[test]
fn test_key_index_maintained_on_delete() {
    let mut db = setup_keyed_db();

    for i in 1..=3 {
        db.insert("TEST", tuple! { id: i as i64, name: "Alice" })
            .unwrap();
    }
    db.delete("TEST", |t| t.get_typed::<i64>("id").unwrap() == 2)
        .unwrap();

    // The deleted key value must be gone from the index: re-insert succeeds.
    assert!(!index_contains(
        &db,
        "TEST",
        &["id"],
        vec![ScalarValue::Int(2)]
    ));
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();
    assert_eq!(db.query("TEST").unwrap().cardinality(), 3);

    // Surviving keys are still enforced.
    let result = db.insert("TEST", tuple! { id: 1i64, name: "Carol" });
    assert!(result.is_err());
}

#[test]
fn test_key_index_rebuilt_on_update() {
    let mut db = setup_keyed_db();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

    db.update(
        "TEST",
        |t| t.get_typed::<i64>("id").unwrap() == 1,
        |_| tuple! { id: 10i64, name: "Alice" },
    )
    .unwrap();

    // Old key freed, new key tracked.
    assert!(!index_contains(
        &db,
        "TEST",
        &["id"],
        vec![ScalarValue::Int(1)]
    ));
    assert!(index_contains(
        &db,
        "TEST",
        &["id"],
        vec![ScalarValue::Int(10)]
    ));
    db.insert("TEST", tuple! { id: 1i64, name: "Carol" })
        .unwrap();
    let result = db.insert("TEST", tuple! { id: 10i64, name: "Dave" });
    assert!(result.is_err());
}

#[test]
fn test_key_index_cleared_on_drop_relvar() {
    let mut db = setup_keyed_db();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.drop_relvar("TEST").unwrap();

    assert!(db.key_index.keys().all(|(name, _)| name != "TEST"));

    // Recreate: the old key values must not haunt the new relvar.
    db.create_relvar("TEST", test_rel_type()).unwrap();
    db.set_key_constraints("TEST", pk_on_id()).unwrap();
    db.insert("TEST", tuple! { id: 1i64, name: "Bob" }).unwrap();
    assert_eq!(db.query("TEST").unwrap().cardinality(), 1);
}

#[test]
fn test_key_index_restored_on_rollback() {
    let mut db = setup_keyed_db();

    db.insert("TEST", tuple! { id: 2i64, name: "Committed" })
        .unwrap();

    db.begin().unwrap();
    db.insert("TEST", tuple! { id: 1i64, name: "Provisional" })
        .unwrap();
    assert!(index_contains(
        &db,
        "TEST",
        &["id"],
        vec![ScalarValue::Int(1)]
    ));
    db.rollback().unwrap();

    // Rolled-back key is insertable again; committed key still enforced.
    assert!(!index_contains(
        &db,
        "TEST",
        &["id"],
        vec![ScalarValue::Int(1)]
    ));
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    let result = db.insert("TEST", tuple! { id: 2i64, name: "Bob" });
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
}

#[test]
fn test_key_index_kept_on_commit() {
    let mut db = setup_keyed_db();

    db.begin().unwrap();
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.commit().unwrap();

    assert!(index_contains(
        &db,
        "TEST",
        &["id"],
        vec![ScalarValue::Int(1)]
    ));
    let result = db.insert("TEST", tuple! { id: 1i64, name: "Bob" });
    assert!(result.is_err());
}

#[test]
fn test_key_index_built_when_constraints_set_after_data() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();

    // Data first, keys later: the index must cover pre-existing tuples.
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.set_key_constraints("TEST", pk_on_id()).unwrap();

    assert!(index_contains(
        &db,
        "TEST",
        &["id"],
        vec![ScalarValue::Int(1)]
    ));
    let result = db.insert("TEST", tuple! { id: 1i64, name: "Bob" });
    assert!(result.is_err());
}

#[test]
fn test_key_index_replaced_when_constraints_reset() {
    let mut db = setup_keyed_db();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Replace the key: (id) -> (name). Stale (id) entries must go.
    let name_key =
        KeyConstraints::new().with_primary_key(PrimaryKey::new(vec!["name".to_string()]).unwrap());
    db.set_key_constraints("TEST", name_key).unwrap();

    assert!(
        db.key_index
            .keys()
            .all(|(_, attrs)| attrs != &vec!["id".to_string()])
    );

    // Duplicate id now allowed; duplicate name rejected.
    db.insert("TEST", tuple! { id: 1i64, name: "Bob" }).unwrap();
    let result = db.insert("TEST", tuple! { id: 2i64, name: "Alice" });
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
}

#[test]
fn test_key_index_enforces_candidate_key() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();
    let ck = KeyConstraints::new()
        .with_primary_key(PrimaryKey::new(vec!["id".to_string()]).unwrap())
        .with_candidate_key(CandidateKey::new(vec!["name".to_string()]).unwrap());
    db.set_key_constraints("TEST", ck).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    let result = db.insert("TEST", tuple! { id: 2i64, name: "Alice" });
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::CandidateKeyViolation
        ))
    ));
}

#[test]
fn test_key_index_handles_composite_keys() {
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    db.create_relvar("TEST", test_rel_type()).unwrap();
    let composite = KeyConstraints::new()
        .with_primary_key(PrimaryKey::new(vec!["id".to_string(), "name".to_string()]).unwrap());
    db.set_key_constraints("TEST", composite).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    // Same id, different name: allowed.
    db.insert("TEST", tuple! { id: 1i64, name: "Bob" }).unwrap();
    // Same (id, name): rejected.
    let result = db.insert("TEST", tuple! { id: 1i64, name: "Alice" });
    assert!(matches!(
        result,
        Err(DatabaseError::Constraint(
            ConstraintManagerError::PrimaryKeyViolation
        ))
    ));
}

#[test]
fn test_bulk_insert_populates_key_index() {
    let mut db = setup_keyed_db();

    db.bulk_insert(
        "TEST",
        vec![
            tuple! { id: 1i64, name: "Alice" },
            tuple! { id: 2i64, name: "Bob" },
        ],
    )
    .unwrap();

    assert!(index_contains(
        &db,
        "TEST",
        &["id"],
        vec![ScalarValue::Int(1)]
    ));
    assert!(index_contains(
        &db,
        "TEST",
        &["id"],
        vec![ScalarValue::Int(2)]
    ));

    // A later single insert sees the batch's keys.
    let result = db.insert("TEST", tuple! { id: 2i64, name: "Carol" });
    assert!(result.is_err());
}

#[test]
fn test_key_index_entry_set_matches_relation_contents() {
    // Brute-force consistency: the index set must always equal the key
    // values derivable from the stored relation.
    let mut db = setup_keyed_db();

    let ids: Vec<i64> = vec![3, 1, 4, 1, 5, 9, 2, 6];
    for (i, id) in ids.iter().enumerate() {
        let _ = db.insert("TEST", tuple! { id: *id, name: format!("N{i}") });
    }

    let expected: HashSet<Vec<ScalarValue>> = [3, 1, 4, 5, 9, 2, 6]
        .iter()
        .map(|id| vec![ScalarValue::Int(*id)])
        .collect();
    let key = ("TEST".to_string(), vec!["id".to_string()]);
    assert_eq!(db.key_index.get(&key), Some(&expected));
    assert_eq!(db.query("TEST").unwrap().cardinality(), expected.len());
}

#[test]
fn test_assertion_rollback_leaves_key_index_clean() {
    // Integration (#23 x #36): an insert rolled back by a database
    // assertion must not leave phantom entries in the key index.
    let mut db = setup_keyed_db();
    let assertion = DatabaseAssertion::new(
        "at_most_one",
        "TEST must hold at most one tuple",
        |db: &mut Database<InMemoryEngine>| db.query("TEST").unwrap().cardinality() <= 1,
    );
    db.add_assertion(assertion).unwrap();

    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // This insert violates the assertion and is rolled back.
    let result = db.insert("TEST", tuple! { id: 2i64, name: "Bob" });
    assert!(matches!(result, Err(DatabaseError::AssertionViolation(_))));
    assert_eq!(db.query("TEST").unwrap().cardinality(), 1);

    // No phantom index entry for the rolled-back key...
    assert!(!index_contains(
        &db,
        "TEST",
        &["id"],
        vec![ScalarValue::Int(2)]
    ));

    // ...so the same key inserts cleanly once the assertion is gone.
    assert!(db.remove_assertion("at_most_one"));
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();
    assert_eq!(db.query("TEST").unwrap().cardinality(), 2);
}

#[test]
fn test_assertion_rollback_on_delete_keeps_key_index_accurate() {
    // Integration (#23 x #36): a delete rolled back by a database
    // assertion must leave the key index reflecting the restored state.
    let mut db = setup_keyed_db();
    db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

    let assertion = DatabaseAssertion::new(
        "keep_bob",
        "Bob must not be deleted",
        |db: &mut Database<InMemoryEngine>| {
            db.query("TEST")
                .unwrap()
                .tuples()
                .any(|t| t.get("name") == Some(&ScalarValue::String("Bob".to_string())))
        },
    );
    db.add_assertion(assertion).unwrap();

    // Delete Bob: violates the assertion, rolled back.
    let deleted = db.delete("TEST", |t| t.get("id") == Some(&ScalarValue::Int(2)));
    assert!(matches!(deleted, Err(DatabaseError::AssertionViolation(_))));
    assert_eq!(db.query("TEST").unwrap().cardinality(), 2);

    // The index must still hold both keys (no stale post-delete rebuild).
    assert!(index_contains(
        &db,
        "TEST",
        &["id"],
        vec![ScalarValue::Int(1)]
    ));
    assert!(index_contains(
        &db,
        "TEST",
        &["id"],
        vec![ScalarValue::Int(2)]
    ));
}
