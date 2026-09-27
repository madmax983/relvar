//! # MVCC visibility checker
//!
//! Determines which tuple version a transaction's snapshot may see, per the
//! Snapshot Isolation rules documented in [`crate::mvcc::snapshot`].
//!
//! ## TTM Compliance
//!
//! Visibility is an internal storage-layer mechanism (Physical Data
//! Independence). The logical layer never sees versions or snapshots.

use crate::mvcc::{TransactionSnapshot, TxnPool};
use crate::wal::TransactionId;
use std::collections::HashSet;

/// Metadata header stored with every tuple version on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionMetadata {
    /// Transaction that created this version.
    pub xmin: TransactionId,
    /// Transaction that deleted (or superseded) this version, if any.
    pub xmax: Option<TransactionId>,
}

/// Returns `true` when `version` is visible to `snapshot`.
///
/// Liveness is resolved through `pool`; `committed` is the engine's set of
/// committed transaction IDs. See [`crate::mvcc::snapshot`] for the rules.
pub fn is_visible(
    version: &VersionMetadata,
    snapshot: &TransactionSnapshot,
    pool: &TxnPool,
    committed: &HashSet<TransactionId>,
) -> bool {
    if !is_created_visible(version, snapshot, pool, committed) {
        return false;
    }

    is_deletion_invisible(version, snapshot, pool, committed)
}

fn is_created_visible(
    version: &VersionMetadata,
    snapshot: &TransactionSnapshot,
    pool: &TxnPool,
    committed: &HashSet<TransactionId>,
) -> bool {
    // Own writes are always visible to the writing transaction.
    if version.xmin == snapshot.txn_id {
        return true;
    }

    // Versions created by transactions that began after this snapshot was
    // taken are from the snapshot's future: invisible even if they have
    // since committed. The commit set and the active set alone cannot
    // exclude them, because a transaction that starts after our snapshot is
    // in neither. This upper bound is what makes Repeatable Read repeat.
    if version.xmin >= snapshot.horizon {
        return false;
    }

    // Versions created by transactions that were still active when the
    // snapshot was taken are in flux: invisible.
    if snapshot.is_active(version.xmin, pool) {
        return false;
    }

    committed.contains(&version.xmin)
}

