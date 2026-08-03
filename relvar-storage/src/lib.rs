//! # Relvar Storage - The Persistent Storage Layer
//!
//! `relvar-storage` provides the physical storage engine for the Relvar database system.
//! It implements a durable, page-based storage architecture that supports ACID transactions,
//! Multi-Version Concurrency Control (MVCC), and Write-Ahead Logging (WAL).
//!
//! While `relvar-core` defines the *logical* relational model (types, values, algebra),
//! `relvar-storage` handles the *physical* reality of bytes on disk.
//!
//! ## Architecture
//!
//! The storage engine is built in layers, from raw bytes up to relational storage:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      PersistentEngine                       │
//! │           (Orchestrates Transactions, WAL, MVCC)            │
//! ├──────────────────────────────────────┬──────────────────────┤
//! │             Catalog                  │       HeapFile       │
//! │   (Metadata: Schema, Keys)           │   (Tuple Storage)    │
//! ├──────────────────────────────────────┴──────────────────────┤
//! │                      StorageManager                         │
//! │             (Manages files and directory layout)            │
//! ├─────────────────────────────────────────────────────────────┤
//! │                        PageFile                             │
//! │              (Fixed-size Page I/O abstraction)              │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Core Components
//!
//! ### 1. Page ([`storage::Page`])
//! The fundamental unit of I/O. All data is stored in fixed-size (4KB) pages.
//! This aligns with operating system virtual memory pages and disk sectors for efficiency.
//!
//! ### 2. HeapFile ([`storage::HeapFile`])
//! Implements unordered tuple storage using a **Slotted Page** architecture.
//! - **Slotted Pages**: Allows variable-length tuples to be stored efficiently within fixed-size pages.
//! - **Tuple IDs**: Tuples are identified internally by `(PageID, SlotIndex)`, but this is never exposed to the logical layer.
//!
//! ### 3. Catalog ([`storage::Catalog`])
//! Stores metadata about relations (names, types, constraints).
//! Currently implemented as a JSON file for simplicity, but designed to be replaced with a system relation.
//!
//! ### 4. PersistentEngine ([`PersistentEngine`])
//! The high-level entry point that implements the `StorageEngine` trait from `relvar-core`.
//! It handles:
//! - **Transactions**: Begin, Commit, Rollback.
//! - **WAL**: Ensures durability by logging changes before applying them.
//! - **MVCC**: Provides Snapshot Isolation by maintaining multiple versions of tuples.
//!
//! ## Theory & Design
//!
//! ### TTM Compliance (The Third Manifesto)
//!
//! This storage layer is designed to support the pure relational model:
//!
//! - **Physical Independence**: The logical model (relations) knows nothing about pages or files.
//! - **No Tuple IDs**: Per *Proscription 6*, internal tuple addresses (Page/Slot) are never exposed to the user or the logical layer. Queries operate on values, not pointers.
//! - **Set Semantics**: While the physical storage (HeapFile) allows duplicates, the logical layer enforces set semantics (no duplicates) via unique constraints and rigorous algebra.
//!
//! ### Concurrency Control (MVCC)
//!
//! Relvar uses Multi-Version Concurrency Control to allow readers and writers to operate without blocking each other.
//!
//! - **Readers** see a consistent snapshot of the database as of the start of their transaction.
//! - **Writers** create new versions of tuples instead of overwriting data in place.
//! - **Garbage Collection** removes old tuple versions that are no longer visible to any active transaction.
//!
//! ## Usage
//!
//! ### Using `PersistentEngine` directly
//!
//! ```no_run
//! use relvar_storage::PersistentEngine;
//! use relvar_core::storage_engine::StorageEngine; // Trait
//! use relvar_core::types::{RelationType, TupleType, ScalarType};
//! use relvar_core::tuple;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Open the database directory
//!     let mut engine = PersistentEngine::open("my_db")?;
//!
//!     // Define schema
//!     let heading = TupleType::new()
//!         .with_attribute("id", ScalarType::Int)
//!         .with_attribute("name", ScalarType::String);
//!     let rel_type = RelationType::new(heading);
//!
//!     // Create relation
//!     engine.create_relation("USERS", rel_type)?;
//!
//!     // Insert data (auto-commit if not in transaction)
//!     engine.insert_tuple("USERS", tuple! { id: 1i64, name: "Alice" })?;
//!
//!     // Load relation
//!     let users = engine.load_relation("USERS")?;
//!     assert_eq!(users.cardinality(), 1);
//!
//!     Ok(())
//! }
//! ```
//!
//! ### Using `HeapFile` (Low-level)
//!
//! ```no_run
//! use relvar_storage::storage::HeapFile;
//! use relvar_core::types::{RelationType, TupleType, ScalarType};
//! use relvar_core::tuple;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let heading = TupleType::new().with_attribute("data", ScalarType::String);
//!     let rel_type = RelationType::new(heading);
//!
//!     // Create a heap file directly
//!     let mut heap = HeapFile::create("data.heap", rel_type)?;
//!
//!     // Write tuples
//!     heap.insert_tuple(&tuple! { data: "Hello" })?;
//!
//!     // Read tuples
//!     let tuples = heap.scan()?;
//!     assert_eq!(tuples.len(), 1);
//!
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]

#[doc(hidden)]
pub(crate) mod persistent_engine;
#[doc(hidden)]
pub(crate) mod storage;

// WAL module is pub(crate) - not exposed to logical layer (TTM compliance)
#[doc(hidden)]
pub mod wal;

// MVCC module is pub(crate) - not exposed to logical layer (TTM compliance)
#[doc(hidden)]
pub mod mvcc;

pub use persistent_engine::PersistentEngine;

// Re-export key storage types
pub use storage::{Catalog, CatalogError, HeapError, HeapFile, Page, PageError, PageFile};
