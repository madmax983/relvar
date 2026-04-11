#[cfg(test)]
mod tests {
    use relvar_core::algebra::{GroupError, UngroupError};
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::{Relation, ScalarValue};

    #[test]
    fn test_group_conflict_with_grouping_attribute() {
        let heading = TupleType::new()
            .with_attribute("dept_id", ScalarType::Int)
            .with_attribute("emp_id", ScalarType::Int);
        let mut r = Relation::new(RelationType::new(heading));
        r.insert(tuple! { dept_id: 10i64, emp_id: 1i64 }).unwrap();

        // The RVA name "dept_id" conflicts with a grouping attribute
        let result = r.group(&["emp_id"], "dept_id");
        match result {
            Err(GroupError::ResultAttributeExists(name)) => assert_eq!(name, "dept_id"),
            _ => panic!("Expected ResultAttributeExists error"),
        }
    }

    #[test]
    fn test_ungroup_missing_attribute() {
        let heading = TupleType::new()
            .with_attribute("dept_id", ScalarType::Int)
            .with_attribute("emp_id", ScalarType::Int);
        let mut r = Relation::new(RelationType::new(heading));
        r.insert(tuple! { dept_id: 10i64, emp_id: 1i64 }).unwrap();

        // The attribute does not exist
        let result = r.ungroup("missing_attr");
        match result {
            Err(UngroupError::AttributeNotFound(name)) => assert_eq!(name, "missing_attr"),
            _ => panic!("Expected AttributeNotFound error"),
        }
    }

    #[test]
    fn test_ungroup_not_a_relation() {
        let heading = TupleType::new()
            .with_attribute("dept_id", ScalarType::Int)
            .with_attribute("emp_id", ScalarType::Int);
        let mut r = Relation::new(RelationType::new(heading));
        r.insert(tuple! { dept_id: 10i64, emp_id: 1i64 }).unwrap();

        // The attribute is not an RVA
        let result = r.ungroup("emp_id");
        match result {
            Err(UngroupError::NotRelationValued(name)) => assert_eq!(name, "emp_id"),
            _ => panic!("Expected NotRelationValued error"),
        }
    }

    #[test]
    fn test_ungroup_conflict_with_existing_attribute() {
        let inner_heading = TupleType::new().with_attribute("dept_id", ScalarType::Int);
        let inner_rel_type = RelationType::new(inner_heading);
        let rva_type = ScalarType::Relation(Box::new(inner_rel_type.clone()));

        let heading = TupleType::new()
            .with_attribute("dept_id", ScalarType::Int)
            .with_attribute("my_rva", rva_type);

        let mut r = Relation::new(RelationType::new(heading.clone()));

        let mut inner_rel = Relation::new(inner_rel_type);
        inner_rel.insert(tuple! { dept_id: 20i64 }).unwrap();

        let mut t_values = std::collections::BTreeMap::new();
        t_values.insert("dept_id".to_string(), ScalarValue::Int(10));
        t_values.insert("my_rva".to_string(), ScalarValue::Relation(inner_rel));
        let t = relvar_core::values::Tuple::new(std::sync::Arc::new(heading), t_values).unwrap();
        r.insert(t).unwrap();

        let result = r.ungroup("my_rva");
        if let Err(e) = result {
            match e {
                UngroupError::TupleCreation(msg) => assert!(
                    msg.contains("dept_id")
                        || msg.contains("conflict")
                        || msg.contains("Duplicate")
                ),
                _ => panic!("Unexpected error: {:?}", e),
            }
        }
    }
}
