//! Project operator for attribute selection.
//!
//! The project operator (π in relational algebra) selects a subset of
//! attributes from a relation, eliminating all other attributes.
//!
//! # TTM Compliance
//!
//! - Duplicates are automatically eliminated (set semantics)
//! - Result is a valid relation with the projected heading
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
//!     .with_attribute("name", ScalarType::String)
//!     .with_attribute("dept_id", ScalarType::Int);
//!
//! let mut relation = Relation::new(RelationType::new(heading));
//! relation.insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 }).unwrap();
//! relation.insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 10i64 }).unwrap();
//!
//! // Project onto just dept_id
//! let result = relation.project(&["dept_id"]);
//! assert_eq!(result.degree(), 1);
//! assert_eq!(result.cardinality(), 1);  // Duplicates eliminated!
//! ```

use crate::types::{RelationType, TupleType};
use crate::values::{Relation, Tuple};

impl Relation {
    /// Projects this relation onto a subset of attributes.
    ///
    /// This is the relational algebra π (pi) operator. It produces a new
    /// relation containing only the specified attributes. Duplicate tuples
    /// that result from the projection are automatically eliminated.
    ///
    /// # Arguments
    ///
    /// * `attributes` - The attribute names to include in the result
    ///
    /// # Returns
    ///
    /// A new relation with only the specified attributes.
    ///
    /// # Behavior
    ///
    /// - Attributes not in the list are removed.
    /// - **Warning:** Attributes requested that do not exist in the relation are silently ignored.
    ///   This follows set intersection logic: the result contains only attributes present in BOTH
    ///   the relation and the request list.
    /// - Duplicate tuples are automatically eliminated.
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
    ///     .with_attribute("name", ScalarType::String);
    ///
    /// let mut relation = Relation::new(RelationType::new(heading));
    /// relation.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
    ///
    /// let names_only = relation.project(&["name"]);
    /// assert_eq!(names_only.degree(), 1);
    ///
    /// // Projecting non-existent attributes results in empty heading (TABLE_DEE or TABLE_DUM)
    /// // Here, since the relation is not empty, we get TABLE_DEE (degree 0, cardinality 1)
    /// let ignored = relation.project(&["non_existent_attr"]);
    /// assert_eq!(ignored.degree(), 0);
    /// assert_eq!(ignored.cardinality(), 1);
    /// ```
    pub fn project(&self, attributes: &[&str]) -> Self {
        // Build new heading with selected attributes
        let mut new_heading = TupleType::new();
        for attr_name in attributes {
            if let Some(attr_type) = self.relation_type().heading().get_attribute_type(attr_name) {
                new_heading = new_heading.with_attribute(*attr_name, attr_type.clone());
            }
        }

        let new_rel_type = RelationType::new(new_heading.clone());

        // Project each tuple
        let projected_tuples = self.tuples().map(|tuple| {
            let values = attributes.iter().filter_map(|&attr_name| {
                tuple
                    .get(attr_name)
                    .map(|value| (attr_name.to_string(), value.clone()))
            });
            Tuple::new(new_heading.clone(), values)
                .expect("Projection should maintain type consistency")
        });

        // Duplicates are automatically removed when creating the relation
        Relation::from_tuples(new_rel_type, projected_tuples)
            .expect("Projected tuples should conform to new relation type")
    }
}

#[cfg(test)]
mod tests {
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::Relation;

    #[test]
    fn test_project_selects_attributes() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("dept_id", ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
            .unwrap();

        let result = relation.project(&["emp_id", "name"]);

        assert_eq!(result.degree(), 2);
        assert_eq!(result.cardinality(), 2);

        // Verify result has correct attributes
        let alice_projected = tuple! { emp_id: 1i64, name: "Alice" };
        assert!(result.contains(&alice_projected));
    }

    #[test]
    fn test_project_removes_duplicates() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("dept_id", ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 10i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 3i64, name: "Charlie", dept_id: 10i64 })
            .unwrap();

        // Project onto just dept_id - should have only one unique value
        let result = relation.project(&["dept_id"]);

        assert_eq!(result.degree(), 1);
        assert_eq!(result.cardinality(), 1); // All have same dept_id, duplicates removed
    }

    #[test]
    fn test_project_empty_relation() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let rel_type = RelationType::new(heading);
        let relation = Relation::new(rel_type);

        let result = relation.project(&["emp_id"]);

        assert_eq!(result.cardinality(), 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_project_all_attributes() {
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

        let result = relation.project(&["emp_id", "name"]);

        assert_eq!(result, relation);
    }

    #[test]
    fn test_project_single_attribute() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("dept_id", ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
            .unwrap();

        let result = relation.project(&["name"]);

        assert_eq!(result.degree(), 1);
        assert_eq!(result.cardinality(), 2);
    }

    /// Verifies TTM behavior for TABLE_DEE (empty heading, 1 tuple)
    ///
    /// When projecting a non-empty relation onto an empty set of attributes,
    /// the result should be a relation with degree 0 and cardinality 1.
    /// This is equivalent to TABLE_DEE (Boolean TRUE).
    #[test]
    fn test_project_empty_attributes_creates_table_dee() {
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

        // Project onto empty set of attributes
        let result = relation.project(&[]);

        // Should have degree 0 (no attributes)
        assert_eq!(result.degree(), 0);

        // Should have cardinality 1 (one empty tuple)
        // Since {Alice} becomes {} and {Bob} becomes {}, they are duplicates
        // and reduced to a single {}.
        assert_eq!(result.cardinality(), 1);

        // Verify the single tuple is empty
        let tuple = result.tuples().next().unwrap();
        assert_eq!(tuple.degree(), 0);
    }

    /// Verifies TTM behavior for TABLE_DUM (empty heading, 0 tuples)
    ///
    /// When projecting an empty relation onto an empty set of attributes,
    /// the result should be a relation with degree 0 and cardinality 0.
    /// This is equivalent to TABLE_DUM (Boolean FALSE).
    #[test]
    fn test_project_empty_attributes_on_empty_relation_creates_table_dum() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let rel_type = RelationType::new(heading);
        let relation = Relation::new(rel_type);

        // Project onto empty set of attributes
        let result = relation.project(&[]);

        // Should have degree 0
        assert_eq!(result.degree(), 0);

        // Should have cardinality 0
        assert_eq!(result.cardinality(), 0);
        assert!(result.is_empty());
    }
}
