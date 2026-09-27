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
//! - [`TxnPool`]: Bounded, caller-owned pool of transaction records. All live
//!   slots and finished-transaction history are pre-allocated at construction,
//!   so begin/commit/abort and snapshot liveness checks never allocate.
//! - [`VersionPool`]: Bounded, caller-owned pool of uncommitted-version
//!   records plus reusable page working buffers. Every versioned heap write
//!   claims a record before touching a page; exhaustion is the typed
//!   [`MvccError::VersionPoolExhausted`]. Page work checks out a
//!   [`PageWorkingSet`] instead of allocating, so snapshot reads iterate
//!   pools without allocating per version.
//! - [`MvccError`]: Typed pool errors (`PoolExhausted`, `HistoryExhausted`);
//!   capacity exhaustion is recoverable, never a panic.
//! - [`TransactionSnapshot`]: A `Copy`, allocation-free visibility boundary
//!   (its ID, snapshot LSN, and horizon). Liveness is derived from the pool.
//! - [`VersionMetadata`]: The header stored with every tuple on disk (in `HeapFile`).
//! - **Garbage Collection (GC)**: Old versions that are no longer visible to any active
//!   transaction are cleaned up by background processes or during checkpoints.
//!
//! # Transaction Lifecycle
//!
//! 1. **Begin**: Transaction `T` starts. The `TxnPool` claims a free slot for `T`
//!    and returns a `TransactionSnapshot` (no active-set copy).
//! 2. **Read**: `T` scans the `HeapFile`. For each tuple,
//!    `is_visible(version, snapshot, pool, committed)` determines which version
//!    (if any) to return.
//! 3. **Write (Insert)**: `T` creates a new tuple with `xmin = T`, `xmax = None`.
//! 4. **Write (Update)**: `T` marks the old version's `xmax = T` and inserts a new version
//!    with `xmin = T`.
//! 5. **Write (Delete)**: `T` marks the current version's `xmax = T`.
//! 6. **Commit**: `T`'s slot drains into the pool's history ring with the commit
//!    LSN; its changes become visible to *new* transactions starting after
//!    this point.
//!
//! # TTM Compliance
//!
//! This module is entirely internal to the storage layer (Physical Data Independence).
//! The logical layer (`relvar-core`) interacts with `Database` and `Relation` abstractions,
//! unaware of versions, snapshots, or transaction IDs.

#[cfg(test)]
mod allocation_tests;
pub(crate) mod pool;
pub(crate) mod snapshot;
pub(crate) mod version_pool;
pub(crate) mod visibility;

pub use pool::{DEFAULT_TXN_POOL_CAPACITY, MvccError, TxnPool};
pub use snapshot::TransactionSnapshot;
pub use version_pool::{
    DEFAULT_VERSION_BUFFER_CAPACITY, DEFAULT_VERSION_POOL_CAPACITY, PageWorkingSet, VersionHandle,
    VersionPool,
};
pub use visibility::VersionMetadata;

impl From<MvccError> for relvar_core::storage_engine::StorageError {
    /// Lifts pool exhaustion to the public engine boundary without losing
    /// its type: callers can match on
    /// [`StorageError::TransactionPoolExhausted`](relvar_core::storage_engine::StorageError::TransactionPoolExhausted),
    /// [`StorageError::VersionPoolExhausted`](relvar_core::storage_engine::StorageError::VersionPoolExhausted),
    /// [`StorageError::VersionClaimantsExhausted`](relvar_core::storage_engine::StorageError::VersionClaimantsExhausted),
    /// and the other exhaustion variants instead of parsing a string.
    fn from(error: MvccError) -> Self {
        use relvar_core::storage_engine::StorageError;
        match error {
            MvccError::PoolExhausted { active, capacity } => {
                StorageError::TransactionPoolExhausted { active, capacity }
            }
            MvccError::HistoryExhausted { len, capacity } => {
                StorageError::TransactionHistoryExhausted { len, capacity }
            }
            MvccError::VersionPoolExhausted { used, capacity } => {
                StorageError::VersionPoolExhausted { used, capacity }
            }
            MvccError::VersionClaimantsExhausted { active, capacity } => {
                StorageError::VersionClaimantsExhausted { active, capacity }
            }
            MvccError::VersionBuffersExhausted { used, capacity } => {
                StorageError::VersionBufferExhausted { used, capacity }
            }
        }
    }
}
