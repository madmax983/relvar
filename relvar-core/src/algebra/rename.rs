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
use std::sync::Arc;

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
        let (new_heading, new_names) =
            build_renamed_heading_and_names(self.relation_type().heading(), mappings);

        let new_rel_type = RelationType::new(new_heading.clone());
        let new_heading_arc = Arc::new(new_heading);
        let new_names_arc = Arc::new(new_names);

        // Rename attributes in each tuple
        let renamed_tuples = self.tuples().map(move |tuple| {
            // Optimization: Zip pre-calculated new names with values.
            // Both iterators follow the sorted order of old attribute names.
            // - new_names_arc was built by iterating heading().attributes() (sorted by old_name)
            // - tuple.values().values() iterates values sorted by old_name (BTreeMap keys)
            let values_map: BTreeMap<String, _> = new_names_arc
                .iter()
                .zip(tuple.values().values())
                .map(|(new_name, value)| (new_name.clone(), value.clone()))
                .collect();

            // Safety:
            // 1. We constructed new_heading directly from old_heading with renames applied.
            // 2. We constructed values_map by zipping new names with old values in the same order.
            // 3. Types are preserved (we clone the type from old heading to new heading).
            // 4. "Last Write Wins" logic for duplicate target names is handled by BTreeMap::collect
            //    overwriting previous entries, matching the behavior of new_heading construction.
            Tuple::new_unchecked(new_heading_arc.clone(), values_map)
        });

        // Safety:
        // We guarantee that renamed_tuples conform to new_rel_type because:
        // - new_rel_type uses new_heading
        // - Tuples are created with new_heading
        Relation::from_tuples_unchecked(new_rel_type, renamed_tuples)
    }

    /// Renames attributes in this relation according to the provided mapping, consuming the relation.
    ///
    /// This is an optimized version of `rename` that avoids allocating a new
    /// collection for the tuples' inner values, instead migrating them in-place
    /// from the old names to the new names while consuming the source relation.
    /// This reduces heap allocations and `.clone()` overhead.
    pub fn rename_into(self, mappings: &[(&str, &str)]) -> Self {
        if mappings.is_empty() {
            return self;
        }

        let (new_heading, new_names) =
            build_renamed_heading_and_names(self.relation_type().heading(), mappings);

        let new_rel_type = RelationType::new(new_heading.clone());
        let new_heading_arc = Arc::new(new_heading);

        // Rename attributes in each tuple by consuming it
        let renamed_tuples = self.into_iter().map(move |tuple| {
            // Optimization: Zip pre-calculated new names with consumed values.
            // Both iterators follow the sorted order of old attribute names.
            let values_map: BTreeMap<String, _> = new_names
                .iter()
                .zip(tuple.into_values().into_values())
                .map(|(new_name, value)| (new_name.clone(), value))
                .collect();

            // Safety:
            // Same as rename: new names and types are guaranteed to match
            // the new heading we constructed above.
            Tuple::new_unchecked(new_heading_arc.clone(), values_map)
        });

        // Safety: We guarantee that renamed_tuples conform to new_rel_type
        Relation::from_tuples_unchecked(new_rel_type, renamed_tuples)
    }
}

/// Helper to build the new heading and name mappings for the rename operator.
fn build_renamed_heading_and_names(
    old_heading: &TupleType,
    mappings: &[(&str, &str)],
) -> (TupleType, Vec<String>) {
    // Build new heading with renamed attributes
    let mut new_heading = TupleType::new();

    // Also pre-calculate the new names in the sorted order of attributes
    // This vector will align perfectly with tuple.values().values() iteration
    // because both follow BTreeMap's sorted key order.
    let mut new_names = Vec::with_capacity(old_heading.degree());

    for (old_name, attr_type) in old_heading.attributes() {
        // Check if this attribute should be renamed
        let new_name = mappings
            .iter()
            .find(|(from, _)| from == old_name)
            .map(|(_, to)| *to)
            .unwrap_or(old_name.as_str());

        new_heading = new_heading.with_attribute(new_name, attr_type.clone());
        new_names.push(new_name.to_string());
    }

    (new_heading, new_names)
}

#[cfg(test)]
mod tests {
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::Relation;

    #[test]
    fn test_rename_into() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("dept_id", ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();

        let renamed =
            relation.rename_into(&[("emp_id", "employee_id"), ("dept_id", "department_id")]);

        assert!(
            renamed
                .relation_type()
                .heading()
                .has_attribute("employee_id")
        );
        assert!(
            renamed
                .relation_type()
                .heading()
                .has_attribute("department_id")
        );
        assert!(renamed.relation_type().heading().has_attribute("name"));
        assert!(!renamed.relation_type().heading().has_attribute("emp_id"));
        assert!(!renamed.relation_type().heading().has_attribute("dept_id"));
        assert_eq!(renamed.cardinality(), 1);

        let tuple = renamed.tuples().next().unwrap();
        assert_eq!(tuple.get_typed::<i64>("employee_id").unwrap(), 1);
        assert_eq!(
            tuple.get_typed::<String>("name").unwrap(),
            "Alice".to_string()
        );
        assert_eq!(tuple.get_typed::<i64>("department_id").unwrap(), 10);
    }

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
