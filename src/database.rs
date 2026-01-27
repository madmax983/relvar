use crate::constraints::{AttributeConstraints, ForeignKey, ForeignKeyConstraints, KeyConstraints};
use crate::storage::{BTreeIndex, Catalog, CatalogError, HeapError, HeapFile};
use crate::types::RelationType;
use crate::values::relation::RelationError;
use crate::values::{Relation, Tuple};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors that can occur during database operations.
#[derive(Debug, Error)]
pub enum DatabaseError {
    /// An I/O error occurred.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// An error occurred in the catalog.
    #[error("Catalog error: {0}")]
    Catalog(#[from] CatalogError),
    /// An error occurred in a heap file.
    #[error("Heap file error: {0}")]
    HeapFile(#[from] HeapError),
    /// An error occurred in a relation.
    #[error("Relation error: {0}")]
    Relation(#[from] RelationError),
    /// A relation with the given name already exists.
    #[error("Relation {0} already exists")]
    RelationAlreadyExists(String),
    /// No relation with the given name was found.
    #[error("Relation {0} not found")]
    RelationNotFound(String),
    /// The tuple type does not match the relation type.
    #[error("Tuple type does not match relation type")]
    TupleMismatch,
    /// A primary key constraint was violated.
    #[error("Primary key constraint violation")]
    PrimaryKeyViolation,
    /// A candidate key constraint was violated.
    #[error("Candidate key constraint violation")]
    CandidateKeyViolation,
    /// A foreign key constraint was violated.
    #[error("Foreign key constraint violation: {0}")]
    ForeignKeyViolation(String),
    /// A type constraint was violated.
    #[error("Type constraint violation: {0}")]
    TypeConstraintViolation(String),
    /// The specified attribute was not found.
    #[error("Attribute {0} not found")]
    AttributeNotFound(String),
    /// A transaction error occurred.
    #[error("Transaction error: {0}")]
    TransactionError(String),
    /// A view with the given name already exists.
    ///
    /// TTM: RM Prescription 10 - View names must be unique within the database.
    #[error("View {0} already exists")]
    ViewAlreadyExists(String),
    /// No view with the given name was found.
    ///
    /// TTM: RM Prescription 10 - Views must exist to be queried or dropped.
    #[error("View {0} not found")]
    ViewNotFound(String),
    /// Cannot modify a view because views are read-only.
    ///
    /// TTM: RM Prescription 10 - Views are virtual relvars and cannot be
    /// directly modified. Modifications must be made to the underlying
    /// base relvars.
    #[error("Cannot modify view {0}: views are read-only")]
    CannotModifyView(String),
}

/// Type alias for view definition functions.
///
/// A view definition is a closure that takes a mutable reference to the database
/// and returns a [`Relation`] value. The closure is re-evaluated each time the
/// view is queried, ensuring the view always reflects the current state of the
/// underlying base relvars.
///
/// TTM: RM Prescription 10 - Views are virtual relation variables defined by
/// relational expressions. They are not stored, but computed on demand.
///
/// # Thread Safety
///
/// View definitions must be `Send + Sync` to allow the database to be used
/// across threads safely.
pub type ViewDefinition =
    Box<dyn Fn(&mut Database) -> Result<Relation, DatabaseError> + Send + Sync>;

/// A database instance managing relations, storage, and constraints
///
/// TTM: RM Prescription 10 - Supports both base relvars (stored relations)
/// and views (virtual relvars defined by expressions).
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
    /// Views (virtual relvars) - TTM RM Prescription 10
    views: HashMap<String, ViewDefinition>,
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
            views: HashMap::new(),
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

        // Check if a view with this name exists
        if self.views.contains_key(name) {
            return Err(DatabaseError::ViewAlreadyExists(name.to_string()));
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

    /// Create a new view (virtual relvar).
    ///
    /// TTM: RM Prescription 10 - The system must support views (virtual relation
    /// variables defined by expressions).
    ///
    /// Views are read-only and re-evaluate their defining expression on each query.
    /// The view definition is a closure that takes a mutable reference to the database
    /// and returns a relation value.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::Database;
    /// use relvar::types::{RelationType, ScalarType, TupleType};
    /// use relvar::tuple;
    ///
    /// let temp_dir = tempfile::TempDir::new().unwrap();
    /// let mut db = Database::open(temp_dir.path()).unwrap();
    ///
    /// // Create a base relvar
    /// let emp_type = TupleType::new()
    ///     .with_attribute("id".to_string(), ScalarType::Int)
    ///     .with_attribute("salary".to_string(), ScalarType::Float);
    /// db.create_relvar("EMP", RelationType::new(emp_type)).unwrap();
    ///
    /// // Insert some data
    /// db.insert("EMP", tuple! { id: 1i64, salary: 150000.0 }).unwrap();
    /// db.insert("EMP", tuple! { id: 2i64, salary: 50000.0 }).unwrap();
    ///
    /// // Create a view for high earners (salary > 100000)
    /// db.create_view("HIGH_EARNERS", |db| {
    ///     Ok(db.query("EMP")?
    ///         .restrict(|t| t.get_typed::<f64>("salary").unwrap() > 100000.0))
    /// }).unwrap();
    ///
    /// // Query the view - re-evaluates each time
    /// let high_earners = db.query("HIGH_EARNERS").unwrap();
    /// assert_eq!(high_earners.cardinality(), 1);
    ///
    /// // Views are read-only - insert fails
    /// let result = db.insert("HIGH_EARNERS", tuple! { id: 3i64, salary: 200000.0 });
    /// assert!(result.is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::RelationAlreadyExists`] if a base relvar with the
    /// same name already exists.
    ///
    /// Returns [`DatabaseError::ViewAlreadyExists`] if a view with the same name
    /// already exists.
    pub fn create_view<F>(&mut self, name: &str, definition: F) -> Result<(), DatabaseError>
    where
        F: Fn(&mut Database) -> Result<Relation, DatabaseError> + Send + Sync + 'static,
    {
        // Check if a base relvar with this name exists
        if self.catalog.get_relation(name).is_ok() {
            return Err(DatabaseError::RelationAlreadyExists(name.to_string()));
        }

        // Check if a view with this name already exists
        if self.views.contains_key(name) {
            return Err(DatabaseError::ViewAlreadyExists(name.to_string()));
        }

        // Store the view definition
        self.views.insert(name.to_string(), Box::new(definition));

        Ok(())
    }

    /// Drop a view.
    ///
    /// Removes the view definition from the database. This does not affect
    /// any base relvars that the view was derived from.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::ViewNotFound`] if no view with the given name exists.
    pub fn drop_view(&mut self, name: &str) -> Result<(), DatabaseError> {
        if self.views.remove(name).is_none() {
            return Err(DatabaseError::ViewNotFound(name.to_string()));
        }
        Ok(())
    }

    /// Check if a view exists.
    ///
    /// Returns `true` if a view with the given name exists, `false` otherwise.
    /// This only checks for views, not base relvars.
    pub fn view_exists(&self, name: &str) -> bool {
        self.views.contains_key(name)
    }

    /// List all view names.
    ///
    /// Returns a vector of all view names currently defined in the database.
    /// This does not include base relvars.
    pub fn list_views(&self) -> Vec<String> {
        self.views.keys().cloned().collect()
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
        // Check if trying to insert into a view
        if self.views.contains_key(relation_name) {
            return Err(DatabaseError::CannotModifyView(relation_name.to_string()));
        }

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

    /// Query a relation or view (returns the full relation for algebra operations).
    ///
    /// For base relvars, this loads the relation from storage.
    /// For views, this re-evaluates the view definition and returns the result.
    ///
    /// TTM: RM Prescription 10 - Views re-evaluate their defining expression
    /// on each query, ensuring they always reflect the current state of the
    /// underlying base relvars.
    pub fn query(&mut self, relation_name: &str) -> Result<Relation, DatabaseError> {
        // Check if this is a view - if so, evaluate the view definition
        if self.views.contains_key(relation_name) {
            return self.query_view(relation_name);
        }

        // Otherwise, query the base relvar
        self.query_base_relvar(relation_name)
    }

    /// Query a base relvar (stored relation).
    fn query_base_relvar(&mut self, relation_name: &str) -> Result<Relation, DatabaseError> {
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

    /// Query a view by evaluating its definition.
    ///
    /// This is tricky because the view definition needs &mut self to query
    /// base relvars, but we can't hold a reference to the view while calling it.
    /// We solve this by temporarily removing the view, evaluating it, and
    /// putting it back.
    fn query_view(&mut self, view_name: &str) -> Result<Relation, DatabaseError> {
        // Temporarily remove the view definition to avoid borrow issues
        let view_def = self
            .views
            .remove(view_name)
            .ok_or_else(|| DatabaseError::ViewNotFound(view_name.to_string()))?;

        // Evaluate the view definition
        let result = view_def(self);

        // Put the view definition back
        self.views.insert(view_name.to_string(), view_def);

        result
    }

    /// Delete tuples matching a predicate
    pub fn delete<F>(&mut self, relation_name: &str, predicate: F) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool,
    {
        // Check if trying to delete from a view
        if self.views.contains_key(relation_name) {
            return Err(DatabaseError::CannotModifyView(relation_name.to_string()));
        }

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
        // Check if trying to update a view
        if self.views.contains_key(relation_name) {
            return Err(DatabaseError::CannotModifyView(relation_name.to_string()));
        }

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

    // ==========================================================================
    // View Tests (TTM RM Prescription 10 - Virtual Relation Variables)
    // ==========================================================================

    #[test]
    fn test_create_view() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create base relvar
        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String)
            .with_attribute("salary".to_string(), ScalarType::Float);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("EMP", relation_type).unwrap();

        // Create view for high earners
        db.create_view("HIGH_EARNERS", |db| {
            Ok(db
                .query("EMP")?
                .restrict(|t| t.get_typed::<f64>("salary").unwrap() > 100000.0))
        })
        .unwrap();

        // View should exist
        assert!(db.view_exists("HIGH_EARNERS"));
    }

    #[test]
    fn test_view_re_evaluates_on_query() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create base relvar
        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String)
            .with_attribute("salary".to_string(), ScalarType::Float);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("EMP", relation_type).unwrap();

        // Insert initial data
        db.insert("EMP", tuple! { id: 1i64, name: "Alice", salary: 150000.0 })
            .unwrap();
        db.insert("EMP", tuple! { id: 2i64, name: "Bob", salary: 50000.0 })
            .unwrap();

        // Create view
        db.create_view("HIGH_EARNERS", |db| {
            Ok(db
                .query("EMP")?
                .restrict(|t| t.get_typed::<f64>("salary").unwrap() > 100000.0))
        })
        .unwrap();

        // Query view - should have 1 tuple
        let result = db.query("HIGH_EARNERS").unwrap();
        assert_eq!(result.cardinality(), 1);

        // Insert another high earner into base relvar
        db.insert(
            "EMP",
            tuple! { id: 3i64, name: "Charlie", salary: 200000.0 },
        )
        .unwrap();

        // Query view again - should now have 2 tuples (re-evaluated)
        let result = db.query("HIGH_EARNERS").unwrap();
        assert_eq!(result.cardinality(), 2);
    }

    #[test]
    fn test_view_appears_in_catalog() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create base relvar
        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("EMP", relation_type).unwrap();

        // Create view
        db.create_view("NAMES_ONLY", |db| Ok(db.query("EMP")?.project(&["name"])))
            .unwrap();

        // View should be listed
        let views = db.list_views();
        assert!(views.contains(&"NAMES_ONLY".to_string()));
    }

    #[test]
    fn test_drop_view() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create base relvar
        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("EMP", relation_type).unwrap();

        // Create view
        db.create_view("NAMES_ONLY", |db| Ok(db.query("EMP")?.project(&["name"])))
            .unwrap();

        assert!(db.view_exists("NAMES_ONLY"));

        // Drop view
        db.drop_view("NAMES_ONLY").unwrap();

        assert!(!db.view_exists("NAMES_ONLY"));
    }

