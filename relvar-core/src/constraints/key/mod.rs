//! Key constraints for ensuring tuple uniqueness.
//!
//! Key constraints define which attributes (or combinations of attributes)
//! must have unique values across all tuples in a relation.
//!
//! # TTM Compliance
//!
//! Per **Proscription 2** (No duplicate tuples), relations are true sets.
//! Key constraints enforce this at the schema level by defining which
//! attributes must be unique across all tuples.
//!
//! # Key Types
//!
//! - **Candidate Key** - Any minimal set of attributes that uniquely identifies tuples
//! - **Primary Key** - A designated candidate key chosen as the main identifier
//!
//! # Example
//!
//! ```
//! use relvar_core::constraints::{PrimaryKey, CandidateKey, KeyConstraints};
//!
//! // Single-attribute primary key
//! let pk = PrimaryKey::new(vec!["emp_id".to_string()]).unwrap();
//!
//! // Composite candidate key (email must also be unique)
//! let ck = CandidateKey::new(vec!["email".to_string()]).unwrap();
//!
//! // Create key constraints
//! let constraints = KeyConstraints::new()
//!     .with_primary_key(pk)
//!     .with_candidate_key(ck);
//! ```
//!
//! # Detecting Key Violations
//!
//! Key constraints can detect when an insert would create duplicates:
//!
//! ```
//! use relvar_core::constraints::{PrimaryKey, KeyConstraints};
//! use relvar_core::types::{TupleType, RelationType, ScalarType};
//! use relvar_core::values::Relation;
//! use relvar_core::tuple;
//!
//! // Create a relation with employee data
//! let heading = TupleType::new()
//!     .with_attribute("emp_id", ScalarType::Int)
//!     .with_attribute("name", ScalarType::String);
//!
//! let mut relation = Relation::new(RelationType::new(heading));
//! relation.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
//! relation.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
//!
//! // Define primary key on emp_id
//! let pk = PrimaryKey::new(vec!["emp_id".to_string()]).unwrap();
//! let constraints = KeyConstraints::new().with_primary_key(pk);
//!
//! // Check if current data satisfies the key constraint
//! assert!(constraints.are_satisfied_by(&relation).unwrap());
//!
//! // Check if inserting a duplicate would violate the constraint
//! let duplicate = tuple! { emp_id: 1i64, name: "Charlie" };  // emp_id=1 already exists!
//! let violation = constraints.would_violate_on_insert(&relation, &duplicate).unwrap();
//!
//! // Violation detected - returns the violated key attributes
//! assert!(violation.is_some());
//! assert_eq!(violation.unwrap(), vec!["emp_id"]);
//! ```

use crate::values::{Relation, Tuple};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

/// Errors that can occur with key constraints.
#[derive(Debug, Error)]
pub enum KeyConstraintError {
    /// A tuple would create a duplicate key value.
    #[error("Key constraint violation: duplicate key value for attributes {0:?}")]
    DuplicateKey(Vec<String>),

    /// The key references attributes that don't exist in the relation.
    #[error("Key attributes {0:?} do not exist in relation")]
    InvalidKeyAttributes(Vec<String>),

    /// The tuple is missing a required key attribute.
    #[error("Tuple is missing key attribute: {0}")]
    TupleMissingAttribute(String),

    /// A key must have at least one attribute.
    #[error("Key cannot be empty")]
    EmptyKey,
}

/// A candidate key - a minimal set of attributes that uniquely identifies tuples.
///
/// A candidate key has two properties:
/// 1. **Uniqueness** - No two tuples can have the same values for all key attributes
/// 2. **Minimality** - No proper subset of the attributes has the uniqueness property
///
/// # Examples
///
/// ```
/// use relvar_core::constraints::CandidateKey;
///
/// // Single-attribute key
/// let emp_key = CandidateKey::new(vec!["emp_id".to_string()]).unwrap();
///
/// // Composite key (multiple attributes together form the key)
/// let composite_key = CandidateKey::new(vec![
///     "dept_id".to_string(),
///     "emp_num".to_string(),
/// ]).unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CandidateKey {
    /// The attribute names that compose this key.
    attributes: Vec<String>,
}

impl CandidateKey {
    /// Create a new candidate key
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn new(attributes: Vec<String>) -> Result<Self, KeyConstraintError> {
        if attributes.is_empty() {
            return Err(KeyConstraintError::EmptyKey);
        }

        Ok(Self { attributes })
    }

    /// Get the key attributes
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn attributes(&self) -> &[String] {
        &self.attributes
    }

