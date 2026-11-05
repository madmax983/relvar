//! Constraint persistence hooks (TTM RM Prescription 16: a database
//! contains its constraints).
//!
//! A test engine wraps [`InMemoryEngine`] and keeps whatever constraint
//! catalog the database stores, so these tests exercise the
//! [`StorageEngine::store_constraints`] / [`StorageEngine::load_constraints`]
//! contract without a persistent engine.

use super::golden_rule::{fk, id_name_type, pk};
use crate::constraints::{ConstraintCatalog, ConstraintManagerError};
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::{InMemoryEngine, RelationMetadata, StorageEngine, StorageError};
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, Tuple};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// An in-memory engine that stores the constraint catalog it is given.
#[derive(Default)]
pub(super) struct CatalogEngine {
    pub(super) inner: InMemoryEngine,
    pub(super) stored: Option<ConstraintCatalog>,
    pub(super) stores: usize,
    pub(super) fail_store: bool,
}

impl StorageEngine for CatalogEngine {
    type Snapshot = <InMemoryEngine as StorageEngine>::Snapshot;

    fn create_relation(&mut self, name: &str, ty: RelationType) -> Result<(), StorageError> {
        self.inner.create_relation(name, ty)
    }
    fn drop_relation(&mut self, name: &str) -> Result<(), StorageError> {
        self.inner.drop_relation(name)
    }
    fn relation_exists(&self, name: &str) -> bool {
        self.inner.relation_exists(name)
    }
    fn get_relation_metadata(&self, name: &str) -> Result<RelationMetadata, StorageError> {
        self.inner.get_relation_metadata(name)
    }
    fn list_relations(&self) -> Vec<String> {
        self.inner.list_relations()
    }
    fn load_relation(&self, name: &str) -> Result<Relation, StorageError> {
        self.inner.load_relation(name)
    }
    fn store_relation(&mut self, name: &str, relation: &Relation) -> Result<(), StorageError> {
        self.inner.store_relation(name, relation)
    }
    fn insert_tuple(&mut self, name: &str, tuple: Tuple) -> Result<(), StorageError> {
        self.inner.insert_tuple(name, tuple)
    }
    fn begin_transaction(&mut self) -> Result<Self::Snapshot, StorageError> {
        self.inner.begin_transaction()
    }
    fn commit_transaction(&mut self, snapshot: Self::Snapshot) -> Result<(), StorageError> {
        self.inner.commit_transaction(snapshot)
    }
    fn rollback_transaction(&mut self, snapshot: Self::Snapshot) -> Result<(), StorageError> {
        self.inner.rollback_transaction(snapshot)
    }
    fn store_constraints(&mut self, catalog: &ConstraintCatalog) -> Result<(), StorageError> {
        if self.fail_store {
            return Err(StorageError::Other("disk full".to_string()));
        }
        self.stores += 1;
        self.stored = Some(catalog.clone());
        Ok(())
    }
    fn load_constraints(&self) -> Option<ConstraintCatalog> {
        self.stored.clone()
    }
}

fn ref_type() -> RelationType {
    RelationType::new(TupleType::new().with_attribute("parent", ScalarType::Int))
}

/// PARENT{id, name} keyed on id; CHILD{parent} referencing PARENT.
fn parent_child_db() -> Database<CatalogEngine> {
    let mut db = Database::new(CatalogEngine::default());
    db.create_relvar("PARENT", id_name_type()).unwrap();
    db.create_relvar("CHILD", ref_type()).unwrap();
    db.set_key_constraints("PARENT", pk(&["id"])).unwrap();
    db.set_foreign_key_constraints("CHILD", fk(&["parent"], "PARENT", &["id"]))
        .unwrap();
    db.insert("PARENT", tuple! { id: 1i64, name: "p" }).unwrap();
    db
}

pub(super) fn reopen(db: Database<CatalogEngine>) -> Database<CatalogEngine> {
    Database::new(db.engine)
}

#[test]
fn every_constraint_change_is_stored() {
    let db = parent_child_db();
    assert_eq!(db.engine.stores, 2);
}

#[test]
fn new_restores_stored_constraints() {
    let mut db = reopen(parent_child_db());

    assert!(db.get_key_constraints("PARENT").is_some());
    assert!(matches!(
        db.insert("CHILD", tuple! { parent: 9i64 }),
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
    // The reverse foreign-key index is rebuilt too.
    assert!(db.delete("PARENT", |_| true).is_ok());
    db.insert("PARENT", tuple! { id: 1i64, name: "p" }).unwrap();
    db.insert("CHILD", tuple! { parent: 1i64 }).unwrap();
    assert!(db.delete("PARENT", |_| true).is_err());
    assert!(matches!(
        db.drop_relvar("PARENT"),
        Err(DatabaseError::Constraint(
            ConstraintManagerError::RelvarReferenced { .. }
        ))
    ));
}

#[test]
fn dropping_a_relvar_stores_the_reduced_catalog() {
    let mut db = parent_child_db();
    db.drop_relvar("CHILD").unwrap();

    let stored = db.engine.stored.clone().unwrap();
    assert!(stored.relvar("CHILD").is_none());
    assert!(stored.relvar("PARENT").is_some());
}

#[test]
fn failed_store_leaves_constraints_unchanged() {
    let mut db = parent_child_db();
    db.engine.fail_store = true;

    let result = db.set_key_constraints("CHILD", pk(&["parent"]));
    assert!(matches!(result, Err(DatabaseError::Storage(_))));
    assert!(db.get_key_constraints("CHILD").is_none());
}

#[test]
fn in_memory_engine_stores_nothing() {
    let mut engine = InMemoryEngine::new();
    assert!(engine.store_constraints(&ConstraintCatalog::new()).is_ok());
    assert!(engine.load_constraints().is_none());
}
