use crate::constraints::{
    AttributeConstraints, CheckConstraints, ForeignKeyConstraints, KeyConstraints,
};
use crate::database::{DatabaseError, QueryContext};
use crate::storage_engine::StorageEngine;
use crate::types::RelationType;
use crate::values::{Relation, ScalarValue, Tuple};
use std::collections::HashMap;

/// Definition of a virtual relvar (view).
#[derive(Debug, Clone)]
pub struct VirtualRelvarDefinition {
    /// The name of the virtual relvar.
    pub name: String,
    /// The relation type (heading).
    pub relation_type: RelationType,
    /// The evaluation function that computes the virtual relvar's contents.
    pub evaluator: fn(&dyn QueryContext) -> Result<Relation, DatabaseError>,
}

/// System catalog managing metadata and constraints.
pub struct SystemCatalog {
    /// Key constraints (primary and candidate) per relation.
    key_constraints: HashMap<String, KeyConstraints>,
    /// Foreign key constraints per relation.
    foreign_key_constraints: HashMap<String, ForeignKeyConstraints>,
    /// Type constraints per relation, per attribute.
    type_constraints: HashMap<String, HashMap<String, AttributeConstraints>>,
    /// CHECK constraints (tuple-level predicates) per relation.
    check_constraints: HashMap<String, CheckConstraints>,
    /// Virtual relvars defined by expressions.
    virtual_relvars: HashMap<String, VirtualRelvarDefinition>,
}

impl Default for SystemCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemCatalog {
    /// Create a new system catalog.
    pub fn new() -> Self {
        Self {
            key_constraints: HashMap::new(),
            foreign_key_constraints: HashMap::new(),
            type_constraints: HashMap::new(),
            check_constraints: HashMap::new(),
            virtual_relvars: HashMap::new(),
        }
    }

    /// Check if a relvar is virtual.
    pub fn is_virtual(&self, name: &str) -> bool {
        self.virtual_relvars.contains_key(name)
    }

    /// Get a virtual relvar definition.
    pub fn get_virtual_relvar(&self, name: &str) -> Option<&VirtualRelvarDefinition> {
        self.virtual_relvars.get(name)
    }

    /// Get all virtual relvar names.
    pub fn virtual_relvar_names(&self) -> impl Iterator<Item = &String> {
        self.virtual_relvars.keys()
    }

    /// Define a virtual relvar (view).
    pub fn define_virtual_relvar(
        &mut self,
        name: &str,
        relation_type: RelationType,
        evaluator: fn(&dyn QueryContext) -> Result<Relation, DatabaseError>,
    ) -> Result<(), DatabaseError> {
        if self.virtual_relvars.contains_key(name) {
            return Err(DatabaseError::RelationAlreadyExists(name.to_string()));
        }

        self.virtual_relvars.insert(
            name.to_string(),
            VirtualRelvarDefinition {
                name: name.to_string(),
                relation_type,
                evaluator,
            },
        );

        Ok(())
    }

    /// Drop a virtual relvar.
    pub fn drop_virtual_relvar(&mut self, name: &str) -> Result<(), DatabaseError> {
        self.virtual_relvars
            .remove(name)
            .ok_or_else(|| DatabaseError::RelationNotFound(name.to_string()))?;
        Ok(())
    }

    /// Drop metadata for a relation (used when dropping a base relvar).
    pub fn drop_relvar_metadata(&mut self, name: &str) {
        self.key_constraints.remove(name);
        self.foreign_key_constraints.remove(name);
        self.type_constraints.remove(name);
        self.check_constraints.remove(name);
    }

    /// Set key constraints for a relation.
    pub fn set_key_constraints(
        &mut self,
        engine: &impl StorageEngine,
        relation_name: &str,
        constraints: KeyConstraints,
    ) -> Result<(), DatabaseError> {
        // Validate constraints against existing data
        let relation = engine.load_relation(relation_name)?;
        self.validate_key_constraints_bulk(&relation, &constraints)?;

        self.key_constraints
            .insert(relation_name.to_string(), constraints);
        Ok(())
    }

