//! Experimental features that may be unstable or subject to change.
//!
//! This module contains features that are still in development or being tested.
//! They are available for use but may change significantly in future versions.
//!
//! # Included Features
//!
//! - **Exporter**: Tools for exporting relations to CSV, JSON, and ASCII tables.
//! - **Differ**: Tools for calculating semantic differences between relations.
//!
//! # Example: Using the Exporter
//!
//! ```
//! use relvar::{Database, InMemoryEngine, tuple};
//! use relvar::types::{TupleType, RelationType, ScalarType};
//! use relvar::experimental::exporter::Exporter;
//!
//! let mut db = Database::new(InMemoryEngine::new());
//! let rel_type = RelationType::new(
//!     TupleType::new().with_attribute("name", ScalarType::String)
//! );
//! db.create_relvar("TEST", rel_type).unwrap();
//! db.insert("TEST", tuple! { name: "Alice" }).unwrap();
//!
//! let relation = db.query("TEST").unwrap();
//! let exporter = Exporter::new(&relation);
//!
//! println!("CSV:\n{}", exporter.to_csv(',').unwrap());
//! println!("JSON:\n{}", exporter.to_json().unwrap());
//! ```

pub mod differ;
pub mod exporter;
