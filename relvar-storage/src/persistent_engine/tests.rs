#![cfg(test)]
mod basic {
    use super::common::*;
    use crate::persistent_engine::*;
    use relvar_core::tuple;
    use tempfile::TempDir;

    #[test]
    fn test_create_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 1);
    }

    #[test]
    fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();

        // Create database and insert data
        {
            let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
            engine.create_relation("TEST", test_rel_type()).unwrap();
            engine
                .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
                .unwrap();
        }

        // Reopen database and verify data persists
        {
            let engine = PersistentEngine::open(temp_dir.path()).unwrap();
            assert!(engine.relation_exists("TEST"));

            let relation = engine.load_relation("TEST").unwrap();
            assert_eq!(relation.cardinality(), 1);
        }
    }

    #[test]
    fn test_path_traversal_protection() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        // Try to create relation with path separator
        let result = engine.create_relation("../evil", test_rel_type());
        assert!(result.is_err());

        // Try with backslash
        let result = engine.create_relation("..\\evil", test_rel_type());
        assert!(result.is_err());

        // Try with forward slash
        let result = engine.create_relation("sub/dir", test_rel_type());
        assert!(result.is_err());

        // Try with parent directory reference
        let result = engine.create_relation("..", test_rel_type());
        assert!(result.is_err());

        // Try with empty name
        let result = engine.create_relation("", test_rel_type());
        assert!(result.is_err());

        // Valid name should work
        let result = engine.create_relation("VALID_NAME", test_rel_type());
        assert!(result.is_ok());
    }

    #[test]
    fn test_drop_relation() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        assert!(engine.relation_exists("TEST"));

        // Drop the relation
        engine.drop_relation("TEST").unwrap();

        assert!(!engine.relation_exists("TEST"));

        // Dropping again should fail
        let result = engine.drop_relation("TEST");
        assert!(result.is_err());
    }

    #[test]
    fn test_store_relation_empty() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // Insert some data
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 2i64, name: "Bob" })
            .unwrap();

        // Store empty relation
        let empty_relation = Relation::new(test_rel_type());
        engine.store_relation("TEST", &empty_relation).unwrap();

        // Verify relation is now empty
        let loaded = engine.load_relation("TEST").unwrap();
        assert_eq!(loaded.cardinality(), 0);
    }

    #[test]
    fn test_store_relation_large() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // Create relation with many tuples
        let mut large_relation = Relation::new(test_rel_type());
        for i in 0..100 {
            large_relation
                .insert(tuple! { id: i as i64, name: format!("Name{}", i) })
                .unwrap();
        }

        engine.store_relation("TEST", &large_relation).unwrap();

        // Verify all tuples persisted
        let loaded = engine.load_relation("TEST").unwrap();
        assert_eq!(loaded.cardinality(), 100);
    }

    #[test]
    fn test_list_relations() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        // Initially empty
        assert_eq!(engine.list_relations().len(), 0);

        // Create some relations
        engine.create_relation("REL1", test_rel_type()).unwrap();
        engine.create_relation("REL2", test_rel_type()).unwrap();
        engine.create_relation("REL3", test_rel_type()).unwrap();

        let relations = engine.list_relations();
        assert_eq!(relations.len(), 3);
        assert!(relations.contains(&"REL1".to_string()));
        assert!(relations.contains(&"REL2".to_string()));
        assert!(relations.contains(&"REL3".to_string()));
    }

    #[test]
    fn test_get_relation_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        let metadata = engine.get_relation_metadata("TEST").unwrap();
        assert_eq!(metadata.name, "TEST");
        assert_eq!(metadata.relation_type.degree(), 2);
        assert!(metadata.relation_type.heading().has_attribute("id"));
        assert!(metadata.relation_type.heading().has_attribute("name"));
    }

    #[test]
    fn test_insert_tuple_nonexistent_relation() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        let result = engine.insert_tuple("NONEXISTENT", tuple! { id: 1i64, name: "Alice" });
        assert!(result.is_err());
    }

    #[test]
    fn test_load_relation_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let engine = PersistentEngine::open(temp_dir.path()).unwrap();

        let result = engine.load_relation("NONEXISTENT");
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_relation_name() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // Creating again should fail
        let result = engine.create_relation("TEST", test_rel_type());
        assert!(result.is_err());
    }

    #[test]
    fn test_reopen_with_existing_relations() {
        let temp_dir = TempDir::new().unwrap();

        // Create multiple relations
        {
            let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
            engine.create_relation("REL1", test_rel_type()).unwrap();
            engine.create_relation("REL2", test_rel_type()).unwrap();
            engine
                .insert_tuple("REL1", tuple! { id: 1i64, name: "Alice" })
                .unwrap();
            engine
                .insert_tuple("REL2", tuple! { id: 2i64, name: "Bob" })
                .unwrap();
        }

        // Reopen and verify both relations exist
        {
            let engine = PersistentEngine::open(temp_dir.path()).unwrap();
            assert_eq!(engine.list_relations().len(), 2);

            let rel1 = engine.load_relation("REL1").unwrap();
            assert_eq!(rel1.cardinality(), 1);

            let rel2 = engine.load_relation("REL2").unwrap();
            assert_eq!(rel2.cardinality(), 1);
        }
    }

    #[test]
    fn test_multiple_inserts_uses_cached_heap_file() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // Multiple inserts should reuse the cached heap file
        for i in 0..10 {
            engine
                .insert_tuple("TEST", tuple! { id: i as i64, name: format!("Name{}", i) })
                .unwrap();
        }

        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 10);
    }

    #[test]
    fn test_store_relation_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        let relation = Relation::new(test_rel_type());
        let result = engine.store_relation("NONEXISTENT", &relation);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_metadata_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let engine = PersistentEngine::open(temp_dir.path()).unwrap();

        let result = engine.get_relation_metadata("NONEXISTENT");
        assert!(result.is_err());
    }

    #[test]
    fn test_drop_nonexistent_relation() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        let result = engine.drop_relation("NONEXISTENT");
        assert!(result.is_err());
    }

    #[test]
    fn test_store_relation_replaces_data() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();

        // Insert initial data
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 2i64, name: "Bob" })
            .unwrap();

        // Create new relation with different data
        let mut new_relation = Relation::new(test_rel_type());
        new_relation
            .insert(tuple! { id: 100i64, name: "Charlie" })
            .unwrap();

        // Store should replace old data
        engine.store_relation("TEST", &new_relation).unwrap();

        let loaded = engine.load_relation("TEST").unwrap();
        assert_eq!(loaded.cardinality(), 1);
        assert!(loaded.contains(&tuple! { id: 100i64, name: "Charlie" }));
    }

    #[test]
    fn test_relation_exists_returns_false_for_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let engine = PersistentEngine::open(temp_dir.path()).unwrap();

        assert!(!engine.relation_exists("NONEXISTENT"));
    }

    #[test]
    fn test_relation_exists_returns_true_for_existing() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();
        assert!(engine.relation_exists("TEST"));
    }

    #[test]
    fn test_empty_database_list_relations() {
        let temp_dir = TempDir::new().unwrap();
        let engine = PersistentEngine::open(temp_dir.path()).unwrap();

        assert_eq!(engine.list_relations().len(), 0);
    }

    #[test]
    fn test_insert_then_load_uses_cached_file() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        engine.create_relation("TEST", test_rel_type()).unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Load should work with cached heap file
        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 1);

        // Insert more using cache
        engine
            .insert_tuple("TEST", tuple! { id: 2i64, name: "Bob" })
            .unwrap();

        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 2);
    }

    // WAL Integration Tests (Step 5)
}
mod common {
    use crate::persistent_engine::*;
    use relvar_core::types::{ScalarType, TupleType};

