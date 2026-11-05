//! TTM RM Proscription 6: no internal-level constructs (transaction ids)
//! leak through the engine's public handles.
//!
//! The field privacy itself is pinned by the `compile_fail` doctest on
//! `PersistentSnapshot`; this test pins its `Debug` rendering.

use crate::persistent_engine::*;
use relvar_core::storage_engine::StorageEngine;
use tempfile::TempDir;

#[test]
fn test_snapshot_debug_does_not_reveal_transaction_id() {
    let dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(dir.path()).unwrap();
    // Burn a few ids so the live id is distinctive.
    for _ in 0..41 {
        let snapshot = engine.begin_transaction().unwrap();
        engine.commit_transaction(snapshot).unwrap();
    }
    let snapshot = engine.begin_transaction().unwrap();
    assert_eq!(snapshot.txn_id.value(), 42);

    let rendered = format!("{snapshot:?}");
    assert!(
        !rendered.contains("42") && !rendered.contains("txn_id"),
        "Debug must be opaque, got {rendered}"
    );
    engine.rollback_transaction(snapshot).unwrap();
}
