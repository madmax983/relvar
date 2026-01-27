//! In-memory storage engine implementation.
//!
//! This module provides a pure in-memory storage engine with no I/O operations.
//! It's useful for:
//! - Testing
//! - Benchmarking pure relational operations
//! - Temporary databases that don't need persistence

use super::{RelationMetadata, StorageEngine, StorageError, TransactionSnapshot};
use crate::types::RelationType;
use crate::values::{Relation, Tuple};
use std::collections::HashMap;

/// Stored relation with metadata.
#[derive(Debug, Clone)]
struct StoredRelation {
    metadata: RelationMetadata,
    relation: Relation,
}

/// Pure in-memory storage engine.
///
/// All data is stored in memory with no persistence. When the engine
/// is dropped, all data is lost.
///
/// # Example
///
/// ```
/// use relvar_core::storage_engine::{StorageEngine, InMemoryEngine};
/// use relvar_core::types::{TupleType, RelationType, ScalarType};
/// use relvar_core::tuple;
///
/// let mut engine = InMemoryEngine::new();
///
/// // Create relation
/// let rel_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("id", ScalarType::Int)
///         .with_attribute("name", ScalarType::String)
/// );
/// engine.create_relation("EMPLOYEES", rel_type).unwrap();
///
/// // Insert tuple
/// engine.insert_tuple("EMPLOYEES", tuple! { id: 1i64, name: "Alice" }).unwrap();
///
/// // Load relation
/// let employees = engine.load_relation("EMPLOYEES").unwrap();
/// assert_eq!(employees.cardinality(), 1);
/// ```
#[derive(Debug, Default)]
pub struct InMemoryEngine {
    relations: HashMap<String, StoredRelation>,
}

impl InMemoryEngine {
    /// Create a new in-memory storage engine.
    pub fn new() -> Self {
        Self {
            relations: HashMap::new(),
        }
    }
}

impl StorageEngine for InMemoryEngine {
    fn create_relation(
        &mut self,
        name: &str,
        relation_type: RelationType,
    ) -> Result<(), StorageError> {
        if self.relations.contains_key(name) {
            return Err(StorageError::RelationAlreadyExists(name.to_string()));
        }

        let metadata = RelationMetadata {
            name: name.to_string(),
            relation_type: relation_type.clone(),
        };

        let relation = Relation::new(relation_type);

        self.relations.insert(
            name.to_string(),
            StoredRelation { metadata, relation },
        );

        Ok(())
    }

    fn drop_relation(&mut self, name: &str) -> Result<(), StorageError> {
        if self.relations.remove(name).is_none() {
            return Err(StorageError::RelationNotFound(name.to_string()));
        }
        Ok(())
    }

    fn relation_exists(&self, name: &str) -> bool {
        self.relations.contains_key(name)
    }

    fn get_relation_metadata(&self, name: &str) -> Result<RelationMetadata, StorageError> {
        self.relations
            .get(name)
            .map(|stored| stored.metadata.clone())
            .ok_or_else(|| StorageError::RelationNotFound(name.to_string()))
    }

    fn list_relations(&self) -> Vec<String> {
        self.relations.keys().cloned().collect()
    }

    fn load_relation(&self, name: &str) -> Result<Relation, StorageError> {
        self.relations
            .get(name)
            .map(|stored| stored.relation.clone())
            .ok_or_else(|| StorageError::RelationNotFound(name.to_string()))
    }

    fn store_relation(&mut self, name: &str, relation: &Relation) -> Result<(), StorageError> {
        let stored = self
            .relations
            .get_mut(name)
            .ok_or_else(|| StorageError::RelationNotFound(name.to_string()))?;

        // Verify the relation type matches
        if stored.relation.relation_type() != relation.relation_type() {
            return Err(StorageError::Other(format!(
                "Relation type mismatch for {}",
                name
            )));
        }

        stored.relation = relation.clone();
        Ok(())
    }

    fn insert_tuple(&mut self, name: &str, tuple: Tuple) -> Result<(), StorageError> {
        let stored = self
            .relations
            .get_mut(name)
            .ok_or_else(|| StorageError::RelationNotFound(name.to_string()))?;

        stored
            .relation
            .insert(tuple)
            .map_err(|e| StorageError::Relation(e.to_string()))?;

        Ok(())
    }

