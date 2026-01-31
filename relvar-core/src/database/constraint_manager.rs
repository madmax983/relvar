use crate::constraints::{
    AttributeConstraints, CheckConstraints, ForeignKeyConstraints,
    KeyConstraints,
};
use crate::storage_engine::StorageEngine;
use crate::values::{Relation, ScalarValue, Tuple};
use super::error::DatabaseError;
use std::collections::HashMap;

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
    ) -> Result<(), DatabaseError> {
        if !engine.relation_exists(relation_name) {
            return Err(DatabaseError::RelationNotFound(relation_name.to_string()));
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
    ) -> Result<(), DatabaseError> {
        if !engine.relation_exists(relation_name) {
            return Err(DatabaseError::RelationNotFound(relation_name.to_string()));
        }

        // Validate constraints against existing data
        let relation = engine.load_relation(relation_name)?;

        for fk in constraints.foreign_keys() {
            let referenced_relation = engine.load_relation(fk.referenced_relation_name())?;
            // Build a HashSet of referenced keys for efficient O(1) lookups.
            let referenced_keys: std::collections::HashSet<Vec<_>> = referenced_relation
                .tuples()
                .map(|ref_tuple| {
                    fk.referenced_attributes()
                        .iter()
                        .map(|attr| ref_tuple.get(attr).cloned().unwrap())
                        .collect()
                })
                .collect();

            for tuple in relation.tuples() {
                let fk_values: Vec<_> = fk
                    .foreign_key_attributes()
                    .iter()
                    .map(|attr| tuple.get(attr).cloned().unwrap())
                    .collect();

                if !referenced_keys.contains(&fk_values) {
                    return Err(DatabaseError::ForeignKeyViolation(
                        "Existing tuple violates foreign key".to_string(),
                    ));
                }
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
    ) -> Result<(), DatabaseError> {
        let metadata = engine.get_relation_metadata(relation_name)?;

        if !metadata.relation_type.has_attribute(attribute_name) {
            return Err(DatabaseError::AttributeNotFound(
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
                    .map_err(|e| DatabaseError::TypeConstraintViolation(e.to_string()))?
            {
                return Err(DatabaseError::TypeConstraintViolation(
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
    ) -> Result<(), DatabaseError> {
        if !engine.relation_exists(relation_name) {
            return Err(DatabaseError::RelationNotFound(relation_name.to_string()));
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
    ) -> Result<(), DatabaseError> {
        let metadata = engine.get_relation_metadata(relation_name)?;
        if !tuple.conforms_to(metadata.relation_type.tuple_type()) {
            Err(DatabaseError::TupleMismatch)
        } else {
            Ok(())
        }
    }

    /// Validate that a tuple satisfies all type constraints.
    pub fn validate_type_constraints(
        &self,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), DatabaseError> {
        if let Some(attr_constraints) = self.type_constraints.get(relation_name) {
            for (attr_name, constraints) in attr_constraints {
                if let Some(value) = tuple.get(attr_name)
                    && !constraints
                        .is_satisfied_by(value)
                        .map_err(|e| DatabaseError::TypeConstraintViolation(e.to_string()))?
                {
                    return Err(DatabaseError::TypeConstraintViolation(format!(
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
    ) -> Result<(), DatabaseError> {
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
    ) -> Result<(), DatabaseError> {
        if let Some(key_constraints) = self.key_constraints.get(relation_name) {
            if let Some(pk) = key_constraints.primary_key()
                && pk
                    .would_violate(current_relation, tuple)
                    .map_err(|e| DatabaseError::TransactionError(e.to_string()))?
            {
                return Err(DatabaseError::PrimaryKeyViolation);
            }

            for ck in key_constraints.candidate_keys() {
                if ck
                    .would_violate(current_relation, tuple)
                    .map_err(|e| DatabaseError::TransactionError(e.to_string()))?
                {
                    return Err(DatabaseError::CandidateKeyViolation);
                }
            }
        }
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
    ) -> Result<(), DatabaseError> {
        let fks_to_check = self
            .foreign_key_constraints
            .get(relation_name)
            .map(|c| c.foreign_keys().to_vec())
            .unwrap_or_default();

        for fk in fks_to_check {
            let referenced_relation = engine.load_relation(fk.referenced_relation_name())?;

            if fk
                .would_violate_on_insert(tuple, &referenced_relation)
                .map_err(|e| DatabaseError::ForeignKeyViolation(e.to_string()))?
            {
                return Err(DatabaseError::ForeignKeyViolation(
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
    ) -> Result<(), DatabaseError> {
        if let Some(pk) = constraints.primary_key()
            && !pk
                .is_satisfied_by(relation)
                .map_err(|e| DatabaseError::TransactionError(e.to_string()))?
        {
            return Err(DatabaseError::PrimaryKeyViolation);
        }

        for ck in constraints.candidate_keys() {
            if !ck
                .is_satisfied_by(relation)
                .map_err(|e| DatabaseError::TransactionError(e.to_string()))?
            {
                return Err(DatabaseError::CandidateKeyViolation);
            }
        }
        Ok(())
    }

    /// Get the key constraints for a relation.
    pub fn get_key_constraints(&self, relation_name: &str) -> Option<&KeyConstraints> {
        self.key_constraints.get(relation_name)
    }

    /// Validate that deleting tuples won't violate foreign keys in other relations.
    pub fn validate_referencing_foreign_keys<E: StorageEngine>(
        &self,
        engine: &mut E,
        relation_name: &str,
        relation_after_delete: &Relation,
    ) -> Result<(), DatabaseError> {
        // Collect referencing foreign keys to avoid borrowing issues
        let referencing_fks: Vec<(String, crate::constraints::ForeignKey)> = self
            .foreign_key_constraints
            .iter()
            .flat_map(|(ref_name, fk_constraints)| {
                fk_constraints
                    .foreign_keys()
                    .iter()
                    .filter(|fk| fk.referenced_relation_name() == relation_name)
                    .map(move |fk| (ref_name.clone(), fk.clone()))
            })
            .collect();

        for (ref_name, fk) in referencing_fks {
            let referencing_relation = engine.load_relation(&ref_name)?;

            // Build a HashSet of keys from the relation after deletion for efficient lookups.
            let existing_keys: std::collections::HashSet<Vec<_>> = relation_after_delete
                .tuples()
                .map(|t| {
                    fk.referenced_attributes()
                        .iter()
                        .filter_map(|attr| t.get(attr).cloned())
                        .collect()
                })
                .collect();

            // Check if any referencing tuples would be orphaned by looking up in the HashSet.
            for ref_tuple in referencing_relation.tuples() {
                let ref_key_values: Vec<ScalarValue> = fk
                    .foreign_key_attributes()
                    .iter()
                    .filter_map(|attr| ref_tuple.get(attr).cloned())
                    .collect();

                if !existing_keys.contains(&ref_key_values) {
                    return Err(DatabaseError::ForeignKeyViolation(format!(
                        "Deleting tuples would orphan referencing tuples in {}",
                        ref_name
                    )));
                }
            }
        }
        Ok(())
    }
}
