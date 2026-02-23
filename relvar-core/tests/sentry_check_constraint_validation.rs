#[cfg(test)]
mod tests {
    use relvar_core::database::Database;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::constraints::{CheckConstraint, CheckConstraints, ConstraintManagerError, ConstraintExpression, CmpOp, ValueOrRef};
    use relvar_core::values::ScalarValue;
    use relvar_core::tuple;
    use relvar_core::error::DatabaseError;

    #[test]
    fn test_check_constraint_invalid_attribute_empty_relation() {
        let mut db = Database::new(InMemoryEngine::new());

        let heading = TupleType::new().with_attribute("a", ScalarType::Int);
        let rel_type = RelationType::new(heading);
        db.create_relvar("test_rel", rel_type).unwrap();

        // Relation is empty.
        // Add a check constraint referencing non-existent attribute "b".
        let constraint = CheckConstraint::new(
            "invalid_attr",
            "b must be positive",
            ConstraintExpression::Cmp {
                left: "b".to_string(), // "b" does not exist
                op: CmpOp::Gt,
                right: ValueOrRef::Value(ScalarValue::Int(0)),
            },
        );
        let constraints = CheckConstraints::new().with_constraint(constraint);

        // This should now FAIL because we validate against schema even if relation is empty
        let result = db.set_check_constraints("test_rel", constraints);

        assert!(result.is_err(), "Setting invalid check constraint should fail");

        match result.unwrap_err() {
            DatabaseError::Constraint(ConstraintManagerError::AttributeNotFound(attr, rel)) => {
                assert_eq!(attr, "b");
                assert_eq!(rel, "test_rel");
            },
            err => panic!("Expected AttributeNotFound error, got: {:?}", err),
        }

        // Insert should work fine (since constraint wasn't added)
        let insert_result = db.insert("test_rel", tuple! { a: 1i64 });
        assert!(insert_result.is_ok());
    }
}
