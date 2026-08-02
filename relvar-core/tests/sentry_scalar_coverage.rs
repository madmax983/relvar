use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

#[test]
fn test_scalar_value_hash_coverage() {
    let bool_val = ScalarValue::Bool(true);
    let bytes_val = ScalarValue::Bytes(vec![1, 2, 3]);
    let rel_type = RelationType::new(TupleType::new().with_attribute("a", ScalarType::Int));
    let rel_val = ScalarValue::Relation(Relation::new(rel_type));

    // Hash coverage for Bool, Bytes, Relation
    assert_ne!(
        calculate_hash(&bool_val),
        calculate_hash(&ScalarValue::Bool(false))
    );
    assert_ne!(
        calculate_hash(&bytes_val),
        calculate_hash(&ScalarValue::Bytes(vec![1, 2]))
    );

    // Hash of Relation
    let _ = calculate_hash(&rel_val);
}

#[test]
fn test_scalar_value_cmp_coverage() {
    // Cross-type comparison coverage
    let int_val = ScalarValue::Int(42);
    let string_val = ScalarValue::String("hello".to_string());
    let bool_val = ScalarValue::Bool(true);
    let bytes_val = ScalarValue::Bytes(vec![1, 2, 3]);
    let rel_type = RelationType::new(TupleType::new().with_attribute("a", ScalarType::Int));
    let rel_val = ScalarValue::Relation(Relation::new(rel_type));

    // Test the type_order logic
    assert!(int_val < string_val);
    assert!(string_val < bool_val);
    assert!(bool_val < bytes_val);
    assert!(bytes_val < rel_val);

    // Same-type comparison for Bool and Bytes
    assert!(ScalarValue::Bool(false) < ScalarValue::Bool(true));
    assert!(ScalarValue::Bytes(vec![1]) < ScalarValue::Bytes(vec![1, 2]));
}

#[test]
fn test_relation_cmp_coverage() {
    let rel_type1 = RelationType::new(TupleType::new().with_attribute("a", ScalarType::Int));
    let rel1 = Relation::new(rel_type1.clone());

    let rel_type2 = RelationType::new(
        TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int),
    );
    let rel2 = Relation::new(rel_type2.clone());

    let sv_rel1 = ScalarValue::Relation(rel1.clone());
    let sv_rel2 = ScalarValue::Relation(rel2.clone());

    // Compare relations (cmp_relations)
    // Both have 0 cardinality, so it falls back to degree.
    // rel1 has degree 1, rel2 has degree 2. So rel1 < rel2.
    assert!(sv_rel1 < sv_rel2);
}

#[test]
fn test_user_defined_value_hash_cmp_coverage() {
    let custom_type = ScalarType::UserDefined {
        name: "MyType".to_string(),
        representation: Box::new(ScalarType::Int),
    };

    let inner_val = ScalarValue::Int(42);

    let ud_val1 = ScalarValue::UserDefined {
        type_def: custom_type.clone(),
        value: Box::new(inner_val.clone()),
    };

    let ud_val2 = ScalarValue::UserDefined {
        type_def: custom_type.clone(),
        value: Box::new(ScalarValue::Int(43)),
    };

    // Test hashing
    assert_ne!(calculate_hash(&ud_val1), calculate_hash(&ud_val2));

    // Nested user defined type
    let nested_type = ScalarType::UserDefined {
        name: "NestedType".to_string(),
        representation: Box::new(custom_type.clone()),
    };

    let nested_val = ScalarValue::UserDefined {
        type_def: nested_type.clone(),
        value: Box::new(ud_val1.clone()),
    };

    let _ = calculate_hash(&nested_val);

    // Test comparison
    assert!(ud_val1 < ud_val2);

    // Type mismatch in user-defined cmp
    let custom_type2 = ScalarType::UserDefined {
        name: "OtherType".to_string(),
        representation: Box::new(ScalarType::Int),
    };
    let ud_val3 = ScalarValue::UserDefined {
        type_def: custom_type2.clone(),
        value: Box::new(inner_val.clone()),
    };

    // Since names differ ("MyType" < "OtherType"), cmp will use name cmp
    // "MyType" vs "OtherType": M < O
    assert!(ud_val1 < ud_val3);
}
