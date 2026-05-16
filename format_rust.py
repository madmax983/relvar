import sys

def format_rust():
    with open("relvar-core/src/storage_engine/mod.rs", "r") as f:
        content = f.read()

    # Restore the proper docs
    old_block = """    /// Grouped Tuple Operations
    ///
    /// `load_relation` - Load a relation (scan all tuples). Returns `StorageError::RelationNotFound` if the relation doesn't exist.
    /// `store_relation` - Store a relation (replaces all tuples). Used for bulk operations like delete/update that rebuild the relation. Returns `StorageError::RelationNotFound` if the relation doesn't exist.
    /// `insert_tuple` - Insert a single tuple. Returns `StorageError::RelationNotFound` if the relation doesn't exist.
    fn load_relation(&self, name: &str) -> Result<Relation, StorageError>;
    fn store_relation(&mut self, name: &str, relation: &Relation) -> Result<(), StorageError>;
    fn insert_tuple(&mut self, name: &str, tuple: Tuple) -> Result<(), StorageError>;"""

    new_block = """    /// Load a relation (scan all tuples).
    ///
    /// # Errors
    ///
    /// Returns `StorageError::RelationNotFound` if the relation doesn't exist.
    fn load_relation(&self, name: &str) -> Result<Relation, StorageError>;

    /// Store a relation (replaces all tuples).
    ///
    /// This is used for bulk operations like delete/update that rebuild the relation.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::RelationNotFound` if the relation doesn't exist.
    fn store_relation(&mut self, name: &str, relation: &Relation) -> Result<(), StorageError>;

    /// Insert a single tuple.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::RelationNotFound` if the relation doesn't exist.
    fn insert_tuple(&mut self, name: &str, tuple: Tuple) -> Result<(), StorageError>;"""

    if old_block in content:
        with open("relvar-core/src/storage_engine/mod.rs", "w") as f:
            f.write(content.replace(old_block, new_block))
        print("Success")
    else:
        print("Failed to find block")

format_rust()
