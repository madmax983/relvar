//! Foreign key constraints for referential integrity.
//!
//! Foreign keys ensure that values in one relation (the referencing relation)
//! correspond to existing values in another relation (the referenced relation).
//! This maintains referential integrity across related relations.
//!
//! # Example
//!
//! ```
//! use relvar::constraints::{ForeignKey, ForeignKeyConstraints};
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
//! use relvar::constraints::ForeignKey;
//! use relvar::types::{TupleType, RelationType, ScalarType};
//! use relvar::values::Relation;
//! use relvar::tuple;
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

use crate::values::{Relation, Tuple};
use serde::{Deserialize, Serialize};
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
/// # Example
///
/// ```
/// use relvar::constraints::ForeignKey;
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
    pub fn foreign_key_attributes(&self) -> &[String] {
        &self.foreign_key_attributes
    }

    /// Get the referenced relation name
    pub fn referenced_relation_name(&self) -> &str {
        &self.referenced_relation_name
    }

    /// Get the referenced attributes
    pub fn referenced_attributes(&self) -> &[String] {
        &self.referenced_attributes
    }

    /// Check if this foreign key is satisfied
    /// (all foreign key values exist in the referenced relation)
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

        // Check each tuple in referencing relation
        for tuple in referencing_relation.tuples() {
            if !self.tuple_references_exist(tuple, referenced_relation) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Check if inserting a tuple would violate this foreign key
    pub fn would_violate_on_insert(
        &self,
        new_tuple: &Tuple,
        referenced_relation: &Relation,
    ) -> Result<bool, ForeignKeyError> {
        Ok(!self.tuple_references_exist(new_tuple, referenced_relation))
    }

    /// Check if a tuple's foreign key values exist in the referenced relation
    fn tuple_references_exist(&self, tuple: &Tuple, referenced_relation: &Relation) -> bool {
        let foreign_key_values: Vec<_> = self
            .foreign_key_attributes
            .iter()
            .map(|attr| tuple.get(attr).unwrap())
            .collect();

        // Check if any tuple in referenced relation matches
        for referenced_tuple in referenced_relation.tuples() {
            let referenced_values: Vec<_> = self
                .referenced_attributes
                .iter()
                .map(|attr| referenced_tuple.get(attr).unwrap())
                .collect();

            if foreign_key_values == referenced_values {
                return true;
            }
        }

        false
    }

    /// Check if deleting a tuple from the referenced relation would violate this constraint
    pub fn would_violate_on_delete(
        &self,
        tuple_to_delete: &Tuple,
        referencing_relation: &Relation,
    ) -> Result<bool, ForeignKeyError> {
        let referenced_values: Vec<_> = self
            .referenced_attributes
            .iter()
            .map(|attr| tuple_to_delete.get(attr).unwrap())
            .collect();

        // Check if any tuple in referencing relation references this tuple
        for referencing_tuple in referencing_relation.tuples() {
            let foreign_key_values: Vec<_> = self
                .foreign_key_attributes
                .iter()
                .map(|attr| referencing_tuple.get(attr).unwrap())
                .collect();

            if foreign_key_values == referenced_values {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

/// Collection of foreign key constraints for a relation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKeyConstraints {
    foreign_keys: Vec<ForeignKey>,
}

impl ForeignKeyConstraints {
    /// Create new empty foreign key constraints
    pub fn new() -> Self {
        Self {
            foreign_keys: Vec::new(),
        }
    }

    /// Add a foreign key constraint
    pub fn with_foreign_key(mut self, fk: ForeignKey) -> Self {
        self.foreign_keys.push(fk);
        self
    }

    /// Get all foreign keys
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
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    fn create_department_relation() -> Relation {
        let heading = TupleType::new()
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("dept_name".to_string(), ScalarType::String);

        let mut relation = Relation::new(RelationType::new(heading));
        relation
            .insert(tuple! { dept_id: 10i64, dept_name: "Engineering" })
            .unwrap();
        relation
            .insert(tuple! { dept_id: 20i64, dept_name: "Sales" })
            .unwrap();
        relation
    }

    fn create_employee_relation() -> Relation {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String)
            .with_attribute("dept_id".to_string(), ScalarType::Int);

        Relation::new(RelationType::new(heading))
    }

    #[test]
    fn test_foreign_key_creation() {
        let fk = ForeignKey::new(
            vec!["dept_id".to_string()],
            "DEPT".to_string(),
            vec!["dept_id".to_string()],
        )
        .unwrap();

        assert_eq!(fk.foreign_key_attributes(), &["dept_id"]);
        assert_eq!(fk.referenced_relation_name(), "DEPT");
        assert_eq!(fk.referenced_attributes(), &["dept_id"]);
    }

    #[test]
    fn test_empty_foreign_key_rejected() {
        let result = ForeignKey::new(vec![], "DEPT".to_string(), vec![]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ForeignKeyError::EmptyForeignKey
        ));
    }

    #[test]
    fn test_attribute_count_mismatch() {
        let result = ForeignKey::new(
            vec!["dept_id".to_string()],
            "DEPT".to_string(),
            vec!["dept_id".to_string(), "dept_name".to_string()],
        );

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ForeignKeyError::AttributeCountMismatch
        ));
    }

    #[test]
    fn test_foreign_key_satisfied() {
        let departments = create_department_relation();
        let mut employees = create_employee_relation();

        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        employees
            .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
            .unwrap();

        let fk = ForeignKey::new(
            vec!["dept_id".to_string()],
            "DEPT".to_string(),
            vec!["dept_id".to_string()],
        )
        .unwrap();

        assert!(fk.is_satisfied_by(&employees, &departments).unwrap());
    }

    #[test]
    fn test_foreign_key_violated() {
        let departments = create_department_relation();
        let mut employees = create_employee_relation();

        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        employees
            .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 99i64 })
            .unwrap(); // dept_id 99 doesn't exist

        let fk = ForeignKey::new(
            vec!["dept_id".to_string()],
            "DEPT".to_string(),
            vec!["dept_id".to_string()],
        )
        .unwrap();

        assert!(!fk.is_satisfied_by(&employees, &departments).unwrap());
    }

    #[test]
    fn test_would_violate_on_insert() {
        let departments = create_department_relation();

        let fk = ForeignKey::new(
            vec!["dept_id".to_string()],
            "DEPT".to_string(),
            vec!["dept_id".to_string()],
        )
        .unwrap();

        // Valid insert
        let valid_tuple = tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 };
        assert!(
            !fk.would_violate_on_insert(&valid_tuple, &departments)
                .unwrap()
        );

        // Invalid insert
        let invalid_tuple = tuple! { emp_id: 2i64, name: "Bob", dept_id: 99i64 };
        assert!(
            fk.would_violate_on_insert(&invalid_tuple, &departments)
                .unwrap()
        );
    }

    #[test]
    fn test_would_violate_on_delete() {
        let departments = create_department_relation();
        let mut employees = create_employee_relation();

        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();

        let fk = ForeignKey::new(
            vec!["dept_id".to_string()],
            "DEPT".to_string(),
            vec!["dept_id".to_string()],
        )
        .unwrap();

        // Get department tuple
        let dept_tuple = departments
            .tuples()
            .find(|t| t.get_typed::<i64>("dept_id").unwrap() == 10)
            .unwrap();

        // Deleting this department would violate FK
        assert!(fk.would_violate_on_delete(dept_tuple, &employees).unwrap());

        // Get unused department
        let unused_dept = departments
            .tuples()
            .find(|t| t.get_typed::<i64>("dept_id").unwrap() == 20)
            .unwrap();

        // Deleting this department wouldn't violate FK
        assert!(!fk.would_violate_on_delete(unused_dept, &employees).unwrap());
    }

    #[test]
    fn test_composite_foreign_key() {
        let heading = TupleType::new()
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("location".to_string(), ScalarType::String)
            .with_attribute("dept_name".to_string(), ScalarType::String);

        let mut departments = Relation::new(RelationType::new(heading));
        departments
            .insert(tuple! { dept_id: 10i64, location: "NYC", dept_name: "Engineering" })
            .unwrap();

        let emp_heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("dept_location".to_string(), ScalarType::String);

        let mut employees = Relation::new(RelationType::new(emp_heading));
        employees
            .insert(tuple! { emp_id: 1i64, dept_id: 10i64, dept_location: "NYC" })
            .unwrap();

        let fk = ForeignKey::new(
            vec!["dept_id".to_string(), "dept_location".to_string()],
            "DEPT".to_string(),
            vec!["dept_id".to_string(), "location".to_string()],
        )
        .unwrap();

        assert!(fk.is_satisfied_by(&employees, &departments).unwrap());
    }

    #[test]
    fn test_foreign_key_constraints_collection() {
        let fk1 = ForeignKey::new(
            vec!["dept_id".to_string()],
            "DEPT".to_string(),
            vec!["dept_id".to_string()],
        )
        .unwrap();

        let fk2 = ForeignKey::new(
            vec!["manager_id".to_string()],
            "EMP".to_string(),
            vec!["emp_id".to_string()],
        )
        .unwrap();

        let constraints = ForeignKeyConstraints::new()
            .with_foreign_key(fk1)
            .with_foreign_key(fk2);

        assert_eq!(constraints.foreign_keys().len(), 2);
    }

    #[test]
    fn test_invalid_foreign_key_attributes() {
        let departments = create_department_relation();
        let employees = create_employee_relation();

        let fk = ForeignKey::new(
            vec!["nonexistent".to_string()],
            "DEPT".to_string(),
            vec!["dept_id".to_string()],
        )
        .unwrap();

        let result = fk.is_satisfied_by(&employees, &departments);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ForeignKeyError::InvalidForeignKeyAttributes(_)
        ));
    }

    #[test]
    fn test_invalid_referenced_attributes() {
        let departments = create_department_relation();
        let employees = create_employee_relation();

        let fk = ForeignKey::new(
            vec!["dept_id".to_string()],
            "DEPT".to_string(),
            vec!["nonexistent".to_string()],
        )
        .unwrap();

        let result = fk.is_satisfied_by(&employees, &departments);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ForeignKeyError::InvalidReferencedAttributes(_)
        ));
    }
}
