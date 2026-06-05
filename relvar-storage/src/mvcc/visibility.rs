//! # MVCC Visibility Rules
//!
//! This module contains the core logic for Snapshot Isolation: determining if a
//! specific tuple version is visible to a specific transaction.
//!
//! A tuple version is visible to transaction `T` if:
//! 1. `version.xmin` committed before `T.snapshot_lsn` AND
//! 2. `version.xmin` not in `T.active_txns` (not concurrent uncommitted) AND
//! 3. `version.xmax` is `None` OR (`version.xmax` committed after `T.snapshot_lsn` OR `version.xmax` in `T.active_txns`)

use crate::mvcc::TransactionSnapshot;
use crate::wal::TransactionId;
use std::collections::HashSet;

/// Version metadata stored with each tuple.
///
/// Every tuple in the heap contains this header. It tracks which transaction
/// inserted the tuple (`xmin`) and which transaction deleted or updated it (`xmax`).
///
/// If `xmax` is `None`, the tuple is considered "alive" (subject to visibility rules).
/// If `xmax` is `Some`, the tuple is a tombstone (deleted or an older version of an update).
///
/// ## Examples
///
/// ```ignore
/// use relvar_storage::mvcc::VersionMetadata;
/// use relvar_storage::wal::TransactionId;
///
/// // A newly inserted tuple
/// let inserted = VersionMetadata {
///     xmin: TransactionId::new(42),
///     xmax: None, // Still alive!
/// };
///
/// // A deleted tuple
/// let deleted = VersionMetadata {
///     xmin: TransactionId::new(42),
///     xmax: Some(TransactionId::new(45)), // Txn 45 deleted this
/// };
/// ```ignore
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionMetadata {
    /// Transaction that created this version
    pub xmin: TransactionId,
    /// Transaction that deleted/updated this version (None = still visible)
    pub xmax: Option<TransactionId>,
}

/// Determines if a tuple version is visible to a transaction.
///
/// This function enforces the Snapshot Isolation guarantees. It evaluates the
/// `version` metadata against the transaction's `snapshot` and the global `committed` set.
///
/// # Visibility Rules
/// A version is visible if:
/// 1. `xmin` is committed (in `committed` set).
/// 2. `xmin` is NOT in the snapshot's `active_txns` (wasn't concurrent uncommitted).
/// 3. `xmax` is `None` (not deleted) OR `xmax` is NOT committed OR `xmax` was concurrent.
///
/// **Exception:** A transaction can always see its own uncommitted changes.
///
/// # Arguments
/// * `version` - The version metadata attached to the tuple.
/// * `snapshot` - The snapshot defining the reading transaction's point-in-time view.
/// * `committed` - The global set of all transaction IDs that have successfully committed.
///
/// # Returns
/// `true` if the version should be returned by a scan, `false` if it should be skipped.
///
/// ## Examples
///
/// ```ignore
/// use relvar_storage::mvcc::{is_visible, VersionMetadata, TransactionSnapshot};
/// use relvar_storage::wal::{Lsn, TransactionId};
/// use std::collections::HashSet;
///
/// let t1 = TransactionId::new(1); // Committed txn
/// let t2 = TransactionId::new(2); // Our scanning txn
///
/// let mut committed = HashSet::new();
/// committed.insert(t1);
///
/// // T2 starts, T1 is already committed, no active txns
/// let snapshot = TransactionSnapshot::new(t2, Lsn::new(100), std::collections::HashSet::from([]));
///
/// // Tuple inserted by T1, never deleted
/// let version = VersionMetadata { xmin: t1, xmax: None };
///
/// // T2 can see T1's insert because T1 is committed and wasn't active during T2's start
/// assert!(is_visible(&version, &snapshot, &committed));
/// ```ignore
pub fn is_visible(
    version: &VersionMetadata,
    snapshot: &TransactionSnapshot,
    committed: &HashSet<TransactionId>,
) -> bool {
    if !is_created_visible(version, snapshot, committed) {
        return false;
    }

    is_deletion_invisible(version, snapshot, committed)
}

