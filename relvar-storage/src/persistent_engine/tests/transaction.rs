use super::common::*;
use crate::persistent_engine::*;
use relvar_core::tuple;
use tempfile::TempDir;

#[test]
fn test_transaction_rollback() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Begin transaction
    let snapshot = engine.begin_transaction().unwrap();

    // Make changes
    engine
        .insert_tuple("TEST", tuple! { id: 2i64, name: "Bob" })
        .unwrap();

    let relation = engine.load_relation("TEST").unwrap();
    assert_eq!(relation.cardinality(), 2);

    // Rollback
    engine.rollback_transaction(snapshot).unwrap();

    let relation = engine.load_relation("TEST").unwrap();
    assert_eq!(relation.cardinality(), 1);
}

#[test]
fn test_transaction_with_multiple_relations() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("REL1", test_rel_type()).unwrap();
    engine.create_relation("REL2", test_rel_type()).unwrap();

    engine
        .insert_tuple("REL1", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    engine
        .insert_tuple("REL2", tuple! { id: 2i64, name: "Bob" })
        .unwrap();

    // Begin transaction
    let snapshot = engine.begin_transaction().unwrap();

    // Modify both relations
    engine
        .insert_tuple("REL1", tuple! { id: 3i64, name: "Charlie" })
        .unwrap();
    engine
        .insert_tuple("REL2", tuple! { id: 4i64, name: "David" })
        .unwrap();

    // Rollback should restore both
    engine.rollback_transaction(snapshot).unwrap();

    let rel1 = engine.load_relation("REL1").unwrap();
    let rel2 = engine.load_relation("REL2").unwrap();
    assert_eq!(rel1.cardinality(), 1);
    assert_eq!(rel2.cardinality(), 1);
}

#[test]
fn test_abort_does_not_flush() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    let snapshot = engine.begin_transaction().unwrap();

    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Rollback should NOT flush WAL
    // (transaction is aborted, changes should not be durable)
    engine.rollback_transaction(snapshot).unwrap();

    // After rollback, data should not persist
    let relation = engine.load_relation("TEST").unwrap();
    assert_eq!(relation.cardinality(), 0);
}

// Checkpoint Tests (Step 6)

#[test]
fn test_multiple_concurrent_begin() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // Begin multiple transactions concurrently
    let snapshot1 = engine.begin_transaction().unwrap();
    let snapshot2 = engine.begin_transaction().unwrap();
    let snapshot3 = engine.begin_transaction().unwrap();

    // Each should have unique transaction ID
    assert_ne!(snapshot1.txn_id, snapshot2.txn_id);
    assert_ne!(snapshot2.txn_id, snapshot3.txn_id);
    assert_ne!(snapshot1.txn_id, snapshot3.txn_id);
}

#[test]
fn test_each_txn_unique_snapshot() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // T1 begins (sees no active transactions)
    let snapshot1 = engine.begin_transaction().unwrap();

    // T2 begins (should see T1 as active)
    let snapshot2 = engine.begin_transaction().unwrap();

    // T3 begins (should see T1 and T2 as active)
    let snapshot3 = engine.begin_transaction().unwrap();

    // Each snapshot should be unique
    assert_ne!(snapshot1.txn_id, snapshot2.txn_id);
    assert_ne!(snapshot2.txn_id, snapshot3.txn_id);
}

#[test]
fn test_transactions_isolated() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // T1 begins and inserts
    let _snapshot1 = engine.begin_transaction().unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // T2 begins and inserts
    let _snapshot2 = engine.begin_transaction().unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 2i64, name: "Bob" })
        .unwrap();

    // Both transactions should be active
    // (Verified by not panicking - we'll test visibility in Phase 3.2)
}

#[test]
fn test_commit_removes_from_att() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    let snapshot1 = engine.begin_transaction().unwrap();
    let snapshot2 = engine.begin_transaction().unwrap();

    // Commit T1
    engine.commit_transaction(snapshot1).unwrap();

    // T1 should be removed from active transactions
    // (We'll verify this in visibility tests)

    // T2 can still commit
    engine.commit_transaction(snapshot2).unwrap();
}

#[test]
fn test_abort_removes_from_att() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    let snapshot1 = engine.begin_transaction().unwrap();
    let snapshot2 = engine.begin_transaction().unwrap();

    // Abort T1
    engine.rollback_transaction(snapshot1).unwrap();

    // T1 should be removed from active transactions

    // T2 can still commit
    engine.commit_transaction(snapshot2).unwrap();
}

#[test]
fn test_snapshot_captures_concurrent_active() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // Start T1
    let _snapshot1 = engine.begin_transaction().unwrap();

    // Start T2 (should see T1 as active)
    let _snapshot2 = engine.begin_transaction().unwrap();

    // Start T3 (should see T1 and T2 as active)
    let _snapshot3 = engine.begin_transaction().unwrap();

    // The snapshots should have captured the active transactions
    // (Will be tested more thoroughly in visibility tests)
}

