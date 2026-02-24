//! Developer tools and utilities.
//!
//! This module provides essential utilities for working with Relvar data outside
//! of the core query engine. It includes tools for data migration (import/export)
//! and schema visualization.
//!
//! # Included Tools
//!
//! - **[`exporter`]:** Export relations to standard formats like CSV, JSON, and ASCII tables.
//!   Useful for reporting, backups, or integrating with other systems.
//! - **[`importer`]:** Import data from JSON and CSV. Features strict type checking
//!   and limit enforcement (DoS protection).
//! - **[`visualizer`]:** Generate Graphviz DOT diagrams of your database schema,
//!   showing tables, columns, keys, and relationships.
//!
//! # Example: Import, Visualize, and Export
//!
//! ```
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
//! ```

pub mod exporter;
pub mod importer;
pub mod visualizer;