fn is_created_visible(
    version: &VersionMetadata,
    snapshot: &TransactionSnapshot,
    committed: &HashSet<TransactionId>,
) -> bool {
    let xmin_visible = committed.contains(&version.xmin) || version.xmin == snapshot.txn_id;

    if snapshot.is_active(version.xmin) && version.xmin != snapshot.txn_id {
        return false;
    }

    xmin_visible
}

fn is_deletion_invisible(
    version: &VersionMetadata,
    snapshot: &TransactionSnapshot,
    committed: &HashSet<TransactionId>,
) -> bool {
    let Some(xmax) = version.xmax else {
        return true;
    };

    if xmax == snapshot.txn_id {
        return false;
    }

    if !committed.contains(&xmax) {
        return true;
    }

    if snapshot.is_active(xmax) {
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
        let txn_id = test_txn(1);
        let version = new_version(txn_id);

        let snapshot = TransactionSnapshot::new(txn_id, test_lsn(100), std::collections::HashSet::from([]));
        let committed = HashSet::new(); // txn not committed yet

        // Creator should see its own uncommitted version
        assert!(is_visible(&version, &snapshot, &committed));
    }

    #[test]
    fn test_version_invisible_to_concurrent() {
        // T1 creates a version, T2 (concurrent) should NOT see it before T1 commits
        let t1 = test_txn(1);
        let t2 = test_txn(2);

        let version = new_version(t1);

        // T2's snapshot shows T1 as active (concurrent)
        let snapshot = TransactionSnapshot::new(t2, test_lsn(100), std::collections::HashSet::from([t1]));
        let committed = HashSet::new(); // T1 not committed

        assert!(!is_visible(&version, &snapshot, &committed));
    }

    #[test]
    fn test_version_visible_after_commit() {
        // T1 creates and commits a version, T2 (starts later) should see it
        let t1 = test_txn(1);
        let t2 = test_txn(2);

        let version = new_version(t1);

        // T2 starts after T1 committed (T1 not in active list)
        let snapshot = TransactionSnapshot::new(t2, test_lsn(200), std::collections::HashSet::from([]));
        let mut committed = HashSet::new();
        committed.insert(t1); // T1 committed

        assert!(is_visible(&version, &snapshot, &committed));
    }

    #[test]
    fn test_deleted_version_invisible() {
        // T1 creates, T2 deletes and commits, T3 should NOT see it
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        let t3 = test_txn(3);

        let version = new_version_with_xmax(t1, t2);

        // T3 starts after both T1 and T2 committed
        let snapshot = TransactionSnapshot::new(t3, test_lsn(300), std::collections::HashSet::from([]));
        let mut committed = HashSet::new();
        committed.insert(t1);
        committed.insert(t2); // Deletion committed

        assert!(!is_visible(&version, &snapshot, &committed));
    }

    #[test]
    fn test_version_visible_deletion_uncommitted() {
        // T1 creates and commits, T2 deletes (uncommitted), T3 should still see it
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        let t3 = test_txn(3);

        let version = new_version_with_xmax(t1, t2);

        // T3 starts, sees T2 as active
        let snapshot = TransactionSnapshot::new(t3, test_lsn(300), std::collections::HashSet::from([t2]));
        let mut committed = HashSet::new();
        committed.insert(t1);
        // T2 not committed

        assert!(is_visible(&version, &snapshot, &committed));
    }

    #[test]
    fn test_aborted_version_never_visible() {
        // T1 creates but aborts, T2 should NOT see it
        let t1 = test_txn(1);
        let t2 = test_txn(2);

        let version = new_version(t1);

        let snapshot = TransactionSnapshot::new(t2, test_lsn(200), std::collections::HashSet::from([]));
        let committed = HashSet::new(); // T1 NOT in committed set (aborted)

        assert!(!is_visible(&version, &snapshot, &committed));
    }

    #[test]
    fn test_version_visible_concurrent_deletion() {
        // T1 creates and commits, T2 and T3 concurrent, T2 deletes
        // T3 should still see it (deletion concurrent with T3)
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        let t3 = test_txn(3);

        let version = new_version_with_xmax(t1, t2);

        // T3's snapshot shows T2 as active
        let snapshot = TransactionSnapshot::new(t3, test_lsn(300), std::collections::HashSet::from([t2]));
        let mut committed = HashSet::new();
        committed.insert(t1);
        committed.insert(t2); // T2 committed AFTER T3 started

        // Should be visible because T2 was active when T3 started
        assert!(is_visible(&version, &snapshot, &committed));
    }

    #[test]
    fn test_version_invisible_not_committed() {
        // T1 creates (uncommitted), T2 should NOT see it
        let t1 = test_txn(1);
        let t2 = test_txn(2);

        let version = new_version(t1);

        let snapshot = TransactionSnapshot::new(t2, test_lsn(200), std::collections::HashSet::from([]));
        let committed = HashSet::new(); // T1 not committed

        // T2 can't see T1's version if T1 != T2
        assert!(!is_visible(&version, &snapshot, &committed));
    }

    #[test]
    fn test_visibility_creator_uncommitted_deleted() {
        // T1 creates and deletes in same txn (uncommitted)
        let t1 = test_txn(1);

        let version = new_version_with_xmax(t1, t1);

        let snapshot = TransactionSnapshot::new(t1, test_lsn(100), std::collections::HashSet::from([]));
        let committed = HashSet::new();

        // T1 deleted its own version, should NOT see it
        assert!(!is_visible(&version, &snapshot, &committed));
    }

    #[test]
    fn test_visibility_empty_committed_set() {
        // No transactions committed yet
        let t1 = test_txn(1);
        let t2 = test_txn(2);

        let version = new_version(t1);

        let snapshot = TransactionSnapshot::new(t2, test_lsn(100), std::collections::HashSet::from([]));
        let committed = HashSet::new();

        assert!(!is_visible(&version, &snapshot, &committed));
    }

    #[test]
    fn test_visibility_multiple_concurrent() {
        // Multiple concurrent transactions
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        let t3 = test_txn(3);
        let t4 = test_txn(4);

        let version = new_version(t1);

        // T4 sees T2 and T3 as active, but not T1 (T1 committed before T4 started)
        let snapshot = TransactionSnapshot::new(t4, test_lsn(400), std::collections::HashSet::from([t2, t3]));
        let mut committed = HashSet::new();
        committed.insert(t1);

        assert!(is_visible(&version, &snapshot, &committed));
    }

    #[test]
    fn test_visibility_deletion_by_creator() {
        // T1 creates, commits, then in a new txn deletes
        let t1 = test_txn(1);
        let t1_new = test_txn(10); // Different txn by same "user"

        let version = new_version_with_xmax(t1, t1_new);

        let snapshot = TransactionSnapshot::new(t1_new, test_lsn(200), std::collections::HashSet::from([]));
        let mut committed = HashSet::new();
        committed.insert(t1);
        committed.insert(t1_new);

        // Version deleted by committed txn
        assert!(!is_visible(&version, &snapshot, &committed));
    }

    #[test]
    fn test_visibility_long_running_transaction() {
        // T1 starts early, T2 creates and commits, T1 should NOT see it
        // (T2 was active when T1 started)
        let t1 = test_txn(1);
        let t2 = test_txn(2);

        let version = new_version(t2);

        // T1's snapshot captured T2 as active
        let snapshot = TransactionSnapshot::new(t1, test_lsn(100), std::collections::HashSet::from([t2]));
        let mut committed = HashSet::new();
        committed.insert(t2);

        // T1 should NOT see T2's changes (snapshot isolation)
        assert!(!is_visible(&version, &snapshot, &committed));
    }

    #[test]
    fn test_visibility_chain_scenario() {
        // T1 creates version V1, commits
        // T2 updates to V2 (marks V1.xmax=T2, creates V2.xmin=T2), commits
        // T3 starts and should see V2, not V1

        let t1 = test_txn(1);
        let t2 = test_txn(2);
        let t3 = test_txn(3);

        let v1 = new_version_with_xmax(t1, t2);
        let v2 = new_version(t2);

        let snapshot = TransactionSnapshot::new(t3, test_lsn(300), std::collections::HashSet::from([]));
        let mut committed = HashSet::new();
        committed.insert(t1);
        committed.insert(t2);

        // V1 should NOT be visible (deleted by committed T2)
        assert!(!is_visible(&v1, &snapshot, &committed));

        // V2 should be visible (created by committed T2)
        assert!(is_visible(&v2, &snapshot, &committed));
    }

    #[test]
    fn should_return_true_when_version_created_by_committed_txn() {
        let t1 = test_txn(1);
        let t2 = test_txn(2);

        let version = VersionMetadata {
            xmin: t1,
            xmax: None,
        };
        let snapshot = TransactionSnapshot::new(t2, test_lsn(100), std::collections::HashSet::from([]));
        let mut committed = HashSet::new();
        committed.insert(t1);

        assert!(is_visible(&version, &snapshot, &committed));
    }

    #[test]
    fn should_return_false_when_version_created_by_uncommitted_concurrent_txn() {
        let t1 = test_txn(1);
        let t2 = test_txn(2);

        let version = VersionMetadata {
            xmin: t1,
            xmax: None,
        };
        // t2 sees t1 as active
        let snapshot = TransactionSnapshot::new(t2, test_lsn(100), std::collections::HashSet::from([t1]));
        let committed = HashSet::new(); // t1 not committed

        assert!(!is_visible(&version, &snapshot, &committed));
    }

    #[test]
    fn should_return_true_when_version_created_by_self() {
        let t1 = test_txn(1);

        let version = VersionMetadata {
            xmin: t1,
            xmax: None,
        };
        let snapshot = TransactionSnapshot::new(t1, test_lsn(100), std::collections::HashSet::from([]));
        let committed = HashSet::new(); // t1 not committed

        // Transaction can see its own changes
        assert!(is_visible(&version, &snapshot, &committed));
    }

    #[test]
    fn should_return_false_when_version_deleted_by_committed_txn() {
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        let t3 = test_txn(3);

        let version = VersionMetadata {
            xmin: t1,
            xmax: Some(t2),
        };
        let snapshot = TransactionSnapshot::new(t3, test_lsn(200), std::collections::HashSet::from([]));
        let mut committed = HashSet::new();
        committed.insert(t1);
        committed.insert(t2); // t2 is committed

        // T3 shouldn't see it because it was deleted by committed T2
        assert!(!is_visible(&version, &snapshot, &committed));
    }

    #[test]
    fn should_return_true_when_version_deleted_by_uncommitted_txn() {
        let t1 = test_txn(1);
        let t2 = test_txn(2);
        let t3 = test_txn(3);

        let version = VersionMetadata {
            xmin: t1,
            xmax: Some(t2),
        };
        // t3 sees t2 as active
        let snapshot = TransactionSnapshot::new(t3, test_lsn(200), std::collections::HashSet::from([t2]));
        let mut committed = HashSet::new();
        committed.insert(t1); // t1 committed, t2 not

        // T3 should see it because deletion is not yet visible
        assert!(is_visible(&version, &snapshot, &committed));
    }

    #[test]
    fn should_return_false_when_version_deleted_by_self() {
        let t1 = test_txn(1);

        let version = VersionMetadata {
            xmin: t1,
            xmax: Some(t1),
        };
        let snapshot = TransactionSnapshot::new(t1, test_lsn(100), std::collections::HashSet::from([]));
        let mut committed = HashSet::new();
        committed.insert(t1); // created by self, deleted by self

        assert!(!is_visible(&version, &snapshot, &committed));
    }
}
