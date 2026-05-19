//! Physical storage layer for persistent data management.
//!
//! This module implements the storage subsystem that persists relations to disk.
//! It provides page-based I/O, heap file storage, and a system catalog for
//! metadata management.
//!
//! # Architecture
//!
//! The storage layer is organized in three tiers:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │                    Catalog                          │
//! │         (relation metadata & schema)                │
//! ├─────────────────────────────────────────────────────┤
//! │                    HeapFile                         │
//! │                 (tuple storage)                     │
//! ├─────────────────────────────────────────────────────┤
//! │                    PageFile                         │
//! │              (fixed-size page I/O)                  │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! # Components
//!
//! - `Page` / `PageFile` - Fixed-size page abstraction for disk I/O
//! - `HeapFile` - Unordered tuple storage with slotted pages
//! - `Catalog` - System catalog storing relation metadata
//!
//! # TTM Compliance
//!
//! This module carefully maintains TTM compliance, particularly Proscription 6:
//! "The database must not generate or expose tuple-level identifiers."
//!
//! - `TupleId` is `pub(crate)` - only visible within the storage layer
//! - Public APIs return `Tuple` values, not physical addresses
//! - Callers work with tuples as values, never as storage references
//!
//! # Example
//!
//! ```no_run
//! use relvar_storage::storage::{HeapFile, Catalog};
//! use relvar_core::types::{RelationType, TupleType, ScalarType};
//! use relvar_core::tuple;
//! use std::path::PathBuf;
//!
//! // Define relation type
//! let heading = TupleType::new()
//!     .with_attribute("id".to_string(), ScalarType::Int)
//!     .with_attribute("name".to_string(), ScalarType::String);
//! let rel_type = RelationType::new(heading);
//!
//! // Create heap file for storage
//! let mut heap = HeapFile::create("employees.heap", rel_type.clone()).unwrap();
//!
//! // Insert tuples (no TupleId returned - TTM compliant)
//! heap.insert_tuple(&tuple! { id: 1i64, name: "Alice" }).unwrap();
//! heap.insert_tuple(&tuple! { id: 2i64, name: "Bob" }).unwrap();
//!
//! // Scan returns tuples without physical identifiers
//! let tuples = heap.scan().unwrap();
//! ```

pub(crate) mod catalog;
pub(crate) mod heap;
pub(crate) mod manager;
pub(crate) mod page;

pub use catalog::{Catalog, CatalogEntry, CatalogError};

pub use heap::{HeapError, HeapFile};
pub use manager::StorageManager;
// TupleId is now pub(crate) in heap.rs, not exported
pub use page::{PAGE_SIZE, Page, PageError, PageFile, PageId};
