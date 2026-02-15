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

        for fk in constraints.foreign_keys() {
            let referenced_relation = engine.load_relation(fk.referenced_relation_name())?;

            if !fk
                .is_satisfied_by(&relation, &referenced_relation)
                .map_err(|e| ConstraintManagerError::ForeignKeyViolation(e.to_string()))?
            {
                return Err(ConstraintManagerError::ForeignKeyViolation(
                    "Existing tuple violates foreign key".to_string(),
                ));
            }
        }

        self.foreign_key_constraints
            .insert(relation_name.to_string(), constraints);
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

        self.type_constraints
            .entry(relation_name.to_string())
            .or_default()
            .insert(attribute_name.to_string(), constraints);
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

        // Validate constraints against existing data
        let relation = engine.load_relation(relation_name)?;

        for tuple in relation.tuples() {
            constraints.are_all_satisfied_by(tuple)?;
        }

        self.check_constraints
            .insert(relation_name.to_string(), constraints);
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
    use crate::constraints::check::{CheckConstraint, CheckConstraints};
    use crate::constraints::expression::{CmpOp, ConstraintExpression, ValueOrRef};
    use crate::constraints::{
        AttributeConstraints, ForeignKey, ForeignKeyConstraints, KeyConstraints, PrimaryKey,
        TypeConstraint,
    };
    use crate::storage_engine::InMemoryEngine;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::{Relation, ScalarValue};

    // --- Helpers ---
    fn setup_engine() -> InMemoryEngine {
        InMemoryEngine::new()
    }

    fn create_test_rel(engine: &mut InMemoryEngine, name: &str) -> RelationType {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);
        let rel_type = RelationType::new(heading);
        engine.create_relation(name, rel_type.clone()).unwrap();
        rel_type
    }

    // --- Tests ---

    #[test]
    fn test_set_key_constraints_valid() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();
        create_test_rel(&mut engine, "TEST");

        let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
        let constraints = KeyConstraints::new().with_primary_key(pk);

        assert!(
            manager
                .set_key_constraints(&mut engine, "TEST", constraints)
                .is_ok()
        );
    }

    #[test]
    fn test_set_key_constraints_missing_relation() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();
        let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
        let constraints = KeyConstraints::new().with_primary_key(pk);

        let result = manager.set_key_constraints(&mut engine, "MISSING", constraints);
        assert!(matches!(
            result,
            Err(ConstraintManagerError::RelationNotFound(_))
        ));
    }

    #[test]
    fn test_validate_key_constraints_bulk_violation() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();
        create_test_rel(&mut engine, "TEST");

        // Insert duplicates
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "A" })
            .unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "B" })
            .unwrap();

        let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
        let constraints = KeyConstraints::new().with_primary_key(pk);

        let result = manager.set_key_constraints(&mut engine, "TEST", constraints);
        assert!(matches!(
            result,
            Err(ConstraintManagerError::PrimaryKeyViolation)
        ));
    }

    #[test]
    fn test_validate_key_constraints_single_tuple() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();
        create_test_rel(&mut engine, "TEST");

        let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
        let constraints = KeyConstraints::new().with_primary_key(pk);
        manager
            .set_key_constraints(&mut engine, "TEST", constraints)
            .unwrap();

        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "A" })
            .unwrap();
        let current_rel = engine.load_relation("TEST").unwrap();

        // New tuple with unique ID
        let t1 = tuple! { id: 2i64, name: "B" };
        assert!(
            manager
                .validate_key_constraints_single_tuple("TEST", &t1, &current_rel)
                .is_ok()
        );

        // Tuple with duplicate ID
        let t2 = tuple! { id: 1i64, name: "C" };
        assert!(matches!(
            manager.validate_key_constraints_single_tuple("TEST", &t2, &current_rel),
            Err(ConstraintManagerError::PrimaryKeyViolation)
        ));
    }

    #[test]
    fn test_foreign_key_violation_on_insert() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();

        // Referenced: PARENT(id)
        let parent_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
        engine
            .create_relation("PARENT", parent_type.clone())
            .unwrap();
        engine.insert_tuple("PARENT", tuple! { id: 1i64 }).unwrap();

        // Referencing: CHILD(pid)
        let child_type = RelationType::new(TupleType::new().with_attribute("pid", ScalarType::Int));
        engine.create_relation("CHILD", child_type.clone()).unwrap();

        // Set FK: CHILD.pid -> PARENT.id
        let fk = ForeignKey::new(
            vec!["pid".to_string()],
            "PARENT".to_string(),
            vec!["id".to_string()],
        )
        .unwrap();
        let constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
        manager
            .set_foreign_key_constraints(&mut engine, "CHILD", constraints)
            .unwrap();

        // Valid insert
        let t1 = tuple! { pid: 1i64 };
        assert!(
            manager
                .validate_foreign_keys_single_tuple(&mut engine, "CHILD", &t1)
                .is_ok()
        );

        // Invalid insert (orphan)
        let t2 = tuple! { pid: 99i64 };
        assert!(matches!(
            manager.validate_foreign_keys_single_tuple(&mut engine, "CHILD", &t2),
            Err(ConstraintManagerError::ForeignKeyViolation(_))
        ));
    }

    #[test]
    fn test_referencing_fk_blocks_delete() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();

        // Referenced: PARENT(id)
        let parent_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
        engine
            .create_relation("PARENT", parent_type.clone())
            .unwrap();
        engine.insert_tuple("PARENT", tuple! { id: 1i64 }).unwrap();
        engine.insert_tuple("PARENT", tuple! { id: 2i64 }).unwrap();

        // Referencing: CHILD(pid)
        let child_type = RelationType::new(TupleType::new().with_attribute("pid", ScalarType::Int));
        engine.create_relation("CHILD", child_type.clone()).unwrap();
        engine.insert_tuple("CHILD", tuple! { pid: 1i64 }).unwrap();

        // Set FK: CHILD.pid -> PARENT.id
        let fk = ForeignKey::new(
            vec!["pid".to_string()],
            "PARENT".to_string(),
            vec!["id".to_string()],
        )
        .unwrap();
        let constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
        manager
            .set_foreign_key_constraints(&mut engine, "CHILD", constraints)
            .unwrap();

        // Try deleting id=1 (referenced by child)
        // Construct relation AFTER delete (simulate delete of id=1)
        let mut rel_after_delete = Relation::new(parent_type.clone());
        rel_after_delete.insert(tuple! { id: 2i64 }).unwrap(); // id=1 is gone

        let result =
            manager.validate_referencing_foreign_keys(&mut engine, "PARENT", &rel_after_delete);
        assert!(matches!(
            result,
            Err(ConstraintManagerError::ForeignKeyViolation(_))
        ));

        // Try deleting id=2 (not referenced)
        let mut rel_after_delete_2 = Relation::new(parent_type.clone());
        rel_after_delete_2.insert(tuple! { id: 1i64 }).unwrap(); // id=2 is gone

        let result_2 =
            manager.validate_referencing_foreign_keys(&mut engine, "PARENT", &rel_after_delete_2);
        assert!(result_2.is_ok());
    }

    #[test]
    fn test_type_constraints() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();
        create_test_rel(&mut engine, "TEST");

        let constraints = AttributeConstraints::new("id".to_string(), ScalarType::Int)
            .with_constraint(TypeConstraint::Range {
                min: ScalarValue::Int(1),
                max: ScalarValue::Int(10),
            });

        manager
            .set_type_constraints(&mut engine, "TEST", "id", constraints)
            .unwrap();

        // Valid
        let t1 = tuple! { id: 5i64, name: "A" };
        assert!(manager.validate_type_constraints("TEST", &t1).is_ok());

        // Invalid
        let t2 = tuple! { id: 11i64, name: "B" };
        assert!(matches!(
            manager.validate_type_constraints("TEST", &t2),
            Err(ConstraintManagerError::TypeConstraintViolation(_))
        ));
    }

    #[test]
    fn test_check_constraints() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();
        create_test_rel(&mut engine, "TEST");

        let check = CheckConstraint::new(
            "check1",
            "id > 0",
            ConstraintExpression::Cmp {
                left: "id".to_string(),
                op: CmpOp::Gt,
                right: ValueOrRef::Value(ScalarValue::Int(0)),
            },
        );
        let constraints = CheckConstraints::new().with_constraint(check);

        manager
            .set_check_constraints(&mut engine, "TEST", constraints)
            .unwrap();

        // Valid
        let t1 = tuple! { id: 1i64, name: "A" };
        assert!(manager.validate_check_constraints("TEST", &t1).is_ok());

        // Invalid
        let t2 = tuple! { id: 0i64, name: "B" };
        assert!(matches!(
            manager.validate_check_constraints("TEST", &t2),
            Err(ConstraintManagerError::CheckConstraintViolation(_))
        ));
    }
}
