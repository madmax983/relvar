//! Foreign key constraints for referential integrity.
//!
//! Foreign keys ensure that values in one relation (the referencing relation)
//! correspond to existing values in another relation (the referenced relation).
//! This maintains referential integrity across related relations.
//!
//! # Example
//!
//! ```
//! use relvar_core::constraints::{ForeignKey, ForeignKeyConstraints};
//!
//! // Employees.dept_id must reference an existing Departments.dept_id
//! let fk = ForeignKey::new(
//!     vec!["dept_id".to_string()],       // Foreign key attribute(s)
//!     "DEPARTMENTS".to_string(),          // Referenced relation name
//!     vec!["dept_id".to_string()],       // Referenced attribute(s)
//! ).unwrap();
//!
//! // Composite foreign key (multiple attributes)
//! let composite_fk = ForeignKey::new(
//!     vec!["project_id".to_string(), "task_id".to_string()],
//!     "TASKS".to_string(),
//!     vec!["proj_id".to_string(), "task_num".to_string()],
//! ).unwrap();
//! ```
//!
//! # Detecting Foreign Key Violations
//!
//! Foreign key constraints can detect when an insert would reference
//! a non-existent tuple (dangling reference):
//!
//! ```
//! use relvar_core::constraints::ForeignKey;
//! use relvar_core::types::{TupleType, RelationType, ScalarType};
//! use relvar_core::values::Relation;
//! use relvar_core::tuple;
//!
//! // Create departments relation (the referenced relation)
//! let dept_heading = TupleType::new()
//!     .with_attribute("dept_id", ScalarType::Int)
//!     .with_attribute("dept_name", ScalarType::String);
//!
//! let mut departments = Relation::new(RelationType::new(dept_heading));
//! departments.insert(tuple! { dept_id: 10i64, dept_name: "Engineering" }).unwrap();
//! departments.insert(tuple! { dept_id: 20i64, dept_name: "Sales" }).unwrap();
//!
//! // Define foreign key: employees.dept_id -> departments.dept_id
//! let fk = ForeignKey::new(
//!     vec!["dept_id".to_string()],
//!     "DEPARTMENTS".to_string(),
//!     vec!["dept_id".to_string()],
//! ).unwrap();
//!
//! // Valid insert - dept_id 10 exists in departments
//! let valid_employee = tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 };
//! assert!(!fk.would_violate_on_insert(&valid_employee, &departments).unwrap());
//!
//! // Invalid insert - dept_id 99 does NOT exist in departments!
//! let invalid_employee = tuple! { emp_id: 2i64, name: "Bob", dept_id: 99i64 };
//! assert!(fk.would_violate_on_insert(&invalid_employee, &departments).unwrap());
//! ```

use crate::values::{Relation, ScalarValue, Tuple};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

/// Errors that can occur with foreign key constraints.
#[derive(Debug, Error)]
pub enum ForeignKeyError {
    /// A foreign key value doesn't correspond to any tuple in the referenced relation.
    #[error("Foreign key constraint violation: referenced tuple not found")]
    ReferencedTupleNotFound,

    /// The foreign key references attributes that don't exist in the referencing relation.
    #[error("Foreign key attributes {0:?} do not exist in referencing relation")]
    InvalidForeignKeyAttributes(Vec<String>),

    /// The referenced attributes don't exist in the referenced relation.
    #[error("Referenced attributes {0:?} do not exist in referenced relation")]
    InvalidReferencedAttributes(Vec<String>),

    /// The number of foreign key attributes must match the number of referenced attributes.
    #[error("Foreign key and referenced attributes must have same count")]
    AttributeCountMismatch,

    /// A foreign key must have at least one attribute.
    #[error("Foreign key cannot be empty")]
    EmptyForeignKey,

    /// The tuple is missing an expected attribute.
    #[error("Missing attribute in tuple: {0}")]
    MissingAttribute(String),
}

