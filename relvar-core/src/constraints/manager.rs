//! Manages constraints for relations.
//!
//! This module provides the [`ConstraintManager`], which is responsible for
//! storing and validating all database constraints (keys, foreign keys, types, CHECKs).

use crate::constraints::{
    AttributeConstraints, CheckConstraintError, CheckConstraints, ForeignKeyConstraints,
    KeyConstraints,
};
use crate::storage_engine::{StorageEngine, StorageError};
use crate::values::relation::RelationError;
use crate::values::{Relation, Tuple};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during constraint management operations.
#[derive(Debug, Error)]
pub enum ConstraintManagerError {
    /// A storage error occurred.
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    /// An error occurred with a relation value.
    #[error("Relation error: {0}")]
    Relation(#[from] RelationError),

    /// The specified relation does not exist.
    #[error("Relation {0} not found")]
    RelationNotFound(String),

    /// The attribute does not exist.
    #[error("Attribute {0} not found in relation {1}")]
    AttributeNotFound(String, String),

    /// The tuple's type does not match the relation's heading.
    #[error("Tuple type does not match relation type")]
    TupleMismatch,

    /// Primary key constraint violation.
    #[error("Primary key violation")]
    PrimaryKeyViolation,

    /// Candidate key constraint violation.
    #[error("Candidate key violation")]
    CandidateKeyViolation,

    /// Foreign key constraint violation.
    #[error("Foreign key violation: {0}")]
    ForeignKeyViolation(String),

    /// Type constraint violation.
    #[error("Type constraint violation: {0}")]
    TypeConstraintViolation(String),

    /// CHECK constraint violation.
    #[error("CHECK constraint violation: {0}")]
    CheckConstraintViolation(#[from] CheckConstraintError),

    /// Transaction error.
    #[error("Transaction error: {0}")]
    TransactionError(String),
}

/// Manages integrity constraints for the database.
///
/// This struct handles storage and validation of:
/// - Key constraints (Primary and Candidate keys)
/// - Foreign key constraints
/// - Type constraints
/// - CHECK constraints
#[derive(Debug, Default)]
pub struct ConstraintManager {
    /// Key constraints (primary and candidate) per relation.
    key_constraints: HashMap<String, KeyConstraints>,
    /// Foreign key constraints per relation.
    foreign_key_constraints: HashMap<String, ForeignKeyConstraints>,
    /// Type constraints per relation, per attribute.
    type_constraints: HashMap<String, HashMap<String, AttributeConstraints>>,
    /// CHECK constraints (tuple-level predicates) per relation.
    check_constraints: HashMap<String, CheckConstraints>,
}

impl ConstraintManager {
    /// Create a new constraint manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Remove all constraints associated with a relation.
    pub fn remove_constraints_for_relation(&mut self, relation_name: &str) {
        self.key_constraints.remove(relation_name);
        self.foreign_key_constraints.remove(relation_name);
        self.type_constraints.remove(relation_name);
        self.check_constraints.remove(relation_name);
    }

    /// Set key constraints for a relation.
    pub fn set_key_constraints<E: StorageEngine>(
        &mut self,
        engine: &mut E,
        relation_name: &str,
        constraints: KeyConstraints,
    ) -> Result<(), ConstraintManagerError> {
        if !engine.relation_exists(relation_name) {
            return Err(ConstraintManagerError::RelationNotFound(
                relation_name.to_string(),
            ));
        }

        // Validate constraints against existing data
        let relation = engine.load_relation(relation_name)?;
        self.validate_key_constraints_bulk(&relation, &constraints)?;

        self.key_constraints
            .insert(relation_name.to_string(), constraints);
        Ok(())
    }

    /// Set foreign key constraints for a relation.
    pub fn set_foreign_key_constraints<E: StorageEngine>(
        &mut self,
        engine: &mut E,
        relation_name: &str,
        constraints: ForeignKeyConstraints,
    ) -> Result<(), ConstraintManagerError> {
        if !engine.relation_exists(relation_name) {
            return Err(ConstraintManagerError::RelationNotFound(
                relation_name.to_string(),
            ));
        }

        // Validate constraints against existing data
        let relation = engine.load_relation(relation_name)?;
        self.validate_foreign_keys_against_existing_data(engine, &relation, &constraints)?;

        self.foreign_key_constraints
            .insert(relation_name.to_string(), constraints);
        Ok(())
    }

    fn validate_foreign_keys_against_existing_data<E: StorageEngine>(
        &self,
        engine: &mut E,
        relation: &Relation,
        constraints: &ForeignKeyConstraints,
    ) -> Result<(), ConstraintManagerError> {
        for fk in constraints.foreign_keys() {
            let referenced_relation = engine.load_relation(fk.referenced_relation_name())?;

            if !fk
                .is_satisfied_by(relation, &referenced_relation)
                .map_err(|e| ConstraintManagerError::ForeignKeyViolation(e.to_string()))?
            {
                return Err(ConstraintManagerError::ForeignKeyViolation(
                    "Existing tuple violates foreign key".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Set type constraints for an attribute.
    pub fn set_type_constraints<E: StorageEngine>(
        &mut self,
        engine: &mut E,
        relation_name: &str,
        attribute_name: &str,
        constraints: AttributeConstraints,
    ) -> Result<(), ConstraintManagerError> {
        let metadata = engine.get_relation_metadata(relation_name)?;

        if !metadata.relation_type.has_attribute(attribute_name) {
            return Err(ConstraintManagerError::AttributeNotFound(
                attribute_name.to_string(),
                relation_name.to_string(),
            ));
        }

        // Validate constraints against existing data
        let relation = engine.load_relation(relation_name)?;
        self.validate_type_constraints_against_existing_data(
            &relation,
            attribute_name,
            &constraints,
        )?;

        self.type_constraints
            .entry(relation_name.to_string())
            .or_default()
            .insert(attribute_name.to_string(), constraints);
        Ok(())
    }

    fn validate_type_constraints_against_existing_data(
        &self,
        relation: &Relation,
        attribute_name: &str,
        constraints: &AttributeConstraints,
    ) -> Result<(), ConstraintManagerError> {
        for tuple in relation.tuples() {
            if let Some(value) = tuple.get(attribute_name)
                && !constraints
                    .is_satisfied_by(value)
                    .map_err(|e| ConstraintManagerError::TypeConstraintViolation(e.to_string()))?
            {
                return Err(ConstraintManagerError::TypeConstraintViolation(
                    "Existing value violates constraint".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Set CHECK constraints for a relation.
    pub fn set_check_constraints<E: StorageEngine>(
        &mut self,
        engine: &mut E,
        relation_name: &str,
        constraints: CheckConstraints,
    ) -> Result<(), ConstraintManagerError> {
        if !engine.relation_exists(relation_name) {
            return Err(ConstraintManagerError::RelationNotFound(
                relation_name.to_string(),
            ));
        }

        self.validate_check_constraint_attributes(engine, relation_name, &constraints)?;

        // Validate constraints against existing data
        let relation = engine.load_relation(relation_name)?;
        self.validate_check_constraints_against_existing_data(&relation, &constraints)?;

        self.check_constraints
            .insert(relation_name.to_string(), constraints);
        Ok(())
    }

    fn validate_check_constraint_attributes<E: StorageEngine>(
        &self,
        engine: &E,
        relation_name: &str,
        constraints: &CheckConstraints,
    ) -> Result<(), ConstraintManagerError> {
        // Validate that all referenced attributes exist in the relation
        let metadata = engine.get_relation_metadata(relation_name)?;
        let heading = metadata.relation_type.heading();

        for attr in constraints.referenced_attributes() {
            if !heading.has_attribute(&attr) {
                return Err(ConstraintManagerError::AttributeNotFound(
                    attr,
                    relation_name.to_string(),
                ));
            }
        }
        Ok(())
    }

    fn validate_check_constraints_against_existing_data(
        &self,
        relation: &Relation,
        constraints: &CheckConstraints,
    ) -> Result<(), ConstraintManagerError> {
        for tuple in relation.tuples() {
            constraints.are_all_satisfied_by(tuple)?;
        }
        Ok(())
    }

    /// Validate that a tuple matches the relation's heading.
    pub fn validate_tuple_type<E: StorageEngine>(
        &self,
        engine: &E,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), ConstraintManagerError> {
        let metadata = engine.get_relation_metadata(relation_name)?;
        if !tuple.conforms_to(metadata.relation_type.tuple_type()) {
            Err(ConstraintManagerError::TupleMismatch)
        } else {
            Ok(())
        }
    }

    /// Validate that a tuple satisfies all type constraints.
    pub fn validate_type_constraints(
        &self,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), ConstraintManagerError> {
        if let Some(attr_constraints) = self.type_constraints.get(relation_name) {
            for (attr_name, constraints) in attr_constraints {
                if let Some(value) = tuple.get(attr_name)
                    && !constraints.is_satisfied_by(value).map_err(|e| {
                        ConstraintManagerError::TypeConstraintViolation(e.to_string())
                    })?
                {
                    return Err(ConstraintManagerError::TypeConstraintViolation(format!(
                        "Attribute {} violates constraint",
                        attr_name
                    )));
                }
            }
        }
        Ok(())
    }

    /// Validate that a tuple satisfies all CHECK constraints.
    pub fn validate_check_constraints(
        &self,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), ConstraintManagerError> {
        if let Some(check_constraints) = self.check_constraints.get(relation_name) {
            check_constraints.are_all_satisfied_by(tuple)?;
        }
        Ok(())
    }

    /// Validate key constraints for a single tuple against the current relation state.
    pub fn validate_key_constraints_single_tuple(
        &self,
        relation_name: &str,
        tuple: &Tuple,
        current_relation: &Relation,
    ) -> Result<(), ConstraintManagerError> {
        if let Some(key_constraints) = self.key_constraints.get(relation_name) {
            if let Some(pk) = key_constraints.primary_key()
                && pk
                    .would_violate(current_relation, tuple)
                    .map_err(|e| ConstraintManagerError::TransactionError(e.to_string()))?
            {
                return Err(ConstraintManagerError::PrimaryKeyViolation);
            }

            for ck in key_constraints.candidate_keys() {
                if ck
                    .would_violate(current_relation, tuple)
                    .map_err(|e| ConstraintManagerError::TransactionError(e.to_string()))?
                {
                    return Err(ConstraintManagerError::CandidateKeyViolation);
                }
            }
        }
        Ok(())
    }

    /// Validate constraints that depend only on the tuple's content and foreign keys.
    ///
    /// This includes Type constraints, CHECK constraints, and Foreign Key constraints.
    pub fn validate_tuple_content_constraints<E: StorageEngine>(
        &self,
        engine: &mut E,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), ConstraintManagerError> {
        self.validate_type_constraints(relation_name, tuple)?;
        self.validate_check_constraints(relation_name, tuple)?;
        self.validate_foreign_keys_single_tuple(engine, relation_name, tuple)?;
        Ok(())
    }

    /// Validate foreign key constraints for a single tuple.
    ///
    /// Checks that values in the tuple exist in the referenced relations.
    pub fn validate_foreign_keys_single_tuple<E: StorageEngine>(
        &self,
        engine: &mut E,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), ConstraintManagerError> {
        let fks_to_check = self
            .foreign_key_constraints
            .get(relation_name)
            .map(|c| c.foreign_keys())
            .unwrap_or_default();

        for fk in fks_to_check {
            let referenced_relation = engine.load_relation(fk.referenced_relation_name())?;

            if fk
                .would_violate_on_insert(tuple, &referenced_relation)
                .map_err(|e| ConstraintManagerError::ForeignKeyViolation(e.to_string()))?
            {
                return Err(ConstraintManagerError::ForeignKeyViolation(
                    "Foreign key constraint violated".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Validate key constraints against a full relation.
    pub fn validate_key_constraints_bulk(
        &self,
        relation: &Relation,
        constraints: &KeyConstraints,
    ) -> Result<(), ConstraintManagerError> {
        if let Some(pk) = constraints.primary_key()
            && !pk
                .is_satisfied_by(relation)
                .map_err(|e| ConstraintManagerError::TransactionError(e.to_string()))?
        {
            return Err(ConstraintManagerError::PrimaryKeyViolation);
        }

        for ck in constraints.candidate_keys() {
            if !ck
                .is_satisfied_by(relation)
                .map_err(|e| ConstraintManagerError::TransactionError(e.to_string()))?
            {
                return Err(ConstraintManagerError::CandidateKeyViolation);
            }
        }
        Ok(())
    }

    /// Get the key constraints for a relation.
    pub fn get_key_constraints(&self, relation_name: &str) -> Option<&KeyConstraints> {
        self.key_constraints.get(relation_name)
    }

    /// Get the foreign key constraints for a relation.
    pub fn get_foreign_key_constraints(
        &self,
        relation_name: &str,
    ) -> Option<&ForeignKeyConstraints> {
        self.foreign_key_constraints.get(relation_name)
    }

    /// Validate that deleting tuples won't violate foreign keys in other relations.
    pub fn validate_referencing_foreign_keys<E: StorageEngine>(
        &self,
        engine: &mut E,
        relation_name: &str,
        relation_after_delete: &Relation,
    ) -> Result<(), ConstraintManagerError> {
        // Iterate over constraints without collecting/cloning
        for (ref_name, fk_constraints) in &self.foreign_key_constraints {
            for fk in fk_constraints.foreign_keys() {
                if fk.referenced_relation_name() == relation_name {
                    let referencing_relation = engine.load_relation(ref_name)?;

                    if !fk
                        .is_satisfied_by(&referencing_relation, relation_after_delete)
                        .map_err(|e| ConstraintManagerError::ForeignKeyViolation(e.to_string()))?
                    {
                        return Err(ConstraintManagerError::ForeignKeyViolation(format!(
                            "Deleting tuples would orphan referencing tuples in {}",
                            ref_name
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage_engine::StorageEngine;
    use crate::storage_engine::in_memory::InMemoryEngine;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::{Relation, ScalarValue};

    #[test]
    fn test_validate_tuple_type_mismatch() {
        let mut engine = InMemoryEngine::new();
        let rel_type = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("name", ScalarType::String),
        );
        engine.create_relation("TEST", rel_type.clone()).unwrap();

        let manager = ConstraintManager::new();

        // Valid tuple
        let valid_tuple = tuple! { id: 1i64, name: "Alice" };
        assert!(
            manager
                .validate_tuple_type(&engine, "TEST", &valid_tuple)
                .is_ok()
        );

        // Invalid tuple (missing attribute, or wrong type)
        // Since tuple! requires types to match, we create a bad tuple manually
        // that belongs to a different heading but try to insert it anyway
        let bad_heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::Int);
        let mut values = std::collections::BTreeMap::new();
        values.insert("id".to_string(), ScalarValue::Int(1));
        values.insert("name".to_string(), ScalarValue::Int(42));

        let invalid_tuple =
            crate::values::Tuple::new_unchecked(std::sync::Arc::new(bad_heading), values);

        let result = manager.validate_tuple_type(&engine, "TEST", &invalid_tuple);
        assert!(matches!(result, Err(ConstraintManagerError::TupleMismatch)));
    }

    #[test]
    fn test_validate_referencing_foreign_keys_violation() {
        let mut engine = InMemoryEngine::new();

        let dept_type = RelationType::new(
            TupleType::new()
                .with_attribute("dept_id", ScalarType::Int)
                .with_attribute("dname", ScalarType::String),
        );
        let emp_type = RelationType::new(
            TupleType::new()
                .with_attribute("emp_id", ScalarType::Int)
                .with_attribute("dept_id", ScalarType::Int),
        );

        engine.create_relation("DEPT", dept_type.clone()).unwrap();
        engine.create_relation("EMP", emp_type.clone()).unwrap();

        let mut dept_rel = Relation::new(dept_type.clone());
        dept_rel
            .insert(tuple! { dept_id: 1i64, dname: "HR" })
            .unwrap();
        engine.store_relation("DEPT", &dept_rel).unwrap();

        let mut emp_rel = Relation::new(emp_type.clone());
        emp_rel
            .insert(tuple! { emp_id: 101i64, dept_id: 1i64 })
            .unwrap();
        engine.store_relation("EMP", &emp_rel).unwrap();

        let mut manager = ConstraintManager::new();

        let fk_constraint = crate::constraints::ForeignKey::new(
            vec!["dept_id".to_string()],
            "DEPT".to_string(),
            vec!["dept_id".to_string()],
        )
        .unwrap();
        let fk_constraints =
            crate::constraints::ForeignKeyConstraints::new().with_foreign_key(fk_constraint);
        manager
            .set_foreign_key_constraints(&mut engine, "EMP", fk_constraints)
            .unwrap();

        // Simulating a state where we attempt to delete the DEPT record
        let empty_dept_rel = Relation::new(dept_type.clone());

        let result =
            manager.validate_referencing_foreign_keys(&mut engine, "DEPT", &empty_dept_rel);

        assert!(matches!(
            result,
            Err(ConstraintManagerError::ForeignKeyViolation(_))
        ));
    }
}
