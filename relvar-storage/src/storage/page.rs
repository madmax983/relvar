//! Page-based I/O for fixed-size disk blocks.
//!
//! This module provides the lowest-level storage abstraction: fixed-size pages
//! that serve as the unit of I/O between disk and memory. All higher-level
//! storage structures (heap files, indexes) are built on top of pages.
//!
//! The [`Page`] type and its exact on-disk framing now live in
//! [`relvar_storage_core::page`] (the `no_std` storage core); this module
//! re-exports them so `storage::Page` and friends keep resolving. File-backed
//! page I/O moved to the host HAL shim
//! [`crate::device::FileBlockDevice`].
//!
//! # Page Layout
//!
//! Each page on disk has the following format:
//!
//! ```text
//! ┌────────────────────────────────────────────────────┐
//! │ data_length (8 bytes, little-endian u64)           │
//! ├────────────────────────────────────────────────────┤
//! │ actual_data (variable, up to PAGE_SIZE - 8 bytes)  │
//! ├────────────────────────────────────────────────────┤
//! │ padding (zeros to fill PAGE_SIZE)                  │
//! └────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```
//! use relvar_storage::storage::{Page, PAGE_SIZE};
//!
//! // Create an empty page
//! let mut page = Page::new(0);
//! assert!(page.is_empty());
//! assert_eq!(page.available_space(), PAGE_SIZE);
//!
//! // Create a page with data
//! let page = Page::from_data(1, vec![1, 2, 3, 4, 5]).unwrap();
//! assert_eq!(page.data().len(), 5);
//! ```

// Re-exported from the no_std storage core: the on-disk page format must be
// identical on every target, so the type, framing, and error type live there.
// (The core `PageError` has no I/O variant — I/O failures belong to the
// `BlockDevice` implementation on the host side.)
pub use relvar_storage_core::page::{PAGE_SIZE, Page, PageError, PageId};
