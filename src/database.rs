//! Core database instance and operations.
//!
//! This module provides the [`Database`] struct, which is the main entry point
//! for all database operations including:
//!
//! - Creating and dropping relations (base relvars)
//! - Inserting, updating, and deleting tuples
//! - Querying relations with relational algebra
//! - Managing transactions
//! - Setting up constraints (primary keys, foreign keys, type constraints)
//!
//! # TTM Compliance
//!
//! The database enforces The Third Manifesto principles:
//!
//! - **No NULL values**: All tuple attributes must have values
//! - **No duplicates**: Relations are true sets
//! - **Constraint enforcement**: Keys and foreign keys are validated on mutation
//! - **Type safety**: Tuples must conform to their relation's heading
//!
//! # Example
//!
//! ```no_run
//! use relvar::Database;
//! use relvar::types::{TupleType, RelationType, ScalarType};
//! use relvar::constraints::{PrimaryKey, KeyConstraints};
//! use relvar::tuple;
//!
//! // Open or create a database
//! let mut db = Database::open("my_database")?;
//!
//! // Define and create a relation
//! let emp_type = TupleType::new()
//!     .with_attribute("emp_id", ScalarType::Int)
//!     .with_attribute("name", ScalarType::String);
//!
//! db.create_relvar("EMP", RelationType::new(emp_type))?;
//!
//! // Set primary key constraint
//! let pk = PrimaryKey::new(vec!["emp_id".to_string()]).unwrap();
//! db.set_key_constraints("EMP", KeyConstraints::new().with_primary_key(pk))?;
//!
//! // Insert data
//! db.insert("EMP", tuple! { emp_id: 1i64, name: "Alice" })?;
//!
//! // Query with relational algebra
//! let result = db.query("EMP")?.project(&["name"]);
//!
//! # Ok::<(), relvar::DatabaseError>(())
//! ```

