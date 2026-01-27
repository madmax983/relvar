//! Physical storage layer for persistent data management.
//!
//! This module implements the storage subsystem that persists relations to disk.
//! It provides page-based I/O, heap file storage, B-tree indexing, and a
//! system catalog for metadata management.
//!
//! # Architecture
//!
//! The storage layer is organized in three tiers:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │                    Catalog                          │
//! │         (relation metadata & schema)                │
//! ├─────────────────────┬───────────────────────────────┤
//! │     HeapFile        │        BTreeIndex             │
//! │  (tuple storage)    │     (key-value lookup)        │
//! ├─────────────────────┴───────────────────────────────┤
//! │                    PageFile                         │
//! │              (fixed-size page I/O)                  │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! # Components
//!
//! - `Page` / `PageFile` - Fixed-size page abstraction for disk I/O
//! - `HeapFile` - Unordered tuple storage with slotted pages
//! - `BTreeIndex` - Ordered index for efficient key lookups
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
//! use relvar::storage::{HeapFile, Catalog};
//! use relvar::types::{RelationType, TupleType, ScalarType};
//! use relvar::tuple;
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

pub mod btree;
pub mod catalog;
pub mod heap;
pub mod page;
pub mod system_relvars;
pub mod type_serializer;

pub use btree::{BTreeIndex, BTreeIndexError};
pub use catalog::{Catalog, CatalogError};
pub use heap::{HeapError, HeapFile};
// TupleId is now pub(crate) in heap.rs, not exported
pub use page::{PAGE_SIZE, Page, PageError, PageFile, PageId};
pub use system_relvars::{
    SYS_ATTRIBUTES, SYS_CONSTRAINTS, SYS_RELVARS, is_system_relvar, sys_attributes_type,
    sys_constraints_type, sys_relvars_type,
};
pub use type_serializer::{TypeSerializerError, deserialize_scalar_type, serialize_scalar_type};
