//! WAL analysis and recovery helpers.
//!
//! This module provides a simplified ARIES-style **analysis pass** over the
//! write-ahead log (WAL). It scans the WAL to determine which transactions
//! have committed or aborted and exposes information about uncommitted work
//! so that higher layers can perform any necessary redo/undo logic.
//!
//! # Current Implementation
//!
//! The module performs the **Analysis Pass** only:
//! - Scans the WAL from beginning to end
//! - Identifies committed and aborted transactions
//! - Tracks the maximum transaction ID seen
//! - Returns all log records in order
//! - Tracks the last checkpoint LSN (if any)
//! - Exposes information about uncommitted inserts for caller-side undo
//!
//! # Redo and Undo
//!
//! **Redo:** Currently handled implicitly - all data modifications are already
//! on disk because we sync after each transaction commit. Future versions may
//! implement explicit redo for better performance.
//!
//! **Undo:** Performed by the caller (PersistentEngine) which receives the list
//! of uncommitted inserts and removes them from relations. This design allows
//! undo to access the catalog and relation types.

use super::error::WalError;
use super::lsn::{Lsn, TransactionId};
use super::manager::WalManager;
use super::record::WalRecord;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// Result of the analysis pass.
#[derive(Debug)]
pub struct AnalysisResult {
    /// Transactions that committed.
    pub committed: HashSet<TransactionId>,
    /// Transactions that aborted.
    pub aborted: HashSet<TransactionId>,
    /// All log records in order.
    pub records: Vec<(Lsn, WalRecord)>,
    /// Last checkpoint LSN (if any).
    pub last_checkpoint_lsn: Option<Lsn>,
}

/// Performs the analysis pass on the WAL.
///
/// Scans the WAL from beginning to end and identifies which transactions
/// committed or aborted.
///
/// # Errors
///
/// Returns `WalError` if WAL cannot be read or is corrupted.
/// # Examples
///
/// ```ignore
/// use relvar_storage::wal::{WalManager, analyze};
/// use tempfile::NamedTempFile;
/// let file = NamedTempFile::new().unwrap();
/// let mut wal = WalManager::create(file.path()).unwrap();
/// let analysis = analyze(&mut wal).unwrap();
/// ```
pub fn analyze(wal: &mut WalManager) -> Result<AnalysisResult, WalError> {
    // Scan all records from WAL
    let records = wal.scan()?;

    let mut committed = HashSet::new();
    let mut aborted = HashSet::new();
    let mut last_checkpoint_lsn = None;

    // Analyze each record to determine transaction states
    for (lsn, record) in &records {
        match record {
            WalRecord::Commit { txn_id } => {
                committed.insert(*txn_id);
            }
            WalRecord::Abort { txn_id } => {
                aborted.insert(*txn_id);
            }
            WalRecord::Checkpoint { .. } => {
                last_checkpoint_lsn = Some(*lsn);
            }
            _ => {
                // Begin, Insert, Delete, PageWrite - just record them
            }
        }
    }

    Ok(AnalysisResult {
        committed,
        aborted,
        records,
        last_checkpoint_lsn,
    })
}

/// Uncommitted insert information.
#[derive(Debug)]
pub struct UncommittedInsert {
    /// Relation name.
    pub relation_name: String,
    /// Serialized tuple data.
    pub tuple_data: Vec<u8>,
}

/// Result of recovery process.
#[derive(Debug)]
pub struct RecoveryResult {
    /// Set of committed transaction IDs (for MVCC visibility).
    pub committed_txns: HashSet<TransactionId>,
    /// List of uncommitted inserts that need to be undone.
    pub uncommitted_inserts: Vec<UncommittedInsert>,
    /// Maximum transaction ID seen in the WAL.
    /// Used to seed the transaction ID generator to avoid reusing IDs.
    pub max_txn_id: TransactionId,
}

