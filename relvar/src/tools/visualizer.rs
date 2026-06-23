//! Schema visualizer for Relvar databases.
//!
//! This module provides the [`SchemaVisualizer`] struct, which can generate
//! Graphviz DOT code representing the database schema, including relations
//! and foreign key constraints.
//!
//! # Example
//!
//! ```
//! use relvar::{Database, InMemoryEngine};
//! use relvar::{TupleType, RelationType, ScalarType};
//! use relvar::constraints::{ForeignKey, ForeignKeyConstraints, KeyConstraints, PrimaryKey};
//! use relvar::tools::SchemaVisualizer;
//!
//! let mut db = Database::new(InMemoryEngine::new());
//!
//! // Define Departments
//! let dept_type = RelationType::new(
//!     TupleType::new()
//!         .with_attribute("dept_id", ScalarType::Int)
//!         .with_attribute("name", ScalarType::String)
//! );
//! db.create_relvar("DEPT", dept_type).unwrap();
//!
//! let pk = PrimaryKey::new(vec!["dept_id".to_string()]).unwrap();
//! db.set_key_constraints("DEPT", KeyConstraints::new().with_primary_key(pk)).unwrap();
//!
//! // Define Employees
//! let emp_type = RelationType::new(
//!     TupleType::new()
//!         .with_attribute("emp_id", ScalarType::Int)
//!         .with_attribute("name", ScalarType::String)
//!         .with_attribute("dept_id", ScalarType::Int)
//! );
//! db.create_relvar("EMP", emp_type).unwrap();
//!
//! let fk = ForeignKey::new(
//!     vec!["dept_id".to_string()],
//!     "DEPT".to_string(),
//!     vec!["dept_id".to_string()]
//! ).unwrap();
//! db.set_foreign_key_constraints("EMP", ForeignKeyConstraints::new().with_foreign_key(fk)).unwrap();
//!
//! // Generate DOT
//! let visualizer = SchemaVisualizer::new(&db);
//! let dot = visualizer.to_dot();
//!
//! assert!(dot.contains("digraph DatabaseSchema"));
//! // Identifiers are now quoted for security
//! assert!(dot.contains("\"EMP\" -> \"DEPT\""));
//! ```

use relvar_core::database::Database;
use relvar_core::storage_engine::StorageEngine;

/// A tool for visualizing the database schema.
/// # Examples
///
/// ```
/// use relvar::{Database, InMemoryEngine, tools::SchemaVisualizer};
/// // Example usage
/// ```
pub struct SchemaVisualizer<'a, E: StorageEngine> {
    db: &'a Database<E>,
}

impl<'a, E: StorageEngine> SchemaVisualizer<'a, E> {
    /// Create a new schema visualizer for the given database.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Database, InMemoryEngine, tools::SchemaVisualizer};
    ///
    /// let db = Database::new(InMemoryEngine::new());
    /// let visualizer = SchemaVisualizer::new(&db);
    /// ```
    pub fn new(db: &'a Database<E>) -> Self {
        Self { db }
    }

    /// Generate a Graphviz DOT string representation of the schema.
    ///
    /// The output is a valid DOT file that can be rendered using Graphviz tools
    /// (e.g. `dot -Tpng schema.dot -o schema.png`).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Database, InMemoryEngine, tools::SchemaVisualizer};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// // ... setup schema ...
    ///
    /// let viz = SchemaVisualizer::new(&db);
    /// let dot_code = viz.to_dot();
    /// println!("{}", dot_code);
    /// ```
    pub fn to_dot(&self) -> String {
        let mut dot = String::from("digraph DatabaseSchema {\n");
        dot.push_str("    rankdir=LR;\n");
        dot.push_str("    node [shape=none, fontname=\"Helvetica\", fontsize=10];\n");
        dot.push_str("    edge [fontname=\"Helvetica\", fontsize=8];\n\n");

        let relvars = self.db.list_relvars();

        self.generate_nodes(&relvars, &mut dot);
        self.generate_edges(&relvars, &mut dot);

        dot.push_str("}\n");
        dot
    }

