//! Developer tools and utilities.
//!
//! This module provides essential utilities for working with Relvar data outside
//! of the core query engine. It includes tools for data migration (import/export)
//! and schema visualization.
//!
//! # Included Tools
//!
//! - **[`exporter`](crate::tools):** Export relations to standard formats like CSV, JSON, and ASCII tables.
//!   Useful for reporting, backups, or integrating with other systems.
//! - **[`importer`](crate::tools):** Import data from JSON and CSV. Features strict type checking
//!   and limit enforcement (DoS protection).
//! - **[`visualizer`](crate::tools):** Generate Graphviz DOT diagrams of your database schema,
//!   showing tables, columns, keys, and relationships.
//!
//! # Example: Import, Visualize, and Export
//!
//! ```
//! use relvar::tools::{from_csv, from_json, to_csv, to_json, to_ascii_table};
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
//! let relation = from_json(json_data.as_bytes(), rel_type).unwrap();
//!
//! // 3. Export to CSV
//! let csv = to_csv(&relation, ',').unwrap();
//! assert!(csv.contains("id,name"));
//! assert!(csv.contains("1,\"Alice\""));
//! ```

pub(crate) mod exporter;
pub(crate) mod importer;
pub(crate) mod visualizer;

pub use exporter::*;
pub use importer::*;
pub use visualizer::*;
