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
//! - Renames are simultaneous (`a -> b, b -> a` swaps); renaming a
//!   non-existent attribute or producing duplicate attribute names is an
//!   error ([`RenameError`])
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
//! let renamed = relation.rename(&[("emp_id", "employee_id")]).unwrap();
//! assert!(renamed.relation_type().heading().has_attribute("employee_id"));
//! assert!(!renamed.relation_type().heading().has_attribute("emp_id"));
//! ```

use crate::error::DatabaseError;
use crate::types::{RelationType, TupleType};
use crate::values::{Relation, Tuple};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
use thiserror::Error;

/// Errors that can occur during renaming.
///
/// TTM: RM Prescription 7 - `r RENAME {a AS b, ...}` is defined only when
/// every source attribute belongs to the heading of `r` and the resulting
/// heading has no duplicate attribute names.
///
/// # Examples
///
/// ```
/// use relvar_core::algebra::RenameError;
///
/// let err = RenameError::DuplicateTarget("id".to_string());
/// assert!(err.to_string().contains("'id'"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RenameError {
    /// A source attribute is not in the relation's heading.
    #[error("Cannot rename attribute '{0}': it is not in the relation's heading")]
    UnknownAttribute(String),

    /// The same source attribute appears in more than one mapping.
    #[error("Attribute '{0}' is renamed more than once")]
    DuplicateSource(String),

    /// The result heading would contain two attributes with this name.
    #[error("Renaming would produce two attributes named '{0}'")]
    DuplicateTarget(String),
}

impl From<RenameError> for DatabaseError {
    fn from(err: RenameError) -> Self {
        match err {
            RenameError::UnknownAttribute(name) => {
                DatabaseError::AttributeNotFound(name, "rename operand".to_string())
            }
            RenameError::DuplicateSource(name) | RenameError::DuplicateTarget(name) => {
                DatabaseError::DuplicateAttributeName(name)
            }
        }
    }
}

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
    /// - All mappings apply simultaneously, so `[("a", "b"), ("b", "a")]` swaps
    ///   two attributes
    ///
    /// # Errors
    ///
    /// - [`RenameError::UnknownAttribute`] - a source attribute is not in the heading
    /// - [`RenameError::DuplicateSource`] - a source attribute is mapped twice
    /// - [`RenameError::DuplicateTarget`] - two attributes would end up with the
    ///   same name (e.g. `A -> C, B -> C`, or `a -> b` while `b` is not renamed)
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
    /// ]).unwrap();
    ///
    /// assert!(renamed.relation_type().heading().has_attribute("employee_id"));
    /// assert!(renamed.relation_type().heading().has_attribute("department_id"));
    /// assert!(renamed.relation_type().heading().has_attribute("name"));
    /// ```
    pub fn rename(&self, mappings: &[(&str, &str)]) -> Result<Self, RenameError> {
        let (new_heading, new_names) =
            build_renamed_heading_and_names(self.relation_type().heading(), mappings)?;

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
            // 4. New names are pairwise distinct (validated), so no value is lost.
            Tuple::new_unchecked(new_heading_arc.clone(), values_map)
        });

        // Safety:
        // We guarantee that renamed_tuples conform to new_rel_type because:
        // - new_rel_type uses new_heading
        // - Tuples are created with new_heading
        Ok(Relation::from_tuples_unchecked(
            new_rel_type,
            renamed_tuples,
        ))
    }

    /// Renames attributes in this relation according to the provided mapping, consuming the relation.
    ///
    /// This is an optimized version of `rename` that avoids allocating a new
    /// collection for the tuples' inner values, instead migrating them in-place
    /// from the old names to the new names while consuming the source relation.
    /// This reduces heap allocations and `.clone()` overhead.
    ///
    /// # Errors
    ///
    /// Same as [`rename`](Self::rename).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new().with_attribute("old_name", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    /// let mut rel = Relation::new(rel_type);
    /// rel.insert(tuple! { old_name: 1i64 }).unwrap();
    ///
    /// let renamed = rel.rename_into(&[("old_name", "new_name")]).unwrap();
    /// assert!(renamed.relation_type().tuple_type().has_attribute("new_name"));
    /// ```
    pub fn rename_into(self, mappings: &[(&str, &str)]) -> Result<Self, RenameError> {
        if mappings.is_empty() {
            return Ok(self);
        }

        let (new_heading, new_names) =
            build_renamed_heading_and_names(self.relation_type().heading(), mappings)?;

        let new_rel_type = RelationType::new(new_heading.clone());
        let new_heading_arc = Arc::new(new_heading);

        // Rename attributes in each tuple by consuming it
        let renamed_tuples = self.into_iter().map(move |tuple| {
            // Optimization: Zip pre-calculated new names with consumed (key, value) pairs.
            // Both iterators follow the sorted order of old attribute names.
            // By keeping the old key string, we can reuse its memory allocation
            // if the name didn't actually change, avoiding String cloning.
            let values_map: BTreeMap<String, _> = new_names
                .iter()
                .zip(tuple.into_values())
                .map(|(new_name, (old_name, value))| {
                    if new_name == &old_name {
                        // Reuse the existing string allocation
                        (old_name, value)
                    } else {
                        // Must allocate a new string for the changed name
                        (new_name.clone(), value)
                    }
                })
                .collect();

            // Safety:
            // Same as rename: new names and types are guaranteed to match
            // the new heading we constructed above.
            Tuple::new_unchecked(new_heading_arc.clone(), values_map)
        });

        // Safety: We guarantee that renamed_tuples conform to new_rel_type
        Ok(Relation::from_tuples_unchecked(
            new_rel_type,
            renamed_tuples,
        ))
    }
}