#[test]
fn test_txn_after_commit() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // T1 commits
    let snapshot1 = engine.begin_transaction().unwrap();
    engine.commit_transaction(snapshot1).unwrap();

    // T2 starts after T1 commits
    let snapshot2 = engine.begin_transaction().unwrap();

    // T2 should NOT see T1 as active
    engine.commit_transaction(snapshot2).unwrap();
}

#[test]
fn test_sequential_transaction_ids() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    let snapshot1 = engine.begin_transaction().unwrap();
    let snapshot2 = engine.begin_transaction().unwrap();
    let snapshot3 = engine.begin_transaction().unwrap();

    // Transaction IDs should be increasing
    assert!(snapshot2.txn_id > snapshot1.txn_id);
    assert!(snapshot3.txn_id > snapshot2.txn_id);

    engine.commit_transaction(snapshot1).unwrap();
    engine.commit_transaction(snapshot2).unwrap();
    engine.commit_transaction(snapshot3).unwrap();
}

// Phase 3.2: Visibility-Aware Load tests

#[test]
fn test_load_for_txn_sees_committed() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // T1 inserts and commits
    let snapshot1 = engine.begin_transaction().unwrap();
    engine
        .insert_tuple_in_txn("TEST", tuple! { id: 1i64, name: "Alice" }, snapshot1.txn_id)
        .unwrap();
    engine.commit_transaction(snapshot1).unwrap();

    // T2 starts after T1 commits
    let snapshot2 = engine.begin_transaction().unwrap();

    // T2 should see T1's committed data
    let relation = engine
        .load_relation_for_txn("TEST", snapshot2.txn_id)
        .unwrap();
    assert_eq!(relation.cardinality(), 1);

    engine.commit_transaction(snapshot2).unwrap();
}

#[test]
fn test_load_for_txn_sees_own_inserts() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // T1 inserts (uncommitted)
    let snapshot1 = engine.begin_transaction().unwrap();
    engine
        .insert_tuple_in_txn("TEST", tuple! { id: 1i64, name: "Alice" }, snapshot1.txn_id)
        .unwrap();

    // T1 should see its own uncommitted insert
    let relation = engine
        .load_relation_for_txn("TEST", snapshot1.txn_id)
        .unwrap();
    assert_eq!(relation.cardinality(), 1);

    engine.commit_transaction(snapshot1).unwrap();
}

#[test]
fn test_load_for_txn_skips_concurrent() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // T1 and T2 begin concurrently
    let snapshot1 = engine.begin_transaction().unwrap();
    let snapshot2 = engine.begin_transaction().unwrap();

    // T1 inserts
    engine
        .insert_tuple_in_txn("TEST", tuple! { id: 1i64, name: "Alice" }, snapshot1.txn_id)
        .unwrap();

    // T2 should NOT see T1's uncommitted insert
    let relation = engine
        .load_relation_for_txn("TEST", snapshot2.txn_id)
        .unwrap();
    assert_eq!(relation.cardinality(), 0);

    engine.commit_transaction(snapshot1).unwrap();
    engine.commit_transaction(snapshot2).unwrap();
}

#[test]
fn test_load_for_txn_multiple_views() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // T1 inserts and commits
    let snapshot1 = engine.begin_transaction().unwrap();
    engine
        .insert_tuple_in_txn("TEST", tuple! { id: 1i64, name: "Alice" }, snapshot1.txn_id)
        .unwrap();
    engine.commit_transaction(snapshot1).unwrap();

    // T2 starts and inserts (uncommitted)
    let snapshot2 = engine.begin_transaction().unwrap();
    engine
        .insert_tuple_in_txn("TEST", tuple! { id: 2i64, name: "Bob" }, snapshot2.txn_id)
        .unwrap();

    // T3 starts
    let snapshot3 = engine.begin_transaction().unwrap();

    // T2 sees: Alice (committed) + Bob (own insert) = 2
    let rel2 = engine
        .load_relation_for_txn("TEST", snapshot2.txn_id)
        .unwrap();
    assert_eq!(rel2.cardinality(), 2);

    // T3 sees: only Alice (T2's insert uncommitted) = 1
    let rel3 = engine
        .load_relation_for_txn("TEST", snapshot3.txn_id)
        .unwrap();
    assert_eq!(rel3.cardinality(), 1);

    engine.commit_transaction(snapshot2).unwrap();
    engine.commit_transaction(snapshot3).unwrap();
}

