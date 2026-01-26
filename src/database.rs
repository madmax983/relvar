use crate::constraints::{AttributeConstraints, ForeignKey, ForeignKeyConstraints, KeyConstraints};
use crate::storage::{BTreeIndex, Catalog, CatalogError, HeapError, HeapFile};
use crate::types::RelationType;
use crate::values::relation::RelationError;
use crate::values::{Relation, Tuple};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Catalog error: {0}")]
    Catalog(#[from] CatalogError),
    #[error("Heap file error: {0}")]
    HeapFile(#[from] HeapError),
    #[error("Relation error: {0}")]
    Relation(#[from] RelationError),
    #[error("Relation {0} already exists")]
    RelationAlreadyExists(String),
    #[error("Relation {0} not found")]
    RelationNotFound(String),
    #[error("Tuple type does not match relation type")]
    TupleMismatch,
    #[error("Primary key constraint violation")]
    PrimaryKeyViolation,
    #[error("Candidate key constraint violation")]
    CandidateKeyViolation,
    #[error("Foreign key constraint violation: {0}")]
    ForeignKeyViolation(String),
    #[error("Type constraint violation: {0}")]
    TypeConstraintViolation(String),
    #[error("Attribute {0} not found")]
    AttributeNotFound(String),
    #[error("Transaction error: {0}")]
    TransactionError(String),
}

/// A database instance managing relations, storage, and constraints
pub struct Database {
    /// Base directory for database files
    db_path: PathBuf,
    /// Path to catalog file
    catalog_path: PathBuf,
    /// System catalog
    catalog: Catalog,
    /// Open heap files (relation_name -> heap_file)
    heap_files: HashMap<String, HeapFile>,
    /// Indexes (relation_name.attribute -> index)
    indexes: HashMap<String, BTreeIndex>,
    /// Key constraints per relation
    key_constraints: HashMap<String, KeyConstraints>,
    /// Foreign key constraints per relation
    foreign_key_constraints: HashMap<String, ForeignKeyConstraints>,
    /// Type constraints per relation per attribute
    type_constraints: HashMap<String, HashMap<String, AttributeConstraints>>,
    /// Transaction state
    in_transaction: bool,
    /// Savepoint for rollback (simplified - just store full relations)
    savepoint: Option<HashMap<String, Relation>>,
}

impl Database {
    /// Open or create a database at the specified path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, DatabaseError> {
        let db_path = path.as_ref().to_path_buf();
        std::fs::create_dir_all(&db_path)?;

        let catalog_path = db_path.join("catalog.json");
        let catalog = if catalog_path.exists() {
            Catalog::load(&catalog_path)?
        } else {
            let cat = Catalog::new();
            cat.save(&catalog_path)?;
            cat
        };

        Ok(Self {
            db_path,
            catalog,
            catalog_path,
            heap_files: HashMap::new(),
            indexes: HashMap::new(),
            key_constraints: HashMap::new(),
            foreign_key_constraints: HashMap::new(),
            type_constraints: HashMap::new(),
            in_transaction: false,
            savepoint: None,
        })
    }

    /// Create a new relation (base relvar)
    pub fn create_relvar(
        &mut self,
        name: &str,
        relation_type: RelationType,
    ) -> Result<(), DatabaseError> {
        // Check if relation already exists
        if self.catalog.get_relation(name).is_ok() {
            return Err(DatabaseError::RelationAlreadyExists(name.to_string()));
        }

        // Create heap file
        let heap_path = self.db_path.join(format!("{}.heap", name));
        let heap_file = HeapFile::create(&heap_path, relation_type.clone())?;

        // Register in catalog
        self.catalog
            .create_relation(name.to_string(), relation_type, heap_path.clone())?;

        // Save catalog
        self.catalog.save(&self.catalog_path)?;

        // Store heap file
        self.heap_files.insert(name.to_string(), heap_file);

        Ok(())
    }

    /// Drop a relation
    pub fn drop_relvar(&mut self, name: &str) -> Result<(), DatabaseError> {
        if self.catalog.get_relation(name).is_err() {
            return Err(DatabaseError::RelationNotFound(name.to_string()));
        }

        // Remove from catalog
        self.catalog.drop_relation(name)?;

        // Save catalog
        self.catalog.save(&self.catalog_path)?;

        // Remove heap file from memory
        self.heap_files.remove(name);

        // Remove indexes
        self.indexes
            .retain(|k, _| !k.starts_with(&format!("{}.", name)));

        // Remove constraints
        self.key_constraints.remove(name);
        self.foreign_key_constraints.remove(name);
        self.type_constraints.remove(name);

        Ok(())
    }

    /// Set key constraints for a relation
    pub fn set_key_constraints(
        &mut self,
        relation_name: &str,
        constraints: KeyConstraints,
    ) -> Result<(), DatabaseError> {
        if self.catalog.get_relation(relation_name).is_err() {
            return Err(DatabaseError::RelationNotFound(relation_name.to_string()));
        }

        self.key_constraints
            .insert(relation_name.to_string(), constraints);
        Ok(())
    }

    /// Add a foreign key constraint
    pub fn add_foreign_key(
        &mut self,
        relation_name: &str,
        foreign_key: ForeignKey,
    ) -> Result<(), DatabaseError> {
        if self.catalog.get_relation(relation_name).is_err() {
            return Err(DatabaseError::RelationNotFound(relation_name.to_string()));
        }

        let constraints = self
            .foreign_key_constraints
            .remove(relation_name)
            .unwrap_or_default();

        let updated = constraints.with_foreign_key(foreign_key);
        self.foreign_key_constraints
            .insert(relation_name.to_string(), updated);

        Ok(())
    }

    /// Set type constraints for an attribute
    pub fn set_type_constraints(
        &mut self,
        relation_name: &str,
        attribute: &str,
        constraints: AttributeConstraints,
    ) -> Result<(), DatabaseError> {
        if self.catalog.get_relation(relation_name).is_err() {
            return Err(DatabaseError::RelationNotFound(relation_name.to_string()));
        }

        self.type_constraints
            .entry(relation_name.to_string())
            .or_default()
            .insert(attribute.to_string(), constraints);

        Ok(())
    }

    /// Insert a tuple into a relation
    pub fn insert(&mut self, relation_name: &str, tuple: Tuple) -> Result<(), DatabaseError> {
        // Get relation metadata
        let metadata = self
            .catalog
            .get_relation(relation_name)
            .map_err(|_| DatabaseError::RelationNotFound(relation_name.to_string()))?;

        // Check tuple type matches relation type
        if !tuple.conforms_to(metadata.relation_type.tuple_type()) {
            return Err(DatabaseError::TupleMismatch);
        }

        // Check type constraints
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

        // Load current relation to check key constraints
        let current_relation = self.query(relation_name)?;

        // Check key constraints
        if let Some(key_constraints) = self.key_constraints.get(relation_name) {
            // Check primary key
            if let Some(pk) = key_constraints.primary_key()
                && pk
                    .would_violate(&current_relation, &tuple)
                    .map_err(|e| DatabaseError::TransactionError(e.to_string()))?
            {
                return Err(DatabaseError::PrimaryKeyViolation);
            }

            // Check candidate keys
            for ck in key_constraints.candidate_keys() {
                if ck
                    .would_violate(&current_relation, &tuple)
                    .map_err(|e| DatabaseError::TransactionError(e.to_string()))?
                {
                    return Err(DatabaseError::CandidateKeyViolation);
                }
            }
        }

        // Check foreign key constraints (clone to avoid borrow issues)
        if let Some(fk_constraints) = self.foreign_key_constraints.get(relation_name).cloned() {
            for fk in fk_constraints.foreign_keys() {
                let referenced_relation = self.query(fk.referenced_relation_name())?;
                if fk
                    .would_violate_on_insert(&tuple, &referenced_relation)
                    .map_err(|e| DatabaseError::ForeignKeyViolation(e.to_string()))?
                {
                    return Err(DatabaseError::ForeignKeyViolation(format!(
                        "Tuple violates foreign key to {}",
                        fk.referenced_relation_name()
                    )));
                }
            }
        }

        // Get or open heap file
        let heap_file = self.get_or_open_heap_file(relation_name)?;

        // Insert tuple
        heap_file.insert_tuple(&tuple)?;

        Ok(())
    }

    /// Query a relation (returns the full relation for algebra operations)
    pub fn query(&mut self, relation_name: &str) -> Result<Relation, DatabaseError> {
        // Get relation metadata (clone to avoid borrow issues)
        let relation_type = self
            .catalog
            .get_relation(relation_name)
            .map_err(|_| DatabaseError::RelationNotFound(relation_name.to_string()))?
            .relation_type
            .clone();

        // Get or open heap file
        let heap_file = self.get_or_open_heap_file(relation_name)?;

        // Create empty relation with correct type
        let mut relation = Relation::new(relation_type);

        // Scan all tuples
        for tuple in heap_file.scan()? {
            relation.insert(tuple)?;
        }

        Ok(relation)
    }

    /// Delete tuples matching a predicate
    pub fn delete<F>(&mut self, relation_name: &str, predicate: F) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool,
    {
        // Clone foreign key constraints to avoid borrowing issues
        let fk_constraints_clone = self.foreign_key_constraints.clone();

        // Check for foreign key violations from other relations
        for (other_rel_name, fk_constraints) in &fk_constraints_clone {
            if other_rel_name == relation_name {
                continue; // Skip self
            }

            for fk in fk_constraints.foreign_keys() {
                if fk.referenced_relation_name() == relation_name {
                    // This relation is referenced, check if deletion would violate
                    let current_relation = self.query(relation_name)?;
                    let referencing_relation = self.query(other_rel_name)?;

                    for tuple in current_relation.tuples() {
                        if predicate(tuple)
                            && fk
                                .would_violate_on_delete(tuple, &referencing_relation)
                                .map_err(|e| DatabaseError::ForeignKeyViolation(e.to_string()))?
                        {
                            return Err(DatabaseError::ForeignKeyViolation(format!(
                                "Cannot delete: referenced by relation {}",
                                other_rel_name
                            )));
                        }
                    }
                }
            }
        }

        // Get metadata (clone to avoid borrow issues)
        let metadata = self
            .catalog
            .get_relation(relation_name)
            .map_err(|_| DatabaseError::RelationNotFound(relation_name.to_string()))?
            .clone();

        // Scan all tuples and collect ones to keep
        let heap_file = self.get_or_open_heap_file(relation_name)?;
        let all_tuples = heap_file.scan()?;

        let mut tuples_to_keep = Vec::new();
        let mut deleted_count = 0;

        for tuple in all_tuples {
            if predicate(&tuple) {
                deleted_count += 1;
            } else {
                tuples_to_keep.push(tuple);
            }
        }

        // Rebuild heap file
        self.heap_files.remove(relation_name);
        let mut new_heap_file =
            HeapFile::create(&metadata.heap_file_path, metadata.relation_type.clone())?;

        for tuple in tuples_to_keep {
            new_heap_file.insert_tuple(&tuple)?;
        }

        self.heap_files
            .insert(relation_name.to_string(), new_heap_file);

        Ok(deleted_count)
    }

    /// Update tuples matching a predicate
    pub fn update<F, U>(
        &mut self,
        relation_name: &str,
        predicate: F,
        updater: U,
    ) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool,
        U: Fn(&mut Tuple),
    {
        // Get metadata (clone to avoid borrow issues)
        let metadata = self
            .catalog
            .get_relation(relation_name)
            .map_err(|_| DatabaseError::RelationNotFound(relation_name.to_string()))?
            .clone();

        // Scan all tuples
        let heap_file = self.get_or_open_heap_file(relation_name)?;
        let all_tuples = heap_file.scan()?;

        let mut updated_tuples = Vec::new();
        let mut updated_count = 0;

        for mut tuple in all_tuples {
            if predicate(&tuple) {
                updater(&mut tuple);

                // Check type conformance
                if !tuple.conforms_to(metadata.relation_type.tuple_type()) {
                    return Err(DatabaseError::TupleMismatch);
                }

                updated_count += 1;
            }
            updated_tuples.push(tuple);
        }

        // Rebuild heap file
        self.heap_files.remove(relation_name);
        let mut new_heap_file =
            HeapFile::create(&metadata.heap_file_path, metadata.relation_type.clone())?;

        for tuple in updated_tuples {
            new_heap_file.insert_tuple(&tuple)?;
        }

        self.heap_files
            .insert(relation_name.to_string(), new_heap_file);

        Ok(updated_count)
    }

    /// Begin a transaction
    pub fn begin(&mut self) -> Result<(), DatabaseError> {
        if self.in_transaction {
            return Err(DatabaseError::TransactionError(
                "Transaction already in progress".to_string(),
            ));
        }

        // Save current state
        let mut savepoint = HashMap::new();
        for relation_name in self.catalog.list_relations() {
            let relation = self.query(&relation_name)?;
            savepoint.insert(relation_name, relation);
        }

        self.savepoint = Some(savepoint);
        self.in_transaction = true;

        Ok(())
    }

    /// Commit a transaction
    pub fn commit(&mut self) -> Result<(), DatabaseError> {
        if !self.in_transaction {
            return Err(DatabaseError::TransactionError(
                "No transaction in progress".to_string(),
            ));
        }

        // Clear savepoint (heap files auto-persist)
        self.savepoint = None;
        self.in_transaction = false;

        Ok(())
    }

    /// Rollback a transaction
    pub fn rollback(&mut self) -> Result<(), DatabaseError> {
        if !self.in_transaction {
            return Err(DatabaseError::TransactionError(
                "No transaction in progress".to_string(),
            ));
        }

        // Restore from savepoint
        if let Some(savepoint) = self.savepoint.take() {
            for (relation_name, saved_relation) in savepoint {
                // Clear heap file
                self.heap_files.remove(&relation_name);
                let metadata = self
                    .catalog
                    .get_relation(&relation_name)
                    .map_err(|_| DatabaseError::RelationNotFound(relation_name.clone()))?;
                let mut heap_file =
                    HeapFile::create(&metadata.heap_file_path, metadata.relation_type.clone())?;

                // Reinsert tuples
                for tuple in saved_relation.tuples() {
                    heap_file.insert_tuple(tuple)?;
                }

                self.heap_files.insert(relation_name.clone(), heap_file);
            }
        }

        self.in_transaction = false;

        Ok(())
    }

    /// Helper to get or open a heap file
    fn get_or_open_heap_file(
        &mut self,
        relation_name: &str,
    ) -> Result<&mut HeapFile, DatabaseError> {
        if !self.heap_files.contains_key(relation_name) {
            let metadata = self
                .catalog
                .get_relation(relation_name)
                .map_err(|_| DatabaseError::RelationNotFound(relation_name.to_string()))?;
            let heap_file =
                HeapFile::open(&metadata.heap_file_path, metadata.relation_type.clone())?;
            self.heap_files.insert(relation_name.to_string(), heap_file);
        }

        Ok(self.heap_files.get_mut(relation_name).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::{PrimaryKey, TypeConstraint};
    use crate::tuple;
    use crate::types::{ScalarType, TupleType};
    use crate::values::ScalarValue;
    use tempfile::TempDir;

    #[test]
    fn test_create_and_open_database() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Create database
        {
            let db = Database::open(&db_path).unwrap();
            assert_eq!(db.catalog.list_relations().len(), 0);
        }

        // Reopen database
        {
            let db = Database::open(&db_path).unwrap();
            assert_eq!(db.catalog.list_relations().len(), 0);
        }
    }

    #[test]
    fn test_create_relvar() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);

        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("TEST", relation_type).unwrap();

        assert!(db.catalog.get_relation("TEST").is_ok());
    }

    #[test]
    fn test_create_duplicate_relvar_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
        let relation_type = RelationType::new(tuple_type.clone());

        db.create_relvar("TEST", relation_type.clone()).unwrap();

        let result = db.create_relvar("TEST", RelationType::new(tuple_type));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::RelationAlreadyExists(_)
        ));
    }

    #[test]
    fn test_drop_relvar() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("TEST", relation_type).unwrap();
        assert!(db.catalog.get_relation("TEST").is_ok());

        db.drop_relvar("TEST").unwrap();
        assert!(db.catalog.get_relation("TEST").is_err());
    }

    #[test]
    fn test_insert_and_query() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("USERS", relation_type).unwrap();

        // Insert tuples
        db.insert("USERS", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        db.insert("USERS", tuple! { id: 2i64, name: "Bob" })
            .unwrap();

        // Query
        let result = db.query("USERS").unwrap();
        assert_eq!(result.cardinality(), 2);
    }

    #[test]
    fn test_query_with_algebra() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String)
            .with_attribute("age".to_string(), ScalarType::Int);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("USERS", relation_type).unwrap();

        db.insert("USERS", tuple! { id: 1i64, name: "Alice", age: 30i64 })
            .unwrap();
        db.insert("USERS", tuple! { id: 2i64, name: "Bob", age: 25i64 })
            .unwrap();
        db.insert("USERS", tuple! { id: 3i64, name: "Charlie", age: 30i64 })
            .unwrap();

        // Query with restrict and project
        let result = db
            .query("USERS")
            .unwrap()
            .restrict(|t| t.get_typed::<i64>("age").unwrap() == 30)
            .project(&["id", "name"]);

        assert_eq!(result.cardinality(), 2);
        assert_eq!(result.degree(), 2);
    }

    #[test]
    fn test_delete_tuples() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("USERS", relation_type).unwrap();

        db.insert("USERS", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        db.insert("USERS", tuple! { id: 2i64, name: "Bob" })
            .unwrap();

        // Delete Bob
        let deleted = db
            .delete("USERS", |t| t.get_typed::<i64>("id").unwrap() == 2)
            .unwrap();
        assert_eq!(deleted, 1);

        let result = db.query("USERS").unwrap();
        assert_eq!(result.cardinality(), 1);
    }

    #[test]
    fn test_update_tuples() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("USERS", relation_type).unwrap();

        db.insert("USERS", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Update name
        let updated = db
            .update(
                "USERS",
                |t| t.get_typed::<i64>("id").unwrap() == 1,
                |t| {
                    t.set(
                        "name".to_string(),
                        ScalarValue::String("Alicia".to_string()),
                    )
                    .unwrap();
                },
            )
            .unwrap();
        assert_eq!(updated, 1);

        let result = db.query("USERS").unwrap();
        let tuple = result.tuples().next().unwrap();
        assert_eq!(
            tuple.get_typed::<String>("name").unwrap(),
            "Alicia".to_string()
        );
    }

    #[test]
    fn test_primary_key_constraint() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("USERS", relation_type).unwrap();

        // Set primary key
        let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
        let key_constraints = KeyConstraints::new().with_primary_key(pk);
        db.set_key_constraints("USERS", key_constraints).unwrap();

        // Insert first tuple
        db.insert("USERS", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Try to insert duplicate key
        let result = db.insert("USERS", tuple! { id: 1i64, name: "Bob" });
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::PrimaryKeyViolation
        ));
    }

    #[test]
    fn test_foreign_key_constraint() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create DEPT relation
        let dept_type = TupleType::new()
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("dept_name".to_string(), ScalarType::String);
        db.create_relvar("DEPT", RelationType::new(dept_type))
            .unwrap();

        // Create EMP relation
        let emp_type = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String)
            .with_attribute("dept_id".to_string(), ScalarType::Int);
        db.create_relvar("EMP", RelationType::new(emp_type))
            .unwrap();

        // Add foreign key
        let fk = ForeignKey::new(
            vec!["dept_id".to_string()],
            "DEPT".to_string(),
            vec!["dept_id".to_string()],
        )
        .unwrap();
        db.add_foreign_key("EMP", fk).unwrap();

        // Insert department
        db.insert("DEPT", tuple! { dept_id: 10i64, dept_name: "Engineering" })
            .unwrap();

        // Insert employee with valid dept_id
        db.insert(
            "EMP",
            tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 },
        )
        .unwrap();

        // Try to insert employee with invalid dept_id
        let result = db.insert("EMP", tuple! { emp_id: 2i64, name: "Bob", dept_id: 99i64 });
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::ForeignKeyViolation(_)
        ));
    }

    #[test]
    fn test_transaction_commit() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("USERS", relation_type).unwrap();

        // Begin transaction
        db.begin().unwrap();

        // Insert tuples
        db.insert("USERS", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        db.insert("USERS", tuple! { id: 2i64, name: "Bob" })
            .unwrap();

        // Commit
        db.commit().unwrap();

        // Verify data persists
        let result = db.query("USERS").unwrap();
        assert_eq!(result.cardinality(), 2);
    }

    #[test]
    fn test_transaction_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("USERS", relation_type).unwrap();

        // Insert initial data
        db.insert("USERS", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Begin transaction
        db.begin().unwrap();

        // Insert more tuples
        db.insert("USERS", tuple! { id: 2i64, name: "Bob" })
            .unwrap();
        db.insert("USERS", tuple! { id: 3i64, name: "Charlie" })
            .unwrap();

        // Rollback
        db.rollback().unwrap();

        // Verify only original data remains
        let result = db.query("USERS").unwrap();
        assert_eq!(result.cardinality(), 1);
    }

    #[test]
    fn test_type_constraint_enforcement() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("age".to_string(), ScalarType::Int);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("USERS", relation_type).unwrap();

        // Set age constraint (must be positive)
        let age_constraint = AttributeConstraints::new("age".to_string(), ScalarType::Int)
            .with_constraint(TypeConstraint::PositiveInt);
        db.set_type_constraints("USERS", "age", age_constraint)
            .unwrap();

        // Insert valid tuple
        db.insert("USERS", tuple! { id: 1i64, age: 25i64 }).unwrap();

        // Try to insert invalid tuple
        let result = db.insert("USERS", tuple! { id: 2i64, age: -5i64 });
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::TypeConstraintViolation(_)
        ));
    }
}