    fn generate_nodes(&self, relvars: &[String], dot: &mut String) {
        // 1. Generate nodes (Tables)
        for name in relvars {
            // Note: We use get_relvar_type() to get the relation structure.
            // This avoids loading the full relation data.
            if let Ok(relation_type) = self.db.get_relvar_type(name) {
                let tuple_type = relation_type.tuple_type();

                // Determine primary key attributes
                let pk_attrs = if let Some(key_constraints) = self.db.get_key_constraints(name) {
                    if let Some(pk) = key_constraints.primary_key() {
                        pk.attributes().to_vec()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };

                let node_id = escape_dot_id(name);
                let table_title = escape_html(name);

                dot.push_str(&format!(
                    "    {} [label=<<table border=\"0\" cellborder=\"1\" cellspacing=\"0\" cellpadding=\"4\">\n",
                    node_id
                ));
                dot.push_str(&format!(
                    "        <tr><td bgcolor=\"lightgrey\" colspan=\"2\"><b>{}</b></td></tr>\n",
                    table_title
                ));

                // Sort attributes for consistent output
                let mut attributes: Vec<_> = tuple_type.attributes().iter().collect();
                attributes.sort_by(|a, b| a.0.cmp(b.0));

                for (attr_name, scalar_type) in attributes {
                    let is_pk = pk_attrs.contains(attr_name);
                    let safe_attr_name = escape_html(attr_name);

                    let display_name = if is_pk {
                        format!("<u>{}</u>", safe_attr_name)
                    } else {
                        safe_attr_name
                    };

                    // Note: scalar_type implements Debug, which might contain special chars.
                    // Ideally we'd escape that too, strictly speaking.
                    let safe_type = escape_html(&format!("{:?}", scalar_type));

                    dot.push_str(&format!(
                        "        <tr><td align=\"left\">{}</td><td align=\"left\">{}</td></tr>\n",
                        display_name, safe_type
                    ));
                }
                dot.push_str("    </table>>];\n\n");
            }
        }
    }

    fn generate_edges(&self, relvars: &[String], dot: &mut String) {
        // 2. Generate edges (Foreign Keys)
        for name in relvars {
            if let Some(fk_constraints) = self.db.get_foreign_key_constraints(name) {
                for fk in fk_constraints.foreign_keys() {
                    let ref_table = fk.referenced_relation_name();
                    let cols = fk.foreign_key_attributes().join(", ");

                    let source_id = escape_dot_id(name);
                    let target_id = escape_dot_id(ref_table);
                    let label = escape_dot_string_content(&cols);

                    dot.push_str(&format!(
                        "    {} -> {} [label=\"({})\"];\n",
                        source_id, target_id, label
                    ));
                }
            }
        }
    }
}

/// Escapes special characters for HTML-like labels in Graphviz.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Escapes a string to be used as a DOT identifier, adding quotes.
fn escape_dot_id(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\\\""))
}

/// Escapes content to be placed inside a DOT double-quoted string.
fn escape_dot_string_content(s: &str) -> String {
    s.replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::constraints::{ForeignKey, ForeignKeyConstraints, KeyConstraints, PrimaryKey};
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_visualizer_dot_output() {
        let mut db = Database::new(InMemoryEngine::new());

        // Create DEPT
        let dept_type = RelationType::new(
            TupleType::new()
                .with_attribute("dept_id", ScalarType::Int)
                .with_attribute("name", ScalarType::String),
        );
        db.create_relvar("DEPT", dept_type).unwrap();

        let pk = PrimaryKey::new(vec!["dept_id".to_string()]).unwrap();
        db.set_key_constraints("DEPT", KeyConstraints::new().with_primary_key(pk))
            .unwrap();

        // Create EMP
        let emp_type = RelationType::new(
            TupleType::new()
                .with_attribute("emp_id", ScalarType::Int)
                .with_attribute("name", ScalarType::String)
                .with_attribute("dept_id", ScalarType::Int),
        );
        db.create_relvar("EMP", emp_type).unwrap();

        let fk = ForeignKey::new(
            vec!["dept_id".to_string()],
            "DEPT".to_string(),
            vec!["dept_id".to_string()],
        )
        .unwrap();
        db.set_foreign_key_constraints("EMP", ForeignKeyConstraints::new().with_foreign_key(fk))
            .unwrap();

        // Visualize
        let visualizer = SchemaVisualizer::new(&db);
        let dot = visualizer.to_dot();

        println!("{}", dot);

        // Assertions - now with quotes
        assert!(dot.contains("digraph DatabaseSchema"));
        assert!(dot.contains("\"DEPT\" [label="));
        assert!(dot.contains("\"EMP\" [label="));
        assert!(dot.contains("<u>dept_id</u>")); // PK underlined
        assert!(dot.contains("\"EMP\" -> \"DEPT\"")); // FK edge
    }

    #[test]
    fn test_escaping_helpers() {
        assert_eq!(escape_html("foo < bar"), "foo &lt; bar");
        assert_eq!(escape_html("foo \" bar"), "foo &quot; bar");
        assert_eq!(escape_dot_id("foo \" bar"), "\"foo \\\" bar\"");
        assert_eq!(escape_dot_string_content("foo \" bar"), "foo \\\" bar");
    }
}
