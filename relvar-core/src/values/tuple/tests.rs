#![allow(clippy::module_inception)]
use super::*;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ScalarType;
    use std::collections::HashMap;

    #[test]
    fn test_tuple_conforms_to_type() {
        let tuple_type = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let mut values = HashMap::new();
        values.insert("emp_id".to_string(), ScalarValue::Int(1));
        values.insert("name".to_string(), ScalarValue::String("Alice".to_string()));

        let tuple = Tuple::new(tuple_type, values).unwrap();
        assert_eq!(tuple.degree(), 2);
    }

    #[test]
    fn test_tuple_missing_value_fails() {
        let tuple_type = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let mut values = HashMap::new();
        values.insert("emp_id".to_string(), ScalarValue::Int(1));
        // Missing "name"

        let result = Tuple::new(tuple_type, values);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TupleError::MissingValue(_)));
    }

    #[test]
    fn test_tuple_type_mismatch_fails() {
        let tuple_type = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let mut values = HashMap::new();
        values.insert("emp_id".to_string(), ScalarValue::Int(1));
        values.insert("name".to_string(), ScalarValue::Int(42)); // Wrong type

        let result = Tuple::new(tuple_type, values);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TupleError::TypeMismatch(_, _, _)
        ));
    }

    #[test]
    fn test_tuple_equality() {
        let tuple_type = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let mut values1 = HashMap::new();
        values1.insert("emp_id".to_string(), ScalarValue::Int(1));
        values1.insert("name".to_string(), ScalarValue::String("Alice".to_string()));

        let mut values2 = HashMap::new();
        values2.insert("name".to_string(), ScalarValue::String("Alice".to_string()));
        values2.insert("emp_id".to_string(), ScalarValue::Int(1)); // Different insertion order

        let tuple1 = Tuple::new(tuple_type.clone(), values1).unwrap();
        let tuple2 = Tuple::new(tuple_type, values2).unwrap();

        assert_eq!(tuple1, tuple2);
    }

    #[test]
    fn test_tuple_attribute_access() {
        let tuple_type = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let mut values = HashMap::new();
        values.insert("emp_id".to_string(), ScalarValue::Int(42));
        values.insert("name".to_string(), ScalarValue::String("Bob".to_string()));

        let tuple = Tuple::new(tuple_type, values).unwrap();

        assert_eq!(tuple.get("emp_id"), Some(&ScalarValue::Int(42)));
        assert_eq!(
            tuple.get("name"),
            Some(&ScalarValue::String("Bob".to_string()))
        );
        assert_eq!(tuple.get("nonexistent"), None);
    }

    #[test]
    fn test_tuple_macro() {
        let tuple = tuple! {
            emp_id: 1i64,
            name: "Alice",
            active: true,
        };

        assert_eq!(tuple.degree(), 3);
        assert_eq!(tuple.get("emp_id"), Some(&ScalarValue::Int(1)));
        assert_eq!(
            tuple.get("name"),
            Some(&ScalarValue::String("Alice".to_string()))
        );
        assert_eq!(tuple.get("active"), Some(&ScalarValue::Bool(true)));
    }

    #[test]
    fn test_get_typed() {
        let tuple = tuple! {
            emp_id: 42i64,
            name: "Bob",
        };

        let emp_id: i64 = tuple.get_typed("emp_id").unwrap();
        assert_eq!(emp_id, 42);

        let name: String = tuple.get_typed("name").unwrap();
        assert_eq!(name, "Bob");
    }

    #[test]
    fn test_tuple_hash_consistency_with_different_insertion_order() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let tuple_type = TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int)
            .with_attribute("c", ScalarType::Int);

        // Insertion order 1: a, b, c
        let mut values1 = HashMap::new();
        values1.insert("a".to_string(), ScalarValue::Int(1));
        values1.insert("b".to_string(), ScalarValue::Int(2));
        values1.insert("c".to_string(), ScalarValue::Int(3));
        let tuple1 = Tuple::new(tuple_type.clone(), values1).unwrap();

        // Insertion order 2: c, b, a (reversed)
        let mut values2 = HashMap::new();
        values2.insert("c".to_string(), ScalarValue::Int(3));
        values2.insert("b".to_string(), ScalarValue::Int(2));
        values2.insert("a".to_string(), ScalarValue::Int(1));
        let tuple2 = Tuple::new(tuple_type, values2).unwrap();

        // Hashes should be identical despite different insertion orders
        let mut hasher1 = DefaultHasher::new();
        tuple1.hash(&mut hasher1);
        let hash1 = hasher1.finish();

        let mut hasher2 = DefaultHasher::new();
        tuple2.hash(&mut hasher2);
        let hash2 = hasher2.finish();

        assert_eq!(
            hash1, hash2,
            "Tuples with same content but different insertion order must hash identically"
        );
    }

    #[test]
    fn test_tuple_extra_attribute_fails() {
        let tuple_type = TupleType::new().with_attribute("emp_id", ScalarType::Int);

        let mut values = HashMap::new();
        values.insert("emp_id".to_string(), ScalarValue::Int(1));
        values.insert("extra".to_string(), ScalarValue::Int(2)); // Extra attribute

        let result = Tuple::new(tuple_type, values);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TupleError::AttributeNotFound(attr) if attr == "extra"
        ));
    }

    #[test]
    fn test_tuple_empty() {
        let tuple_type = TupleType::new(); // Empty type (degree 0)
        let values: HashMap<String, ScalarValue> = HashMap::new(); // Empty values

        let tuple = Tuple::new(tuple_type, values).unwrap();
        assert_eq!(tuple.degree(), 0);
        assert!(tuple.values().is_empty());
        assert_eq!(tuple.get("any"), None);
    }

    #[test]
    fn test_tuple_set_valid() {
        let tuple_type = TupleType::new().with_attribute("val", ScalarType::Int);

        let mut values = HashMap::new();
        values.insert("val".to_string(), ScalarValue::Int(10));

        let mut tuple = Tuple::new(tuple_type, values).unwrap();

        // Update valid attribute with valid type
        let result = tuple.set("val".to_string(), ScalarValue::Int(20));
        assert!(result.is_ok());
        assert_eq!(tuple.get("val"), Some(&ScalarValue::Int(20)));
    }

    #[test]
    fn test_tuple_set_invalid_type() {
        let tuple_type = TupleType::new().with_attribute("val", ScalarType::Int);

        let mut values = HashMap::new();
        values.insert("val".to_string(), ScalarValue::Int(10));

        let mut tuple = Tuple::new(tuple_type, values).unwrap();

        // Update valid attribute with WRONG type
        let result = tuple.set("val".to_string(), ScalarValue::String("wrong".to_string()));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TupleError::TypeMismatch(_, _, _)
        ));

        // Verify value was NOT updated
        assert_eq!(tuple.get("val"), Some(&ScalarValue::Int(10)));
    }

    #[test]
    fn test_tuple_set_attribute_not_found() {
        let tuple_type = TupleType::new().with_attribute("val", ScalarType::Int);

        let mut values = HashMap::new();
        values.insert("val".to_string(), ScalarValue::Int(10));

        let mut tuple = Tuple::new(tuple_type, values).unwrap();

        // Try to set non-existent attribute
        let result = tuple.set("new_attr".to_string(), ScalarValue::Int(20));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TupleError::AttributeNotFound(attr) if attr == "new_attr"
        ));
    }

    #[test]
    fn test_tuple_from_different_iterators() {
        let tuple_type = TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int);

        // Case 1: Vec<(String, ScalarValue)>
        let vec_values = vec![
            ("a".to_string(), ScalarValue::Int(1)),
            ("b".to_string(), ScalarValue::Int(2)),
        ];
        let tuple_from_vec = Tuple::new(tuple_type.clone(), vec_values).unwrap();
        assert_eq!(tuple_from_vec.degree(), 2);

        // Case 2: BTreeMap<String, ScalarValue>
        use std::collections::BTreeMap;
        let mut btree_values = BTreeMap::new();
        btree_values.insert("a".to_string(), ScalarValue::Int(1));
        btree_values.insert("b".to_string(), ScalarValue::Int(2));

        let tuple_from_btree = Tuple::new(tuple_type, btree_values).unwrap();
        assert_eq!(tuple_from_btree.degree(), 2);

        assert_eq!(tuple_from_vec, tuple_from_btree);
    }

    #[test]
    fn test_try_from_scalar_value_mismatch() {
        let int_val = ScalarValue::Int(42);
        let float_val = ScalarValue::Float(1.23);
        let str_val = ScalarValue::String("hello".to_string());
        let bool_val = ScalarValue::Bool(true);
        let bytes_val = ScalarValue::Bytes(vec![1, 2, 3]);

        // Test TryFrom<ScalarValue> mismatch (consuming)
        assert_eq!(i64::try_from(float_val.clone()), Err(()));
        assert_eq!(f64::try_from(int_val.clone()), Err(()));
        assert_eq!(String::try_from(int_val.clone()), Err(()));
        assert_eq!(bool::try_from(int_val.clone()), Err(()));
        assert_eq!(Vec::<u8>::try_from(int_val.clone()), Err(()));

        // Test TryFrom<&ScalarValue> mismatch (borrowing)
        assert_eq!(<&str>::try_from(&int_val), Err(()));
        assert_eq!(i64::try_from(&str_val), Err(()));
        assert_eq!(f64::try_from(&str_val), Err(()));
        assert_eq!(String::try_from(&bool_val), Err(()));
        assert_eq!(bool::try_from(&bytes_val), Err(()));
        assert_eq!(Vec::<u8>::try_from(&str_val), Err(()));

        // Test get_typed mismatch
        let tuple = tuple! {
            id: 1i64,
            name: "Alice",
        };

        assert_eq!(tuple.get_typed::<f64>("id"), None);
        assert_eq!(tuple.get_typed::<String>("id"), None);
        assert_eq!(tuple.get_typed::<i64>("name"), None);
    }
}
