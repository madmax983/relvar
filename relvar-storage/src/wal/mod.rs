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
//! │  - PageFile, HeapFile, BTree                            │
//! │  - WAL, LSN, TransactionId  ← THIS MODULE               │
//! │  - Checkpoint, Recovery                                 │
//! └─────────────────────────────────────────────────────────┘
//! ```
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

pub mod error;
pub mod lsn;
pub mod manager;
pub mod record;

pub(crate) use error::WalError;
pub(crate) use lsn::{Lsn, TransactionId, TransactionIdGenerator};
pub(crate) use manager::{DEFAULT_BUFFER_SIZE, WalManager};
pub(crate) use record::{MAX_RECORD_SIZE, WalRecord, WalRecordError};
