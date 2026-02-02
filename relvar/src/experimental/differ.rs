//! Difference Engine for Relations.
//!
//! This module provides tools to calculate the semantic difference between two relations,
//! optionally respecting a primary key or candidate key to identify updates.
//!
//! # Concepts
//!
//! - **Set Difference**: Without a key, differences are purely set-based (Insert/Delete).
//! - **Key-Based Difference**: With a key, tuples with the same key but different values are identified as Updates.
//!
//! # Example
//!
//! ```
//! use relvar_core::values::{Relation, Tuple};
//! use relvar_core::types::{RelationType, TupleType, ScalarType};
//! use relvar_core::tuple;
//! use relvar_core::constraints::CandidateKey;
//! use relvar::experimental::differ::{diff, DiffReport};
//!
//! // Setup relations
//! let heading = TupleType::new()
//!     .with_attribute("id", ScalarType::Int)
//!     .with_attribute("val", ScalarType::String);
//! let rel_type = RelationType::new(heading);
//!
//! let mut old_rel = Relation::new(rel_type.clone());
//! old_rel.insert(tuple! { id: 1i64, val: "A" }).unwrap();
//!
//! let mut new_rel = Relation::new(rel_type);
//! new_rel.insert(tuple! { id: 1i64, val: "B" }).unwrap(); // Update
//! new_rel.insert(tuple! { id: 2i64, val: "C" }).unwrap(); // Insert
//!
//! // Diff with key
//! let key = CandidateKey::new(vec!["id".to_string()]).unwrap();
//! let report = diff(&old_rel, &new_rel, Some(&key)).unwrap();
//!
//! assert_eq!(report.updated.len(), 1); // id: 1 changed A -> B
//! assert_eq!(report.inserted.cardinality(), 1); // id: 2 inserted
//! ```

use relvar_core::constraints::CandidateKey;
use relvar_core::values::{Relation, ScalarValue, Tuple};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// Errors that can occur during difference calculation.
#[derive(Debug, Error)]
pub enum DiffError {
    /// The relations have different types (headings).
    #[error("Relations have different types")]
    TypeMismatch,

    /// The provided key refers to attributes missing from the relation.
    #[error("Key attribute missing: {0}")]
    KeyAttributeMissing(String),
}

/// A report of the differences between two relations.
#[derive(Debug, Clone)]
pub struct DiffReport {
    /// Tuples present in the new relation but not in the old.
    pub inserted: Relation,
    /// Tuples present in the old relation but not in the new.
    pub deleted: Relation,
    /// Tuples that exist in both (by key) but have different values.
    /// Stores a vector of (old_tuple, new_tuple).
    pub updated: Vec<(Tuple, Tuple)>,
}

impl DiffReport {
    /// Returns true if there are no differences.
    pub fn is_empty(&self) -> bool {
        self.inserted.is_empty() && self.deleted.is_empty() && self.updated.is_empty()
    }
}

