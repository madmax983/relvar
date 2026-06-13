#[cfg(test)]
mod tests {
    use relvar_core::constraints::{
        AttributeConstraints, CheckConstraint, CheckConstraints, CmpOp, ConstraintExpression,
        ForeignKey, ForeignKeyConstraints, KeyConstraints, PrimaryKey, TypeConstraint, ValueOrRef,
    };
    use relvar_core::database::Database;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::ScalarValue;

    #[test]
    fn test_delete_foreign_key_violation() {
        let mut db = Database::new(InMemoryEngine::new());
        let dept_type =
            RelationType::new(TupleType::new().with_attribute("dept_id", ScalarType::Int));
        let emp_type =
            RelationType::new(TupleType::new().with_attribute("dept_id", ScalarType::Int));

        db.create_relvar("DEPT", dept_type).unwrap();
        db.create_relvar("EMP", emp_type).unwrap();

        db.set_key_constraints(
            "DEPT",
            KeyConstraints::new()
                .with_primary_key(PrimaryKey::new(vec!["dept_id".to_string()]).unwrap()),
        )
        .unwrap();

        let fk = ForeignKey::new(
            vec!["dept_id".to_string()],
            "DEPT".to_string(),
            vec!["dept_id".to_string()],
        )
        .unwrap();
        db.set_foreign_key_constraints("EMP", ForeignKeyConstraints::new().with_foreign_key(fk))
            .unwrap();

        db.insert("DEPT", tuple! { dept_id: 1i64 }).unwrap();
        db.insert("EMP", tuple! { dept_id: 1i64 }).unwrap();

        // This should fail because EMP references DEPT
        let res = db.delete("DEPT", |t| t.get_typed::<i64>("dept_id").unwrap() == 1);
        assert!(res.is_err());
    }

    #[test]
    fn test_update_check_constraint_violation() {
        let mut db = Database::new(InMemoryEngine::new());
        let rel_type = RelationType::new(TupleType::new().with_attribute("age", ScalarType::Int));
        db.create_relvar("PEOPLE", rel_type).unwrap();

        let constraints = CheckConstraints::new().with_constraint(CheckConstraint::new(
            "valid_age",
            "Age must be non-negative",
            ConstraintExpression::Cmp {
                left: "age".to_string(),
                op: CmpOp::Gt,
                right: ValueOrRef::Value(ScalarValue::Int(-1)),
            },
        ));

        db.set_check_constraints("PEOPLE", constraints).unwrap();

        db.insert("PEOPLE", tuple! { age: 10i64 }).unwrap();

        let res = db.update(
            "PEOPLE",
            |t| t.get_typed::<i64>("age").unwrap() == 10,
            |_| {
                tuple! { age: -5i64 }
            },
        );

        assert!(res.is_err());
    }

    #[test]
    fn test_update_type_constraint_violation() {
        let mut db = Database::new(InMemoryEngine::new());
        let rel_type = RelationType::new(TupleType::new().with_attribute("count", ScalarType::Int));
        db.create_relvar("TEST", rel_type).unwrap();

        let attr_constraints = AttributeConstraints::new("count".to_string(), ScalarType::Int)
            .with_constraint(TypeConstraint::Range {
                min: ScalarValue::Int(1),
                max: ScalarValue::Int(100),
            });

        db.set_type_constraints("TEST", "count", attr_constraints)
            .unwrap();

        db.insert("TEST", tuple! { count: 50i64 }).unwrap();

        let res = db.update(
            "TEST",
            |t| t.get_typed::<i64>("count").unwrap() == 50,
            |_| {
                tuple! { count: 150i64 }
            },
        );

        assert!(res.is_err());
    }

    #[test]
    fn test_create_relvar_virtual_exists() {
        let mut db = Database::new(InMemoryEngine::new());
        let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));

        db.define_virtual_relvar("MY_VIEW", rel_type.clone(), |_| {
            Ok(relvar_core::values::Relation::new(
                relvar_core::types::RelationType::new(relvar_core::types::TupleType::new()),
            ))
        })
        .unwrap();

        // Should fail because a virtual relvar with this name already exists
        let res = db.create_relvar("MY_VIEW", rel_type);
        assert!(res.is_err());
    }
}