/// A foreign key constraint ensuring referential integrity between relations.
///
/// A foreign key links attributes in one relation (referencing) to attributes
/// in another relation (referenced). Every combination of foreign key values
/// must exist as a corresponding combination in the referenced relation.
///
/// # Referential Integrity Rules
///
/// - **Insert**: Cannot insert a tuple with foreign key values that don't exist
///   in the referenced relation
/// - **Delete**: Cannot delete a tuple from the referenced relation if it's
///   referenced by tuples in the referencing relation
/// - **Update**: Updates must maintain the integrity of references
///
/// # Examples
///
/// ```
/// use relvar_core::constraints::ForeignKey;
///
/// // Simple foreign key: employees.dept_id -> departments.dept_id
/// let fk = ForeignKey::new(
///     vec!["dept_id".to_string()],
///     "DEPARTMENTS".to_string(),
///     vec!["dept_id".to_string()],
/// ).unwrap();
///
/// assert_eq!(fk.foreign_key_attributes(), &["dept_id"]);
/// assert_eq!(fk.referenced_relation_name(), "DEPARTMENTS");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForeignKey {
    /// Attributes in the referencing relation.
    foreign_key_attributes: Vec<String>,
    /// Name of the referenced relation.
    referenced_relation_name: String,
    /// Attributes in the referenced relation.
    referenced_attributes: Vec<String>,
}

impl ForeignKey {
    /// Create a new foreign key constraint
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn new(
        foreign_key_attributes: Vec<String>,
        referenced_relation_name: String,
        referenced_attributes: Vec<String>,
    ) -> Result<Self, ForeignKeyError> {
        if foreign_key_attributes.is_empty() {
            return Err(ForeignKeyError::EmptyForeignKey);
        }

        if foreign_key_attributes.len() != referenced_attributes.len() {
            return Err(ForeignKeyError::AttributeCountMismatch);
        }

        Ok(Self {
            foreign_key_attributes,
            referenced_relation_name,
            referenced_attributes,
        })
    }

    /// Get the foreign key attributes
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn foreign_key_attributes(&self) -> &[String] {
        &self.foreign_key_attributes
    }

    /// Get the referenced relation name
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn referenced_relation_name(&self) -> &str {
        &self.referenced_relation_name
    }

    /// Get the referenced attributes
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn referenced_attributes(&self) -> &[String] {
        &self.referenced_attributes
    }

