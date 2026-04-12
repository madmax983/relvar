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
use std::sync::Arc;

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

        // Optimization: If projecting all attributes, just return a clone of self
        if new_heading == *self.relation_type().heading() {
            return self.clone();
        }

        let new_rel_type = RelationType::new(new_heading.clone());

        // Share the heading via Arc to avoid cloning it for every tuple
        let shared_heading = Arc::new(new_heading);

        // Project each tuple
        let projected_tuples = self
            .tuples()
            .map(|tuple| project_tuple_values(tuple, &shared_heading));

        // Duplicates are automatically removed when creating the relation
        // Safety: projected_tuples use shared_heading which matches new_rel_type.heading()
        Relation::from_tuples_unchecked(new_rel_type, projected_tuples)
    }

    /// Projects this relation onto a subset of attributes, consuming the relation.
    ///
    /// This is an optimized version of `project` that avoids allocating a new
    /// collection by mutating the tuples in-place when possible.
    pub fn project_into(self, attributes: &[&str]) -> Self {
        // Build new heading with selected attributes
        let mut new_heading = TupleType::new();
        for attr_name in attributes {
            if let Some(attr_type) = self.relation_type().heading().get_attribute_type(attr_name) {
                new_heading = new_heading.with_attribute(*attr_name, attr_type.clone());
            }
        }

        if new_heading == *self.relation_type().heading() {
            return self;
        }

        let new_rel_type = RelationType::new(new_heading.clone());
        let shared_heading = Arc::new(new_heading);

        let projected_tuples = self
            .into_iter()
            .map(|tuple| project_tuple_values_owned(tuple, &shared_heading));

        Relation::from_tuples_unchecked(new_rel_type, projected_tuples)
    }
}

/// Helper function to project values from a source tuple based on a target heading.
///
/// Optimization: Uses synchronized iteration (merge-join style) between source tuple values
/// and result heading attributes. Both are sorted BTreeMaps (or iterate in sorted order).
/// This avoids O(log N) lookup for each attribute, reducing complexity from O(M log N) to O(N).
fn project_tuple_values(source_tuple: &Tuple, target_heading: &Arc<TupleType>) -> Tuple {
    // Optimization: Uses synchronized iteration (merge-sort style) between source tuple values
    // and result heading attributes. Both are sorted BTreeMaps.
    // This avoids O(log K) lookup for each attribute, reducing complexity from O(N log K) to O(N + K).
    let mut tuple_iter = source_tuple.values().iter();
    let mut heading_iter = target_heading.attributes().iter();

    let mut current_tuple = tuple_iter.next();
    let mut current_heading = heading_iter.next();

    let mut result_items = Vec::with_capacity(target_heading.degree());

    while let (Some((t_attr, t_val)), Some((h_attr, _))) = (current_tuple, current_heading) {
        use std::cmp::Ordering;
        match t_attr.cmp(h_attr) {
            Ordering::Equal => {
                result_items.push((t_attr.clone(), t_val.clone()));
                current_tuple = tuple_iter.next();
                current_heading = heading_iter.next();
            }
            Ordering::Less => {
                current_tuple = tuple_iter.next();
            }
            Ordering::Greater => {
                current_heading = heading_iter.next();
            }
        }
    }

    let values = std::collections::BTreeMap::from_iter(result_items);

    // Safety: We constructed values exactly from attributes present in new_heading
    // derived from the source relation schema, so types match by definition.
    Tuple::new_unchecked(target_heading.clone(), values)
}

fn project_tuple_values_owned(source_tuple: Tuple, target_heading: &Arc<TupleType>) -> Tuple {
    // Optimization: Retain only the attributes present in the target heading
    // using a merge-sort style iteration. Both are sorted BTreeMaps.
    let mut values = source_tuple.into_values();
    let mut heading_iter = target_heading.attributes().keys().peekable();

    values.retain(|k, _| {
        while let Some(&h_attr) = heading_iter.peek() {
            use std::cmp::Ordering;
            match h_attr.cmp(k) {
                Ordering::Less => {
                    heading_iter.next();
                }
                Ordering::Equal => {
                    return true;
                }
                Ordering::Greater => {
                    return false;
                }
            }
        }
        false
    });

    // Safety: We retained only attributes present in target_heading
    Tuple::new_unchecked(target_heading.clone(), values)
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