    /// Set foreign key constraints for a relation.
    pub fn set_foreign_key_constraints(
        &mut self,
        engine: &impl StorageEngine,
        relation_name: &str,
        constraints: ForeignKeyConstraints,
    ) -> Result<(), DatabaseError> {
        // Validate constraints against existing data
        let relation = engine.load_relation(relation_name)?;

        for fk in constraints.foreign_keys() {
            let referenced_relation = engine.load_relation(fk.referenced_relation_name())?;

            for tuple in relation.tuples() {
                if fk
                    .would_violate_on_insert(tuple, &referenced_relation)
                    .map_err(|e| DatabaseError::ForeignKeyViolation(e.to_string()))?
                {
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
    pub fn set_type_constraints(
        &mut self,
        engine: &impl StorageEngine,
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
    pub fn set_check_constraints(
        &mut self,
        engine: &impl StorageEngine,
        relation_name: &str,
        constraints: CheckConstraints,
    ) -> Result<(), DatabaseError> {
        // Validate constraints against existing data
        let relation = engine.load_relation(relation_name)?;

        for tuple in relation.tuples() {
            constraints.are_all_satisfied_by(tuple)?;
        }

        self.check_constraints
            .insert(relation_name.to_string(), constraints);
        Ok(())
    }

    // --- Validation Methods ---

    /// Validate tuple insertion.
    pub fn validate_insert(
        &self,
        context: &dyn QueryContext,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), DatabaseError> {
        self.ensure_not_virtual(relation_name)?;
        self.validate_tuple_type(context, relation_name, tuple)?;
        self.validate_type_constraints(relation_name, tuple)?;
        self.validate_check_constraints(relation_name, tuple)?;

        // Load current relation to check key constraints
        let current_relation = context.query(relation_name)?;
        self.validate_key_constraints_single_tuple(relation_name, tuple, &current_relation)?;
        self.validate_foreign_keys_single_tuple(context, relation_name, tuple)?;

        Ok(())
    }

    /// Validate relation update.
    pub fn validate_update(
        &self,
        relation_name: &str,
        new_relation: &Relation,
    ) -> Result<(), DatabaseError> {
        if let Some(key_constraints) = self.key_constraints.get(relation_name) {
            self.validate_key_constraints_bulk(new_relation, key_constraints)?;
        }
        Ok(())
    }

    /// Validate tuple deletion.
    pub fn validate_delete(
        &self,
        context: &dyn QueryContext,
        relation_name: &str,
        relation_after_delete: &Relation,
    ) -> Result<(), DatabaseError> {
        self.validate_referencing_foreign_keys(context, relation_name, relation_after_delete)
    }

    /// Ensure a relvar is not virtual.
    pub fn ensure_not_virtual(&self, relation_name: &str) -> Result<(), DatabaseError> {
        if self.virtual_relvars.contains_key(relation_name) {
            Err(DatabaseError::CannotModifyVirtualRelvar(
                relation_name.to_string(),
            ))
        } else {
            Ok(())
        }
    }

    fn validate_tuple_type(
        &self,
        context: &dyn QueryContext,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), DatabaseError> {
        let relation_type = context.relation_type(relation_name)?;
        if !tuple.conforms_to(relation_type.tuple_type()) {
            Err(DatabaseError::TupleMismatch)
        } else {
            Ok(())
        }
    }

    fn validate_type_constraints(
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

    fn validate_check_constraints(
        &self,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), DatabaseError> {
        if let Some(check_constraints) = self.check_constraints.get(relation_name) {
            check_constraints.are_all_satisfied_by(tuple)?;
        }
        Ok(())
    }

    fn validate_key_constraints_single_tuple(
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

    fn validate_foreign_keys_single_tuple(
        &self,
        context: &dyn QueryContext,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), DatabaseError> {
        let fks_to_check = self
            .foreign_key_constraints
            .get(relation_name)
            .map(|c| c.foreign_keys().to_vec())
            .unwrap_or_default();

        for fk in fks_to_check {
            let referenced_relation = context.query(fk.referenced_relation_name())?;

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

    fn validate_key_constraints_bulk(
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

    fn validate_referencing_foreign_keys(
        &self,
        context: &dyn QueryContext,
        relation_name: &str,
        relation_after_delete: &Relation,
    ) -> Result<(), DatabaseError> {
        // Collect referencing foreign keys
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
            let referencing_relation = context.query(&ref_name)?;

            // Check if any referencing tuples would be orphaned
            for ref_tuple in referencing_relation.tuples() {
                let ref_key_values: Vec<ScalarValue> = fk
                    .foreign_key_attributes()
                    .iter()
                    .filter_map(|attr| ref_tuple.get(attr).cloned())
                    .collect();

                // Check if the key exists in the new relation
                let exists = relation_after_delete.tuples().any(|t| {
                    let key_values: Vec<ScalarValue> = fk
                        .referenced_attributes()
                        .iter()
                        .filter_map(|attr| t.get(attr).cloned())
                        .collect();
                    key_values == ref_key_values
                });

                if !exists {
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