/// Helper to build the new heading and name mappings for the rename operator.
///
/// # Errors
///
/// Rejects unknown or repeated source attributes and duplicate result names
/// (see [`RenameError`]).
fn build_renamed_heading_and_names(
    old_heading: &TupleType,
    mappings: &[(&str, &str)],
) -> Result<(TupleType, Vec<String>), RenameError> {
    let mut sources = BTreeSet::new();
    for (from, _) in mappings {
        if !old_heading.has_attribute(from) {
            return Err(RenameError::UnknownAttribute((*from).to_string()));
        }
        if !sources.insert(*from) {
            return Err(RenameError::DuplicateSource((*from).to_string()));
        }
    }

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

        if new_heading.has_attribute(new_name) {
            return Err(RenameError::DuplicateTarget(new_name.to_string()));
        }
        new_heading = new_heading.with_attribute(new_name, attr_type.clone());
        new_names.push(new_name.to_string());
    }

    Ok((new_heading, new_names))
}

#[cfg(test)]
mod tests {
    use super::RenameError;
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

        let renamed = relation
            .rename_into(&[("emp_id", "employee_id"), ("dept_id", "department_id")])
            .unwrap();

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

        let result = relation.rename(&[("emp_id", "id")]).unwrap();

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

        let result = relation.rename(&[("emp_id", "employee_id")]).unwrap();

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

        let result = relation
            .rename(&[("emp_id", "employee_id"), ("dept_id", "department_id")])
            .unwrap();

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

        let result = relation.rename(&[]).unwrap();

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

        let result = relation.rename(&[("emp_id", "id")]).unwrap();

        assert!(result.is_empty());
        assert!(result.relation_type().heading().has_attribute("id"));
    }

    fn abc_relation() -> Relation {
        let heading = TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int)
            .with_attribute("c", ScalarType::String);
        let mut relation = Relation::new(RelationType::new(heading));
        relation
            .insert(tuple! { a: 1i64, b: 2i64, c: "x" })
            .unwrap();
        relation
            .insert(tuple! { a: 1i64, b: 3i64, c: "x" })
            .unwrap();
        relation
    }

    /// TTM: renaming an attribute that is not in the heading is an error.
    #[test]
    fn test_rename_rejects_unknown_attribute() {
        let result = abc_relation().rename(&[("nope", "x")]);
        assert_eq!(
            result.map(|r| r.degree()),
            Err(RenameError::UnknownAttribute("nope".to_string()))
        );
    }

    /// Two attributes renamed to the same name would collapse the heading
    /// (and could merge tuples): rejected.
    #[test]
    fn test_rename_rejects_two_sources_with_one_target() {
        let result = abc_relation().rename(&[("a", "z"), ("b", "z")]);
        assert_eq!(
            result.map(|r| r.degree()),
            Err(RenameError::DuplicateTarget("z".to_string()))
        );
    }

    /// Renaming onto an existing, unrenamed attribute name is rejected.
    #[test]
    fn test_rename_rejects_target_that_collides_with_unrenamed_attribute() {
        let result = abc_relation().rename(&[("a", "b")]);
        assert_eq!(
            result.map(|r| r.degree()),
            Err(RenameError::DuplicateTarget("b".to_string()))
        );
    }

    #[test]
    fn test_rename_into_rejects_collision() {
        let result = abc_relation().rename_into(&[("a", "b")]);
        assert_eq!(
            result.map(|r| r.degree()),
            Err(RenameError::DuplicateTarget("b".to_string()))
        );
    }

    /// The same source renamed twice is ambiguous: rejected.
    #[test]
    fn test_rename_rejects_duplicate_source() {
        let result = abc_relation().rename(&[("a", "x"), ("a", "y")]);
        assert_eq!(
            result.map(|r| r.degree()),
            Err(RenameError::DuplicateSource("a".to_string()))
        );
    }

    /// Renames are simultaneous: swapping two names is legal.
    #[test]
    fn test_rename_swap_is_simultaneous() {
        let result = abc_relation().rename(&[("a", "b"), ("b", "a")]).unwrap();
        assert_eq!(result.degree(), 3);
        assert_eq!(result.cardinality(), 2);
        assert!(result.contains(&tuple! { a: 2i64, b: 1i64, c: "x" }));
        assert!(result.contains(&tuple! { a: 3i64, b: 1i64, c: "x" }));

        let into = abc_relation()
            .rename_into(&[("a", "b"), ("b", "a")])
            .unwrap();
        assert_eq!(into, result);
    }

    /// Renaming an attribute to itself is the identity.
    #[test]
    fn test_rename_to_same_name_is_identity() {
        let result = abc_relation().rename(&[("a", "a")]).unwrap();
        assert_eq!(result, abc_relation());
    }
}