use crate::constraints::{AttributeConstraints, ForeignKey, ForeignKeyConstraints, KeyConstraints};
use crate::storage::{
    BTreeIndex, Catalog, CatalogError, HeapError, HeapFile, SYS_ATTRIBUTES, SYS_CONSTRAINTS,
    SYS_RELVARS, is_system_relvar, serialize_scalar_type, sys_attributes_type,
    sys_constraints_type, sys_relvars_type,
};
use crate::tuple;
use crate::types::RelationType;
use crate::values::relation::RelationError;
use crate::values::{Relation, Tuple};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors that can occur during database operations.
///
/// This enum covers all error conditions including I/O errors, constraint
/// violations, and transaction errors.
#[derive(Debug, Error)]
pub enum DatabaseError {
    /// An I/O error occurred during file operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// An error occurred in the system catalog.
    #[error("Catalog error: {0}")]
    Catalog(#[from] CatalogError),

    /// An error occurred in the heap file storage.
    #[error("Heap file error: {0}")]
    HeapFile(#[from] HeapError),

    /// An error occurred with a relation value.
    #[error("Relation error: {0}")]
    Relation(#[from] RelationError),

    /// Attempted to create a relation that already exists.
    #[error("Relation {0} already exists")]
    RelationAlreadyExists(String),

    /// The specified relation does not exist.
    #[error("Relation {0} not found")]
    RelationNotFound(String),

    /// The tuple's type does not match the relation's heading.
    ///
    /// This occurs when inserting or updating a tuple that doesn't
    /// conform to the relation's declared type.
    #[error("Tuple type does not match relation type")]
    TupleMismatch,

    /// Inserting a tuple would violate the primary key constraint.
    ///
    /// Primary key values must be unique across all tuples in a relation.
    #[error("Primary key constraint violation")]
    PrimaryKeyViolation,

    /// Inserting a tuple would violate a candidate key constraint.
    ///
    /// Candidate key values must be unique across all tuples in a relation.
    #[error("Candidate key constraint violation")]
    CandidateKeyViolation,

    /// A foreign key constraint was violated.
    ///
    /// This occurs when:
    /// - Inserting a tuple with a foreign key value that doesn't exist in the referenced relation
    /// - Deleting a tuple that is referenced by another relation
    #[error("Foreign key constraint violation: {0}")]
    ForeignKeyViolation(String),

    /// A type constraint was violated.
    ///
    /// This occurs when a value doesn't satisfy the defined type constraints
    /// (e.g., range, enum, string length).
    #[error("Type constraint violation: {0}")]
    TypeConstraintViolation(String),

    /// The specified attribute does not exist in the relation.
    #[error("Attribute {0} not found")]
    AttributeNotFound(String),

    /// A transaction-related error occurred.
    ///
    /// This includes attempting to commit/rollback without an active transaction,
    /// or beginning a transaction when one is already in progress.
    #[error("Transaction error: {0}")]
    TransactionError(String),

    /// A virtual relvar with the given name already exists.
    ///
    /// TTM: RM Prescription 10 - Virtual relvar names must be unique within the database.
    #[error("Virtual relvar {0} already exists")]
    VirtualRelvarAlreadyExists(String),

    /// No virtual relvar with the given name was found.
    ///
    /// TTM: RM Prescription 10 - Virtual relvars must exist to be queried or dropped.
    #[error("Virtual relvar {0} not found")]
    VirtualRelvarNotFound(String),

    /// Cannot modify a virtual relvar because virtual relvars are read-only.
    ///
    /// TTM: RM Prescription 10 - Virtual relvars cannot be directly modified.
    /// Modifications must be made to the underlying base relvars.
    #[error("Cannot modify virtual relvar {0}: virtual relvars are read-only")]
    CannotModifyVirtualRelvar(String),

    /// The SYS_ prefix is reserved for system relvars.
    ///
    /// TTM: RM Prescription 12 - System catalog relvars use the SYS_ prefix.
    /// User-defined relvars cannot use this reserved prefix.
    #[error("Relvar name {0} uses reserved SYS_ prefix")]
    SystemRelvarReserved(String),

    /// Cannot drop a system relvar.
    ///
    /// TTM: RM Prescription 12 - System catalog relvars cannot be dropped.
    #[error("Cannot drop system relvar {0}: system relvars are protected")]
    CannotDropSystemRelvar(String),
}

/// Type alias for virtual relvar definition functions.
///
/// A virtual relvar definition is a closure that takes a mutable reference to the
/// database and returns a [`Relation`] value. The closure is re-evaluated each time
/// the virtual relvar is queried, ensuring it always reflects the current state of
/// the underlying base relvars.
///
/// TTM: RM Prescription 10 - Virtual relvars (virtual relation variables) are defined
/// by relational expressions. They are not stored, but computed on demand.
///
/// # Thread Safety
///
/// Virtual relvar definitions must be `Send + Sync` to allow the database to be used
/// across threads safely.
pub type VirtualRelvarDefinition =
    Box<dyn Fn(&mut Database) -> Result<Relation, DatabaseError> + Send + Sync>;

/// A database instance managing relations, storage, and constraints.
///
/// `Database` is the main entry point for all database operations. It manages:
///
/// - **Relations (Relvars)**: Create, drop, and query base relations
/// - **Virtual Relvars**: Virtual relation variables defined by expressions (TTM RM Prescription 10)
/// - **Data Manipulation**: Insert, update, and delete tuples
/// - **Constraints**: Primary keys, foreign keys, and type constraints
/// - **Transactions**: Begin, commit, and rollback operations
/// - **Storage**: Heap files and indexes for persistent storage
///
/// # Architecture
///
/// ```text
/// Database
/// ├── Catalog (metadata about all relations)
/// ├── HeapFiles (tuple storage, keyed by relation name)
/// ├── Indexes (B-tree indexes for efficient lookups)
/// └── Constraints
///     ├── KeyConstraints (primary and candidate keys)
///     ├── ForeignKeyConstraints (referential integrity)
///     └── TypeConstraints (value domain restrictions)
/// ```
///
/// # Example
///
/// ```no_run
/// use relvar::Database;
/// use relvar::types::{TupleType, RelationType, ScalarType};
/// use relvar::tuple;
///
/// // Create or open a database
/// let mut db = Database::open("my_db")?;
///
/// // Create a relation
/// let heading = TupleType::new()
///     .with_attribute("id", ScalarType::Int)
///     .with_attribute("name", ScalarType::String);
/// db.create_relvar("USERS", RelationType::new(heading))?;
///
/// // Insert tuples
/// db.insert("USERS", tuple! { id: 1i64, name: "Alice" })?;
///
/// // Query with relational algebra
/// let users = db.query("USERS")?;
/// let names = users.project(&["name"]);
///
/// // Use transactions for atomic operations
/// db.begin()?;
/// db.insert("USERS", tuple! { id: 2i64, name: "Bob" })?;
/// db.commit()?;  // or db.rollback()?
///
/// # Ok::<(), relvar::DatabaseError>(())
/// ```
pub struct Database {
    /// Base directory for database files.
    db_path: PathBuf,
    /// Path to the catalog file.
    catalog_path: PathBuf,
    /// System catalog containing relation metadata.
    catalog: Catalog,
    /// Open heap files, keyed by relation name.
    heap_files: HashMap<String, HeapFile>,
    /// B-tree indexes, keyed by "relation_name.attribute".
    indexes: HashMap<String, BTreeIndex>,
    /// Key constraints (primary and candidate) per relation.
    key_constraints: HashMap<String, KeyConstraints>,
    /// Foreign key constraints per relation.
    foreign_key_constraints: HashMap<String, ForeignKeyConstraints>,
    /// Type constraints per relation, per attribute.
    type_constraints: HashMap<String, HashMap<String, AttributeConstraints>>,
    /// Whether a transaction is currently in progress.
    in_transaction: bool,
    /// Savepoint data for transaction rollback.
    ///
    /// This is a simplified implementation that stores full relation snapshots.
    /// A production system would use write-ahead logging (WAL).
    savepoint: Option<HashMap<String, Relation>>,
    /// Virtual relvars defined by expressions.
    ///
    /// TTM: RM Prescription 10 - Virtual relvars (virtual relation variables)
    /// re-evaluate their defining expression on each query.
    virtual_relvars: HashMap<String, VirtualRelvarDefinition>,
}

impl Database {
    /// Opens or creates a database at the specified path.
    ///
    /// If the database directory does not exist, it will be created along with
    /// an empty catalog. If the database already exists, the catalog is loaded
    /// from disk.
    ///
    /// # Arguments
    ///
    /// * `path` - The directory path where the database files will be stored
    ///
    /// # Returns
    ///
    /// A `Database` instance ready for use.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The directory cannot be created
    /// - The catalog file cannot be read or created
    ///
    /// # Example
    ///
    /// ```no_run
    /// use relvar::Database;
    ///
    /// let db = Database::open("my_database")?;
    /// # Ok::<(), relvar::DatabaseError>(())
    /// ```
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

        let mut db = Self {
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
            virtual_relvars: HashMap::new(),
        };

        // Register system catalog relvars
        db.register_system_relvars();

        Ok(db)
    }

    /// Creates a new relation (base relvar) in the database.
    ///
    /// A relation is defined by its [`RelationType`], which specifies the
    /// heading (attribute names and types). The relation is initially empty.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the relation (case-sensitive)
    /// * `relation_type` - The type definition for the relation
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::RelationAlreadyExists`] if a relation with
    /// the given name already exists.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use relvar::Database;
    /// use relvar::types::{TupleType, RelationType, ScalarType};
    ///
    /// let mut db = Database::open("test_db")?;
    ///
    /// let emp_type = TupleType::new()
    ///     .with_attribute("emp_id", ScalarType::Int)
    ///     .with_attribute("name", ScalarType::String);
    ///
    /// db.create_relvar("EMPLOYEES", RelationType::new(emp_type))?;
    /// # Ok::<(), relvar::DatabaseError>(())
    /// ```
    pub fn create_relvar(
        &mut self,
        name: &str,
        relation_type: RelationType,
    ) -> Result<(), DatabaseError> {
        // Check if name uses reserved SYS_ prefix
        if is_system_relvar(name) {
            return Err(DatabaseError::SystemRelvarReserved(name.to_string()));
        }

        // Check if relation already exists
        if self.catalog.get_relation(name).is_ok() {
            return Err(DatabaseError::RelationAlreadyExists(name.to_string()));
        }

        // Check if a virtual relvar with this name exists
        if self.virtual_relvars.contains_key(name) {
            return Err(DatabaseError::VirtualRelvarAlreadyExists(name.to_string()));
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

    /// Drops (deletes) a relation from the database.
    ///
    /// This removes the relation and all its data, including associated
    /// constraints and indexes.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the relation to drop
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::RelationNotFound`] if the relation does not exist.
    ///
    /// # Warning
    ///
    /// This operation is irreversible outside of a transaction. All data in
    /// the relation will be permanently deleted.
    pub fn drop_relvar(&mut self, name: &str) -> Result<(), DatabaseError> {
        // Protect system relvars from being dropped
        if is_system_relvar(name) {
            return Err(DatabaseError::CannotDropSystemRelvar(name.to_string()));
        }

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

    /// Creates a new virtual relvar.
    ///
    /// TTM: RM Prescription 10 - The system must support virtual relvars (virtual
    /// relation variables defined by expressions).
    ///
    /// Virtual relvars are read-only and re-evaluate their defining expression on
    /// each query. The definition is a closure that takes a mutable reference to
    /// the database and returns a relation value.
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
    /// // Create a virtual relvar for high earners (salary > 100000)
    /// db.create_virtual_relvar("HIGH_EARNERS", |db| {
    ///     Ok(db.query("EMP")?
    ///         .restrict(|t| t.get_typed::<f64>("salary").unwrap() > 100000.0))
    /// }).unwrap();
    ///
    /// // Query the virtual relvar - re-evaluates each time
    /// let high_earners = db.query("HIGH_EARNERS").unwrap();
    /// assert_eq!(high_earners.cardinality(), 1);
    ///
    /// // Virtual relvars are read-only - insert fails
    /// let result = db.insert("HIGH_EARNERS", tuple! { id: 3i64, salary: 200000.0 });
    /// assert!(result.is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::RelationAlreadyExists`] if a base relvar with the
    /// same name already exists.
    ///
    /// Returns [`DatabaseError::VirtualRelvarAlreadyExists`] if a virtual relvar
    /// with the same name already exists.
    pub fn create_virtual_relvar<F>(
        &mut self,
        name: &str,
        definition: F,
    ) -> Result<(), DatabaseError>
    where
        F: Fn(&mut Database) -> Result<Relation, DatabaseError> + Send + Sync + 'static,
    {
        // Check if name uses reserved SYS_ prefix
        if is_system_relvar(name) {
            return Err(DatabaseError::SystemRelvarReserved(name.to_string()));
        }

        // Check if a base relvar with this name exists
        if self.catalog.get_relation(name).is_ok() {
            return Err(DatabaseError::RelationAlreadyExists(name.to_string()));
        }

        // Check if a virtual relvar with this name already exists
        if self.virtual_relvars.contains_key(name) {
            return Err(DatabaseError::VirtualRelvarAlreadyExists(name.to_string()));
        }

        // Store the virtual relvar definition
        self.virtual_relvars
            .insert(name.to_string(), Box::new(definition));

        Ok(())
    }

    /// Drops a virtual relvar.
    ///
    /// Removes the virtual relvar definition from the database. This does not
    /// affect any base relvars that the virtual relvar was derived from.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::VirtualRelvarNotFound`] if no virtual relvar with
    /// the given name exists.
    pub fn drop_virtual_relvar(&mut self, name: &str) -> Result<(), DatabaseError> {
        // Protect system relvars from being dropped
        if is_system_relvar(name) {
            return Err(DatabaseError::CannotDropSystemRelvar(name.to_string()));
        }

        if self.virtual_relvars.remove(name).is_none() {
            return Err(DatabaseError::VirtualRelvarNotFound(name.to_string()));
        }
        Ok(())
    }

    /// Checks if a virtual relvar exists.
    ///
    /// Returns `true` if a virtual relvar with the given name exists, `false` otherwise.
    /// This only checks for virtual relvars, not base relvars.
    pub fn virtual_relvar_exists(&self, name: &str) -> bool {
        self.virtual_relvars.contains_key(name)
    }

    /// Lists all virtual relvar names.
    ///
    /// Returns a vector of all virtual relvar names currently defined in the database.
    /// This does not include base relvars.
    pub fn list_virtual_relvars(&self) -> Vec<String> {
        self.virtual_relvars.keys().cloned().collect()
    }

    /// Sets key constraints (primary and candidate keys) for a relation.
    ///
    /// Key constraints ensure uniqueness of specified attribute combinations.
    /// A primary key is a designated candidate key that uniquely identifies tuples.
    ///
    /// # Arguments
    ///
    /// * `relation_name` - The name of the relation
    /// * `constraints` - The key constraints to apply
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::RelationNotFound`] if the relation does not exist.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use relvar::Database;
    /// use relvar::constraints::{PrimaryKey, CandidateKey, KeyConstraints};
    ///
    /// # let mut db = Database::open("test")?;
    /// let pk = PrimaryKey::new(vec!["emp_id".to_string()])?;
    /// let ck = CandidateKey::new(vec!["email".to_string()])?;
    ///
    /// let constraints = KeyConstraints::new()
    ///     .with_primary_key(pk)
    ///     .with_candidate_key(ck);
    ///
    /// db.set_key_constraints("EMPLOYEES", constraints)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
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

    /// Adds a foreign key constraint to a relation.
    ///
    /// Foreign keys enforce referential integrity by requiring that values in
    /// specified attributes exist in another relation's corresponding attributes.
    ///
    /// # Arguments
    ///
    /// * `relation_name` - The name of the referencing relation
    /// * `foreign_key` - The foreign key constraint to add
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::RelationNotFound`] if the relation does not exist.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use relvar::Database;
    /// use relvar::constraints::ForeignKey;
    ///
    /// # let mut db = Database::open("test")?;
    /// // EMP.dept_id references DEPT.dept_id
    /// let fk = ForeignKey::new(
    ///     vec!["dept_id".to_string()],
    ///     "DEPT".to_string(),
    ///     vec!["dept_id".to_string()],
    /// )?;
    ///
    /// db.add_foreign_key("EMP", fk)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
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

    /// Sets type constraints for an attribute in a relation.
    ///
    /// Type constraints restrict the allowed values for an attribute beyond
    /// its base type. Supported constraints include ranges, enumerations,
    /// string length limits, and custom validators.
    ///
    /// # Arguments
    ///
    /// * `relation_name` - The name of the relation
    /// * `attribute` - The name of the attribute to constrain
    /// * `constraints` - The constraints to apply
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::RelationNotFound`] if the relation does not exist.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use relvar::Database;
    /// use relvar::types::ScalarType;
    /// use relvar::constraints::{AttributeConstraints, TypeConstraint};
    ///
    /// # let mut db = Database::open("test")?;
    /// // Age must be positive
    /// let age_constraint = AttributeConstraints::new("age".to_string(), ScalarType::Int)
    ///     .with_constraint(TypeConstraint::PositiveInt);
    ///
    /// db.set_type_constraints("EMPLOYEES", "age", age_constraint)?;
    /// # Ok::<(), relvar::DatabaseError>(())
    /// ```
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

    /// Inserts a tuple into a relation.
    ///
    /// The tuple must conform to the relation's type (heading). All constraints
    /// are validated before insertion:
    ///
    /// 1. **Type matching**: Tuple type must match relation type
    /// 2. **Type constraints**: Values must satisfy attribute constraints
    /// 3. **Primary key**: No duplicate key values
    /// 4. **Candidate keys**: No duplicate key values
    /// 5. **Foreign keys**: Referenced values must exist
    ///
    /// # Arguments
    ///
    /// * `relation_name` - The name of the relation
    /// * `tuple` - The tuple to insert
    ///
    /// # Errors
    ///
    /// - [`DatabaseError::RelationNotFound`] - Relation does not exist
    /// - [`DatabaseError::TupleMismatch`] - Tuple type doesn't match relation type
    /// - [`DatabaseError::TypeConstraintViolation`] - Value violates type constraint
    /// - [`DatabaseError::PrimaryKeyViolation`] - Duplicate primary key
    /// - [`DatabaseError::CandidateKeyViolation`] - Duplicate candidate key
    /// - [`DatabaseError::ForeignKeyViolation`] - Referenced value doesn't exist
    ///
    /// # Example
    ///
    /// ```no_run
    /// use relvar::Database;
    /// use relvar::tuple;
    ///
    /// # let mut db = Database::open("test")?;
    /// db.insert("EMPLOYEES", tuple! {
    ///     emp_id: 1i64,
    ///     name: "Alice",
    ///     dept_id: 10i64
    /// })?;
    /// # Ok::<(), relvar::DatabaseError>(())
    /// ```
    pub fn insert(&mut self, relation_name: &str, tuple: Tuple) -> Result<(), DatabaseError> {
        // Check if trying to insert into a virtual relvar
        if self.virtual_relvars.contains_key(relation_name) {
            return Err(DatabaseError::CannotModifyVirtualRelvar(
                relation_name.to_string(),
            ));
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

    /// Queries a relation or virtual relvar, returning all tuples as a [`Relation`] value.
    ///
    /// For base relvars, this loads the relation from storage.
    /// For virtual relvars, this re-evaluates the definition and returns the result.
    ///
    /// TTM: RM Prescription 10 - Virtual relvars re-evaluate their defining expression
    /// on each query, ensuring they always reflect the current state of the
    /// underlying base relvars.
    ///
    /// The returned relation can be transformed using relational algebra
    /// operators such as `project`, `restrict`, `join`, `union`, etc.
    ///
    /// # Arguments
    ///
    /// * `relation_name` - The name of the relation or virtual relvar to query
    ///
    /// # Returns
    ///
    /// A [`Relation`] containing all tuples.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::RelationNotFound`] if neither a base relvar nor
    /// a virtual relvar with the given name exists.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use relvar::Database;
    ///
    /// # let mut db = Database::open("test")?;
    /// // Get all employees
    /// let employees = db.query("EMPLOYEES")?;
    ///
    /// // Use relational algebra
    /// let senior_employees = employees
    ///     .restrict(|t| t.get_typed::<i64>("salary").unwrap() > 100000)
    ///     .project(&["name", "salary"]);
    /// # Ok::<(), relvar::DatabaseError>(())
    /// ```
    pub fn query(&mut self, relation_name: &str) -> Result<Relation, DatabaseError> {
        // Check if this is a virtual relvar - if so, evaluate the definition
        if self.virtual_relvars.contains_key(relation_name) {
            return self.query_virtual_relvar(relation_name);
        }

        // Otherwise, query the base relvar
        self.query_base_relvar(relation_name)
    }

    /// Queries a base relvar (stored relation).
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

    /// Queries a virtual relvar by evaluating its definition.
    ///
    /// This temporarily removes the virtual relvar definition to avoid borrow issues,
    /// evaluates it, and puts it back.
    fn query_virtual_relvar(&mut self, name: &str) -> Result<Relation, DatabaseError> {
        // Temporarily remove the virtual relvar definition to avoid borrow issues
        let definition = self
            .virtual_relvars
            .remove(name)
            .ok_or_else(|| DatabaseError::VirtualRelvarNotFound(name.to_string()))?;

        // Evaluate the virtual relvar definition
        let result = definition(self);

        // Put the virtual relvar definition back
        self.virtual_relvars.insert(name.to_string(), definition);

        result
    }

    /// Registers system catalog relvars as virtual relvars.
    ///
    /// TTM: RM Prescription 12 - The system catalog is relational.
    /// System relvars are virtual relvars that compute their content from internal
    /// metadata structures.
    ///
    /// This method is called during database initialization to set up the three
    /// system relvars: SYS_RELVARS, SYS_ATTRIBUTES, and SYS_CONSTRAINTS.
    ///
    /// Note: This bypasses the is_system_relvar() check by directly inserting into
    /// the virtual_relvars map, since system relvars themselves use the SYS_ prefix.
    fn register_system_relvars(&mut self) {
        // Register SYS_RELVARS
        self.virtual_relvars.insert(
            SYS_RELVARS.to_string(),
            Box::new(|db: &mut Database| db.build_sys_relvars()),
        );

        // Register SYS_ATTRIBUTES
        self.virtual_relvars.insert(
            SYS_ATTRIBUTES.to_string(),
            Box::new(|db: &mut Database| db.build_sys_attributes()),
        );

        // Register SYS_CONSTRAINTS
        self.virtual_relvars.insert(
            SYS_CONSTRAINTS.to_string(),
            Box::new(|db: &mut Database| db.build_sys_constraints()),
        );
    }

    /// Builds the SYS_RELVARS system catalog relation.
    ///
    /// Returns a relation containing one tuple for each relvar (base or virtual)
    /// in the database.
    fn build_sys_relvars(&mut self) -> Result<Relation, DatabaseError> {
        let mut relation = Relation::new(sys_relvars_type());

        // Add base relvars from catalog
        for rel_name in self.catalog.list_relations() {
            if let Ok(metadata) = self.catalog.get_relation(&rel_name) {
                let tuple = tuple! {
                    relvar_name: rel_name.clone(),
                    relvar_type: "BASE",
                    heap_file_path: metadata.heap_file_path.to_string_lossy().to_string()
                };
                relation.insert(tuple)?;
            }
        }

        // Add virtual relvars (including system relvars)
        // Note: The system relvar being queried may not be in the map temporarily
        // (due to query_virtual_relvar removing it), so we explicitly add all system relvars
        let system_relvars = vec![SYS_RELVARS, SYS_ATTRIBUTES, SYS_CONSTRAINTS];
        for sys_relvar in &system_relvars {
            let tuple = tuple! {
                relvar_name: sys_relvar.to_string(),
                relvar_type: "VIRTUAL",
                heap_file_path: ""
            };
            relation.insert(tuple)?;
        }

        // Add other virtual relvars (non-system)
        for virtual_rel_name in self.virtual_relvars.keys() {
            if !is_system_relvar(virtual_rel_name) {
                let tuple = tuple! {
                    relvar_name: virtual_rel_name.clone(),
                    relvar_type: "VIRTUAL",
                    heap_file_path: ""
                };
                relation.insert(tuple)?;
            }
        }

        Ok(relation)
    }

    /// Builds the SYS_ATTRIBUTES system catalog relation.
    ///
    /// Returns a relation containing one tuple for each attribute in each relvar.
    fn build_sys_attributes(&mut self) -> Result<Relation, DatabaseError> {
        let mut relation = Relation::new(sys_attributes_type());

        // Add attributes from base relvars
        for rel_name in self.catalog.list_relations() {
            if let Ok(metadata) = self.catalog.get_relation(&rel_name) {
                let attributes = metadata.relation_type.heading().attributes();
                for (position, (attr_name, attr_type)) in attributes.iter().enumerate() {
                    let tuple = tuple! {
                        relvar_name: rel_name.clone(),
                        attr_name: attr_name.clone(),
                        attr_type: serialize_scalar_type(attr_type),
                        attr_position: position as i64
                    };
                    relation.insert(tuple)?;
                }
            }
        }

        // Add attributes from virtual relvars
        // We need to evaluate each virtual relvar to get its schema
        // System relvars are excluded because they're still being built
        let virtual_relvar_names: Vec<_> = self
            .virtual_relvars
            .keys()
            .filter(|name| !is_system_relvar(name))
            .cloned()
            .collect();

        for vr_name in virtual_relvar_names {
            // Temporarily remove and evaluate the virtual relvar to get its type
            if let Some(definition) = self.virtual_relvars.remove(&vr_name) {
                // Evaluate to get the relation and its type
                if let Ok(vr_relation) = definition(self) {
                    let attributes = vr_relation.relation_type().heading().attributes();
                    for (position, (attr_name, attr_type)) in attributes.iter().enumerate() {
                        let tuple = tuple! {
                            relvar_name: vr_name.clone(),
                            attr_name: attr_name.clone(),
                            attr_type: serialize_scalar_type(attr_type),
                            attr_position: position as i64
                        };
                        relation.insert(tuple)?;
                    }
                }
                // Put the definition back
                self.virtual_relvars.insert(vr_name, definition);
            }
        }

        Ok(relation)
    }

    /// Formats a type constraint as a human-readable string for the system catalog.
    fn format_type_constraint(constraint: &crate::constraints::TypeConstraint) -> String {
        use crate::constraints::TypeConstraint;
        match constraint {
            TypeConstraint::Range { min, max } => {
                format!("BETWEEN {:?} AND {:?}", min, max)
            }
            TypeConstraint::Enum { allowed_values } => {
                let values: Vec<String> =
                    allowed_values.iter().map(|v| format!("{:?}", v)).collect();
                format!("IN ({})", values.join(", "))
            }
            TypeConstraint::StringLength { min, max } => {
                format!("LENGTH BETWEEN {} AND {}", min, max)
            }
            TypeConstraint::PositiveInt => "> 0".to_string(),
            TypeConstraint::NonNegativeInt => ">= 0".to_string(),
            TypeConstraint::Custom { description, .. } => description.clone(),
        }
    }

    /// Builds the SYS_CONSTRAINTS system catalog relation.
    ///
    /// Returns a relation containing one tuple for each constraint in the database.
    fn build_sys_constraints(&mut self) -> Result<Relation, DatabaseError> {
        let mut relation = Relation::new(sys_constraints_type());

        // Add primary key constraints
        for (rel_name, key_constraints) in &self.key_constraints {
            if let Some(pk) = key_constraints.primary_key() {
                let attrs = pk.attributes().join(",");
                let tuple = tuple! {
                    relvar_name: rel_name.clone(),
                    constraint_name: format!("{}_pk", rel_name),
                    constraint_type: "PRIMARY_KEY",
                    constraint_def: format!("PRIMARY KEY ({})", attrs)
                };
                relation.insert(tuple)?;
            }

            // Add candidate key constraints
            for (i, ck) in key_constraints.candidate_keys().iter().enumerate() {
                let attrs = ck.attributes().join(",");
                let tuple = tuple! {
                    relvar_name: rel_name.clone(),
                    constraint_name: format!("{}_ck_{}", rel_name, i),
                    constraint_type: "CANDIDATE_KEY",
                    constraint_def: format!("CANDIDATE KEY ({})", attrs)
                };
                relation.insert(tuple)?;
            }
        }

        // Add foreign key constraints
        for (rel_name, fk_constraints) in &self.foreign_key_constraints {
            for (i, fk) in fk_constraints.foreign_keys().iter().enumerate() {
                let local_attrs = fk.foreign_key_attributes().join(",");
                let foreign_attrs = fk.referenced_attributes().join(",");
                let tuple = tuple! {
                    relvar_name: rel_name.clone(),
                    constraint_name: format!("{}_fk_{}", rel_name, i),
                    constraint_type: "FOREIGN_KEY",
                    constraint_def: format!(
                        "FOREIGN KEY ({}) REFERENCES {} ({})",
                        local_attrs,
                        fk.referenced_relation_name(),
                        foreign_attrs
                    )
                };
                relation.insert(tuple)?;
            }
        }

        // Add type constraints
        for (rel_name, attr_constraints_map) in &self.type_constraints {
            for (attr_name, attr_constraints) in attr_constraints_map {
                // Each AttributeConstraints can have multiple TypeConstraint entries
                for (i, type_constraint) in attr_constraints.constraints().iter().enumerate() {
                    let constraint_def = format!(
                        "{} CHECK {}",
                        attr_name,
                        Self::format_type_constraint(type_constraint)
                    );
                    let tuple = tuple! {
                        relvar_name: rel_name.clone(),
                        constraint_name: format!("{}_{}_tc_{}", rel_name, attr_name, i),
                        constraint_type: "TYPE",
                        constraint_def: constraint_def
                    };
                    relation.insert(tuple)?;
                }
            }
        }

        Ok(relation)
    }

    /// Deletes tuples matching a predicate from a relation.
    ///
    /// All tuples for which the predicate returns `true` are removed. Foreign
    /// key constraints are checked before deletion to prevent orphaned references.
    ///
    /// # Arguments
    ///
    /// * `relation_name` - The name of the relation
    /// * `predicate` - A function that returns `true` for tuples to delete
    ///
    /// # Returns
    ///
    /// The number of tuples deleted.
    ///
    /// # Errors
    ///
    /// - [`DatabaseError::RelationNotFound`] - Relation does not exist
    /// - [`DatabaseError::ForeignKeyViolation`] - Deletion would orphan references
    ///
    /// # Example
    ///
    /// ```no_run
    /// use relvar::Database;
    ///
    /// # let mut db = Database::open("test")?;
    /// // Delete employee with emp_id = 42
    /// let deleted = db.delete("EMPLOYEES", |t| {
    ///     t.get_typed::<i64>("emp_id").unwrap() == 42
    /// })?;
    /// println!("Deleted {} tuples", deleted);
    /// # Ok::<(), relvar::DatabaseError>(())
    /// ```
    pub fn delete<F>(&mut self, relation_name: &str, predicate: F) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool,
    {
        // Check if trying to delete from a virtual relvar
        if self.virtual_relvars.contains_key(relation_name) {
            return Err(DatabaseError::CannotModifyVirtualRelvar(
                relation_name.to_string(),
            ));
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

    /// Updates tuples matching a predicate.
    ///
    /// All tuples for which the predicate returns `true` are modified by
    /// the updater function. After modification, type conformance is verified.
    ///
    /// # Arguments
    ///
    /// * `relation_name` - The name of the relation
    /// * `predicate` - A function that returns `true` for tuples to update
    /// * `updater` - A function that modifies the tuple in place
    ///
    /// # Returns
    ///
    /// The number of tuples updated.
    ///
    /// # Errors
    ///
    /// - [`DatabaseError::RelationNotFound`] - Relation does not exist
    /// - [`DatabaseError::TupleMismatch`] - Updated tuple doesn't conform to type
    ///
    /// # Example
    ///
    /// ```no_run
    /// use relvar::Database;
    /// use relvar::values::ScalarValue;
    ///
    /// # let mut db = Database::open("test")?;
    /// // Give all employees in dept 10 a 10% raise
    /// let updated = db.update(
    ///     "EMPLOYEES",
    ///     |t| t.get_typed::<i64>("dept_id").unwrap() == 10,
    ///     |t| {
    ///         let old_salary = t.get_typed::<i64>("salary").unwrap();
    ///         t.set("salary".to_string(), ScalarValue::Int(old_salary * 11 / 10)).unwrap();
    ///     },
    /// )?;
    /// println!("Updated {} tuples", updated);
    /// # Ok::<(), relvar::DatabaseError>(())
    /// ```
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
        // Check if trying to update a virtual relvar
        if self.virtual_relvars.contains_key(relation_name) {
            return Err(DatabaseError::CannotModifyVirtualRelvar(
                relation_name.to_string(),
            ));
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

    /// Begins a new transaction.
    ///
    /// A transaction provides atomicity - all changes within the transaction
    /// are either committed together or rolled back together.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::TransactionError`] if a transaction is already
    /// in progress.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use relvar::Database;
    /// use relvar::tuple;
    ///
    /// # let mut db = Database::open("test")?;
    /// db.begin()?;
    ///
    /// // Multiple operations
    /// db.insert("EMPLOYEES", tuple! { emp_id: 1i64, name: "Alice" })?;
    /// db.insert("EMPLOYEES", tuple! { emp_id: 2i64, name: "Bob" })?;
    ///
    /// // Commit all changes atomically
    /// db.commit()?;
    /// # Ok::<(), relvar::DatabaseError>(())
    /// ```
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

    /// Commits the current transaction.
    ///
    /// All changes made since [`begin()`](Self::begin) are made permanent.
    /// The transaction is ended after commit.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::TransactionError`] if no transaction is in progress.
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

    /// Rolls back the current transaction.
    ///
    /// All changes made since [`begin()`](Self::begin) are discarded and
    /// the database is restored to its pre-transaction state.
    /// The transaction is ended after rollback.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::TransactionError`] if no transaction is in progress.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use relvar::Database;
    /// use relvar::tuple;
    ///
    /// # let mut db = Database::open("test")?;
    /// db.begin()?;
    /// db.insert("EMPLOYEES", tuple! { emp_id: 999i64, name: "Test" })?;
    ///
    /// // Oops, changed our mind - rollback
    /// db.rollback()?;
    /// // The insert never happened
    /// # Ok::<(), relvar::DatabaseError>(())
    /// ```
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

    /// Gets or opens a heap file for a relation.
    ///
    /// This is an internal helper that lazily opens heap files as needed.
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
    // Virtual Relvar Tests (TTM RM Prescription 10 - Virtual Relation Variables)
    // ==========================================================================

    #[test]
    fn test_create_virtual_relvar() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String)
            .with_attribute("salary".to_string(), ScalarType::Float);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("EMP", relation_type).unwrap();

        db.create_virtual_relvar("HIGH_EARNERS", |db| {
            Ok(db
                .query("EMP")?
                .restrict(|t| t.get_typed::<f64>("salary").unwrap() > 100000.0))
        })
        .unwrap();

        assert!(db.virtual_relvar_exists("HIGH_EARNERS"));
    }

    #[test]
    fn test_virtual_relvar_re_evaluates_on_query() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String)
            .with_attribute("salary".to_string(), ScalarType::Float);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("EMP", relation_type).unwrap();

        db.insert("EMP", tuple! { id: 1i64, name: "Alice", salary: 150000.0 })
            .unwrap();
        db.insert("EMP", tuple! { id: 2i64, name: "Bob", salary: 50000.0 })
            .unwrap();

        db.create_virtual_relvar("HIGH_EARNERS", |db| {
            Ok(db
                .query("EMP")?
                .restrict(|t| t.get_typed::<f64>("salary").unwrap() > 100000.0))
        })
        .unwrap();

        let result = db.query("HIGH_EARNERS").unwrap();
        assert_eq!(result.cardinality(), 1);

        db.insert(
            "EMP",
            tuple! { id: 3i64, name: "Charlie", salary: 200000.0 },
        )
        .unwrap();

        let result = db.query("HIGH_EARNERS").unwrap();
        assert_eq!(result.cardinality(), 2);
    }

    #[test]
    fn test_virtual_relvar_appears_in_listing() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("EMP", relation_type).unwrap();

        db.create_virtual_relvar("NAMES_ONLY", |db| Ok(db.query("EMP")?.project(&["name"])))
            .unwrap();

        let virtual_relvars = db.list_virtual_relvars();
        assert!(virtual_relvars.contains(&"NAMES_ONLY".to_string()));
    }

    #[test]
    fn test_drop_virtual_relvar() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("EMP", relation_type).unwrap();

        db.create_virtual_relvar("NAMES_ONLY", |db| Ok(db.query("EMP")?.project(&["name"])))
            .unwrap();

        assert!(db.virtual_relvar_exists("NAMES_ONLY"));

        db.drop_virtual_relvar("NAMES_ONLY").unwrap();

        assert!(!db.virtual_relvar_exists("NAMES_ONLY"));
    }

    #[test]
    fn test_insert_into_virtual_relvar_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("EMP", relation_type).unwrap();

        db.create_virtual_relvar("EMP_VIRTUAL", |db| db.query("EMP"))
            .unwrap();

        let result = db.insert("EMP_VIRTUAL", tuple! { id: 1i64, name: "Alice" });
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::CannotModifyVirtualRelvar(_)
        ));
    }

    #[test]
    fn test_delete_from_virtual_relvar_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("EMP", relation_type).unwrap();
        db.insert("EMP", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        db.create_virtual_relvar("EMP_VIRTUAL", |db| db.query("EMP"))
            .unwrap();

        let result = db.delete("EMP_VIRTUAL", |_| true);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::CannotModifyVirtualRelvar(_)
        ));
    }

    #[test]
    fn test_update_virtual_relvar_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("EMP", relation_type).unwrap();
        db.insert("EMP", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        db.create_virtual_relvar("EMP_VIRTUAL", |db| db.query("EMP"))
            .unwrap();

        let result = db.update(
            "EMP_VIRTUAL",
            |_| true,
            |t| {
                t.set("name".to_string(), ScalarValue::String("Bob".to_string()))
                    .unwrap();
            },
        );
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::CannotModifyVirtualRelvar(_)
        ));
    }

    #[test]
    fn test_query_nonexistent_relvar_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let result = db.query("NONEXISTENT");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::RelationNotFound(_)
        ));
    }

    #[test]
    fn test_create_duplicate_virtual_relvar_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("EMP", relation_type).unwrap();

        db.create_virtual_relvar("EMP_VIRTUAL", |db| db.query("EMP"))
            .unwrap();

        let result = db.create_virtual_relvar("EMP_VIRTUAL", |db| db.query("EMP"));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::VirtualRelvarAlreadyExists(_)
        ));
    }

    #[test]
    fn test_virtual_relvar_with_same_name_as_relvar_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
        let relation_type = RelationType::new(tuple_type);

        db.create_relvar("EMP", relation_type).unwrap();

        let result = db.create_virtual_relvar("EMP", |db| db.query("EMP"));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::RelationAlreadyExists(_)
        ));
    }

    #[test]
    fn test_relvar_with_same_name_as_virtual_relvar_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
        let relation_type = RelationType::new(tuple_type.clone());

        db.create_relvar("EMP", relation_type).unwrap();

        db.create_virtual_relvar("EMP_VIRTUAL", |db| db.query("EMP"))
            .unwrap();

        let result = db.create_relvar("EMP_VIRTUAL", RelationType::new(tuple_type));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::VirtualRelvarAlreadyExists(_)
        ));
    }

    #[test]
    fn test_drop_nonexistent_virtual_relvar_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let result = db.drop_virtual_relvar("NONEXISTENT");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::VirtualRelvarNotFound(_)
        ));
    }

    #[test]
    fn test_virtual_relvar_with_join() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let emp_type = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String)
            .with_attribute("dept_id".to_string(), ScalarType::Int);
        db.create_relvar("EMP", RelationType::new(emp_type))
            .unwrap();

        let dept_type = TupleType::new()
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("dept_name".to_string(), ScalarType::String);
        db.create_relvar("DEPT", RelationType::new(dept_type))
            .unwrap();

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

        db.create_virtual_relvar("EMP_WITH_DEPT", |db| {
            let emp = db.query("EMP")?;
            let dept = db.query("DEPT")?;
            Ok(emp.join(&dept))
        })
        .unwrap();

        let result = db.query("EMP_WITH_DEPT").unwrap();
        assert_eq!(result.cardinality(), 2);
        assert_eq!(result.degree(), 4);
    }

    #[test]
    fn test_virtual_relvar_evaluation_error_preserves_definition() {
        // Verifies that if a virtual relvar's definition returns an error,
        // the virtual relvar is still preserved and can be queried again.
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create a virtual relvar that queries a non-existent base relvar
        db.create_virtual_relvar("BAD_VIRTUAL", |db| db.query("NONEXISTENT"))
            .unwrap();

        // First query fails (NONEXISTENT doesn't exist)
        let result = db.query("BAD_VIRTUAL");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::RelationNotFound(_)
        ));

        // Virtual relvar should still exist after the error
        assert!(db.virtual_relvar_exists("BAD_VIRTUAL"));

        // Can query it again (still fails, but proves definition wasn't lost)
        let result2 = db.query("BAD_VIRTUAL");
        assert!(result2.is_err());
    }

    #[test]
    fn test_drop_relvar_breaks_dependent_virtual_relvar() {
        // Verifies behavior when a base relvar is dropped while a virtual relvar depends on it.
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
        db.create_relvar("EMP", RelationType::new(tuple_type))
            .unwrap();

        db.insert("EMP", tuple! { id: 1i64 }).unwrap();

        // Create a virtual relvar that depends on EMP
        db.create_virtual_relvar("EMP_VIRTUAL", |db| db.query("EMP"))
            .unwrap();

        // Virtual relvar works initially
        let result = db.query("EMP_VIRTUAL").unwrap();
        assert_eq!(result.cardinality(), 1);

        // Drop the base relvar
        db.drop_relvar("EMP").unwrap();

        // Virtual relvar still exists
        assert!(db.virtual_relvar_exists("EMP_VIRTUAL"));

        // But querying it now fails because EMP no longer exists
        let result = db.query("EMP_VIRTUAL");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::RelationNotFound(_)
        ));
    }

    // ====================================================================================
    // System Catalog Tests (TTM RM Prescription 12 - Relational System Catalog)
    // ====================================================================================

    #[test]
    fn test_sys_prefix_reserved_for_create_relvar() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
        let relation_type = RelationType::new(tuple_type);

        // Should reject SYS_ prefix
        let result = db.create_relvar("SYS_MY_RELVAR", relation_type);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::SystemRelvarReserved(_)
        ));
    }

    #[test]
    fn test_sys_prefix_reserved_for_create_virtual_relvar() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Should reject SYS_ prefix
        let result = db.create_virtual_relvar("SYS_MY_VIRTUAL", |_db| {
            Ok(Relation::new(sys_relvars_type()))
        });
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::SystemRelvarReserved(_)
        ));
    }

    #[test]
    fn test_cannot_drop_sys_relvars() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Should not be able to drop system relvars
        let result = db.drop_relvar(SYS_RELVARS);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::CannotDropSystemRelvar(_)
        ));

        let result = db.drop_virtual_relvar(SYS_RELVARS);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::CannotDropSystemRelvar(_)
        ));
    }

    #[test]
    fn test_cannot_insert_into_sys_relvars() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // System relvars are virtual, so insert should fail
        let result = db.insert(
            SYS_RELVARS,
            tuple! {
                relvar_name: "TEST",
                relvar_type: "BASE",
                heap_file_path: "test.heap"
            },
        );
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::CannotModifyVirtualRelvar(_)
        ));
    }

    #[test]
    fn test_cannot_delete_from_sys_relvars() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // System relvars are virtual, so delete should fail
        let result = db.delete(SYS_RELVARS, |_| true);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::CannotModifyVirtualRelvar(_)
        ));
    }

    #[test]
    fn test_cannot_update_sys_relvars() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // System relvars are virtual, so update should fail
        let result = db.update(SYS_RELVARS, |_| true, |_| {});
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatabaseError::CannotModifyVirtualRelvar(_)
        ));
    }

    #[test]
    fn test_query_sys_relvars_lists_base_relvars() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create a base relvar
        let tuple_type = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
        db.create_relvar("EMP", RelationType::new(tuple_type))
            .unwrap();

        // Query SYS_RELVARS
        let sys_relvars = db.query(SYS_RELVARS).unwrap();

        // Should contain EMP and the three system relvars
        assert!(sys_relvars.cardinality() >= 4);

        // Check that EMP is in the result
        let emp_tuples: Vec<_> = sys_relvars
            .tuples()
            .filter(|t| t.get_typed::<String>("relvar_name").unwrap() == "EMP")
            .collect();
        assert_eq!(emp_tuples.len(), 1);

        let emp_tuple = emp_tuples[0];
        assert_eq!(
            emp_tuple.get_typed::<String>("relvar_type").unwrap(),
            "BASE"
        );
        assert!(
            emp_tuple
                .get_typed::<String>("heap_file_path")
                .unwrap()
                .ends_with("EMP.heap")
        );
    }

    #[test]
    fn test_query_sys_relvars_lists_virtual_relvars() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create a base relvar
        let tuple_type = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
        db.create_relvar("EMP", RelationType::new(tuple_type))
            .unwrap();

        // Create a virtual relvar
        db.create_virtual_relvar("EMP_VIEW", |db| db.query("EMP"))
            .unwrap();

        // Query SYS_RELVARS
        let sys_relvars = db.query(SYS_RELVARS).unwrap();

        // Check that EMP_VIEW is in the result
        let view_tuples: Vec<_> = sys_relvars
            .tuples()
            .filter(|t| t.get_typed::<String>("relvar_name").unwrap() == "EMP_VIEW")
            .collect();
        assert_eq!(view_tuples.len(), 1);

        let view_tuple = view_tuples[0];
        assert_eq!(
            view_tuple.get_typed::<String>("relvar_type").unwrap(),
            "VIRTUAL"
        );
        assert_eq!(
            view_tuple.get_typed::<String>("heap_file_path").unwrap(),
            ""
        );
    }

    #[test]
    fn test_query_sys_attributes_lists_attributes() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create a relvar with multiple attributes
        let tuple_type = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String)
            .with_attribute("salary".to_string(), ScalarType::Float);
        db.create_relvar("EMP", RelationType::new(tuple_type))
            .unwrap();

        // Query SYS_ATTRIBUTES
        let sys_attrs = db.query(SYS_ATTRIBUTES).unwrap();

        // Filter for EMP attributes
        let emp_attrs: Vec<_> = sys_attrs
            .tuples()
            .filter(|t| t.get_typed::<String>("relvar_name").unwrap() == "EMP")
            .collect();

        // Should have 3 attributes
        assert_eq!(emp_attrs.len(), 3);

        // Check that all attribute names are present
        let attr_names: Vec<String> = emp_attrs
            .iter()
            .map(|t| t.get_typed::<String>("attr_name").unwrap())
            .collect();
        assert!(attr_names.contains(&"emp_id".to_string()));
        assert!(attr_names.contains(&"name".to_string()));
        assert!(attr_names.contains(&"salary".to_string()));
    }

    #[test]
    fn test_sys_relvars_reflects_create_relvar() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Check initial count
        let initial = db.query(SYS_RELVARS).unwrap();
        let initial_count = initial.cardinality();

        // Create a relvar
        let tuple_type = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
        db.create_relvar("TEST", RelationType::new(tuple_type))
            .unwrap();

        // Check that SYS_RELVARS reflects the change
        let after = db.query(SYS_RELVARS).unwrap();
        assert_eq!(after.cardinality(), initial_count + 1);

        // Verify TEST is present
        let test_tuples: Vec<_> = after
            .tuples()
            .filter(|t| t.get_typed::<String>("relvar_name").unwrap() == "TEST")
            .collect();
        assert_eq!(test_tuples.len(), 1);
    }

    #[test]
    fn test_sys_relvars_reflects_drop_relvar() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create a relvar
        let tuple_type = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
        db.create_relvar("TEMP", RelationType::new(tuple_type))
            .unwrap();

        // Check count after creation
        let after_create = db.query(SYS_RELVARS).unwrap();
        let count_after_create = after_create.cardinality();

        // Drop the relvar
        db.drop_relvar("TEMP").unwrap();

        // Check that SYS_RELVARS reflects the change
        let after_drop = db.query(SYS_RELVARS).unwrap();
        assert_eq!(after_drop.cardinality(), count_after_create - 1);

        // Verify TEMP is not present
        let temp_tuples: Vec<_> = after_drop
            .tuples()
            .filter(|t| t.get_typed::<String>("relvar_name").unwrap() == "TEMP")
            .collect();
        assert_eq!(temp_tuples.len(), 0);
    }

    #[test]
    fn test_restrict_on_sys_relvars() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create some relvars
        let tuple_type = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
        db.create_relvar("EMP", RelationType::new(tuple_type.clone()))
            .unwrap();
        db.create_relvar("DEPT", RelationType::new(tuple_type))
            .unwrap();

        // Query and restrict
        let base_relvars = db
            .query(SYS_RELVARS)
            .unwrap()
            .restrict(|t| t.get_typed::<String>("relvar_type").unwrap() == "BASE");

        // Should have at least EMP and DEPT
        assert!(base_relvars.cardinality() >= 2);

        // All should be BASE type
        for tuple in base_relvars.tuples() {
            assert_eq!(tuple.get_typed::<String>("relvar_type").unwrap(), "BASE");
        }
    }

    #[test]
    fn test_project_on_sys_attributes() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        db.create_relvar("EMP", RelationType::new(tuple_type))
            .unwrap();

        // Project to just attribute names for EMP
        let attr_names = db
            .query(SYS_ATTRIBUTES)
            .unwrap()
            .restrict(|t| t.get_typed::<String>("relvar_name").unwrap() == "EMP")
            .project(&["attr_name"]);

        // Should have 2 tuples (id and name)
        assert_eq!(attr_names.cardinality(), 2);
        assert_eq!(attr_names.degree(), 1);
    }

    #[test]
    fn test_join_sys_relvars_and_sys_attributes() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        let tuple_type = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
        db.create_relvar("TEST", RelationType::new(tuple_type))
            .unwrap();

        // Join SYS_RELVARS and SYS_ATTRIBUTES on relvar_name
        let relvars = db.query(SYS_RELVARS).unwrap();
        let attributes = db.query(SYS_ATTRIBUTES).unwrap();

        let joined = relvars.join(&attributes);

        // Should have combined schema
        assert!(joined.relation_type().has_attribute("relvar_name"));
        assert!(joined.relation_type().has_attribute("relvar_type"));
        assert!(joined.relation_type().has_attribute("attr_name"));
        assert!(joined.relation_type().has_attribute("attr_type"));
    }

    #[test]
    fn test_sys_attributes_includes_virtual_relvar_attributes() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create a base relvar
        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        db.create_relvar("EMP", RelationType::new(tuple_type))
            .unwrap();

        // Create a virtual relvar that projects only name
        db.create_virtual_relvar("EMP_NAMES", |db| Ok(db.query("EMP")?.project(&["name"])))
            .unwrap();

        // Query SYS_ATTRIBUTES
        let sys_attrs = db.query(SYS_ATTRIBUTES).unwrap();

        // Check for EMP_NAMES attributes
        let emp_names_attrs: Vec<_> = sys_attrs
            .tuples()
            .filter(|t| t.get_typed::<String>("relvar_name").unwrap() == "EMP_NAMES")
            .collect();

        // Should have 1 attribute (name)
        assert_eq!(emp_names_attrs.len(), 1);
        assert_eq!(
            emp_names_attrs[0].get_typed::<String>("attr_name").unwrap(),
            "name"
        );
    }

    #[test]
    fn test_sys_constraints_includes_type_constraints() {
        let temp_dir = TempDir::new().unwrap();
        let mut db = Database::open(temp_dir.path()).unwrap();

        // Create a relvar with type constraints
        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("age".to_string(), ScalarType::Int);
        db.create_relvar("USERS", RelationType::new(tuple_type))
            .unwrap();

        // Add type constraint on age
        let age_constraint = AttributeConstraints::new("age".to_string(), ScalarType::Int)
            .with_constraint(crate::constraints::TypeConstraint::PositiveInt);
        db.set_type_constraints("USERS", "age", age_constraint)
            .unwrap();

        // Query SYS_CONSTRAINTS
        let sys_constraints = db.query(SYS_CONSTRAINTS).unwrap();

        // Check for type constraints
        let type_constraints: Vec<_> = sys_constraints
            .tuples()
            .filter(|t| {
                t.get_typed::<String>("relvar_name").unwrap() == "USERS"
                    && t.get_typed::<String>("constraint_type").unwrap() == "TYPE"
            })
            .collect();

        // Should have 1 type constraint
        assert_eq!(type_constraints.len(), 1);
        assert!(
            type_constraints[0]
                .get_typed::<String>("constraint_def")
                .unwrap()
                .contains("age")
        );
        assert!(
            type_constraints[0]
                .get_typed::<String>("constraint_def")
                .unwrap()
                .contains("> 0")
        );
    }
}