    fn begin_transaction(&mut self) -> Result<TransactionSnapshot, StorageError> {
        // Save current state
        let saved_relations: HashMap<String, Relation> = self
            .relations
            .iter()
            .map(|(name, stored)| (name.clone(), stored.relation.clone()))
            .collect();

        Ok(TransactionSnapshot { saved_relations })
    }

    fn rollback_transaction(
        &mut self,
        snapshot: TransactionSnapshot,
    ) -> Result<(), StorageError> {
        // Restore saved state
        for (name, relation) in snapshot.saved_relations {
            if let Some(stored) = self.relations.get_mut(&name) {
                stored.relation = relation;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{ScalarType, TupleType};

    fn test_rel_type() -> RelationType {
        RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("name", ScalarType::String),
        )
    }

    #[test]
    fn test_create_relation() {
        let mut engine = InMemoryEngine::new();
        let rel_type = test_rel_type();

        assert!(engine.create_relation("TEST", rel_type.clone()).is_ok());
        assert!(engine.relation_exists("TEST"));

        // Duplicate creation should fail
        assert!(engine.create_relation("TEST", rel_type).is_err());
    }

    #[test]
    fn test_drop_relation() {
        let mut engine = InMemoryEngine::new();
        engine
            .create_relation("TEST", test_rel_type())
            .unwrap();

        assert!(engine.drop_relation("TEST").is_ok());
        assert!(!engine.relation_exists("TEST"));

        // Dropping non-existent relation should fail
        assert!(engine.drop_relation("TEST").is_err());
    }

    #[test]
    fn test_get_metadata() {
        let mut engine = InMemoryEngine::new();
        let rel_type = test_rel_type();

        engine
            .create_relation("TEST", rel_type.clone())
            .unwrap();

        let metadata = engine.get_relation_metadata("TEST").unwrap();
        assert_eq!(metadata.name, "TEST");
        assert_eq!(metadata.relation_type, rel_type);
    }

    #[test]
    fn test_list_relations() {
        let mut engine = InMemoryEngine::new();

        engine.create_relation("A", test_rel_type()).unwrap();
        engine.create_relation("B", test_rel_type()).unwrap();

        let names = engine.list_relations();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"A".to_string()));
        assert!(names.contains(&"B".to_string()));
    }

    #[test]
    fn test_insert_and_load() {
        let mut engine = InMemoryEngine::new();
        engine.create_relation("TEST", test_rel_type()).unwrap();

        let tuple1 = tuple! { id: 1i64, name: "Alice" };
        let tuple2 = tuple! { id: 2i64, name: "Bob" };

        engine.insert_tuple("TEST", tuple1).unwrap();
        engine.insert_tuple("TEST", tuple2).unwrap();

        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 2);
    }

    #[test]
    fn test_store_relation() {
        let mut engine = InMemoryEngine::new();
        engine.create_relation("TEST", test_rel_type()).unwrap();

        let tuple1 = tuple! { id: 1i64, name: "Alice" };
        engine.insert_tuple("TEST", tuple1).unwrap();

        // Create new relation and store it
        let mut new_relation = Relation::new(test_rel_type());
        new_relation
            .insert(tuple! { id: 2i64, name: "Bob" })
            .unwrap();

        engine.store_relation("TEST", &new_relation).unwrap();

        let loaded = engine.load_relation("TEST").unwrap();
        assert_eq!(loaded.cardinality(), 1);
    }

    #[test]
    fn test_transaction_rollback() {
        let mut engine = InMemoryEngine::new();
        engine.create_relation("TEST", test_rel_type()).unwrap();

        let tuple1 = tuple! { id: 1i64, name: "Alice" };
        engine.insert_tuple("TEST", tuple1).unwrap();

        // Begin transaction and save state
        let snapshot = engine.begin_transaction().unwrap();

        // Make changes
        let tuple2 = tuple! { id: 2i64, name: "Bob" };
        engine.insert_tuple("TEST", tuple2).unwrap();

        // Should have 2 tuples now
        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 2);

        // Rollback
        engine.rollback_transaction(snapshot).unwrap();

        // Should be back to 1 tuple
        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 1);
    }
}
