//! WAL recovery using Analysis/Redo/Undo passes.
//!
//! This module implements crash recovery using a simplified ARIES-style
//! algorithm with three passes:
//!
//! 1. **Analysis Pass**: Scan WAL to identify committed/aborted transactions
//! 2. **Redo Pass**: Replay all operations to restore state (idempotent)
//! 3. **Undo Pass**: Roll back uncommitted transactions
//!
//! # Recovery Algorithm
//!
//! ```text
//! Analysis: Scan WAL → Build committed/aborted sets
//! Redo: Replay all operations → Restore database state
//! Undo: Roll back uncommitted → Remove partial work
//! ```

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
pub fn recover(wal: &mut WalManager) -> Result<Vec<UncommittedInsert>, WalError> {
    // Analysis pass
    let analysis = analyze(wal)?;

    // Find uncommitted transactions (those with BEGIN but no COMMIT/ABORT)
    let mut active_txns = HashSet::new();
    for (_, record) in &analysis.records {
        if let WalRecord::Begin { txn_id } = record {
            active_txns.insert(*txn_id);
        }
    }

    // Remove committed and aborted transactions from active set
    for txn_id in &analysis.committed {
        active_txns.remove(txn_id);
    }
    for txn_id in &analysis.aborted {
        active_txns.remove(txn_id);
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

    Ok(uncommitted_inserts)
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
}