    #[test]
    fn test_insert_into_view_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create base relvar
        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("EMP", relation_type).unwrap();

        // Create view
        db.create_view("EMP_VIEW", |db| db.query("EMP")).unwrap();

        // Try to insert into view - should fail
        let result = db.insert("EMP_VIEW", tuple! { id: 1i64, name: "Alice" });
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::CannotModifyView(_)
        ));
    }

    #[test]
    fn test_delete_from_view_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create base relvar
        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("EMP", relation_type).unwrap();
        db.insert("EMP", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Create view
        db.create_view("EMP_VIEW", |db| db.query("EMP")).unwrap();

        // Try to delete from view - should fail
        let result = db.delete("EMP_VIEW", |_| true);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::CannotModifyView(_)
        ));
    }

    #[test]
    fn test_update_view_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create base relvar
        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("EMP", relation_type).unwrap();
        db.insert("EMP", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Create view
        db.create_view("EMP_VIEW", |db| db.query("EMP")).unwrap();

        // Try to update view - should fail
        let result = db.update(
            "EMP_VIEW",
            |_| true,
            |t| {
                t.set("name".to_string(), ScalarValue::String("Bob".to_string()))
                    .unwrap();
            },
        );
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::CannotModifyView(_)
        ));
    }

    #[test]
    fn test_query_nonexistent_view_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Query nonexistent view/relvar
        let result = db.query("NONEXISTENT");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::RelationNotFound(_)
        ));
    }

    #[test]
    fn test_create_duplicate_view_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create base relvar
        let tuple_type = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("EMP", relation_type).unwrap();

        // Create view
        db.create_view("EMP_VIEW", |db| db.query("EMP")).unwrap();

        // Try to create duplicate view
        let result = db.create_view("EMP_VIEW", |db| db.query("EMP"));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::ViewAlreadyExists(_)
        ));
    }

    #[test]
    fn test_view_with_same_name_as_relvar_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create base relvar
        let tuple_type = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("EMP", relation_type).unwrap();

        // Try to create view with same name as relvar
        let result = db.create_view("EMP", |db| db.query("EMP"));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::RelationAlreadyExists(_)
        ));
    }

    #[test]
    fn test_relvar_with_same_name_as_view_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create base relvar
        let tuple_type = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
        let relation_type = RelationType::new(tuple_type.clone());

        db.create_relvar("EMP", relation_type).unwrap();

        // Create view
        db.create_view("EMP_VIEW", |db| db.query("EMP")).unwrap();

        // Try to create relvar with same name as view
        let result = db.create_relvar("EMP_VIEW", RelationType::new(tuple_type));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::ViewAlreadyExists(_)
        ));
    }

    #[test]
    fn test_drop_nonexistent_view_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let result = db.drop_view("NONEXISTENT");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::ViewNotFound(_)
        ));
    }

    #[test]
    fn test_view_with_join() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create EMP relvar
        let emp_type = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String)
            .with_attribute("dept_id".to_string(), ScalarType::Int);
        db.create_relvar("EMP", RelationType::new(emp_type))
            .unwrap();

        // Create DEPT relvar
        let dept_type = TupleType::new()
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("dept_name".to_string(), ScalarType::String);
        db.create_relvar("DEPT", RelationType::new(dept_type))
            .unwrap();

        // Insert data
        db.insert(
            "EMP",
            tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 },
        )
        .unwrap();
        db.insert("EMP", tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
            .unwrap();
        db.insert("DEPT", tuple! { dept_id: 10i64, dept_name: "Engineering" })
            .unwrap();
        db.insert("DEPT", tuple! { dept_id: 20i64, dept_name: "Sales" })
            .unwrap();

        // Create view joining EMP and DEPT
        db.create_view("EMP_WITH_DEPT", |db| {
            let emp = db.query("EMP")?;
            let dept = db.query("DEPT")?;
            Ok(emp.join(&dept))
        })
        .unwrap();

        // Query view
        let result = db.query("EMP_WITH_DEPT").unwrap();
        assert_eq!(result.cardinality(), 2);
        assert_eq!(result.degree(), 4); // emp_id, name, dept_id, dept_name
    }
}
