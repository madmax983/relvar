//! WAL record types and serialization.
//!
//! This module defines all log record types and their serialization format.
//! Records are serialized using bincode for efficient storage and recovery.

use super::lsn::{Lsn, TransactionId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

use crate::storage::PageId;

/// Errors that can occur during WAL record operations.
#[derive(Debug, Error)]
pub enum WalRecordError {
    /// Serialization error.
    #[error("Failed to serialize WAL record: {0}")]
    Serialization(#[from] bincode::Error),

    /// Record is too large to fit in buffer.
    #[error("WAL record too large: {0} bytes (max: {1})")]
    RecordTooLarge(usize, usize),
}

/// Maximum size of a single WAL record in bytes.
///
/// This is set to 1MB to allow for reasonable tuple sizes while preventing
/// unbounded memory usage.
pub const MAX_RECORD_SIZE: usize = 1024 * 1024; // 1MB

/// A log record in the Write-Ahead Log.
///
/// Each record represents a single logged operation. Records are serialized
/// to the WAL file in order of their LSN.
///
/// # TTM Compliance
///
/// Per Proscription 6, log records use relation names and serialized tuple
/// data, never physical TupleId values. This maintains physical data
/// independence - the logical layer has no knowledge of physical storage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WalRecord {
    /// Transaction begin marker.
    Begin {
        /// The transaction ID.
        txn_id: TransactionId,
    },

    /// Transaction commit marker.
    Commit {
        /// The transaction ID.
        txn_id: TransactionId,
    },

    /// Transaction abort marker.
    Abort {
        /// The transaction ID.
        txn_id: TransactionId,
    },

    /// A page write operation.
    ///
    /// Records the before-image of a page for undo recovery.
    PageWrite {
        /// The transaction ID.
        txn_id: TransactionId,
        /// The name of the relation being modified.
        relation_name: String,
        /// The page ID being written.
        page_id: PageId,
        /// The page data (before-image for undo).
        page_data: Vec<u8>,
    },

    /// Insert a tuple into a relation.
    ///
    /// Records the serialized tuple data for redo recovery.
    Insert {
        /// The transaction ID.
        txn_id: TransactionId,
        /// The name of the relation.
        relation_name: String,
        /// The serialized tuple data.
        tuple_data: Vec<u8>,
    },

    /// Delete a tuple from a relation.
    ///
    /// Records the key values needed to identify the tuple during recovery.
    Delete {
        /// The transaction ID.
        txn_id: TransactionId,
        /// The name of the relation.
        relation_name: String,
        /// The serialized key values identifying the tuple.
        key_values: Vec<u8>,
    },

    /// Checkpoint marker.
    ///
    /// Records the current state of dirty pages and active transactions.
    Checkpoint {
        /// The minimum LSN of any active transaction.
        min_active_lsn: Lsn,
        /// Dirty pages per relation.
        dirty_pages: HashMap<String, Vec<PageId>>,
    },
}

impl WalRecord {
    /// Serializes this record to bytes using bincode.
    ///
    /// # Errors
    ///
    /// Returns `WalRecordError::Serialization` if serialization fails.
    /// Returns `WalRecordError::RecordTooLarge` if the record exceeds MAX_RECORD_SIZE.
    pub fn serialize(&self) -> Result<Vec<u8>, WalRecordError> {
        let bytes = bincode::serialize(self)?;

        if bytes.len() > MAX_RECORD_SIZE {
            return Err(WalRecordError::RecordTooLarge(bytes.len(), MAX_RECORD_SIZE));
        }

        Ok(bytes)
    }

    /// Deserializes a record from bytes using bincode.
    ///
    /// # Errors
    ///
    /// Returns `WalRecordError::Serialization` if deserialization fails.
    pub fn deserialize(bytes: &[u8]) -> Result<Self, WalRecordError> {
        let record = bincode::deserialize(bytes)?;
        Ok(record)
    }

    /// Returns the transaction ID associated with this record, if any.
    ///
    /// Checkpoint records don't have a transaction ID.
    pub fn txn_id(&self) -> Option<TransactionId> {
        match self {
            WalRecord::Begin { txn_id }
            | WalRecord::Commit { txn_id }
            | WalRecord::Abort { txn_id }
            | WalRecord::PageWrite { txn_id, .. }
            | WalRecord::Insert { txn_id, .. }
            | WalRecord::Delete { txn_id, .. } => Some(*txn_id),
            WalRecord::Checkpoint { .. } => None,
        }
    }

    /// Returns true if this is a transaction end marker (commit or abort).
    pub fn is_txn_end(&self) -> bool {
        matches!(self, WalRecord::Commit { .. } | WalRecord::Abort { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_roundtrip_begin() {
        let txn_id = TransactionId::new(1);
        let record = WalRecord::Begin { txn_id };

        let bytes = record.serialize().expect("serialization failed");
        let deserialized = WalRecord::deserialize(&bytes).expect("deserialization failed");

        assert_eq!(record, deserialized);
    }

    #[test]
    fn test_record_roundtrip_commit() {
        let txn_id = TransactionId::new(42);
        let record = WalRecord::Commit { txn_id };

        let bytes = record.serialize().expect("serialization failed");
        let deserialized = WalRecord::deserialize(&bytes).expect("deserialization failed");

        assert_eq!(record, deserialized);
    }

    #[test]
    fn test_record_roundtrip_abort() {
        let txn_id = TransactionId::new(99);
        let record = WalRecord::Abort { txn_id };

        let bytes = record.serialize().expect("serialization failed");
        let deserialized = WalRecord::deserialize(&bytes).expect("deserialization failed");

        assert_eq!(record, deserialized);
    }

    #[test]
    fn test_record_roundtrip_insert() {
        let txn_id = TransactionId::new(1);
        let tuple_data = vec![1, 2, 3, 4, 5];
        let record = WalRecord::Insert {
            txn_id,
            relation_name: "employees".to_string(),
            tuple_data: tuple_data.clone(),
        };

        let bytes = record.serialize().expect("serialization failed");
        let deserialized = WalRecord::deserialize(&bytes).expect("deserialization failed");

        assert_eq!(record, deserialized);

        // Verify tuple_data is preserved
        if let WalRecord::Insert {
            tuple_data: recovered_data,
            ..
        } = deserialized
        {
            assert_eq!(tuple_data, recovered_data);
        } else {
            panic!("Expected Insert record");
        }
    }

    #[test]
    fn test_record_roundtrip_delete() {
        let txn_id = TransactionId::new(1);
        let key_values = vec![42, 43, 44];
        let record = WalRecord::Delete {
            txn_id,
            relation_name: "employees".to_string(),
            key_values: key_values.clone(),
        };

        let bytes = record.serialize().expect("serialization failed");
        let deserialized = WalRecord::deserialize(&bytes).expect("deserialization failed");

        assert_eq!(record, deserialized);
    }

    #[test]
    fn test_record_roundtrip_page_write() {
        let txn_id = TransactionId::new(1);
        let page_data = vec![0u8; 4096]; // Simulated page
        let record = WalRecord::PageWrite {
            txn_id,
            relation_name: "employees".to_string(),
            page_id: 5,
            page_data: page_data.clone(),
        };

        let bytes = record.serialize().expect("serialization failed");
        let deserialized = WalRecord::deserialize(&bytes).expect("deserialization failed");

        assert_eq!(record, deserialized);
    }

    #[test]
    fn test_record_roundtrip_checkpoint() {
        let mut dirty_pages = HashMap::new();
        dirty_pages.insert("employees".to_string(), vec![1, 2]);
        dirty_pages.insert("departments".to_string(), vec![3]);

        let record = WalRecord::Checkpoint {
            min_active_lsn: Lsn::new(100),
            dirty_pages,
        };

        let bytes = record.serialize().expect("serialization failed");
        let deserialized = WalRecord::deserialize(&bytes).expect("deserialization failed");

        assert_eq!(record, deserialized);
    }

    #[test]
    fn test_record_size_bounded() {
        // Small record should succeed
        let small_record = WalRecord::Begin {
            txn_id: TransactionId::new(1),
        };
        assert!(small_record.serialize().is_ok());

        // Large record should fail
        let huge_data = vec![0u8; MAX_RECORD_SIZE + 1];
        let large_record = WalRecord::Insert {
            txn_id: TransactionId::new(1),
            relation_name: "test".to_string(),
            tuple_data: huge_data,
        };

        let result = large_record.serialize();
        assert!(result.is_err());
        assert!(matches!(result, Err(WalRecordError::RecordTooLarge(_, _))));
    }

    #[test]
    fn test_txn_id_extraction() {
        let txn_id = TransactionId::new(42);

        assert_eq!(WalRecord::Begin { txn_id }.txn_id(), Some(txn_id));
        assert_eq!(WalRecord::Commit { txn_id }.txn_id(), Some(txn_id));
        assert_eq!(WalRecord::Abort { txn_id }.txn_id(), Some(txn_id));
        assert_eq!(
            WalRecord::Insert {
                txn_id,
                relation_name: "test".to_string(),
                tuple_data: vec![],
            }
            .txn_id(),
            Some(txn_id)
        );

        // Checkpoint has no transaction ID
        assert_eq!(
            WalRecord::Checkpoint {
                min_active_lsn: Lsn::new(1),
                dirty_pages: HashMap::new(),
            }
            .txn_id(),
            None
        );
    }

    #[test]
    fn test_is_txn_end() {
        let txn_id = TransactionId::new(1);

        assert!(!WalRecord::Begin { txn_id }.is_txn_end());
        assert!(WalRecord::Commit { txn_id }.is_txn_end());
        assert!(WalRecord::Abort { txn_id }.is_txn_end());
        assert!(
            !WalRecord::Insert {
                txn_id,
                relation_name: "test".to_string(),
                tuple_data: vec![],
            }
            .is_txn_end()
        );
    }
}
