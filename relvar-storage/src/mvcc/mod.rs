// MVCC (Multi-Version Concurrency Control) implementation
//
// This module provides snapshot isolation for concurrent transactions.
// All MVCC logic is internal to the storage layer (TTM compliant - physical independence).

pub mod active_txn_table;
pub mod gc;
pub mod snapshot;
pub mod visibility;

pub use active_txn_table::ActiveTransactionTable;
pub use snapshot::TransactionSnapshot;
pub use visibility::VersionMetadata;
