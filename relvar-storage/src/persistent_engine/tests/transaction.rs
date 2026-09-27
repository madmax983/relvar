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

    // T1 began (repeatable read) before T2 committed: T2's insert is newer
    // than T1's snapshot, so T1 must NOT see it. The snapshot's visibility
    // horizon excludes transactions that began after the snapshot was taken,
    // even when they have committed since.
    let rel1 = engine
        .load_relation_for_txn("TEST", snapshot1.txn_id)
        .unwrap();
    assert_eq!(rel1.cardinality(), 0);

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

// Bounded pool: exhaustion at the public boundary.

#[test]
fn test_begin_past_pool_capacity_returns_typed_exhaustion() {
    use relvar_core::storage_engine::{StorageEngine, StorageError};

    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open_with_capacity(temp_dir.path(), 1).unwrap();

    let t1 = engine.begin_transaction().unwrap();

    // The pool holds one live transaction: the next begin is a typed
    // exhaustion error — never a panic, never an untyped string.
    let err = engine.begin_transaction().unwrap_err();
    assert!(
        matches!(
            err,
            StorageError::TransactionPoolExhausted {
                active: 1,
                capacity: 1
            }
        ),
        "expected typed pool exhaustion, got: {err:?}"
    );

    // The failed begin claimed no slot and logged no begin: aborting t1
    // frees the single slot and the next begin succeeds.
    assert_eq!(engine.txn_pool.active_count(), 1);
    engine.rollback_transaction(t1).unwrap();
    assert_eq!(engine.txn_pool.active_count(), 0);
    let t2 = engine.begin_transaction().unwrap();
    engine.commit_transaction(t2).unwrap();
}

#[test]
fn test_pool_slot_reused_across_commit_cycles() {
    use relvar_core::storage_engine::StorageEngine;

    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open_with_capacity(temp_dir.path(), 2).unwrap();

    // Twenty begin/commit cycles through a two-slot pool: slots are
    // recycled, so this never exhausts and never allocates past open.
    for _ in 0..20 {
        let t = engine.begin_transaction().unwrap();
        engine.commit_transaction(t).unwrap();
        assert!(engine.txn_pool.active_count() <= 2);
    }
}

#[test]
fn test_suspended_transaction_holds_pool_slot() {
    use relvar_core::storage_engine::{StorageEngine, StorageError};

    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open_with_capacity(temp_dir.path(), 1).unwrap();

    let t1 = engine.begin_transaction().unwrap();
    let suspended = engine.suspend_transaction().unwrap();
    assert_eq!(suspended.txn_id, t1.txn_id);

    // Suspending does not end the transaction: its pool slot stays live,
    // so no second transaction can begin at capacity 1.
    let err = engine.begin_transaction().unwrap_err();
    assert!(
        matches!(
            err,
            StorageError::TransactionPoolExhausted {
                active: 1,
                capacity: 1
            }
        ),
        "suspended transaction must keep its pool slot, got: {err:?}"
    );

    // Resume and roll back: the slot is freed and begin works again.
    engine.resume_transaction(&suspended).unwrap();
    engine.rollback_transaction(t1).unwrap();
    let t2 = engine.begin_transaction().unwrap();
    engine.commit_transaction(t2).unwrap();
}

#[test]
fn test_zero_capacity_engine_clamps_to_one() {
    use relvar_core::storage_engine::StorageEngine;

    // A zero capacity is meaningless; the pool clamps it to one live
    // transaction rather than refusing every begin.
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open_with_capacity(temp_dir.path(), 0).unwrap();
    let t1 = engine.begin_transaction().unwrap();
    engine.commit_transaction(t1).unwrap();
}

