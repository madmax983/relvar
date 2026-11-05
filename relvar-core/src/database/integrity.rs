//! Database Integrity Constraint operations.

use crate::constraints::{
    AttributeConstraints, CheckConstraints, ConstraintManager, ConstraintManagerError,
    ForeignKeyConstraints, KeyConstraints,
};
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::StorageEngine;

impl<E: StorageEngine> Database<E> {
    /// Applies a constraint definition change and persists the resulting
    /// constraint catalog (TTM RM Prescription 16).
    ///
    /// `change` runs against a copy of the constraint manager; the copy
    /// replaces the live one only once the engine has stored its catalog,
    /// so a failed validation *or* a failed write leaves both the in-memory
    /// and the stored constraints as they were.
    pub(crate) fn change_constraints(
        &mut self,
        change: impl FnOnce(&mut ConstraintManager, &mut E) -> Result<(), ConstraintManagerError>,
    ) -> Result<(), DatabaseError> {
        let mut updated = self.constraints.clone();
        change(&mut updated, &mut self.engine)?;
        self.engine.store_constraints(&updated.to_catalog())?;
        self.constraints = updated;
        Ok(())
    }

    /// Get the key constraints for a relation.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::constraints::{KeyConstraints, PrimaryKey};
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// db.create_relvar("USERS", RelationType::new(heading)).unwrap();
    ///
    /// let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    /// db.set_key_constraints("USERS", KeyConstraints::new().with_primary_key(pk)).unwrap();
    ///
    /// let constraints = db.get_key_constraints("USERS").unwrap();
    /// assert!(constraints.primary_key().is_some());
    /// ```
    pub fn get_key_constraints(&self, relation_name: &str) -> Option<&KeyConstraints> {
        self.constraints.get_key_constraints(relation_name)
    }

    /// Get the foreign key constraints for a relation.
    ///
    /// Retrieving foreign keys is primarily used by the schema visualizer
    /// to draw relationships between relvars, or by automated testing tools
    /// to understand dependency insertion order.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::constraints::{ForeignKeyConstraints, ForeignKey, KeyConstraints, PrimaryKey};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let type1 = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    /// let type2 = RelationType::new(TupleType::new().with_attribute("ref_id", ScalarType::Int));
    /// db.create_relvar("A", type1).unwrap();
    /// db.create_relvar("B", type2).unwrap();
    /// // A foreign key references a declared key.
    /// let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    /// db.set_key_constraints("A", KeyConstraints::new().with_primary_key(pk)).unwrap();
    ///
    /// let fk = ForeignKey::new(
    ///     vec!["ref_id".to_string()],
    ///     "A".to_string(),
    ///     vec!["id".to_string()]
    /// ).unwrap();
    /// let constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
    /// db.set_foreign_key_constraints("B", constraints).unwrap();
    ///
    /// let current_fks = db.get_foreign_key_constraints("B").unwrap();
    /// assert_eq!(current_fks.foreign_keys().len(), 1);
    /// ```
    pub fn get_foreign_key_constraints(
        &self,
        relation_name: &str,
    ) -> Option<&ForeignKeyConstraints> {
        self.constraints.get_foreign_key_constraints(relation_name)
    }

    /// Set key constraints for a relation.
    ///
    /// Replaces any keys previously declared on `relation_name`.
    ///
    /// TTM: RM Prescription 15. Every key attribute must exist in the
    /// heading and appear at most once; no declared key may be a proper
    /// superset of another (candidate keys are irreducible). The empty key
    /// is legal and limits the relvar to at most one tuple. A key that some
    /// foreign key references cannot be dropped by the replacement, and the
    /// existing data must satisfy every key.
    ///
    /// # Errors
    ///
    /// [`DatabaseError::Constraint`] wrapping
    /// [`RelationNotFound`](crate::constraints::ConstraintManagerError::RelationNotFound),
    /// [`AttributeNotFound`](crate::constraints::ConstraintManagerError::AttributeNotFound),
    /// [`DuplicateKeyAttribute`](crate::constraints::ConstraintManagerError::DuplicateKeyAttribute),
    /// [`ReducibleKey`](crate::constraints::ConstraintManagerError::ReducibleKey),
    /// [`KeyReferencedByForeignKey`](crate::constraints::ConstraintManagerError::KeyReferencedByForeignKey),
    /// [`PrimaryKeyViolation`](crate::constraints::ConstraintManagerError::PrimaryKeyViolation)
    /// or [`CandidateKeyViolation`](crate::constraints::ConstraintManagerError::CandidateKeyViolation).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::constraints::{KeyConstraints, PrimaryKey};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new().with_attribute("id", ScalarType::Int)
    /// );
    /// db.create_relvar("TEST", rel_type).unwrap();
    ///
    /// let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    /// let constraints = KeyConstraints::new().with_primary_key(pk);
    ///
    /// db.set_key_constraints("TEST", constraints).unwrap();
    /// ```
    pub fn set_key_constraints(
        &mut self,
        relation_name: &str,
        constraints: KeyConstraints,
    ) -> Result<(), DatabaseError> {
        self.ensure_no_transaction("set_key_constraints")?;
        self.change_constraints(|manager, engine| {
            manager.set_key_constraints(engine, relation_name, constraints)
        })?;
        // (Re)build the key index from current data so subsequent inserts
        // validate against O(1) lookups (#23).
        self.rebuild_key_index(relation_name)?;
        Ok(())
    }