fn is_deletion_invisible(
    version: &VersionMetadata,
    snapshot: &TransactionSnapshot,
    pool: &TxnPool,
    committed: &HashSet<TransactionId>,
) -> bool {
    let Some(xmax) = version.xmax else {
        return true;
    };

    // A transaction's own deletes hide the version from itself.
    if xmax == snapshot.txn_id {
        return false;
    }

    // Deletes made by transactions that began after the snapshot was taken
    // are future changes: the version is still visible.
    if xmax >= snapshot.horizon {
        return true;
    }

    if !committed.contains(&xmax) {
        return true;
    }

    if snapshot.is_active(xmax, pool) {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wal::Lsn;

    // Helper to create test TransactionId
    fn test_txn(value: u64) -> TransactionId {
        TransactionId::new(value)
    }

    // Helper to create test LSN
    fn test_lsn(value: u64) -> Lsn {
        Lsn::new(value)
    }

    fn new_version(xmin: TransactionId) -> VersionMetadata {
        VersionMetadata { xmin, xmax: None }
    }

    fn new_version_with_xmax(xmin: TransactionId, xmax: TransactionId) -> VersionMetadata {
        VersionMetadata {
            xmin,
            xmax: Some(xmax),
        }
    }

    /// Test fixture: a bounded pool plus the engine's committed set.
    struct Fixture {
        pool: TxnPool,
        committed: HashSet<TransactionId>,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                pool: TxnPool::new(16),
                committed: HashSet::new(),
            }
        }

        /// Begins `txn_id` at `lsn` with an open horizon; returns its snapshot.
        fn begin(&mut self, txn_id: TransactionId, lsn: u64) -> TransactionSnapshot {
            self.pool
                .begin(txn_id, test_lsn(lsn), test_txn(u64::MAX))
                .unwrap()
        }

        /// Commits `txn_id` at `lsn` and records it in the committed set.
        fn commit(&mut self, txn_id: TransactionId, lsn: u64) {
            self.pool.commit(txn_id, test_lsn(lsn)).unwrap();
            self.committed.insert(txn_id);
        }

        /// Aborts `txn_id` (stays out of the committed set).
        fn abort(&mut self, txn_id: TransactionId) {
            self.pool.abort(txn_id).unwrap();
        }

        fn is_visible(&self, version: &VersionMetadata, snapshot: &TransactionSnapshot) -> bool {
            is_visible(version, snapshot, &self.pool, &self.committed)
        }
    }

    #[test]
    fn test_version_metadata_new() {
        let xmin = test_txn(1);
        let version = new_version(xmin);

        assert_eq!(version.xmin, xmin);
        assert_eq!(version.xmax, None);
    }

    #[test]
    fn test_version_metadata_new_with_xmax() {
        let xmin = test_txn(1);
        let xmax = test_txn(2);
        let version = new_version_with_xmax(xmin, xmax);

        assert_eq!(version.xmin, xmin);
        assert_eq!(version.xmax, Some(xmax));
    }

    #[test]
    fn test_version_visible_to_creator() {
        // Transaction creates a version and should see it
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let snapshot = fx.begin(t1, 100);

        let version = new_version(t1);

        // Creator should see its own uncommitted version
        assert!(fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn test_version_invisible_to_concurrent() {
        // T1 creates a version, T2 (concurrent) should NOT see it before T1 commits
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        fx.begin(t1, 50);
        let snapshot = fx.begin(t2, 100);

        let version = new_version(t1);

        assert!(!fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn test_version_visible_after_commit() {
        // T1 creates and commits a version, T2 (starts later) should see it
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        fx.begin(t1, 50);
        fx.commit(t1, 100);
        let snapshot = fx.begin(t2, 200);

        let version = new_version(t1);

        assert!(fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn test_deleted_version_invisible() {
        // T1 creates, T2 deletes and commits, T3 should NOT see it
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        let t3 = test_txn(3);
        fx.begin(t1, 50);
        fx.commit(t1, 100);
        fx.begin(t2, 150);
        fx.commit(t2, 200);
        let snapshot = fx.begin(t3, 300);

        let version = new_version_with_xmax(t1, t2);

        assert!(!fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn test_version_visible_deletion_uncommitted() {
        // T1 creates and commits, T2 deletes (uncommitted), T3 should still see it
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        let t3 = test_txn(3);
        fx.begin(t1, 50);
        fx.commit(t1, 100);
        fx.begin(t2, 150); // T2 live: its delete is not yet visible
        let snapshot = fx.begin(t3, 300);

        let version = new_version_with_xmax(t1, t2);

        assert!(fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn test_aborted_version_never_visible() {
        // T1 creates but aborts, T2 should NOT see it
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        fx.begin(t1, 50);
        fx.abort(t1);
        let snapshot = fx.begin(t2, 200);

        let version = new_version(t1);

        assert!(!fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn test_version_visible_concurrent_deletion() {
        // T1 creates and commits, T2 and T3 concurrent, T2 deletes then commits
        // after T3's snapshot: T3 should still see it (deletion concurrent).
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        let t3 = test_txn(3);
        fx.begin(t1, 50);
        fx.commit(t1, 100);
        fx.begin(t2, 150);
        let snapshot = fx.begin(t3, 300);
        fx.commit(t2, 350); // T2 committed AFTER T3 started

        let version = new_version_with_xmax(t1, t2);

        // Visible because T2 was active when T3's snapshot was taken.
        assert!(fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn test_version_invisible_not_committed() {
        // T1 creates (uncommitted), T2 should NOT see it
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        fx.begin(t1, 50);
        let snapshot = fx.begin(t2, 200);

        let version = new_version(t1);

        assert!(!fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn test_visibility_creator_uncommitted_deleted() {
        // T1 creates and deletes in same txn (uncommitted)
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let snapshot = fx.begin(t1, 100);

        let version = new_version_with_xmax(t1, t1);

        // T1 deleted its own version, should NOT see it
        assert!(!fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn test_visibility_empty_committed_set() {
        // No transactions committed yet
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        fx.begin(t1, 50);
        let snapshot = fx.begin(t2, 100);

        let version = new_version(t1);

        assert!(!fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn test_visibility_multiple_concurrent() {
        // T4 sees T2 and T3 as active, but T1 committed before T4 started.
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        let t3 = test_txn(3);
        let t4 = test_txn(4);
        fx.begin(t1, 50);
        fx.commit(t1, 100);
        fx.begin(t2, 150);
        fx.begin(t3, 160);
        let snapshot = fx.begin(t4, 400);

        let version = new_version(t1);

        assert!(fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn test_visibility_deletion_by_creator() {
        // T1 creates, commits, then in a new txn deletes
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t1_new = test_txn(10); // Different txn by same "user"
        fx.begin(t1, 50);
        fx.commit(t1, 100);
        let snapshot = fx.begin(t1_new, 150);
        fx.commit(t1_new, 200);

        let version = new_version_with_xmax(t1, t1_new);

        assert!(!fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn test_visibility_long_running_transaction() {
        // T1 starts early; T2 begins after T1's snapshot and commits.
        // T1 must NOT see T2's version: T2 is in T1's future (horizon).
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        // T1's horizon is 2: T2 has not begun yet at T1's snapshot.
        let snapshot = fx.begin(t1, 100).with_horizon(test_txn(2));
        fx.begin(t2, 150);
        fx.commit(t2, 200);

        let version = new_version(t2);

        assert!(!fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn test_visibility_chain_scenario() {
        // T1 creates version V1, commits
        // T2 updates to V2 (marks V1.xmax=T2, creates V2.xmin=T2), commits
        // T3 starts and should see V2, not V1
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        let t3 = test_txn(3);
        fx.begin(t1, 50);
        fx.commit(t1, 100);
        fx.begin(t2, 150);
        fx.commit(t2, 200);
        let snapshot = fx.begin(t3, 300);

        let v1 = new_version_with_xmax(t1, t2);
        let v2 = new_version(t2);

        // V1 should NOT be visible (deleted by committed T2)
        assert!(!fx.is_visible(&v1, &snapshot));

        // V2 should be visible (created by committed T2)
        assert!(fx.is_visible(&v2, &snapshot));
    }

    #[test]
    fn should_return_true_when_version_created_by_committed_txn() {
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        fx.begin(t1, 50);
        fx.commit(t1, 100);
        let snapshot = fx.begin(t2, 200);

        let version = VersionMetadata {
            xmin: t1,
            xmax: None,
        };

        assert!(fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn should_return_false_when_version_created_by_uncommitted_concurrent_txn() {
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        fx.begin(t1, 50);
        let snapshot = fx.begin(t2, 100);

        let version = VersionMetadata {
            xmin: t1,
            xmax: None,
        };

        assert!(!fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn should_return_true_when_version_created_by_self() {
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let snapshot = fx.begin(t1, 100);

        let version = VersionMetadata {
            xmin: t1,
            xmax: None,
        };

        // Transaction can see its own changes
        assert!(fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn should_return_false_when_version_deleted_by_committed_txn() {
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        let t3 = test_txn(3);
        fx.begin(t1, 50);
        fx.commit(t1, 100);
        fx.begin(t2, 120);
        fx.commit(t2, 150);
        let snapshot = fx.begin(t3, 200);

        let version = VersionMetadata {
            xmin: t1,
            xmax: Some(t2),
        };

        // T3 shouldn't see it because it was deleted by committed T2
        assert!(!fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn should_return_true_when_version_deleted_by_uncommitted_txn() {
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        let t3 = test_txn(3);
        fx.begin(t1, 50);
        fx.commit(t1, 100);
        fx.begin(t2, 150); // t2 live: its delete is not yet visible
        let snapshot = fx.begin(t3, 200);

        let version = VersionMetadata {
            xmin: t1,
            xmax: Some(t2),
        };

        // T3 should see it because deletion is not yet visible
        assert!(fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn should_return_false_when_version_deleted_by_self() {
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let snapshot = fx.begin(t1, 100);
        fx.commit(t1, 150); // created by self, deleted by self

        let version = VersionMetadata {
            xmin: t1,
            xmax: Some(t1),
        };

        assert!(!fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn should_return_false_when_creator_began_after_snapshot_even_if_committed() {
        // T1's snapshot is taken with horizon 2: only transactions with
        // IDs < 2 began before the snapshot.
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        let snapshot = fx.begin(t1, 100).with_horizon(test_txn(2));
        fx.commit(t2, 150); // t2 committed AFTER t1's snapshot was taken

        let version = VersionMetadata {
            xmin: t2,
            xmax: None,
        };

        // T2 is committed and was never in T1's active set, but it began
        // after the snapshot: its version is from T1's future.
        assert!(!fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn should_return_true_when_creator_committed_before_snapshot_horizon() {
        let mut fx = Fixture::new();
        let t2 = test_txn(2);
        let t3 = test_txn(3);
        fx.begin(t2, 50);
        fx.commit(t2, 100);
        // T3's snapshot horizon is 4: T2 began before it.
        let snapshot = fx.begin(t3, 200).with_horizon(test_txn(4));

        let version = VersionMetadata {
            xmin: t2,
            xmax: None,
        };

        assert!(fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn should_return_true_when_deleter_began_after_snapshot() {
        // T2 deletes a version after T3's snapshot was taken; T3 must still
        // see the version even though T2 has committed.
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t2 = test_txn(4);
        let t3 = test_txn(3);
        fx.begin(t1, 50);
        fx.commit(t1, 100);
        // T3's snapshot horizon is 4: T2 began after it.
        let snapshot = fx.begin(t3, 200).with_horizon(test_txn(4));
        fx.begin(t2, 210);
        fx.commit(t2, 250);

        let version = VersionMetadata {
            xmin: t1,
            xmax: Some(t2),
        };

        assert!(fx.is_visible(&version, &snapshot));
    }

    #[test]
    fn should_return_false_when_deleter_committed_before_snapshot_horizon() {
        let mut fx = Fixture::new();
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        let t3 = test_txn(3);
        fx.begin(t1, 50);
        fx.commit(t1, 100);
        fx.begin(t2, 120);
        fx.commit(t2, 150);
        // Horizon 3: the delete by T2 predates T3's snapshot.
        let snapshot = fx.begin(t3, 200).with_horizon(test_txn(3));

        let version = VersionMetadata {
            xmin: t1,
            xmax: Some(t2),
        };

        assert!(!fx.is_visible(&version, &snapshot));
    }
}