#[test]
fn test_committed_versions_stay_visible_after_history_eviction() {
    use relvar_core::storage_engine::StorageEngine;

    // History-ring churn must never hide committed data: once a
    // transaction's record is evicted from the pool, visibility falls back
    // to the committed set.
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open_with_capacity(temp_dir.path(), 2).unwrap();
    engine.create_relation("TEST", test_rel_type()).unwrap();

    let t1 = engine.begin_transaction().unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    engine.commit_transaction(t1).unwrap();

    // Churn twenty commits through the ring (capacity 8): t1's record is
    // evicted long before the end.
    for _ in 0..20 {
        let t = engine.begin_transaction().unwrap();
        engine.commit_transaction(t).unwrap();
    }

    let t = engine.begin_transaction().unwrap();
    let relation = engine.load_relation("TEST").unwrap();
    assert_eq!(relation.cardinality(), 1);
    assert!(
        relation
            .tuples()
            .any(|t| t == &tuple! { id: 1i64, name: "Alice" })
    );
    engine.commit_transaction(t).unwrap();
}

// WAL failure-injection tests: begin/commit/abort ordering.
//
// Failures are injected through `WalManager`'s test-only fail points
// (`engine.wal.inject_next_log_failure()` /
// `engine.wal.inject_next_flush_failure()` /
// `engine.wal.inject_flush_failure_in(n)`). The engine's fields are
// reachable because these tests live inside the crate.

use crate::wal::{TransactionId, WalRecord};
use relvar_core::storage_engine::StorageError;

#[test]
fn test_begin_wal_log_failure_releases_slot() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
    let active_before = engine.txn_pool.active_count();

    engine.wal.inject_next_log_failure();
    let result = engine.begin_transaction();
    assert!(
        result.is_err(),
        "begin must fail when the Begin WAL record cannot be logged"
    );

    // The pool slot claimed before the WAL write is released: no live
    // transaction is stranded.
    assert_eq!(
        engine.txn_pool.active_count(),
        active_before,
        "failed begin must not strand a pool slot"
    );

    // The engine stays usable.
    let snapshot = engine.begin_transaction().unwrap();
    engine.rollback_transaction(snapshot).unwrap();
}

#[test]
fn test_begin_wal_flush_failure_releases_slot() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
    let active_before = engine.txn_pool.active_count();

    // The Begin record is logged, then its durability flush fails: the
    // slot must still be released (a lost Begin would let recovery reuse
    // the transaction ID).
    engine.wal.inject_next_flush_failure();
    let result = engine.begin_transaction();
    assert!(
        result.is_err(),
        "begin must fail when the Begin record cannot be made durable"
    );
    assert_eq!(
        engine.txn_pool.active_count(),
        active_before,
        "failed begin must not strand a pool slot"
    );

    let snapshot = engine.begin_transaction().unwrap();
    engine.rollback_transaction(snapshot).unwrap();
}

#[test]
fn test_commit_wal_log_failure_aborts_transaction() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
    engine.create_relation("TEST", test_rel_type()).unwrap();

    let snapshot = engine.begin_transaction().unwrap();
    let txn_id = snapshot.txn_id;
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Fail logging the Commit record: the commit must abort the
    // transaction, leaving no Commit behind.
    engine.wal.inject_next_log_failure();
    let result = engine.commit_transaction(snapshot);
    assert!(
        result.is_err(),
        "commit must fail when the Commit record cannot be logged"
    );

    // The transaction is ended (aborted): not live, not committed.
    assert!(
        engine.txn_pool.get_snapshot(txn_id).is_none(),
        "failed commit must end the transaction"
    );
    assert!(
        !engine.committed_txns.contains(&txn_id),
        "failed commit must not join the committed set"
    );

    // The tuple stays invisible: nothing was committed.
    let relation = engine.load_relation("TEST").unwrap();
    assert_eq!(relation.cardinality(), 0);

    // The WAL carries the Abort but no Commit for this transaction.
    let records = engine.wal.scan().unwrap();
    assert!(
        !records
            .iter()
            .any(|(_, record)| matches!(record, WalRecord::Commit { txn_id: t } if *t == txn_id)),
        "no Commit record may exist for the aborted transaction"
    );
    assert!(
        records
            .iter()
            .any(|(_, record)| matches!(record, WalRecord::Abort { txn_id: t } if *t == txn_id)),
        "the abort must be logged"
    );
}