    /// Set foreign key constraints for a relation.
    ///
    /// Replaces any foreign keys previously declared on `relation_name`.
    ///
    /// TTM: a foreign key references a candidate key. Each foreign key is
    /// validated before it is accepted: its attributes must exist (and be
    /// distinct) in `relation_name`, the referenced relvar must exist, the
    /// referenced attributes must be exactly one of its declared keys
    /// (primary or candidate, compared as sets), and each referencing
    /// attribute must have the type of the attribute it references. The
    /// existing data must satisfy every foreign key.
    ///
    /// # Errors
    ///
    /// [`DatabaseError::Constraint`] wrapping
    /// [`ConstraintManagerError::RelationNotFound`],
    /// [`AttributeNotFound`](crate::constraints::ConstraintManagerError::AttributeNotFound),
    /// [`DuplicateKeyAttribute`](crate::constraints::ConstraintManagerError::DuplicateKeyAttribute),
    /// [`ForeignKeyTargetNotKey`](crate::constraints::ConstraintManagerError::ForeignKeyTargetNotKey),
    /// [`ForeignKeyTypeMismatch`](crate::constraints::ConstraintManagerError::ForeignKeyTypeMismatch)
    /// or [`ForeignKeyViolation`](crate::constraints::ConstraintManagerError::ForeignKeyViolation).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::constraints::{ForeignKeyConstraints, ForeignKey, KeyConstraints, PrimaryKey};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    ///
    /// // Create referenced relvar, keyed on dept_id
    /// let dept_type = RelationType::new(
    ///     TupleType::new().with_attribute("dept_id", ScalarType::Int)
    /// );
    /// db.create_relvar("DEPT", dept_type).unwrap();
    /// let pk = PrimaryKey::new(vec!["dept_id".to_string()]).unwrap();
    /// db.set_key_constraints("DEPT", KeyConstraints::new().with_primary_key(pk)).unwrap();
    ///
    /// // Create referencing relvar
    /// let emp_type = RelationType::new(
    ///     TupleType::new().with_attribute("dept_id", ScalarType::Int)
    /// );
    /// db.create_relvar("EMP", emp_type).unwrap();
    ///
    /// let fk = ForeignKey::new(
    ///     vec!["dept_id".to_string()],
    ///     "DEPT".to_string(),
    ///     vec!["dept_id".to_string()]
    /// ).unwrap();
    /// let constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
    ///
    /// db.set_foreign_key_constraints("EMP", constraints).unwrap();
    /// ```
    pub fn set_foreign_key_constraints(
        &mut self,
        relation_name: &str,
        constraints: ForeignKeyConstraints,
    ) -> Result<(), DatabaseError> {
        self.ensure_no_transaction("set_foreign_key_constraints")?;
        self.change_constraints(|manager, engine| {
            manager.set_foreign_key_constraints(engine, relation_name, constraints)
        })
    }

    /// Set type constraints for an attribute.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::constraints::AttributeConstraints;
    /// use relvar_core::constraints::TypeConstraint;
    /// use relvar_core::values::ScalarValue;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new().with_attribute("count", ScalarType::Int)
    /// );
    /// db.create_relvar("TEST", rel_type).unwrap();
    ///
    /// let attr_constraints = AttributeConstraints::new("count".to_string(), ScalarType::Int)
    ///     .with_constraint(TypeConstraint::Range { min: ScalarValue::Int(1), max: ScalarValue::Int(i64::MAX) });
    ///
    /// db.set_type_constraints("TEST", "count", attr_constraints).unwrap();
    /// ```
    pub fn set_type_constraints(
        &mut self,
        relation_name: &str,
        attribute_name: &str,
        constraints: AttributeConstraints,
    ) -> Result<(), DatabaseError> {
        self.ensure_no_transaction("set_type_constraints")?;
        self.change_constraints(|manager, engine| {
            manager.set_type_constraints(engine, relation_name, attribute_name, constraints)
        })
    }

    /// Set CHECK constraints for a relation.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::constraints::{CheckConstraints, CheckConstraint, ConstraintExpression, CmpOp, ValueOrRef};
    /// use relvar_core::values::ScalarValue;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("age", ScalarType::Int)
    /// );
    /// db.create_relvar("PEOPLE", rel_type).unwrap();
    ///
    /// // Age must be >= 0
    /// let constraints = CheckConstraints::new()
    ///     .with_constraint(CheckConstraint::new(
    ///         "valid_age",
    ///         "Age must be non-negative",
    ///         ConstraintExpression::Cmp {
    ///             left: "age".to_string(),
    ///             op: CmpOp::Gt,
    ///             right: ValueOrRef::Value(ScalarValue::Int(-1))
    ///         }
    ///     ));
    ///
    /// db.set_check_constraints("PEOPLE", constraints).unwrap();
    /// ```
    pub fn set_check_constraints(
        &mut self,
        relation_name: &str,
        constraints: CheckConstraints,
    ) -> Result<(), DatabaseError> {
        self.ensure_no_transaction("set_check_constraints")?;
        self.change_constraints(|manager, engine| {
            manager.set_check_constraints(engine, relation_name, constraints)
        })
    }
}