    /// Check if this foreign key is satisfied
    /// (all foreign key values exist in the referenced relation)
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn is_satisfied_by(
        &self,
        referencing_relation: &Relation,
        referenced_relation: &Relation,
    ) -> Result<bool, ForeignKeyError> {
        // Verify foreign key attributes exist in referencing relation
        for attr in &self.foreign_key_attributes {
            if !referencing_relation.relation_type().has_attribute(attr) {
                return Err(ForeignKeyError::InvalidForeignKeyAttributes(
                    self.foreign_key_attributes.clone(),
                ));
            }
        }

        // Verify referenced attributes exist in referenced relation
        for attr in &self.referenced_attributes {
            if !referenced_relation.relation_type().has_attribute(attr) {
                return Err(ForeignKeyError::InvalidReferencedAttributes(
                    self.referenced_attributes.clone(),
                ));
            }
        }

        // Extract referenced keys into a HashSet for O(1) lookups
        let referenced_keys = extract_keys(referenced_relation, &self.referenced_attributes);

        // Pre-allocate buffer for key construction to avoid repeated allocations
        let mut key_buffer = Vec::with_capacity(self.foreign_key_attributes.len());

        // Check each tuple in referencing relation
        for tuple in referencing_relation.tuples() {
            key_buffer.clear();
            for attr in &self.foreign_key_attributes {
                // We know attribute exists because of check above
                key_buffer.push(
                    tuple
                        .get(attr)
                        .ok_or_else(|| ForeignKeyError::MissingAttribute(attr.clone()))?,
                );
            }

            if !referenced_keys.contains(&key_buffer) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Check if inserting a tuple would violate this foreign key
    /// Check if inserting a tuple would violate this foreign key
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn would_violate_on_insert(
        &self,
        new_tuple: &Tuple,
        referenced_relation: &Relation,
    ) -> Result<bool, ForeignKeyError> {
        // PERF: By hoisting the lookups for `new_tuple` outside the loop, we eliminate
        // the inner allocation of intermediate `Vec` inside the loop for every tuple in
        // `referenced_relation`. This makes `would_violate_on_insert` significantly faster.
        let mut new_values = Vec::with_capacity(self.foreign_key_attributes.len());
        for attr in &self.foreign_key_attributes {
            let val = new_tuple
                .get(attr)
                .ok_or_else(|| ForeignKeyError::MissingAttribute(attr.clone()))?;
            new_values.push(val);
        }

        // Check if any tuple in referenced relation matches
        for referenced_tuple in referenced_relation.tuples() {
            let mut matches = true;
            for (i, ref_attr) in self.referenced_attributes.iter().enumerate() {
                let ref_val = referenced_tuple
                    .get(ref_attr)
                    .ok_or_else(|| ForeignKeyError::MissingAttribute(ref_attr.clone()))?;
                if new_values[i] != ref_val {
                    matches = false;
                    break;
                }
            }
            if matches {
                return Ok(false); // Found a match, so no violation
            }
        }

        Ok(true) // No match found, violation
    }

    /// Check if deleting a tuple from the referenced relation would violate this constraint
    /// Check if deleting a tuple from the referenced relation would violate this constraint
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn would_violate_on_delete(
        &self,
        tuple_to_delete: &Tuple,
        referencing_relation: &Relation,
    ) -> Result<bool, ForeignKeyError> {
        // PERF: By hoisting the lookups for `tuple_to_delete` outside the loop, we eliminate
        // the inner allocation of intermediate `Vec` inside the loop for every tuple in
        // `referencing_relation`. This makes `would_violate_on_delete` significantly faster.
        let mut ref_values = Vec::with_capacity(self.referenced_attributes.len());
        for attr in &self.referenced_attributes {
            let val = tuple_to_delete
                .get(attr)
                .ok_or_else(|| ForeignKeyError::MissingAttribute(attr.clone()))?;
            ref_values.push(val);
        }

        // Check if any tuple in referencing relation references this tuple
        for referencing_tuple in referencing_relation.tuples() {
            let mut matches = true;
            for (i, fk_attr) in self.foreign_key_attributes.iter().enumerate() {
                let fk_val = referencing_tuple
                    .get(fk_attr)
                    .ok_or_else(|| ForeignKeyError::MissingAttribute(fk_attr.clone()))?;
                if fk_val != ref_values[i] {
                    matches = false;
                    break;
                }
            }
            if matches {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

/// Helper to extract a set of attribute value combinations from a relation.
fn extract_keys<'a>(
    relation: &'a Relation,
    attributes: &[String],
) -> HashSet<Vec<&'a ScalarValue>> {
    relation
        .tuples()
        .map(|tuple| {
            attributes
                .iter()
                .map(|attr| {
                    tuple
                        .get(attr)
                        .expect("Attribute must exist in relation schema")
                })
                .collect()
        })
        .collect()
}

/// Collection of foreign key constraints for a relation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKeyConstraints {
    foreign_keys: Vec<ForeignKey>,
}

impl ForeignKeyConstraints {
    /// Create new empty foreign key constraints
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn new() -> Self {
        Self {
            foreign_keys: Vec::new(),
        }
    }

    /// Add a foreign key constraint
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn with_foreign_key(mut self, fk: ForeignKey) -> Self {
        self.foreign_keys.push(fk);
        self
    }

    /// Get all foreign keys
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn foreign_keys(&self) -> &[ForeignKey] {
        &self.foreign_keys
    }
}

impl Default for ForeignKeyConstraints {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[cfg(test)]
mod tests;
