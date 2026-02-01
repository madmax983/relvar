//! Schema visualizer for Relvar databases.
//!
//! This module provides the [`to_dot`] function, which can generate
//! Graphviz DOT code representing the database schema, including relations
//! and foreign key constraints.

use relvar_core::database::Database;
use relvar_core::storage_engine::StorageEngine;

/// Generate a Graphviz DOT string representation of the schema.
pub fn to_dot<E: StorageEngine>(db: &mut Database<E>) -> String {
    let mut dot = String::from("digraph DatabaseSchema {\n");
    dot.push_str("    rankdir=LR;\n");
    dot.push_str("    node [shape=none, fontname=\"Helvetica\", fontsize=10];\n");
    dot.push_str("    edge [fontname=\"Helvetica\", fontsize=8];\n\n");

    let relvars = db.list_relvars();

    // 1. Generate nodes (Tables)
    for name in &relvars {
        // Note: We use query() to get the relation structure.
        // Since this is for visualization, we ignore potential errors (e.g. temporary locks)
        // or if the relation somehow disappears.
        if let Ok(relation) = db.query(name) {
            let relation_type = relation.relation_type();
            let tuple_type = relation_type.tuple_type();

            // Determine primary key attributes
            let pk_attrs = if let Some(key_constraints) = db.get_key_constraints(name) {
                if let Some(pk) = key_constraints.primary_key() {
                    pk.attributes().to_vec()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            dot.push_str(&format!("    {} [label=<<table border=\"0\" cellborder=\"1\" cellspacing=\"0\" cellpadding=\"4\">\n", name));
            dot.push_str(&format!(
                "        <tr><td bgcolor=\"lightgrey\" colspan=\"2\"><b>{}</b></td></tr>\n",
                name
            ));

            // Sort attributes for consistent output
            let mut attributes: Vec<_> = tuple_type.attributes().iter().collect();
            attributes.sort_by(|a, b| a.0.cmp(b.0));

            for (attr_name, scalar_type) in attributes {
                let is_pk = pk_attrs.contains(attr_name);
                let display_name = if is_pk {
                    format!("<u>{}</u>", attr_name)
                } else {
                    attr_name.to_string()
                };

                dot.push_str(&format!(
                    "        <tr><td align=\"left\">{}</td><td align=\"left\">{:?}</td></tr>\n",
                    display_name, scalar_type
                ));
            }
            dot.push_str("    </table>>];\n\n");
        }
    }

    // 2. Generate edges (Foreign Keys)
    for name in &relvars {
        if let Some(fk_constraints) = db.get_foreign_key_constraints(name) {
            for fk in fk_constraints.foreign_keys() {
                let ref_table = fk.referenced_relation_name();
                let cols = fk.foreign_key_attributes().join(", ");
                // We label the edge with the columns involved

                dot.push_str(&format!(
                    "    {} -> {} [label=\"({})\"];\n",
                    name, ref_table, cols
                ));
            }
        }
    }

    dot.push_str("}\n");
    dot
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
        let dot = to_dot(&mut db);

        println!("{}", dot);

        // Assertions
        assert!(dot.contains("digraph DatabaseSchema"));
        assert!(dot.contains("DEPT [label="));
        assert!(dot.contains("EMP [label="));
        assert!(dot.contains("<u>dept_id</u>")); // PK underlined
        assert!(dot.contains("EMP -> DEPT")); // FK edge
    }
}
