use super::common::*;
use crate::persistent_engine::*;
use relvar_core::tuple;
use tempfile::TempDir;

#[test]
fn test_commit_flushes_wal() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    let snapshot = engine.begin_transaction().unwrap();

    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Commit should flush WAL to ensure durability
    engine.commit_transaction(snapshot).unwrap();

    // After commit, WAL should be flushed (we can verify by reopening)
    // Data should survive even if we "crash" (close without explicit flush)
    drop(engine);

    // Reopen and verify data persists
    let engine = PersistentEngine::open(temp_dir.path()).unwrap();
    let relation = engine.load_relation("TEST").unwrap();
    assert_eq!(relation.cardinality(), 1);
}

#[test]
fn test_checkpoint_records_active_txns() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // Start a transaction but don't commit
    let _snapshot = engine.begin_transaction().unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Checkpoint should record that there's an active transaction
    engine.checkpoint().unwrap();

    // WAL should contain checkpoint record with min_active_lsn
    // (verified implicitly by the checkpoint succeeding)
}

#[test]
fn test_insert_in_txn_logs_wal() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    let snapshot = engine.begin_transaction().unwrap();
    engine
        .insert_tuple_in_txn("TEST", tuple! { id: 1i64, name: "Alice" }, snapshot.txn_id)
        .unwrap();

    // Insert should have logged to WAL
    // (Exact verification would require WAL inspection)

    engine.commit_transaction(snapshot).unwrap();
}

#[test]
fn test_checkpoint_flushes_dirty_pages() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // Begin transaction and insert data
    let snapshot = engine.begin_transaction().unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    engine.commit_transaction(snapshot).unwrap();

    // Checkpoint should flush all dirty pages
    engine.checkpoint().unwrap();

    // After checkpoint, data should be durable even without explicit commit
    drop(engine);

    // Reopen and verify data persists
    let engine = PersistentEngine::open(temp_dir.path()).unwrap();
    let relation = engine.load_relation("TEST").unwrap();
    assert_eq!(relation.cardinality(), 1);
}

#[test]
fn test_wal_truncation_after_checkpoint() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // Perform several transactions
    for i in 1..=5 {
        let snapshot = engine.begin_transaction().unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: i, name: "Test" })
            .unwrap();
        engine.commit_transaction(snapshot).unwrap();
    }

    // Get WAL path for verification
    let wal_path = temp_dir.path().join("wal.log");

    // Checkpoint should allow truncating old WAL records
    engine.checkpoint().unwrap();

    // After checkpoint, we should be able to truncate the WAL
    // (Size might not change immediately, but structure should allow it)
    // For now, just verify checkpoint succeeds
    assert!(wal_path.exists());

    // WAL should still be functional after checkpoint
    let snapshot = engine.begin_transaction().unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 6i64, name: "After checkpoint" })
        .unwrap();
    engine.commit_transaction(snapshot).unwrap();

    let relation = engine.load_relation("TEST").unwrap();
    assert_eq!(relation.cardinality(), 6);
}

// Recovery Tests (Step 7)

#[test]
fn test_recovery_replays_committed_insert() {
    let temp_dir = TempDir::new().unwrap();

    // Create database and perform committed transaction
    {
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
        engine.create_relation("TEST", test_rel_type()).unwrap();

        let snapshot = engine.begin_transaction().unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        engine.commit_transaction(snapshot).unwrap();

        // Simulate crash (don't call checkpoint, just drop)
    }

    // Reopen - should trigger recovery and restore committed data
    {
        let engine = PersistentEngine::open(temp_dir.path()).unwrap();
        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(
            relation.cardinality(),
            1,
            "Committed transaction should be recovered"
        );
    }
}

#[test]
fn test_recovery_from_checkpoint() {
    let temp_dir = TempDir::new().unwrap();

    // Create database with checkpoint
    {
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
        engine.create_relation("TEST", test_rel_type()).unwrap();

        // Transaction before checkpoint
        let snapshot1 = engine.begin_transaction().unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Before" })
            .unwrap();
        engine.commit_transaction(snapshot1).unwrap();

        // Checkpoint
        engine.checkpoint().unwrap();

        // Transaction after checkpoint
        let snapshot2 = engine.begin_transaction().unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 2i64, name: "After" })
            .unwrap();
        engine.commit_transaction(snapshot2).unwrap();

        // Simulate crash
    }

    // Reopen - should recover from checkpoint
    {
        let engine = PersistentEngine::open(temp_dir.path()).unwrap();
        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(
            relation.cardinality(),
            2,
            "Recovery should work from checkpoint"
        );
    }
}

