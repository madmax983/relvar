//! Developer tools and utilities.
//!
//! This module provides essential utilities for working with Relvar data outside
//! of the core query engine. It includes tools for data migration (import/export)
//! and schema visualization.
//!
//! # Included Tools
//!
//! - **`exporter`** (requires `std`): Export relations to standard formats like CSV, JSON, and ASCII tables.
//!   Useful for reporting, backups, or integrating with other systems.
//! - **`importer`** (requires `std`): Import data from JSON and CSV. Features strict type checking
//!   and limit enforcement (DoS protection).
//!
//! - **[`visualizer`](crate::tools::visualizer):** Generate Graphviz DOT diagrams of your database schema,
//!   showing tables, columns, keys, and relationships.
//!
//! The importer and exporter are built on `std::io` and `serde_json`, so they
//! require the `std` feature. The visualizer is `no_std` + `alloc`.
//!
//! # Example: Import, Visualize, and Export
//!
//! ```
//! # #[cfg(feature = "std")]
//! # {
//! use relvar::tools::{importer, exporter};
//! use relvar_core::types::{RelationType, TupleType, ScalarType};
//!
//! // 1. Define schema
//! let heading = TupleType::new()
//!     .with_attribute("id", ScalarType::Int)
//!     .with_attribute("name", ScalarType::String);
//! let rel_type = RelationType::new(heading);
//!
//! // 2. Import from JSON
//! let json_data = r#"[{"id": 1, "name": "Alice"}, {"id": 2, "name": "Bob"}]"#;
//! let relation = importer::from_json(json_data.as_bytes(), rel_type).unwrap();
//!
//! // 3. Export to CSV
//! let csv = exporter::to_csv(&relation, ',').unwrap();
//! assert!(csv.contains("id,name"));
//! assert!(csv.contains("1,\"Alice\""));
//! # }
//! ```

#[cfg(feature = "std")]
pub mod exporter;
#[cfg(feature = "std")]
pub mod importer;
pub mod visualizer;
