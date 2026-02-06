//! Management of virtual relvars (views).

use crate::error::DatabaseError;
use crate::traits::QueryExecutor;
use crate::types::RelationType;
use crate::values::Relation;
use std::collections::HashMap;

/// Definition of a virtual relvar (view).
///
/// TTM: RM Prescription 10 - Virtual relvars (views) re-evaluate their
/// defining expression on each query.
#[derive(Debug, Clone)]
pub struct VirtualRelvarDefinition {
    /// The name of the virtual relvar.
    pub name: String,
    /// The relation type (heading).
    pub relation_type: RelationType,
    /// The evaluation function that computes the virtual relvar's contents.
    ///
    /// Takes a mutable reference to a [`QueryExecutor`] and returns the computed relation.
    pub evaluator: fn(&mut dyn QueryExecutor) -> Result<Relation, DatabaseError>,
}

/// Manages virtual relvars (views).
#[derive(Debug, Default)]
pub struct VirtualRelvarManager {
    /// Map of virtual relvar definitions by name.
    virtual_relvars: HashMap<String, VirtualRelvarDefinition>,
}

impl VirtualRelvarManager {
    /// Create a new virtual relvar manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Define a virtual relvar.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::RelationAlreadyExists` if a virtual relvar with this name already exists.
    pub fn define_virtual_relvar(
        &mut self,
        name: &str,
        relation_type: RelationType,
        evaluator: fn(&mut dyn QueryExecutor) -> Result<Relation, DatabaseError>,
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
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::RelationNotFound` if the virtual relvar doesn't exist.
    pub fn drop_virtual_relvar(&mut self, name: &str) -> Result<(), DatabaseError> {
        self.virtual_relvars
            .remove(name)
            .ok_or_else(|| DatabaseError::RelationNotFound(name.to_string()))?;
        Ok(())
    }

    /// Check if a virtual relvar exists.
    pub fn exists(&self, name: &str) -> bool {
        self.virtual_relvars.contains_key(name)
    }

    /// Get the relation type of a virtual relvar.
    pub fn get_type(&self, name: &str) -> Option<&RelationType> {
        self.virtual_relvars.get(name).map(|def| &def.relation_type)
    }

    /// List all virtual relvar names.
    pub fn list_names(&self) -> impl Iterator<Item = &String> {
        self.virtual_relvars.keys()
    }

    /// Get the evaluator function for a virtual relvar.
    ///
    /// Returns `None` if the relvar does not exist.
    pub fn get_evaluator(
        &self,
        name: &str,
    ) -> Option<fn(&mut dyn QueryExecutor) -> Result<Relation, DatabaseError>> {
        self.virtual_relvars.get(name).map(|def| def.evaluator)
    }
}