#[test]
fn test_commit_point_flush_failure_ends_as_committed() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
    engine.create_relation("TEST", test_rel_type()).unwrap();

    let snapshot = engine.begin_transaction().unwrap();
    let txn_id = snapshot.txn_id;
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Commit performs two WAL flushes (write records, then the commit
    // record): fail the second — the commit point itself. Its outcome is
    // unknowable (the record may be on stable storage), so the engine
    // ends the transaction as COMMITTED rather than risk a later
    // recovery resurrecting an "aborted" transaction as committed.
    engine.wal.inject_flush_failure_in(1);
    let result = engine.commit_transaction(snapshot);
    let Err(error) = result else {
        panic!("commit-point flush failure must return Err");
    };
    assert!(
        error.to_string().contains("outcome uncertain"),
        "the error must name the uncertainty, got: {error}"
    );

    // The transaction is ended as committed: not live, in the committed set.
    assert!(
        engine.txn_pool.get_snapshot(txn_id).is_none(),
        "uncertain commit must still end the transaction"
    );
    assert!(
        engine.committed_txns.contains(&txn_id),
        "uncertain commit must join the committed set (presumed commit)"
    );

    // New transactions observe the tuple.
    let relation = engine.load_relation("TEST").unwrap();
    assert_eq!(relation.cardinality(), 1);
}

#[test]
fn test_abort_wal_log_failure_leaves_txn_live() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    let snapshot = engine.begin_transaction().unwrap();
    let txn_id: TransactionId = snapshot.txn_id;

    // The Abort record is logged before any in-memory state mutates, so a
    // log failure leaves the transaction live and retryable.
    engine.wal.inject_next_log_failure();
    let result = engine.rollback_transaction(snapshot);
    assert!(
        result.is_err(),
        "abort must fail when the Abort record cannot be logged"
    );
    assert!(
        engine.txn_pool.get_snapshot(txn_id).is_some(),
        "failed abort must leave the transaction live"
    );

    // And the live transaction can still commit afterwards.
    engine
        .commit_transaction(PersistentSnapshot { txn_id })
        .unwrap();
}

#[test]
fn test_commit_history_preflight_fails_before_durable_commit() {
    // Pool capacity 16 -> history ring capacity 64.
    let temp_dir = TempDir::new().unwrap();
    let mut engine =
        PersistentEngine::open_with_pool_capacities(temp_dir.path(), 16, 64, 4).unwrap();

    // A long-lived transaction pins the history horizon: every later
    // commit record is newer than its snapshot, so none is evictable.
    let long = engine.begin_transaction().unwrap();
    let long_id = long.txn_id;

    // Fill the history ring (64 records). Once full, further commits fail
    // their pre-flight and their transactions stay live (wedged pool).
    for _ in 0..70 {
        let snapshot = engine.begin_transaction().unwrap();
        let _ = engine.commit_transaction(snapshot);
    }
    assert_eq!(engine.txn_pool.history_len(), 64);

    // Committing the long-lived transaction must fail with the typed
    // exhaustion BEFORE anything is made durable: the pre-flight fires,
    // not a stranded durable commit the pool cannot record.
    let result = engine.commit_transaction(long);
    assert!(
        matches!(
            result,
            Err(StorageError::TransactionHistoryExhausted { .. })
        ),
        "expected typed history exhaustion, got: {result:?}"
    );

    // Discriminator: no Commit record for the transaction reached the WAL.
    let records = engine.wal.scan().unwrap();
    assert!(
        !records
            .iter()
            .any(|(_, record)| matches!(record, WalRecord::Commit { txn_id: t } if *t == long_id)),
        "pre-flight must fire before the commit record is logged"
    );
}