/// Calculates the difference between two relations.
///
/// # Arguments
///
/// * `old` - The base relation.
/// * `new` - The target relation.
/// * `key` - Optional candidate key to identify updates.
pub fn diff(
    old: &Relation,
    new: &Relation,
    key: Option<&CandidateKey>,
) -> Result<DiffReport, DiffError> {
    if old.relation_type() != new.relation_type() {
        return Err(DiffError::TypeMismatch);
    }

    // Initialize result containers
    let mut inserted = Relation::new(new.relation_type().clone());
    let mut deleted = Relation::new(old.relation_type().clone());
    let mut updated = Vec::new();

    if let Some(key_constraint) = key {
        // Key-based difference (detects Updates)
        let key_attrs = key_constraint.attributes();

        // Helper to extract key values
        let extract_key = |t: &Tuple| -> Result<Vec<ScalarValue>, DiffError> {
            let mut values = Vec::with_capacity(key_attrs.len());
            for attr in key_attrs {
                if let Some(val) = t.get(attr) {
                    values.push(val.clone());
                } else {
                    return Err(DiffError::KeyAttributeMissing(attr.clone()));
                }
            }
            Ok(values)
        };

        // Map keys to tuples for O(1) lookup
        let mut old_map = HashMap::new();
        for t in old.tuples() {
            let k = extract_key(t)?;
            old_map.insert(k, t);
        }

        let mut new_keys = HashSet::new();

        // Pass 1: Scan new relation
        for new_tuple in new.tuples() {
            let k = extract_key(new_tuple)?;
            new_keys.insert(k.clone());

            if let Some(old_tuple) = old_map.get(&k) {
                if *old_tuple != new_tuple {
                    // Key matches, content differs -> UPDATE
                    updated.push(((*old_tuple).clone(), new_tuple.clone()));
                }
                // Else: Unchanged, do nothing
            } else {
                // Key not in old -> INSERT
                inserted.insert(new_tuple.clone()).unwrap();
            }
        }

        // Pass 2: Scan old relation for DELETES
        for (k, old_tuple) in old_map {
            if !new_keys.contains(&k) {
                // Key not in new -> DELETE
                deleted.insert(old_tuple.clone()).unwrap();
            }
        }
    } else {
        // Set-based difference (INSERT/DELETE only)
        // Inserted = New - Old
        for t in new.tuples() {
            if !old.contains(t) {
                inserted.insert(t.clone()).unwrap();
            }
        }

        // Deleted = Old - New
        for t in old.tuples() {
            if !new.contains(t) {
                deleted.insert(t.clone()).unwrap();
            }
        }
    }

    Ok(DiffReport {
        inserted,
        deleted,
        updated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    fn setup_schema() -> RelationType {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("val", ScalarType::String);
        RelationType::new(heading)
    }

    #[test]
    fn test_diff_no_key_insert_delete() {
        let rt = setup_schema();
        let mut old = Relation::new(rt.clone());
        let mut new = Relation::new(rt);

        old.insert(tuple! { id: 1i64, val: "A" }).unwrap(); // Deleted
        old.insert(tuple! { id: 2i64, val: "B" }).unwrap(); // Kept

        new.insert(tuple! { id: 2i64, val: "B" }).unwrap(); // Kept
        new.insert(tuple! { id: 3i64, val: "C" }).unwrap(); // Inserted

        let report = diff(&old, &new, None).unwrap();

        assert_eq!(report.inserted.cardinality(), 1);
        assert!(report.inserted.contains(&tuple! { id: 3i64, val: "C" }));

        assert_eq!(report.deleted.cardinality(), 1);
        assert!(report.deleted.contains(&tuple! { id: 1i64, val: "A" }));

        assert!(report.updated.is_empty());
    }

    #[test]
    fn test_diff_with_key_update() {
        let rt = setup_schema();
        let mut old = Relation::new(rt.clone());
        let mut new = Relation::new(rt);

        old.insert(tuple! { id: 1i64, val: "A" }).unwrap(); // Will update

        new.insert(tuple! { id: 1i64, val: "B" }).unwrap(); // Updated value

        let key = CandidateKey::new(vec!["id".to_string()]).unwrap();
        let report = diff(&old, &new, Some(&key)).unwrap();

        assert!(report.inserted.is_empty());
        assert!(report.deleted.is_empty());
        assert_eq!(report.updated.len(), 1);

        let (old_t, new_t) = &report.updated[0];
        assert_eq!(old_t, &tuple! { id: 1i64, val: "A" });
        assert_eq!(new_t, &tuple! { id: 1i64, val: "B" });
    }

    #[test]
    fn test_diff_with_key_mixed() {
        let rt = setup_schema();
        let mut old = Relation::new(rt.clone());
        let mut new = Relation::new(rt);

        old.insert(tuple! { id: 1i64, val: "Keep" }).unwrap();
        old.insert(tuple! { id: 2i64, val: "Delete" }).unwrap();
        old.insert(tuple! { id: 3i64, val: "UpdateOld" }).unwrap();

        new.insert(tuple! { id: 1i64, val: "Keep" }).unwrap();
        new.insert(tuple! { id: 3i64, val: "UpdateNew" }).unwrap();
        new.insert(tuple! { id: 4i64, val: "Insert" }).unwrap();

        let key = CandidateKey::new(vec!["id".to_string()]).unwrap();
        let report = diff(&old, &new, Some(&key)).unwrap();

        assert_eq!(report.inserted.cardinality(), 1); // id: 4
        assert_eq!(report.deleted.cardinality(), 1); // id: 2
        assert_eq!(report.updated.len(), 1); // id: 3
    }
}
