#[cfg(test)]
mod tests {
    use crate::values::{Relation, Tuple, ScalarValue};
    use crate::types::{RelationType, TupleType, ScalarType};
    use crate::error::DatabaseError;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    #[test]
    fn test_join_missing_attributes() {
        let type1 = Arc::new(TupleType::new().with_attribute("id", ScalarType::Int));
        let rel1_type = RelationType::new((*type1).clone());

        let mut values = BTreeMap::new();
        values.insert("wrong".to_string(), ScalarValue::Int(1));
        let tuple1 = Tuple::new_unchecked(type1, values);

        let rel1 = Relation::from_tuples_unchecked(rel1_type, vec![tuple1].into_iter());

        let type2 = Arc::new(TupleType::new().with_attribute("id", ScalarType::Int).with_attribute("name", ScalarType::String));
        let rel2_type = RelationType::new((*type2).clone());
        let mut values2 = BTreeMap::new();
        values2.insert("id".to_string(), ScalarValue::Int(1));
        values2.insert("name".to_string(), ScalarValue::String("Alice".to_string()));
        let tuple2 = Tuple::new_unchecked(type2, values2);

        let rel2 = Relation::from_tuples_unchecked(rel2_type, vec![tuple2].into_iter());

        let res = rel1.join(&rel2);

        assert!(res.is_err());
        if let Err(DatabaseError::AttributeNotFound(attr, _rel)) = res {
            assert_eq!(attr, "id");
        } else {
            panic!("Expected AttributeNotFound error, got {:?}", res);
        }
    }
}
