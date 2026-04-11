#[cfg(test)]
mod tests {
    use relvar_core::error::DatabaseError;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::Relation;

    #[test]
    fn test_tclose_missing_from_attr() {
        let heading = TupleType::new()
            .with_attribute("start", ScalarType::Int)
            .with_attribute("end", ScalarType::Int);
        let mut r = Relation::new(RelationType::new(heading));
        r.insert(tuple! { start: 1i64, end: 2i64 }).unwrap();

        let result = r.tclose("missing_start", "end");
        match result {
            Err(DatabaseError::AttributeNotFound(attr, _)) => assert_eq!(attr, "missing_start"),
            _ => panic!("Expected AttributeNotFound error"),
        }
    }

    #[test]
    fn test_tclose_missing_to_attr() {
        let heading = TupleType::new()
            .with_attribute("start", ScalarType::Int)
            .with_attribute("end", ScalarType::Int);
        let mut r = Relation::new(RelationType::new(heading));
        r.insert(tuple! { start: 1i64, end: 2i64 }).unwrap();

        let result = r.tclose("start", "missing_end");
        match result {
            Err(DatabaseError::AttributeNotFound(attr, _)) => assert_eq!(attr, "missing_end"),
            _ => panic!("Expected AttributeNotFound error"),
        }
    }
}
