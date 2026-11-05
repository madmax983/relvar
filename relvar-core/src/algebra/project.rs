//! Project operator for attribute selection.
//!
//! The project operator (π in relational algebra) selects a subset of
//! attributes from a relation, eliminating all other attributes.
//!
//! # TTM Compliance
//!
//! - Duplicates are automatically eliminated (set semantics)
//! - Result is a valid relation with the projected heading
//! - Projecting on an attribute that is not in the heading is a type error
//!   ([`ProjectError::UnknownAttribute`])
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
//! let result = relation.project(&["dept_id"]).unwrap();
//! assert_eq!(result.degree(), 1);
//! assert_eq!(result.cardinality(), 1);  // Duplicates eliminated!
//! ```

use crate::error::DatabaseError;
use crate::types::{RelationType, TupleType};
use crate::values::{Relation, Tuple};
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use thiserror::Error;

/// Errors that can occur during projection.
///
/// TTM: RM Prescription 7 - `r{A, B, ...}` is defined only when every named
/// attribute belongs to the heading of `r`.
///
/// # Examples
///
/// ```
/// use relvar_core::algebra::ProjectError;
///
/// let err = ProjectError::UnknownAttribute("salary".to_string());
/// assert!(err.to_string().contains("'salary'"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProjectError {
    /// A projection attribute is not in the relation's heading.
    #[error("Cannot project on attribute '{0}': it is not in the relation's heading")]
    UnknownAttribute(String),
}

impl From<ProjectError> for DatabaseError {
    fn from(err: ProjectError) -> Self {
        match err {
            ProjectError::UnknownAttribute(name) => {
                DatabaseError::AttributeNotFound(name, "projection operand".to_string())
            }
        }
    }
}

/// Builds the projected heading, rejecting names not in `heading`.
///
/// The attribute list is treated as a set: repeating a name is harmless.
fn projected_heading(heading: &TupleType, attributes: &[&str]) -> Result<TupleType, ProjectError> {
    let mut new_heading = TupleType::new();
    for attr_name in attributes {
        let attr_type = heading
            .get_attribute_type(attr_name)
            .ok_or_else(|| ProjectError::UnknownAttribute((*attr_name).to_string()))?;
        new_heading = new_heading.with_attribute(*attr_name, attr_type.clone());
    }
    Ok(new_heading)
}

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
    /// - The list is a set: naming an attribute twice is the same as naming it once.
    /// - Projecting on no attributes yields TABLE_DEE (non-empty input) or
    ///   TABLE_DUM (empty input).
    /// - Duplicate tuples are automatically eliminated.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectError::UnknownAttribute`] if a named attribute is not
    /// in the relation's heading (TTM: projection on a non-existent attribute
    /// is a type error).
    ///
    /// # Examples
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
    /// let names_only = relation.project(&["name"]).unwrap();
    /// assert_eq!(names_only.degree(), 1);
    ///
    /// // Projecting on no attributes gives TABLE_DEE here (the input is non-empty)
    /// let dee = relation.project(&[]).unwrap();
    /// assert_eq!(dee.degree(), 0);
    /// assert_eq!(dee.cardinality(), 1);
    ///
    /// // Projecting on a non-existent attribute is an error
    /// assert!(relation.project(&["non_existent_attr"]).is_err());
    /// ```
    pub fn project(&self, attributes: &[&str]) -> Result<Self, ProjectError> {
        // Build new heading with selected attributes
        let new_heading = projected_heading(self.relation_type().heading(), attributes)?;

        // Optimization: If projecting all attributes, just return a clone of self
        if new_heading == *self.relation_type().heading() {
            return Ok(self.clone());
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
        Ok(Relation::from_tuples_unchecked(
            new_rel_type,
            projected_tuples,
        ))
    }

    /// Projects this relation onto a subset of attributes, consuming the relation.
    ///
    /// This is an optimized version of `project` that avoids allocating a new
    /// collection by mutating the tuples in-place when possible.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectError::UnknownAttribute`] if a named attribute is not
    /// in the relation's heading.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("x", ScalarType::Int)
    ///     .with_attribute("y", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    /// let mut rel = Relation::new(rel_type);
    /// rel.insert(tuple! { x: 1i64, y: 2i64 }).unwrap();
    ///
    /// let projected = rel.project_into(&["x"]).unwrap();
    /// assert_eq!(projected.relation_type().degree(), 1);
    /// ```
    pub fn project_into(self, attributes: &[&str]) -> Result<Self, ProjectError> {
        // Build new heading with selected attributes
        let new_heading = projected_heading(self.relation_type().heading(), attributes)?;

        if new_heading == *self.relation_type().heading() {
            return Ok(self);
        }

        let new_rel_type = RelationType::new(new_heading.clone());
        let shared_heading = Arc::new(new_heading);

        let projected_tuples = self
            .into_iter()
            .map(|tuple| project_tuple_values_owned(tuple, &shared_heading));

        Ok(Relation::from_tuples_unchecked(
            new_rel_type,
            projected_tuples,
        ))
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
        use core::cmp::Ordering;
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

    let values = alloc::collections::BTreeMap::from_iter(result_items);

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
            use core::cmp::Ordering;
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

    fn emp_relation() -> Relation {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);
        let mut relation = Relation::new(RelationType::new(heading));
        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice" })
            .unwrap();
        relation
    }

    /// TTM: projecting on an attribute that is not in the heading is a type
    /// error, not a silent DEE/DUM.
    #[test]
    fn test_project_rejects_unknown_attribute() {
        let result = emp_relation().project(&["emp_id", "nope"]);
        assert_eq!(
            result.map(|r| r.degree()),
            Err(super::ProjectError::UnknownAttribute("nope".to_string()))
        );
    }

    #[test]
    fn test_project_into_rejects_unknown_attribute() {
        let result = emp_relation().project_into(&["nope"]);
        assert_eq!(
            result.map(|r| r.degree()),
            Err(super::ProjectError::UnknownAttribute("nope".to_string()))
        );
    }

    /// Repeating a name in the projection list is harmless: the list is a set.
    #[test]
    fn test_project_duplicate_names_are_a_set() {
        let result = emp_relation().project(&["name", "name"]).unwrap();
        assert_eq!(result.degree(), 1);
        assert_eq!(result.cardinality(), 1);
    }

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

        let result = relation.project(&["emp_id", "name"]).unwrap();

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
        let result = relation.project(&["dept_id"]).unwrap();

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

        let result = relation.project(&["emp_id"]).unwrap();

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

        let result = relation.project(&["emp_id", "name"]).unwrap();

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

        let result = relation.project(&["name"]).unwrap();

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
        let result = relation.project(&[]).unwrap();

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
        let result = relation.project(&[]).unwrap();

        // Should have degree 0
        assert_eq!(result.degree(), 0);

        // Should have cardinality 0
        assert_eq!(result.cardinality(), 0);
        assert!(result.is_empty());
    }
}
