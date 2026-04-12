//! # Multi-Version Concurrency Control (MVCC)
//!
//! This module implements MVCC to provide **Snapshot Isolation** for concurrent transactions.
//! MVCC allows multiple transactions to read and write the database simultaneously without
//! blocking each other, by maintaining multiple versions of data.
//!
//! # Core Concepts
//!
//! ## 1. Snapshot Isolation
//!
//! Each transaction operates on a consistent snapshot of the database taken at the start
//! of the transaction. The snapshot guarantees that:
//!
//! - The transaction sees all changes committed by transactions that completed *before* it started.
//! - The transaction sees its own uncommitted changes.
//! - The transaction does *not* see changes from concurrent transactions (even if they commit during its execution).
//!
//! ## 2. Tuple Versioning
//!
//! Instead of overwriting tuples in place, updates create a new version of the tuple.
//! Each version is tagged with metadata determining its visibility:
//!
//! - **xmin**: The transaction ID that created (inserted/updated) this version.
//! - **xmax**: The transaction ID that deleted (or updated) this version. `None` if the version is still current.
//!
//! ```text
//! ┌──────────────┐       ┌──────────────┐       ┌──────────────┐
//! │ Version 1    │       │ Version 2    │       │ Version 3    │
//! │ xmin: 100    │  ──►  │ xmin: 105    │  ──►  │ xmin: 110    │
//! │ xmax: 105    │       │ xmax: 110    │       │ xmax: None   │
//! │ Value: "A"   │       │ Value: "B"   │       │ Value: "C"   │
//! └──────────────┘       └──────────────┘       └──────────────┘
//! ```ignore
//!
//! ## 3. Visibility Rules
//!
//! When a transaction `T` reads a tuple, it traverses the version chain to find the
//! version visible to its snapshot. A version is visible if:
//!
//! 1. `xmin` is committed and was *not* active when `T` started (or `xmin` is `T` itself).
//! 2. `xmax` is either `None` (not deleted), aborted, or was active when `T` started (deletion not yet visible).
//!
//! See [`visibility::is_visible`] for the exact logic.
//!
//! # Architecture
//!
//! - [`ActiveTransactionTable`]: Tracks all currently running transactions. Used to generate
//!   snapshots by capturing the set of active transaction IDs.
//! - [`TransactionSnapshot`]: A lightweight object carried by a transaction containing its
//!   visibility boundaries (its ID and the list of concurrent transactions to ignore).
//! - [`VersionMetadata`]: The header stored with every tuple on disk (in `HeapFile`).
//! - **Garbage Collection (GC)**: Old versions that are no longer visible to any active
//!   transaction are cleaned up by background processes or during checkpoints.
//!
//! # Transaction Lifecycle
//!
//! 1. **Begin**: Transaction `T` starts. The `ActiveTransactionTable` records `T` and
//!    returns a `TransactionSnapshot` listing all other currently active transactions.
//! 2. **Read**: `T` scans the `HeapFile`. For each tuple, `is_visible(version, snapshot)`
//!    determines which version (if any) to return.
//! 3. **Write (Insert)**: `T` creates a new tuple with `xmin = T`, `xmax = None`.
//! 4. **Write (Update)**: `T` marks the old version's `xmax = T` and inserts a new version
//!    with `xmin = T`.
//! 5. **Write (Delete)**: `T` marks the current version's `xmax = T`.
//! 6. **Commit**: `T` is removed from the `ActiveTransactionTable`. Its changes become
//!    visible to *new* transactions starting after this point.
//!
//! # TTM Compliance
//!
//! This module is entirely internal to the storage layer (Physical Data Independence).
//! The logical layer (`relvar-core`) interacts with `Database` and `Relation` abstractions,
//! unaware of versions, snapshots, or transaction IDs.

pub(crate) mod active_txn_table;
pub(crate) mod gc;
pub(crate) mod snapshot;
pub(crate) mod visibility;

pub use active_txn_table::ActiveTransactionTable;
pub use snapshot::TransactionSnapshot;
pub use visibility::VersionMetadata;
