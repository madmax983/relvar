use crate::constraints::{
    AttributeConstraints, CheckConstraints, ForeignKeyConstraints, KeyConstraints,
};
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::StorageEngine;

impl<E: StorageEngine> Database<E> {
    /// Get the key constraints for a relation.
    ///
    /// Retrieving constraints is necessary when dynamically generating data entry
    /// forms or building query optimizers that rely on uniqueness guarantees.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::constraints::{KeyConstraints, PrimaryKey};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    /// db.create_relvar("TEST", rel_type).unwrap();
    ///
    /// let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    /// let constraints = KeyConstraints::new().with_primary_key(pk);
    /// db.set_key_constraints("TEST", constraints).unwrap();
    ///
    /// let current_constraints = db.get_key_constraints("TEST").unwrap();
    /// assert!(current_constraints.primary_key().is_some());
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
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::constraints::{ForeignKeyConstraints, ForeignKey};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let type1 = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    /// let type2 = RelationType::new(TupleType::new().with_attribute("ref_id", ScalarType::Int));
    /// db.create_relvar("A", type1).unwrap();
    /// db.create_relvar("B", type2).unwrap();
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
    /// # Example
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
        Ok(self
            .constraints
            .set_key_constraints(&mut self.engine, relation_name, constraints)?)
    }

    /// Set foreign key constraints for a relation.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::constraints::{ForeignKeyConstraints, ForeignKey};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    ///
    /// // Create referenced relvar
    /// let dept_type = RelationType::new(
    ///     TupleType::new().with_attribute("dept_id", ScalarType::Int)
    /// );
    /// db.create_relvar("DEPT", dept_type).unwrap();
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
        Ok(self.constraints.set_foreign_key_constraints(
            &mut self.engine,
            relation_name,
            constraints,
        )?)
    }

    /// Set type constraints for an attribute.
    ///
    /// # Example
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
        Ok(self.constraints.set_type_constraints(
            &mut self.engine,
            relation_name,
            attribute_name,
            constraints,
        )?)
    }

    /// Set CHECK constraints for a relation.
    ///
    /// # Example
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
        Ok(self
            .constraints
            .set_check_constraints(&mut self.engine, relation_name, constraints)?)
    }
}