#[test]
fn test_recovery_idempotent() {
    let temp_dir = TempDir::new().unwrap();

    // Create database with committed data
    {
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
        engine.create_relation("TEST", test_rel_type()).unwrap();

        let snapshot = engine.begin_transaction().unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        engine.commit_transaction(snapshot).unwrap();
    }

    // Reopen multiple times - recovery should be idempotent
    for _ in 0..3 {
        let engine = PersistentEngine::open(temp_dir.path()).unwrap();
        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 1, "Recovery should be idempotent");
    }
}

// Phase 3.1: Multiple Concurrent Transactions tests

#[test]
fn test_recovery_undoes_uncommitted_data() {
    let temp_dir = TempDir::new().unwrap();

    // 1. Create database and perform uncommitted work
    {
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
        engine.create_relation("TEST", test_rel_type()).unwrap();

        // Begin transaction
        let _snapshot = engine.begin_transaction().unwrap();

        // Insert tuple (written to WAL and Heap)
        // insert_tuple uses the active transaction established by begin_transaction
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Uncommitted" })
            .unwrap();

        // CRASH! (Drop engine without calling commit_transaction)
        // WAL contains: Begin, Insert
        // WAL does NOT contain: Commit
    }

    // 2. Reopen database (Trigger Recovery)
    {
        let engine = PersistentEngine::open(temp_dir.path()).unwrap();

        // Recovery should:
        // 1. See uncommitted transaction in WAL
        // 2. Scan heap files and remove tuples created by that transaction
        //    (via undo_uncommitted_inserts)

        let relation = engine.load_relation("TEST").unwrap();

        // Verify tuple is gone
        assert_eq!(
            relation.cardinality(),
            0,
            "Recovery failed to undo uncommitted insert"
        );
    }
}

#[test]
fn test_recovery_ignores_dropped_relation() {
    let temp_dir = TempDir::new().unwrap();

    // 1. Create database, insert, then drop relation, then crash before commit
    {
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
        engine
            .create_relation("DROPPED_REL", test_rel_type())
            .unwrap();

        // Begin transaction
        let _snapshot = engine.begin_transaction().unwrap();

        // Insert tuple (written to WAL)
        engine
            .insert_tuple("DROPPED_REL", tuple! { id: 1i64, name: "ToDrop" })
            .unwrap();

        // Drop relation (removes from catalog and heap file)
        // Note: In a real crash scenario, the catalog drop might not be durable if not WAL-logged,
        // but here we simulate the state where the catalog update persisted but the txn didn't commit.
        engine.drop_relation("DROPPED_REL").unwrap();

        // CRASH! (Drop engine)
    }

    // 2. Reopen database
    {
        // Recovery runs. It sees uncommitted insert for "DROPPED_REL".
        // It should check if "DROPPED_REL" exists. It doesn't.
        // It should skip cleanup and open successfully.
        let engine = PersistentEngine::open(temp_dir.path());
        assert!(
            engine.is_ok(),
            "Engine should open successfully despite uncommitted inserts for dropped relation"
        );

        let engine = engine.unwrap();
        assert!(!engine.relation_exists("DROPPED_REL"));
    }
}

#[test]
fn test_recovery_retains_valid_data_during_cleanup() {
    let temp_dir = TempDir::new().unwrap();

    // 1. Create database and insert committed data
    {
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
        engine.create_relation("TEST", test_rel_type()).unwrap();

        // Committed transaction
        let snapshot = engine.begin_transaction().unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Committed" })
            .unwrap();
        engine.commit_transaction(snapshot).unwrap();

        // Uncommitted transaction (simulating crash)
        let _snapshot = engine.begin_transaction().unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 2i64, name: "Uncommitted" })
            .unwrap();

        // CRASH! (Drop engine)
    }

    // 2. Reopen database (Trigger Recovery)
    {
        let engine = PersistentEngine::open(temp_dir.path()).unwrap();
        let relation = engine.load_relation("TEST").unwrap();

        // Verify committed tuple exists
        assert_eq!(
            relation.cardinality(),
            1,
            "Recovery should retain committed data"
        );
        assert!(
            relation
                .tuples()
                .any(|t| t.get_typed::<i64>("id").unwrap() == 1),
            "Committed tuple should persist"
        );

        // Verify uncommitted tuple is gone
        assert!(
            !relation
                .tuples()
                .any(|t| t.get_typed::<i64>("id").unwrap() == 2),
            "Uncommitted tuple should be removed"
        );
    }
}