    /// Check if this key is satisfied by a relation (all tuples have unique key values)
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn is_satisfied_by(&self, relation: &Relation) -> Result<bool, KeyConstraintError> {
        // Verify all key attributes exist
        for attr in &self.attributes {
            if !relation.relation_type().has_attribute(attr) {
                return Err(KeyConstraintError::InvalidKeyAttributes(
                    self.attributes.clone(),
                ));
            }
        }

        // Collect all key values
        let mut key_values = HashSet::new();

        for tuple in relation.tuples() {
            let key_value = self.extract_key_value(tuple)?;
            if !key_values.insert(key_value) {
                // Duplicate found
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Check if a tuple would violate this key constraint when inserted into a relation
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn would_violate(
        &self,
        relation: &Relation,
        new_tuple: &Tuple,
    ) -> Result<bool, KeyConstraintError> {
        let new_key_value = self.extract_key_value(new_tuple)?;

        for existing_tuple in relation.tuples() {
            let existing_key_value = self.extract_key_value(existing_tuple)?;
            if new_key_value == existing_key_value {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Extract key value from a tuple
    fn extract_key_value(
        &self,
        tuple: &Tuple,
    ) -> Result<Vec<crate::values::ScalarValue>, KeyConstraintError> {
        let mut values = Vec::with_capacity(self.attributes.len());
        for attr in &self.attributes {
            if let Some(val) = tuple.get(attr) {
                values.push(val.clone());
            } else {
                return Err(KeyConstraintError::TupleMissingAttribute(attr.clone()));
            }
        }
        Ok(values)
    }
}

/// A primary key is a designated candidate key
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrimaryKey {
    key: CandidateKey,
}

impl PrimaryKey {
    /// Create a new primary key
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn new(attributes: Vec<String>) -> Result<Self, KeyConstraintError> {
        Ok(Self {
            key: CandidateKey::new(attributes)?,
        })
    }

    /// Get the key attributes
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn attributes(&self) -> &[String] {
        self.key.attributes()
    }

    /// Get the underlying candidate key
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn as_candidate_key(&self) -> &CandidateKey {
        &self.key
    }

    /// Check if this key is satisfied by a relation
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn is_satisfied_by(&self, relation: &Relation) -> Result<bool, KeyConstraintError> {
        self.key.is_satisfied_by(relation)
    }

    /// Check if a tuple would violate this key constraint
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn would_violate(
        &self,
        relation: &Relation,
        new_tuple: &Tuple,
    ) -> Result<bool, KeyConstraintError> {
        self.key.would_violate(relation, new_tuple)
    }
}

/// Key constraints for a relation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyConstraints {
    primary_key: Option<PrimaryKey>,
    candidate_keys: Vec<CandidateKey>,
}

impl KeyConstraints {
    /// Create new empty key constraints
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn new() -> Self {
        Self {
            primary_key: None,
            candidate_keys: Vec::new(),
        }
    }

    /// Set the primary key
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn with_primary_key(mut self, key: PrimaryKey) -> Self {
        self.primary_key = Some(key);
        self
    }

    /// Add a candidate key
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn with_candidate_key(mut self, key: CandidateKey) -> Self {
        self.candidate_keys.push(key);
        self
    }

    /// Get the primary key
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn primary_key(&self) -> Option<&PrimaryKey> {
        self.primary_key.as_ref()
    }

    /// Get all candidate keys
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn candidate_keys(&self) -> &[CandidateKey] {
        &self.candidate_keys
    }

    /// Check if all constraints are satisfied
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn are_satisfied_by(&self, relation: &Relation) -> Result<bool, KeyConstraintError> {
        // Check primary key
        if let Some(pk) = &self.primary_key
            && !pk.is_satisfied_by(relation)?
        {
            return Ok(false);
        }

        // Check candidate keys
        for ck in &self.candidate_keys {
            if !ck.is_satisfied_by(relation)? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Check if inserting a tuple would violate any key constraints
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn would_violate_on_insert(
        &self,
        relation: &Relation,
        new_tuple: &Tuple,
    ) -> Result<Option<Vec<String>>, KeyConstraintError> {
        // Check primary key
        if let Some(pk) = &self.primary_key
            && pk.would_violate(relation, new_tuple)?
        {
            return Ok(Some(pk.attributes().to_vec()));
        }

        // Check candidate keys
        for ck in &self.candidate_keys {
            if ck.would_violate(relation, new_tuple)? {
                return Ok(Some(ck.attributes().to_vec()));
            }
        }

        Ok(None)
    }
}

impl Default for KeyConstraints {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[cfg(test)]
mod tests;