/// Performs recovery analysis and returns uncommitted operations.
///
/// This performs the analysis pass and identifies which operations
/// need to be undone. The actual undo is performed by the caller
/// (PersistentEngine) which has access to the catalog.
///
/// # Arguments
///
/// * `wal` - The WAL manager
///
/// # Errors
///
/// Returns `WalError` if analysis fails.
/// # Examples
///
/// ```ignore
/// use relvar_storage::wal::{WalManager, recover};
/// use tempfile::NamedTempFile;
/// let file = NamedTempFile::new().unwrap();
/// let mut wal = WalManager::create(file.path()).unwrap();
/// let result = recover(&mut wal).unwrap();
/// ```
pub fn recover(wal: &mut WalManager) -> Result<RecoveryResult, WalError> {
    // Analysis pass
    let analysis = analyze(wal)?;

    // Find uncommitted transactions (those with BEGIN but no COMMIT/ABORT)
    // Also track maximum transaction ID seen
    let mut active_txns = HashSet::new();
    let mut max_txn_id = TransactionId::new(0);

    for (_, record) in &analysis.records {
        // Extract transaction ID from record
        let txn_id = record.txn_id();
        if let Some(tid) = txn_id
            && tid.value() > max_txn_id.value()
        {
            max_txn_id = tid;
        }

        if let WalRecord::Begin { txn_id } = record {
            active_txns.insert(*txn_id);
        }
    }

    // Remove committed and aborted transactions from active set
    for txn_id in &analysis.committed {
        active_txns.remove(txn_id);
        if txn_id.value() > max_txn_id.value() {
            max_txn_id = *txn_id;
        }
    }
    for txn_id in &analysis.aborted {
        active_txns.remove(txn_id);
        if txn_id.value() > max_txn_id.value() {
            max_txn_id = *txn_id;
        }
    }

    // Collect uncommitted inserts
    let mut uncommitted_inserts = Vec::new();

    for (_, record) in &analysis.records {
        if let WalRecord::Insert {
            txn_id,
            relation_name,
            tuple_data,
        } = record
            && active_txns.contains(txn_id)
        {
            uncommitted_inserts.push(UncommittedInsert {
                relation_name: relation_name.clone(),
                tuple_data: tuple_data.clone(),
            });
        }
    }

    Ok(RecoveryResult {
        committed_txns: analysis.committed,
        uncommitted_inserts,
        max_txn_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_analyze_empty_wal() {
        let temp = NamedTempFile::new().unwrap();
        let mut wal = WalManager::create(temp.path()).unwrap();

        let result = analyze(&mut wal).unwrap();

        assert!(result.committed.is_empty());
        assert!(result.aborted.is_empty());
        assert!(result.records.is_empty());
    }

    // Phase 4.2: Recovery with Versions tests

    #[test]
    fn test_recovery_populates_committed_set() {
        let temp = NamedTempFile::new().unwrap();
        let mut wal = WalManager::create(temp.path()).unwrap();

        let txn1 = TransactionId::new(1);
        let txn2 = TransactionId::new(2);

        // T1 commits
        wal.log(WalRecord::Begin { txn_id: txn1 }).unwrap();
        wal.log(WalRecord::Insert {
            txn_id: txn1,
            relation_name: "test".to_string(),
            tuple_data: vec![1, 2, 3],
        })
        .unwrap();
        wal.log(WalRecord::Commit { txn_id: txn1 }).unwrap();

        // T2 aborts
        wal.log(WalRecord::Begin { txn_id: txn2 }).unwrap();
        wal.log(WalRecord::Insert {
            txn_id: txn2,
            relation_name: "test".to_string(),
            tuple_data: vec![4, 5, 6],
        })
        .unwrap();
        wal.log(WalRecord::Abort { txn_id: txn2 }).unwrap();

        let result = recover(&mut wal).unwrap();

        // T1 should be in committed set
        assert!(result.committed_txns.contains(&txn1));
        // T2 should NOT be in committed set (aborted)
        assert!(!result.committed_txns.contains(&txn2));
    }

    #[test]
    fn test_recovery_removes_uncommitted_versions() {
        let temp = NamedTempFile::new().unwrap();
        let mut wal = WalManager::create(temp.path()).unwrap();

        let txn1 = TransactionId::new(1);
        let txn2 = TransactionId::new(2);

        // T1 commits
        wal.log(WalRecord::Begin { txn_id: txn1 }).unwrap();
        wal.log(WalRecord::Insert {
            txn_id: txn1,
            relation_name: "test".to_string(),
            tuple_data: vec![1, 2, 3],
        })
        .unwrap();
        wal.log(WalRecord::Commit { txn_id: txn1 }).unwrap();

        // T2 doesn't commit (crash)
        wal.log(WalRecord::Begin { txn_id: txn2 }).unwrap();
        wal.log(WalRecord::Insert {
            txn_id: txn2,
            relation_name: "test".to_string(),
            tuple_data: vec![4, 5, 6],
        })
        .unwrap();

        let result = recover(&mut wal).unwrap();

        // T1's insert should NOT be in uncommitted list
        assert!(
            result
                .uncommitted_inserts
                .iter()
                .all(|ui| ui.tuple_data != vec![1, 2, 3])
        );

        // T2's insert should be in uncommitted list
        assert!(
            result
                .uncommitted_inserts
                .iter()
                .any(|ui| ui.tuple_data == vec![4, 5, 6])
        );
    }

    #[test]
    fn test_recovery_committed_versions_survive() {
        let temp = NamedTempFile::new().unwrap();
        let mut wal = WalManager::create(temp.path()).unwrap();

        let txn1 = TransactionId::new(10);
        let txn2 = TransactionId::new(20);
        let txn3 = TransactionId::new(30);

        // Multiple committed transactions
        for txn_id in [txn1, txn2, txn3] {
            wal.log(WalRecord::Begin { txn_id }).unwrap();
            wal.log(WalRecord::Insert {
                txn_id,
                relation_name: "test".to_string(),
                tuple_data: vec![txn_id.value() as u8],
            })
            .unwrap();
            wal.log(WalRecord::Commit { txn_id }).unwrap();
        }

        let result = recover(&mut wal).unwrap();

        // All should be in committed set
        assert_eq!(result.committed_txns.len(), 3);
        assert!(result.committed_txns.contains(&txn1));
        assert!(result.committed_txns.contains(&txn2));
        assert!(result.committed_txns.contains(&txn3));

        // None should be in uncommitted
        assert!(result.uncommitted_inserts.is_empty());
    }

    #[test]
    fn test_recovery_mixed_committed_uncommitted() {
        let temp = NamedTempFile::new().unwrap();
        let mut wal = WalManager::create(temp.path()).unwrap();

        let t1 = TransactionId::new(1);
        let t2 = TransactionId::new(2);
        let t3 = TransactionId::new(3);

        // T1 commits
        wal.log(WalRecord::Begin { txn_id: t1 }).unwrap();
        wal.log(WalRecord::Commit { txn_id: t1 }).unwrap();

        // T2 uncommitted
        wal.log(WalRecord::Begin { txn_id: t2 }).unwrap();
        wal.log(WalRecord::Insert {
            txn_id: t2,
            relation_name: "test".to_string(),
            tuple_data: vec![2],
        })
        .unwrap();

        // T3 commits
        wal.log(WalRecord::Begin { txn_id: t3 }).unwrap();
        wal.log(WalRecord::Commit { txn_id: t3 }).unwrap();

        let result = recover(&mut wal).unwrap();

        assert!(result.committed_txns.contains(&t1));
        assert!(!result.committed_txns.contains(&t2));
        assert!(result.committed_txns.contains(&t3));

        assert_eq!(result.uncommitted_inserts.len(), 1);
    }

    #[test]
    fn test_recovery_result_structure() {
        let temp = NamedTempFile::new().unwrap();
        let mut wal = WalManager::create(temp.path()).unwrap();

        let txn1 = TransactionId::new(1);

        wal.log(WalRecord::Begin { txn_id: txn1 }).unwrap();
        wal.log(WalRecord::Insert {
            txn_id: txn1,
            relation_name: "employees".to_string(),
            tuple_data: vec![1, 2, 3],
        })
        .unwrap();
        wal.log(WalRecord::Commit { txn_id: txn1 }).unwrap();

        let result = recover(&mut wal).unwrap();

        // Verify RecoveryResult has both fields
        assert!(!result.committed_txns.is_empty());
        assert!(result.uncommitted_inserts.is_empty());
    }

    #[test]
    fn test_recovery_empty_wal_returns_empty_committed() {
        let temp = NamedTempFile::new().unwrap();
        let mut wal = WalManager::create(temp.path()).unwrap();

        let result = recover(&mut wal).unwrap();

        assert!(result.committed_txns.is_empty());
        assert!(result.uncommitted_inserts.is_empty());
    }

    #[test]
    fn test_recovery_aborted_not_in_committed() {
        let temp = NamedTempFile::new().unwrap();
        let mut wal = WalManager::create(temp.path()).unwrap();

        let txn1 = TransactionId::new(1);

        wal.log(WalRecord::Begin { txn_id: txn1 }).unwrap();
        wal.log(WalRecord::Insert {
            txn_id: txn1,
            relation_name: "test".to_string(),
            tuple_data: vec![1],
        })
        .unwrap();
        wal.log(WalRecord::Abort { txn_id: txn1 }).unwrap();

        let result = recover(&mut wal).unwrap();

        // Aborted transaction should NOT be in committed set
        assert!(!result.committed_txns.contains(&txn1));
    }
}
