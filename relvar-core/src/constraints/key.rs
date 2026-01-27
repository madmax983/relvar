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
//! use relvar::constraints::{PrimaryKey, CandidateKey, KeyConstraints};
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
//! use relvar::constraints::{PrimaryKey, KeyConstraints};
//! use relvar::types::{TupleType, RelationType, ScalarType};
//! use relvar::values::Relation;
//! use relvar::tuple;
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
/// # Example
///
/// ```
/// use relvar::constraints::CandidateKey;
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
    pub fn new(attributes: Vec<String>) -> Result<Self, KeyConstraintError> {
        if attributes.is_empty() {
            return Err(KeyConstraintError::EmptyKey);
        }

        Ok(Self { attributes })
    }

    /// Get the key attributes
    pub fn attributes(&self) -> &[String] {
        &self.attributes
    }

    /// Check if this key is satisfied by a relation (all tuples have unique key values)
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
            let key_value = self.extract_key_value(tuple);
            if !key_values.insert(key_value) {
                // Duplicate found
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Check if a tuple would violate this key constraint when inserted into a relation
    pub fn would_violate(
        &self,
        relation: &Relation,
        new_tuple: &Tuple,
    ) -> Result<bool, KeyConstraintError> {
        let new_key_value = self.extract_key_value(new_tuple);

        for existing_tuple in relation.tuples() {
            let existing_key_value = self.extract_key_value(existing_tuple);
            if new_key_value == existing_key_value {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Extract key value from a tuple
    fn extract_key_value(&self, tuple: &Tuple) -> Vec<crate::values::ScalarValue> {
        self.attributes
            .iter()
            .map(|attr| tuple.get(attr).unwrap().clone())
            .collect()
    }
}

/// A primary key is a designated candidate key
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrimaryKey {
    key: CandidateKey,
}

impl PrimaryKey {
    /// Create a new primary key
    pub fn new(attributes: Vec<String>) -> Result<Self, KeyConstraintError> {
        Ok(Self {
            key: CandidateKey::new(attributes)?,
        })
    }

    /// Get the key attributes
    pub fn attributes(&self) -> &[String] {
        self.key.attributes()
    }

    /// Get the underlying candidate key
    pub fn as_candidate_key(&self) -> &CandidateKey {
        &self.key
    }

    /// Check if this key is satisfied by a relation
    pub fn is_satisfied_by(&self, relation: &Relation) -> Result<bool, KeyConstraintError> {
        self.key.is_satisfied_by(relation)
    }

    /// Check if a tuple would violate this key constraint
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
    pub fn new() -> Self {
        Self {
            primary_key: None,
            candidate_keys: Vec::new(),
        }
    }

    /// Set the primary key
    pub fn with_primary_key(mut self, key: PrimaryKey) -> Self {
        self.primary_key = Some(key);
        self
    }

    /// Add a candidate key
    pub fn with_candidate_key(mut self, key: CandidateKey) -> Self {
        self.candidate_keys.push(key);
        self
    }

    /// Get the primary key
    pub fn primary_key(&self) -> Option<&PrimaryKey> {
        self.primary_key.as_ref()
    }

    /// Get all candidate keys
    pub fn candidate_keys(&self) -> &[CandidateKey] {
        &self.candidate_keys
    }

    /// Check if all constraints are satisfied
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
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    fn create_employee_relation() -> Relation {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String)
            .with_attribute("dept_id".to_string(), ScalarType::Int);

        Relation::new(RelationType::new(heading))
    }

    #[test]
    fn test_candidate_key_creation() {
        let key = CandidateKey::new(vec!["emp_id".to_string()]).unwrap();
        assert_eq!(key.attributes(), &["emp_id"]);
    }

    #[test]
    fn test_empty_key_rejected() {
        let result = CandidateKey::new(vec![]);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), KeyConstraintError::EmptyKey));
    }

    #[test]
    fn test_key_uniqueness_satisfied() {
        let mut relation = create_employee_relation();
        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
            .unwrap();

        let key = CandidateKey::new(vec!["emp_id".to_string()]).unwrap();
        assert!(key.is_satisfied_by(&relation).unwrap());
    }

    #[test]
    fn test_key_uniqueness_violated() {
        let mut relation = create_employee_relation();
        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 1i64, name: "Bob", dept_id: 20i64 })
            .unwrap();

        let key = CandidateKey::new(vec!["emp_id".to_string()]).unwrap();
        assert!(!key.is_satisfied_by(&relation).unwrap());
    }

    #[test]
    fn test_composite_key() {
        let mut relation = create_employee_relation();
        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 1i64, name: "Bob", dept_id: 20i64 })
            .unwrap();

        let key = CandidateKey::new(vec!["emp_id".to_string(), "dept_id".to_string()]).unwrap();
        assert!(key.is_satisfied_by(&relation).unwrap());
    }

    #[test]
    fn test_would_violate_on_insert() {
        let mut relation = create_employee_relation();
        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();

        let key = CandidateKey::new(vec!["emp_id".to_string()]).unwrap();

        let new_tuple = tuple! { emp_id: 1i64, name: "Bob", dept_id: 20i64 };
        assert!(key.would_violate(&relation, &new_tuple).unwrap());

        let new_tuple2 = tuple! { emp_id: 2i64, name: "Charlie", dept_id: 30i64 };
        assert!(!key.would_violate(&relation, &new_tuple2).unwrap());
    }

    #[test]
    fn test_primary_key() {
        let pk = PrimaryKey::new(vec!["emp_id".to_string()]).unwrap();
        assert_eq!(pk.attributes(), &["emp_id"]);

        let mut relation = create_employee_relation();
        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
            .unwrap();

        assert!(pk.is_satisfied_by(&relation).unwrap());
    }

    #[test]
    fn test_key_constraints_with_primary_key() {
        let pk = PrimaryKey::new(vec!["emp_id".to_string()]).unwrap();
        let constraints = KeyConstraints::new().with_primary_key(pk);

        let mut relation = create_employee_relation();
        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
            .unwrap();

        assert!(constraints.are_satisfied_by(&relation).unwrap());
    }

    #[test]
    fn test_key_constraints_violation_detection() {
        let pk = PrimaryKey::new(vec!["emp_id".to_string()]).unwrap();
        let constraints = KeyConstraints::new().with_primary_key(pk);

        let mut relation = create_employee_relation();
        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();

        let new_tuple = tuple! { emp_id: 1i64, name: "Bob", dept_id: 20i64 };
        let violation = constraints
            .would_violate_on_insert(&relation, &new_tuple)
            .unwrap();

        assert!(violation.is_some());
        assert_eq!(violation.unwrap(), vec!["emp_id"]);
    }

    #[test]
    fn test_multiple_candidate_keys() {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("email".to_string(), ScalarType::String)
            .with_attribute("name".to_string(), ScalarType::String);

        let mut relation = Relation::new(RelationType::new(heading));
        relation
            .insert(tuple! { emp_id: 1i64, email: "alice@example.com", name: "Alice" })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, email: "bob@example.com", name: "Bob" })
            .unwrap();

        let pk = PrimaryKey::new(vec!["emp_id".to_string()]).unwrap();
        let ck = CandidateKey::new(vec!["email".to_string()]).unwrap();

        let constraints = KeyConstraints::new()
            .with_primary_key(pk)
            .with_candidate_key(ck);

        assert!(constraints.are_satisfied_by(&relation).unwrap());

        // Try to insert with duplicate email
        let new_tuple = tuple! { emp_id: 3i64, email: "alice@example.com", name: "Alice2" };
        let violation = constraints
            .would_violate_on_insert(&relation, &new_tuple)
            .unwrap();
        assert!(violation.is_some());
    }

    #[test]
    fn test_invalid_key_attributes() {
        let key = CandidateKey::new(vec!["nonexistent".to_string()]).unwrap();
        let relation = create_employee_relation();

        let result = key.is_satisfied_by(&relation);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            KeyConstraintError::InvalidKeyAttributes(_)
        ));
    }
}
