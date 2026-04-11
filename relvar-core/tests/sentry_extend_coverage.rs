#[cfg(test)]
mod tests {
    use relvar_core::algebra::ExtendError;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::{Relation, ScalarValue};

    #[test]
    fn test_extend_computation_type_mismatch() {
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let mut r = Relation::new(RelationType::new(heading));
        r.insert(tuple! { id: 1i64 }).unwrap();

        // The computation function returns a String, but the expected type is Int.
        let result = r.extend("new_attr", ScalarType::Int, |_t| {
            ScalarValue::String("not an int".to_string())
        });

        match result {
            Err(ExtendError::TupleCreation(msg)) => {
                assert!(
                    msg.contains(
                        "Type mismatch for attribute 'new_attr': expected Int, got String"
                    )
                );
            }
            _ => panic!("Expected TupleCreation error for type mismatch"),
        }
    }

    #[test]
    fn test_extend_empty_relation() {
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let r = Relation::new(RelationType::new(heading));

        // Extend empty relation, this should hit the lines returning Ok(Relation::from_body_unchecked)
        // and loop iteration. Wait, the loop will be empty, but it should cover the empty case!
        let result = r.extend("new_attr", ScalarType::Int, |_t| ScalarValue::Int(42));

        assert!(result.is_ok());
        let new_r = result.unwrap();
        assert_eq!(new_r.cardinality(), 0);
    }
}
