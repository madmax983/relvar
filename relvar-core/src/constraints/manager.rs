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
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Remove all constraints associated with a relation.
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn remove_constraints_for_relation(&mut self, relation_name: &str) {
        self.key_constraints.remove(relation_name);
        self.foreign_key_constraints.remove(relation_name);
        self.type_constraints.remove(relation_name);
        self.check_constraints.remove(relation_name);
    }

    /// Set key constraints for a relation.
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn validate_type_constraints(
        &self,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), ConstraintManagerError> {
        let Some(attr_constraints) = self.type_constraints.get(relation_name) else {
            return Ok(());
        };
        for (attr_name, constraints) in attr_constraints {
            let Some(value) = tuple.get(attr_name) else {
                continue;
            };
            if !constraints
                .is_satisfied_by(value)
                .map_err(|e| ConstraintManagerError::TypeConstraintViolation(e.to_string()))?
            {
                return Err(ConstraintManagerError::TypeConstraintViolation(format!(
                    "Attribute {} violates constraint",
                    attr_name
                )));
            }
        }
        Ok(())
    }

    /// Validate that a tuple satisfies all CHECK constraints.
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn validate_check_constraints(
        &self,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), ConstraintManagerError> {
        let Some(check_constraints) = self.check_constraints.get(relation_name) else {
            return Ok(());
        };
        check_constraints.are_all_satisfied_by(tuple)?;
        Ok(())
    }

    /// Validate key constraints for a single tuple against the current relation state.
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn validate_key_constraints_single_tuple(
        &self,
        relation_name: &str,
        tuple: &Tuple,
        current_relation: &Relation,
    ) -> Result<(), ConstraintManagerError> {
        let Some(key_constraints) = self.key_constraints.get(relation_name) else {
            return Ok(());
        };

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
        Ok(())
    }

    /// Validate constraints that depend only on the tuple's content and foreign keys
    /// against a full relation.
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn validate_tuple_content_constraints_bulk<E: StorageEngine>(
        &self,
        engine: &mut E,
        relation_name: &str,
        relation: &Relation,
    ) -> Result<(), ConstraintManagerError> {
        relation.tuples().try_for_each(|tuple| {
            self.validate_tuple_content_constraints(engine, relation_name, tuple)
        })
    }

    /// Validate constraints that depend only on the tuple's content and foreign keys.
    ///
    /// This includes Type constraints, CHECK constraints, and Foreign Key constraints.
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn get_key_constraints(&self, relation_name: &str) -> Option<&KeyConstraints> {
        self.key_constraints.get(relation_name)
    }

    /// Get the foreign key constraints for a relation.
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn get_foreign_key_constraints(
        &self,
        relation_name: &str,
    ) -> Option<&ForeignKeyConstraints> {
        self.foreign_key_constraints.get(relation_name)
    }

    /// Validate that deleting tuples won't violate foreign keys in other relations.
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
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
    use crate::constraints::{
        AttributeConstraints, CandidateKey, CheckConstraint, ConstraintExpression, PrimaryKey,
        TypeConstraint,
    };
    use crate::storage_engine::InMemoryEngine;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::{CmpOp, Relation, ScalarValue, ValueOrRef};

    fn setup_engine() -> InMemoryEngine {
        let mut engine = InMemoryEngine::new();

        let t1 = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
        engine.create_relation("users", t1).unwrap();

        let mut users = Relation::new(RelationType::new(
            TupleType::new().with_attribute("id", ScalarType::Int),
        ));
        users.insert(tuple! { id: 1i64 }).unwrap();
        engine.store_relation("users", &users).unwrap();

        engine
    }

    #[test]
    fn should_return_error_when_check_constraint_violates_existing_data() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();

        let check_expr = ConstraintExpression::Cmp {
            left: "id".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(2)), // current data has id:1
        };
        let checks = CheckConstraints::new().with_constraint(CheckConstraint::new(
            "id_is_2".to_string(),
            "must be 2".to_string(),
            check_expr,
        ));

        let res = manager.set_check_constraints(&mut engine, "users", checks);
        assert!(res.is_err());
    }

    #[test]
    fn should_allow_tuple_type_validation_without_constraints() {
        let engine = setup_engine();
        let manager = ConstraintManager::new();

        let good_tuple = tuple! { id: 1i64 };
        let bad_tuple = tuple! { id: "wrong_type".to_string() };

        assert!(
            manager
                .validate_type_constraints("users", &bad_tuple)
                .is_ok()
        );
        assert!(
            manager
                .validate_tuple_type(&engine, "users", &good_tuple)
                .is_ok()
        );
        assert!(
            manager
                .validate_tuple_type(&engine, "users", &bad_tuple)
                .is_err()
        );
    }

    #[test]
    fn should_return_error_when_type_constraint_violated() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();

        let mut attr_cons = AttributeConstraints::new("id".to_string(), ScalarType::Int);
        let type_constraint = TypeConstraint::Range {
            min: ScalarValue::Int(1),
            max: ScalarValue::Int(1),
        };
        attr_cons = attr_cons.with_constraint(type_constraint);
        manager
            .set_type_constraints(&mut engine, "users", "id", attr_cons)
            .unwrap();

        let res = manager.validate_type_constraints("users", &tuple! { id: 2i64 });
        assert!(res.is_err());

        let res = manager.validate_check_constraints("users", &tuple! { id: 1i64 });
        assert!(res.is_ok());
    }

    #[test]
    fn should_return_error_when_key_constraints_violated_in_single_tuple() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();

        let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
        let ck = CandidateKey::new(vec!["id".to_string()]).unwrap();
        let keys = KeyConstraints::new()
            .with_primary_key(pk)
            .with_candidate_key(ck);

        manager
            .set_key_constraints(&mut engine, "users", keys.clone())
            .unwrap();

        let rel = engine.load_relation("users").unwrap();
        // single tuple pk violation (1i64 already exists)
        assert!(
            manager
                .validate_key_constraints_single_tuple("users", &tuple! { id: 1i64 }, &rel)
                .is_err()
        );
    }

    #[test]
    fn should_return_error_when_candidate_key_violated_in_single_tuple() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();

        let bad_keys = KeyConstraints::new()
            .with_candidate_key(CandidateKey::new(vec!["id".to_string()]).unwrap());
        manager
            .set_key_constraints(&mut engine, "users", bad_keys)
            .unwrap();

        let rel = engine.load_relation("users").unwrap();
        assert!(
            manager
                .validate_key_constraints_single_tuple("users", &tuple! { id: 1i64 }, &rel)
                .is_err()
        );
    }

    #[test]
    fn should_return_error_when_bulk_key_constraints_violated() {
        let manager = ConstraintManager::new();
        let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
        let ck = CandidateKey::new(vec!["id".to_string()]).unwrap();
        let keys = KeyConstraints::new()
            .with_primary_key(pk)
            .with_candidate_key(ck);

        let t3 = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("val", ScalarType::Int),
        );
        let mut dup_rel = Relation::new(t3);
        dup_rel.insert(tuple! { id: 1i64, val: 1i64 }).unwrap();
        dup_rel.insert(tuple! { id: 1i64, val: 2i64 }).unwrap();

        assert!(
            manager
                .validate_key_constraints_bulk(&dup_rel, &keys)
                .is_err()
        );
    }

    #[test]
    fn should_return_correct_constraints_when_queried_and_removed() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();

        let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
        let ck = CandidateKey::new(vec!["id".to_string()]).unwrap();
        let keys = KeyConstraints::new()
            .with_primary_key(pk)
            .with_candidate_key(ck);

        manager
            .set_key_constraints(&mut engine, "users", keys.clone())
            .unwrap();

        assert!(manager.get_key_constraints("users").is_some());
        assert!(manager.get_foreign_key_constraints("users").is_none());

        manager.remove_constraints_for_relation("users");
        assert!(manager.get_key_constraints("users").is_none());
    }

    fn setup_engine_with_posts() -> InMemoryEngine {
        let mut engine = setup_engine();

        let t_posts = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("user_id", ScalarType::Int),
        );
        engine.create_relation("posts", t_posts).unwrap();

        let mut posts = Relation::new(RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("user_id", ScalarType::Int),
        ));
        posts.insert(tuple! { id: 100i64, user_id: 1i64 }).unwrap();
        engine.store_relation("posts", &posts).unwrap();

        engine
    }

    #[test]
    fn should_return_error_when_setting_foreign_keys_on_nonexistent_relation() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();

        let fk_constraints = ForeignKeyConstraints::new();

        let res = manager.set_foreign_key_constraints(&mut engine, "missing", fk_constraints);
        assert!(matches!(
            res,
            Err(ConstraintManagerError::RelationNotFound(_))
        ));
    }

    #[test]
    fn should_set_foreign_keys_when_data_is_valid() {
        let mut engine = setup_engine_with_posts();
        let mut manager = ConstraintManager::new();

        let fk = crate::constraints::ForeignKey::new(
            vec!["user_id".to_string()],
            "users".to_string(),
            vec!["id".to_string()],
        )
        .unwrap();

        let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);

        let res = manager.set_foreign_key_constraints(&mut engine, "posts", fk_constraints);
        assert!(res.is_ok());
    }

    #[test]
    fn should_return_error_when_setting_foreign_keys_violates_existing_data() {
        let mut engine = setup_engine_with_posts();
        let mut manager = ConstraintManager::new();

        // Insert a post with a non-existent user_id
        let mut posts = engine.load_relation("posts").unwrap();
        posts.insert(tuple! { id: 101i64, user_id: 99i64 }).unwrap();
        engine.store_relation("posts", &posts).unwrap();

        let fk = crate::constraints::ForeignKey::new(
            vec!["user_id".to_string()],
            "users".to_string(),
            vec!["id".to_string()],
        )
        .unwrap();

        let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);

        let res = manager.set_foreign_key_constraints(&mut engine, "posts", fk_constraints);
        assert!(matches!(
            res,
            Err(ConstraintManagerError::ForeignKeyViolation(_))
        ));
    }

    #[test]
    fn should_validate_foreign_keys_for_single_tuple() {
        let mut engine = setup_engine_with_posts();
        let mut manager = ConstraintManager::new();

        let fk = crate::constraints::ForeignKey::new(
            vec!["user_id".to_string()],
            "users".to_string(),
            vec!["id".to_string()],
        )
        .unwrap();

        let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
        manager
            .set_foreign_key_constraints(&mut engine, "posts", fk_constraints)
            .unwrap();

        // Valid tuple
        let valid_tuple = tuple! { id: 102i64, user_id: 1i64 };
        let res = manager.validate_foreign_keys_single_tuple(&mut engine, "posts", &valid_tuple);
        assert!(res.is_ok());

        // Invalid tuple (violates foreign key)
        let invalid_tuple = tuple! { id: 103i64, user_id: 99i64 };
        let res = manager.validate_foreign_keys_single_tuple(&mut engine, "posts", &invalid_tuple);
        assert!(matches!(
            res,
            Err(ConstraintManagerError::ForeignKeyViolation(_))
        ));
    }

    #[test]
    fn should_validate_referencing_foreign_keys_on_delete() {
        let mut engine = setup_engine_with_posts();
        let mut manager = ConstraintManager::new();

        let fk = crate::constraints::ForeignKey::new(
            vec!["user_id".to_string()],
            "users".to_string(),
            vec!["id".to_string()],
        )
        .unwrap();

        let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
        manager
            .set_foreign_key_constraints(&mut engine, "posts", fk_constraints)
            .unwrap();

        // Simulate deleting the user with id=1 from users
        let empty_users = Relation::new(RelationType::new(
            TupleType::new().with_attribute("id", ScalarType::Int),
        ));

        // This should fail because `posts` has a record referencing `user_id = 1`
        let res = manager.validate_referencing_foreign_keys(&mut engine, "users", &empty_users);
        assert!(matches!(
            res,
            Err(ConstraintManagerError::ForeignKeyViolation(_))
        ));

        // Empty out posts so no references remain
        let empty_posts = Relation::new(RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("user_id", ScalarType::Int),
        ));
        engine.store_relation("posts", &empty_posts).unwrap();

        // Now validating deletion of users should pass
        let res = manager.validate_referencing_foreign_keys(&mut engine, "users", &empty_users);
        assert!(res.is_ok());
    }

    #[test]
    fn should_validate_tuple_content_constraints() {
        let mut engine = setup_engine_with_posts();
        let mut manager = ConstraintManager::new();

        // 1. Add Type Constraint on posts.id (min: 10)
        let type_cons = TypeConstraint::Range {
            min: ScalarValue::Int(10),
            max: ScalarValue::Int(1000),
        };
        manager
            .set_type_constraints(
                &mut engine,
                "posts",
                "id",
                AttributeConstraints::new("id".to_string(), ScalarType::Int)
                    .with_constraint(type_cons),
            )
            .unwrap();

        // 2. Add CHECK Constraint on posts.user_id (must be > 0)
        let check_expr = ConstraintExpression::Cmp {
            left: "user_id".to_string(),
            op: CmpOp::Gt,
            right: ValueOrRef::Value(ScalarValue::Int(0)),
        };
        let checks = CheckConstraints::new().with_constraint(CheckConstraint::new(
            "user_id_positive".to_string(),
            "must be positive".to_string(),
            check_expr,
        ));
        manager
            .set_check_constraints(&mut engine, "posts", checks)
            .unwrap();

        // 3. Add Foreign Key Constraint on posts.user_id -> users.id
        let fk = crate::constraints::ForeignKey::new(
            vec!["user_id".to_string()],
            "users".to_string(),
            vec!["id".to_string()],
        )
        .unwrap();
        let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
        manager
            .set_foreign_key_constraints(&mut engine, "posts", fk_constraints)
            .unwrap();

        // Test Type violation (id < 10)
        let bad_type_tuple = tuple! { id: 5i64, user_id: 1i64 };
        assert!(matches!(
            manager.validate_tuple_content_constraints(&mut engine, "posts", &bad_type_tuple),
            Err(ConstraintManagerError::TypeConstraintViolation(_))
        ));

        // Test CHECK violation (user_id <= 0)
        let bad_check_tuple = tuple! { id: 100i64, user_id: 0i64 };
        assert!(matches!(
            manager.validate_tuple_content_constraints(&mut engine, "posts", &bad_check_tuple),
            Err(ConstraintManagerError::CheckConstraintViolation(_))
        ));

        // Test Foreign Key violation (user_id does not exist in users)
        let bad_fk_tuple = tuple! { id: 100i64, user_id: 999i64 };
        assert!(matches!(
            manager.validate_tuple_content_constraints(&mut engine, "posts", &bad_fk_tuple),
            Err(ConstraintManagerError::ForeignKeyViolation(_))
        ));

        // Test valid tuple
        let valid_tuple = tuple! { id: 150i64, user_id: 1i64 };
        assert!(
            manager
                .validate_tuple_content_constraints(&mut engine, "posts", &valid_tuple)
                .is_ok()
        );
    }

    #[test]
    fn should_return_error_when_setting_check_constraints_on_nonexistent_relation() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();

        let check_expr = ConstraintExpression::Cmp {
            left: "id".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(2)),
        };
        let checks = CheckConstraints::new().with_constraint(CheckConstraint::new(
            "test_check".to_string(),
            "test".to_string(),
            check_expr,
        ));

        let res = manager.set_check_constraints(&mut engine, "missing", checks);
        assert!(matches!(
            res,
            Err(ConstraintManagerError::RelationNotFound(_))
        ));
    }

    #[test]
    fn should_return_error_when_setting_check_constraints_on_nonexistent_attribute() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();

        let check_expr = ConstraintExpression::Cmp {
            left: "missing_attr".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(2)),
        };
        let checks = CheckConstraints::new().with_constraint(CheckConstraint::new(
            "test_check".to_string(),
            "test".to_string(),
            check_expr,
        ));

        let res = manager.set_check_constraints(&mut engine, "users", checks);
        assert!(
            matches!(res, Err(ConstraintManagerError::AttributeNotFound(attr, _)) if attr == "missing_attr")
        );
    }
}
