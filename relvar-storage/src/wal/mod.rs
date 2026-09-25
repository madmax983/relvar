//! Write-Ahead Logging (WAL) for ACID transactions.
//!
//! This module implements Write-Ahead Logging to provide durability and
//! crash recovery for the Relvar database, per TTM RM Prescription 11.
//!
//! # Architecture
//!
//! The WAL subsystem is purely physical infrastructure - it's invisible to
//! the relational/logical layer (Codd/Date/Darwen principle of physical
//! data independence).
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │  LOGICAL LAYER (relvar-core)                            │
//! │  - Relation, Tuple, RelationType                        │
//! │  - Relational algebra operators                         │
//! │  - Constraints (keys, foreign keys)                     │
//! │  - NO knowledge of pages, files, WAL, LSN               │
//! ├─────────────────────────────────────────────────────────┤
//! │  PHYSICAL LAYER (relvar-storage)                        │
//! │  - StorageEngine trait (abstraction boundary)           │
//! │  - FileBlockDevice, HeapFile, BTree                    │
//! │  - WAL, LSN, TransactionId  ← THIS MODULE               │
//! │  - Checkpoint, Recovery                                 │
//! └─────────────────────────────────────────────────────────┘
//! ```ignore
//!
//! # Components
//!
//! - `Lsn` - Log Sequence Number for ordering log records
//! - `TransactionId` - Unique identifier for transactions
//! - `WalRecord` - Enumeration of all log record types
//! - `WalManager` - Coordinates logging, flushing, and buffering
//! - `Recovery` - Crash recovery using Analysis/Redo/Undo passes
//!
//! # TTM Compliance
//!
//! - **Proscription 6:** Log records use relation names + serialized tuples,
//!   never TupleId
//! - **Physical independence:** WAL is an implementation detail, invisible
//!   to the relational API
//! - **No NULLs:** All log record fields are required values

// Allow unused code during development - WAL is not yet integrated
#![allow(dead_code)]
#![allow(unused_imports)]

pub(crate) mod error;
pub(crate) mod manager;
pub(crate) mod recovery;

pub(crate) use error::WalError;
pub(crate) use manager::{DEFAULT_BUFFER_SIZE, WalManager};
pub(crate) use recovery::{AnalysisResult, UncommittedInsert, recover};
// WAL primitives (LSN, transaction IDs, records, framing) moved to the
// no_std storage core; re-exported here so the storage layer keeps a single
// import path. The local `WalError` stays: it adds the host `Io` variant the
// core's error type deliberately omits.
pub(crate) use relvar_storage_core::wal::{
    Lsn, MAX_RECORD_SIZE, TransactionId, TransactionIdGenerator, WAL_MAGIC_V1, WAL_MAGIC_V2,
    WalRecord, WalRecordError, decode_frames, decode_legacy_frames, encode_frame,
};
