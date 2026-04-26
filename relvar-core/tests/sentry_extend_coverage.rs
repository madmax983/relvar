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
#[cfg(test)]
mod tests_extend_into {
    use relvar_core::algebra::ExtendError;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::{Relation, ScalarValue};

    #[test]
    fn test_extend_into_computation_type_mismatch() {
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let mut r = Relation::new(RelationType::new(heading));
        r.insert(tuple! { id: 1i64 }).unwrap();

        // The computation function returns a String, but the expected type is Int.
        let result = r.extend_into("new_attr", ScalarType::Int, |_t| {
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
    fn test_extend_into_empty_relation() {
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let r = Relation::new(RelationType::new(heading));

        let result = r.extend_into("new_attr", ScalarType::Int, |_t| ScalarValue::Int(42));

        assert!(result.is_ok());
        let new_r = result.unwrap();
        assert_eq!(new_r.cardinality(), 0);
    }
}

// Extend error tests coverage

#[test]
fn test_extend_into_success() {
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::{Relation, ScalarValue};

    let rel_type = RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int));
    let mut relation = Relation::new(rel_type.clone());
    relation.insert(tuple! { x: 10i64 }).unwrap();

    let extended = relation
        .extend_into("y", ScalarType::Int, |t| {
            let x = t.get_typed::<i64>("x").unwrap();
            ScalarValue::Int(x + 5)
        })
        .unwrap();

    let result = extended.tuples().next().unwrap();
    assert_eq!(result.get_typed::<i64>("y").unwrap(), 15);
}

#[test]
fn test_extend_into_attribute_exists() {
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::{Relation, ScalarValue};

    let rel_type = RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int));
    let mut relation = Relation::new(rel_type.clone());
    relation.insert(tuple! { x: 10i64 }).unwrap();

    let result = relation.extend_into("x", ScalarType::Int, |_| ScalarValue::Int(15));

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(relvar_core::algebra::ExtendError::AttributeExists(_))
    ));
}

#[test]
fn test_extend_into_type_mismatch() {
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::{Relation, ScalarValue};

    let rel_type = RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int));
    let mut relation = Relation::new(rel_type.clone());
    relation.insert(tuple! { x: 10i64 }).unwrap();

    let result = relation.extend_into("y", ScalarType::Int, |_| {
        // Return a string instead of an int
        ScalarValue::String("hello".to_string())
    });

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(relvar_core::algebra::ExtendError::TupleCreation(_))
    ));
}

#[test]
fn test_extend_into_empty() {
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::{Relation, ScalarValue};

    let rel_type = RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int));
    let relation = Relation::new(rel_type.clone());

    let extended = relation
        .extend_into("y", ScalarType::Int, |_| ScalarValue::Int(15))
        .unwrap();

    assert_eq!(extended.cardinality(), 0);
}
