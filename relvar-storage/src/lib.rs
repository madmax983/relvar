//! # Relvar Storage - Persistent Storage Layer
//!
//! This crate provides the persistent storage implementation for Relvar:
//!
//! - HeapFile - Unordered tuple storage
//! - Page - Fixed-size disk blocks
//! - Catalog - Relation metadata storage
//! - PersistentEngine - StorageEngine implementation with persistence
//!
//! This crate implements the physical storage layer and is feature-flagged
//! as optional in the main relvar crate.

#![warn(missing_docs)]

pub mod persistent_engine;
pub mod storage;

// WAL module is pub(crate) - not exposed to logical layer (TTM compliance)
pub(crate) mod wal;

// MVCC module is pub(crate) - not exposed to logical layer (TTM compliance)
pub(crate) mod mvcc;

pub use persistent_engine::PersistentEngine;

// Re-export key storage types
pub use storage::{Catalog, CatalogError, HeapError, HeapFile, Page, PageError, PageFile};