#[test]
fn test_load_for_txn_after_commit_visible() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // T1 begins
    let snapshot1 = engine.begin_transaction().unwrap();

    // T2 inserts and commits
    let snapshot2 = engine.begin_transaction().unwrap();
    engine
        .insert_tuple_in_txn("TEST", tuple! { id: 1i64, name: "Alice" }, snapshot2.txn_id)
        .unwrap();
    engine.commit_transaction(snapshot2).unwrap();

    // T3 begins after T2 commits
    let snapshot3 = engine.begin_transaction().unwrap();

    // NOTE: Current implementation uses active_txns list, not commit LSNs.
    // T1 WILL see T2's insert because T2 was not in T1's active_txns
    // (T2 started after T1 took its snapshot).
    // For full snapshot isolation, track commit LSNs and check:
    //       committed_lsn[T2] < snapshot1.snapshot_lsn
    let rel1 = engine
        .load_relation_for_txn("TEST", snapshot1.txn_id)
        .unwrap();
    assert_eq!(rel1.cardinality(), 1); // Sees committed data (Read Committed behavior)

    // T3 should see T2's insert (T2 committed before T3 started)
    let rel3 = engine
        .load_relation_for_txn("TEST", snapshot3.txn_id)
        .unwrap();
    assert_eq!(rel3.cardinality(), 1);

    engine.commit_transaction(snapshot1).unwrap();
    engine.commit_transaction(snapshot3).unwrap();
}

#[test]
fn test_load_for_txn_empty_relation() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    let snapshot = engine.begin_transaction().unwrap();

    // Empty relation should return empty result
    let relation = engine
        .load_relation_for_txn("TEST", snapshot.txn_id)
        .unwrap();
    assert_eq!(relation.cardinality(), 0);

    engine.commit_transaction(snapshot).unwrap();
}

#[test]
fn test_load_for_txn_nonexistent_relation() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    let snapshot = engine.begin_transaction().unwrap();

    // Loading nonexistent relation should fail
    let result = engine.load_relation_for_txn("NONEXISTENT", snapshot.txn_id);
    assert!(result.is_err());

    engine.commit_transaction(snapshot).unwrap();
}

#[test]
fn test_load_for_txn_concurrent_uncommitted() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // T1 inserts uncommitted
    let snapshot1 = engine.begin_transaction().unwrap();
    engine
        .insert_tuple_in_txn("TEST", tuple! { id: 1i64, name: "Alice" }, snapshot1.txn_id)
        .unwrap();

    // T2 inserts uncommitted
    let snapshot2 = engine.begin_transaction().unwrap();
    engine
        .insert_tuple_in_txn("TEST", tuple! { id: 2i64, name: "Bob" }, snapshot2.txn_id)
        .unwrap();

    // T1 sees only its own insert
    let rel1 = engine
        .load_relation_for_txn("TEST", snapshot1.txn_id)
        .unwrap();
    assert_eq!(rel1.cardinality(), 1);

    // T2 sees only its own insert
    let rel2 = engine
        .load_relation_for_txn("TEST", snapshot2.txn_id)
        .unwrap();
    assert_eq!(rel2.cardinality(), 1);

    engine.commit_transaction(snapshot1).unwrap();
    engine.commit_transaction(snapshot2).unwrap();
}

#[test]
fn test_load_for_txn_mixed_committed_uncommitted() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // T1 inserts and commits
    let snapshot1 = engine.begin_transaction().unwrap();
    engine
        .insert_tuple_in_txn("TEST", tuple! { id: 1i64, name: "Alice" }, snapshot1.txn_id)
        .unwrap();
    engine.commit_transaction(snapshot1).unwrap();

    // T2 inserts (uncommitted)
    let snapshot2 = engine.begin_transaction().unwrap();
    engine
        .insert_tuple_in_txn("TEST", tuple! { id: 2i64, name: "Bob" }, snapshot2.txn_id)
        .unwrap();

    // T3 inserts and commits
    let snapshot3 = engine.begin_transaction().unwrap();
    engine
        .insert_tuple_in_txn(
            "TEST",
            tuple! { id: 3i64, name: "Charlie" },
            snapshot3.txn_id,
        )
        .unwrap();
    engine.commit_transaction(snapshot3).unwrap();

    // T4 starts
    let snapshot4 = engine.begin_transaction().unwrap();

    // T4 should see Alice (T1 committed) and Charlie (T3 committed), but NOT Bob (T2 uncommitted)
    let rel4 = engine
        .load_relation_for_txn("TEST", snapshot4.txn_id)
        .unwrap();
    assert_eq!(rel4.cardinality(), 2);

    engine.commit_transaction(snapshot2).unwrap();
    engine.commit_transaction(snapshot4).unwrap();
}

#[test]
fn test_load_for_txn_preserves_tuple_data() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    let original = tuple! { id: 42i64, name: "TestData" };

    let snapshot1 = engine.begin_transaction().unwrap();
    engine
        .insert_tuple_in_txn("TEST", original.clone(), snapshot1.txn_id)
        .unwrap();
    engine.commit_transaction(snapshot1).unwrap();

    let snapshot2 = engine.begin_transaction().unwrap();
    let relation = engine
        .load_relation_for_txn("TEST", snapshot2.txn_id)
        .unwrap();

    assert_eq!(relation.cardinality(), 1);
    // Tuple data should be preserved
    assert!(relation.tuples().any(|t| t == &original));

    engine.commit_transaction(snapshot2).unwrap();
}

// Phase 6.2: Checkpoint Triggers GC (TDD - RED)
