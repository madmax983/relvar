//! Rename operator for attribute renaming.
//!
//! The rename operator (ρ in relational algebra) changes the names of attributes
//! in a relation while preserving their types and values.
//!
//! # TTM Compliance
//!
//! - Attribute types are preserved during renaming
//! - Result is a valid relation with the renamed heading
//! - Set semantics are maintained
//!
//! # Example
//!
//! ```
//! use relvar_core::types::{TupleType, RelationType, ScalarType};
//! use relvar_core::values::Relation;
//! use relvar_core::tuple;
//!
//! let heading = TupleType::new()
//!     .with_attribute("emp_id", ScalarType::Int)
//!     .with_attribute("name", ScalarType::String);
//!
//! let mut relation = Relation::new(RelationType::new(heading));
//! relation.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
//!
//! // Rename emp_id to employee_id
//! let renamed = relation.rename(&[("emp_id", "employee_id")]);
//! assert!(renamed.relation_type().heading().has_attribute("employee_id"));
//! assert!(!renamed.relation_type().heading().has_attribute("emp_id"));
//! ```

use crate::types::{RelationType, TupleType};
use crate::values::{Relation, Tuple};
use std::collections::BTreeMap;

impl Relation {
    /// Renames attributes in this relation according to the provided mapping.
    ///
    /// This is the relational algebra ρ (rho) operator. It produces a new relation
    /// with the same tuples but with some attribute names changed. The attribute
    /// types and values are preserved.
    ///
    /// # Arguments
    ///
    /// * `mappings` - A slice of (old_name, new_name) pairs specifying which
    ///   attributes to rename. Attributes not in the mapping are unchanged.
    ///
    /// # Returns
    ///
    /// A new relation with the renamed attributes.
    ///
    /// # Behavior
    ///
    /// - Attributes listed in mappings are renamed to their new names
    /// - Attributes not in mappings retain their original names
    /// - Attribute types are preserved
    /// - Tuple values are preserved (associated with new names)
    /// - Mappings for non-existent attributes are silently ignored
    ///
    /// # Naming Collisions
    ///
    /// If multiple attributes are mapped to the same target name (e.g., A -> C, B -> C),
    /// the "Last Write Wins" rule applies based on the **lexicographical order of the source attributes**.
    ///
    /// Since attributes are stored in a `BTreeMap`, iteration order is determined by attribute name.
    /// The attribute that comes later alphabetically will overwrite the value of the earlier one.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("emp_id", ScalarType::Int)
    ///     .with_attribute("name", ScalarType::String)
    ///     .with_attribute("dept_id", ScalarType::Int);
    ///
    /// let mut relation = Relation::new(RelationType::new(heading));
    /// relation.insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 }).unwrap();
    ///
    /// // Rename multiple attributes
    /// let renamed = relation.rename(&[
    ///     ("emp_id", "employee_id"),
    ///     ("dept_id", "department_id"),
    /// ]);
    ///
    /// assert!(renamed.relation_type().heading().has_attribute("employee_id"));
    /// assert!(renamed.relation_type().heading().has_attribute("department_id"));
    /// assert!(renamed.relation_type().heading().has_attribute("name"));
    /// ```
    pub fn rename(&self, mappings: &[(&str, &str)]) -> Self {
        // Build new heading with renamed attributes
        let mut new_heading = TupleType::new();

        for (old_name, attr_type) in self.relation_type().heading().attributes() {
            // Check if this attribute should be renamed
            let new_name = mappings
                .iter()
                .find(|(from, _)| from == old_name)
                .map(|(_, to)| *to)
                .unwrap_or(old_name.as_str());

            new_heading = new_heading.with_attribute(new_name, attr_type.clone());
        }

        let new_rel_type = RelationType::new(new_heading.clone());

        // Rename attributes in each tuple
        let renamed_tuples = self.tuples().map(|tuple| {
            let mut values = BTreeMap::new();

            for (old_name, value) in tuple.values() {
                let new_name = mappings
                    .iter()
                    .find(|(from, _)| from == old_name)
                    .map(|(_, to)| *to)
                    .unwrap_or(old_name.as_str());

                values.insert(new_name.to_string(), value.clone());
            }

            Tuple::new_unchecked(new_heading.clone(), values)
        });

        Relation::from_tuples_unchecked(new_rel_type, renamed_tuples)
    }
}

#[cfg(test)]
mod tests {
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::Relation;

    #[test]
    fn test_rename_changes_attribute_name() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice" })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, name: "Bob" })
            .unwrap();

        let result = relation.rename(&[("emp_id", "id")]);

        // Check that new attribute exists and old doesn't
        assert!(result.relation_type().heading().has_attribute("id"));
        assert!(!result.relation_type().heading().has_attribute("emp_id"));
        assert!(result.relation_type().heading().has_attribute("name"));

        // Check cardinality preserved
        assert_eq!(result.cardinality(), 2);
    }

    #[test]
    fn test_rename_preserves_type() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice" })
            .unwrap();

        let result = relation.rename(&[("emp_id", "employee_id")]);

        // Verify type preserved
        assert_eq!(
            result
                .relation_type()
                .heading()
                .get_attribute_type("employee_id"),
            Some(&ScalarType::Int)
        );
    }

    #[test]
    fn test_rename_multiple_attributes() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("dept_id", ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();

        let result = relation.rename(&[("emp_id", "employee_id"), ("dept_id", "department_id")]);

        assert!(
            result
                .relation_type()
                .heading()
                .has_attribute("employee_id")
        );
        assert!(
            result
                .relation_type()
                .heading()
                .has_attribute("department_id")
        );
        assert!(result.relation_type().heading().has_attribute("name"));
        assert!(!result.relation_type().heading().has_attribute("emp_id"));
        assert!(!result.relation_type().heading().has_attribute("dept_id"));
    }

    #[test]
    fn test_rename_empty_mappings() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let rel_type = RelationType::new(heading.clone());
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice" })
            .unwrap();

        let result = relation.rename(&[]);

        // Should be unchanged
        assert_eq!(result.relation_type().heading(), &heading);
        assert_eq!(result.cardinality(), 1);
    }

    #[test]
    fn test_rename_on_empty_relation() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let rel_type = RelationType::new(heading);
        let relation = Relation::new(rel_type);

        let result = relation.rename(&[("emp_id", "id")]);

        assert!(result.is_empty());
        assert!(result.relation_type().heading().has_attribute("id"));
    }

    /// Verifies the behavior when multiple attributes are renamed to the same name.
    ///
    /// CURRENT BEHAVIOR: Silent overwriting.
    /// Since iteration over attributes is based on BTreeMap (sorted by name),
    /// the attribute that comes later lexicographically overwrites earlier ones.
    ///
    /// This test documents this "Last Write Wins" behavior.
    #[test]
    fn test_rename_collision_overwrites_values() {
        let heading = TupleType::new()
            .with_attribute("A", ScalarType::Int)
            .with_attribute("B", ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation.insert(tuple! { A: 1i64, B: 2i64 }).unwrap();

        // Rename both A and B to C
        // A comes before B, so we expect B to overwrite A.
        let result = relation.rename(&[("A", "C"), ("B", "C")]);

        assert_eq!(result.degree(), 1);
        assert!(result.relation_type().heading().has_attribute("C"));

        let tuple = result.tuples().next().unwrap();
        // Value should be 2 (from B)
        assert_eq!(tuple.get_typed::<i64>("C").unwrap(), 2);
    }
}
