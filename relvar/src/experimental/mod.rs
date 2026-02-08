//! Experimental features that may be unstable or subject to change.
//!
//! This module contains features that are still in development or being tested.
//! They are available for use but may change significantly in future versions.
//!
//! # Included Features
//!
//! - **Exporter**: Tools for exporting relations to CSV, JSON, and ASCII tables.
//!
//! # Example: Using the Exporter
//!
//! ```
//! use relvar::{Database, InMemoryEngine, tuple};
//! use relvar::types::{TupleType, RelationType, ScalarType};
//! use relvar::experimental::exporter;
//!
//! let mut db = Database::new(InMemoryEngine::new());
//! let rel_type = RelationType::new(
//!     TupleType::new().with_attribute("name", ScalarType::String)
//! );
//! db.create_relvar("TEST", rel_type).unwrap();
//! db.insert("TEST", tuple! { name: "Alice" }).unwrap();
//!
//! let relation = db.query("TEST").unwrap();
//!
//! println!("CSV:\n{}", exporter::to_csv(&relation, ',').unwrap());
//! println!("JSON:\n{}", exporter::to_json(&relation).unwrap());
//! ```

pub mod exporter;
pub mod mock;
/// System catalog views (`_RELVARS`, `_ATTRIBUTES`, etc.) for database reflection.
pub mod system_views;