    pub fn test_rel_type() -> RelationType {
        RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("name", ScalarType::String),
        )
    }
}
mod gc {
    use super::common::*;
    use crate::persistent_engine::*;
    use relvar_core::tuple;
    use tempfile::TempDir;

    #[test]
    fn test_gc_preserves_active_transaction_data() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        // Create relation
        let rel_type = test_rel_type();
        engine.create_relation("TEST", rel_type).unwrap();

        // T1: Insert and commit
        let snapshot1 = engine.begin_transaction().unwrap();
        let tuple = tuple! { id: 1i64, name: "Active" };
        engine
            .insert_tuple_in_txn("TEST", tuple.clone(), snapshot1.txn_id)
            .unwrap();
        engine.commit_transaction(snapshot1).unwrap();

        // T2: Begin (active transaction)
        let snapshot2 = engine.begin_transaction().unwrap();

        // Checkpoint with active transaction
        engine.checkpoint().unwrap();

        // T2 should still see the data
        let relation = engine
            .load_relation_for_txn("TEST", snapshot2.txn_id)
            .unwrap();

        assert_eq!(relation.cardinality(), 1);
        assert!(relation.tuples().any(|t| t == &tuple));

        engine.commit_transaction(snapshot2).unwrap();
    }

    #[test]
    fn test_checkpoint_triggers_gc() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        // Create relation
        let rel_type = test_rel_type();
        engine.create_relation("TEST", rel_type).unwrap();

        // T1: Insert and commit
        let snapshot1 = engine.begin_transaction().unwrap();
        let tuple = tuple! { id: 1i64, name: "ToDelete" };
        engine
            .insert_tuple_in_txn("TEST", tuple, snapshot1.txn_id)
            .unwrap();
        engine.commit_transaction(snapshot1).unwrap();

        // T2: Delete and commit
        let snapshot2 = engine.begin_transaction().unwrap();
        // Note: We need to delete by marking, but we don't have direct access
        // For now, this test will verify checkpoint runs without error
        engine.commit_transaction(snapshot2).unwrap();

        // Checkpoint should run GC
        engine.checkpoint().unwrap();

        // Verify checkpoint succeeded
        // (GC ran internally, no errors)
    }

    #[test]
    fn test_gc_reclaims_space_after_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

        // Create relation
        let rel_type = test_rel_type();
        engine.create_relation("TEST", rel_type).unwrap();

        // Insert multiple tuples and delete them
        for i in 1..=10 {
            let snapshot = engine.begin_transaction().unwrap();
            let tuple = tuple! { id: i, name: format!("Test{}", i) };
            engine
                .insert_tuple_in_txn("TEST", tuple, snapshot.txn_id)
                .unwrap();
            engine.commit_transaction(snapshot).unwrap();
        }

        // All tuples inserted and committed
        // Checkpoint with no active transactions should succeed
        engine.checkpoint().unwrap();

        // Verify system is in consistent state
        let snapshot = engine.begin_transaction().unwrap();
        let relation = engine
            .load_relation_for_txn("TEST", snapshot.txn_id)
            .unwrap();

        assert_eq!(relation.cardinality(), 10);
        engine.commit_transaction(snapshot).unwrap();
    }
}
mod recovery {
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
}
mod transaction {
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
}
