use crate::database::Database;
use crate::database::dml::{compute_relation_after_delete, compute_relation_after_update};
use crate::error::DatabaseError;
use crate::storage_engine::StorageEngine;
use crate::values::{Relation, Tuple};

impl<E: StorageEngine> Database<E> {
    /// Insert a tuple into a relation.
    ///
    /// This operation validates all constraints (Types, Keys, Foreign Keys, CHECKs)
    /// before modifying the database state.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("id", ScalarType::Int)
    ///         .with_attribute("name", ScalarType::String)
    /// );
    /// db.create_relvar("USERS", rel_type).unwrap();
    ///
    /// // Successful insert
    /// db.insert("USERS", tuple! { id: 1i64, name: "Alice" }).unwrap();
    ///
    /// // Fails: Tuple type mismatch (missing 'name')
    /// let result = db.insert("USERS", tuple! { id: 2i64 });
    /// assert!(result.is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The relation doesn't exist ([`DatabaseError::RelationNotFound`])
    /// - The tuple type doesn't match the relation's heading ([`DatabaseError::TupleMismatch`])
    /// - Any constraint is violated (Key, Foreign Key, Type, Check)
    pub fn insert(&mut self, relation_name: &str, tuple: Tuple) -> Result<(), DatabaseError> {
        self.ensure_not_virtual(relation_name)?;
        self.validate_insert(relation_name, &tuple)?;

        // Insert into engine
        self.engine.insert_tuple(relation_name, tuple)?;
        Ok(())
    }

    /// Query a relation (base or virtual).
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new().with_attribute("id", ScalarType::Int)
    /// );
    /// db.create_relvar("TEST", rel_type).unwrap();
    /// db.insert("TEST", tuple! { id: 1i64 }).unwrap();
    ///
    /// let result = db.query("TEST").unwrap();
    /// assert_eq!(result.cardinality(), 1);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::RelationNotFound` if the relation doesn't exist.
    pub fn query(&self, relation_name: &str) -> Result<Relation, DatabaseError> {
        // Check if this is a virtual relvar
        if let Some(virtual_relvar) = self.virtual_relvars.get(relation_name) {
            return (virtual_relvar.evaluator)(self);
        }

        // Otherwise, load from engine
        Ok(self.engine.load_relation(relation_name)?)
    }

    /// Delete tuples matching a predicate.
    ///
    /// Retrieves the total count of tuples removed from the relation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The relation doesn't exist ([`DatabaseError::RelationNotFound`])
    /// - Deleting the tuples would violate a foreign key constraint in another relation
    ///   ([`crate::constraints::ConstraintManagerError::ForeignKeyViolation`])
    ///
    /// # Performance
    ///
    /// This operation scans the entire relation to evaluate the predicate.
    /// It then constructs a new relation containing the remaining tuples.
    /// Complexity is O(N) where N is the relation size.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new().with_attribute("id", ScalarType::Int)
    /// );
    /// db.create_relvar("TEST", rel_type).unwrap();
    /// db.insert("TEST", tuple! { id: 1i64 }).unwrap();
    /// db.insert("TEST", tuple! { id: 2i64 }).unwrap();
    ///
    /// // Delete id 1
    /// let deleted_count = db.delete("TEST", |t| {
    ///     t.get_typed::<i64>("id").unwrap() == 1
    /// }).unwrap();
    ///
    /// assert_eq!(deleted_count, 1);
    /// assert_eq!(db.query("TEST").unwrap().cardinality(), 1);
    /// ```
    pub fn delete<F>(&mut self, relation_name: &str, predicate: F) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool,
    {
        self.ensure_not_virtual(relation_name)?;

        // Load current relation
        let current_relation = self.query(relation_name)?;

        // Filter out tuples to delete
        let (new_relation, delete_count) =
            compute_relation_after_delete(current_relation, predicate)?;

        self.constraints.validate_referencing_foreign_keys(
            &mut self.engine,
            relation_name,
            &new_relation,
        )?;

        // Store the new relation
        self.engine.store_relation(relation_name, &new_relation)?;
        Ok(delete_count)
    }

    /// Update tuples matching a predicate.
    ///
    /// Retrieves the total count of tuples modified during the operation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The relation doesn't exist ([`DatabaseError::RelationNotFound`])
    /// - The updated tuple doesn't match the relation type ([`DatabaseError::TupleMismatch`])
    /// - A key constraint is violated ([`crate::constraints::ConstraintManagerError::PrimaryKeyViolation`], [`crate::constraints::ConstraintManagerError::CandidateKeyViolation`])
    /// - A foreign key constraint is violated ([`crate::constraints::ConstraintManagerError::ForeignKeyViolation`])
    /// - A type constraint is violated ([`crate::constraints::ConstraintManagerError::TypeConstraintViolation`])
    /// - A CHECK constraint is violated ([`crate::constraints::ConstraintManagerError::CheckConstraintViolation`])
    ///
    /// # Performance
    ///
    /// - **Constraint Validation:** Currently, all constraints (CHECK, Type, Foreign Key) are re-validated
    ///   against **every tuple** in the relation after the update, not just the modified ones.
    ///   This ensures total consistency but has O(N) complexity where N is the relation size.
    ///   Future versions may optimize this to O(K) where K is the number of updated tuples.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("id", ScalarType::Int)
    ///         .with_attribute("salary", ScalarType::Int)
    /// );
    /// db.create_relvar("EMPLOYEES", rel_type).unwrap();
    /// db.insert("EMPLOYEES", tuple! { id: 1i64, salary: 50000i64 }).unwrap();
    ///
    /// // Give a 10% raise to employee 1
    /// let count = db.update(
    ///     "EMPLOYEES",
    ///     |t| t.get_typed::<i64>("id").unwrap() == 1,
    ///     |t| {
    ///         let old_salary = t.get_typed::<i64>("salary").unwrap();
    ///         tuple! {
    ///             id: t.get_typed::<i64>("id").unwrap(),
    ///             salary: old_salary + (old_salary / 10)
    ///         }
    ///     }
    /// ).unwrap();
    ///
    /// assert_eq!(count, 1);
    /// let employees = db.query("EMPLOYEES").unwrap();
    /// let emp1 = employees.tuples().next().unwrap();
    /// assert_eq!(emp1.get_typed::<i64>("salary").unwrap(), 55000);
    /// ```
    pub fn update<F, U>(
        &mut self,
        relation_name: &str,
        predicate: F,
        updater: U,
    ) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool,
        U: Fn(&Tuple) -> Tuple,
    {
        self.ensure_not_virtual(relation_name)?;

        // Load current relation
        let current_relation = self.query(relation_name)?;

        // Apply updates
        let (new_relation, update_count) =
            compute_relation_after_update(current_relation, predicate, updater)?;

        self.validate_relation_constraints(relation_name, &new_relation)?;

        // Store the new relation
        self.engine.store_relation(relation_name, &new_relation)?;
        Ok(update_count)
    }
}
