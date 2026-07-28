use super::*;

#[test]
fn test_scalar_value_error_display() {
    let err = ScalarValueError;
    assert_eq!(
        err.to_string(),
        "Cannot extract observer from built-in type"
    );
}

#[test]
fn test_values_carry_their_type() {
    let int_val = ScalarValue::Int(42);
    let float_val = ScalarValue::Float(3.5);
    let string_val = ScalarValue::String("hello".to_string());
    let bool_val = ScalarValue::Bool(true);
    let bytes_val = ScalarValue::Bytes(vec![1, 2, 3]);

    assert_eq!(int_val.scalar_type(), ScalarType::Int);
    assert_eq!(float_val.scalar_type(), ScalarType::Float);
    assert_eq!(string_val.scalar_type(), ScalarType::String);
    assert_eq!(bool_val.scalar_type(), ScalarType::Bool);
    assert_eq!(bytes_val.scalar_type(), ScalarType::Bytes);
}

#[test]
fn test_values_of_same_type_can_be_compared() {
    let int1 = ScalarValue::Int(42);
    let int2 = ScalarValue::Int(42);
    let int3 = ScalarValue::Int(43);

    assert_eq!(int1, int2);
    assert_ne!(int1, int3);

    let str1 = ScalarValue::String("hello".to_string());
    let str2 = ScalarValue::String("hello".to_string());
    let str3 = ScalarValue::String("world".to_string());

    assert_eq!(str1, str2);
    assert_ne!(str1, str3);
}

#[test]
fn test_values_of_different_types_are_not_equal() {
    let int_val = ScalarValue::Int(42);
    let float_val = ScalarValue::Float(42.0);

    assert_ne!(int_val, float_val);
}

#[test]
fn test_values_can_be_serialized_and_deserialized() {
    let original = ScalarValue::Int(42);
    let serialized = serde_json::to_string(&original).unwrap();
    let deserialized: ScalarValue = serde_json::from_str(&serialized).unwrap();

    assert_eq!(original, deserialized);

    let original = ScalarValue::String("test".to_string());
    let serialized = serde_json::to_string(&original).unwrap();
    let deserialized: ScalarValue = serde_json::from_str(&serialized).unwrap();

    assert_eq!(original, deserialized);
}

#[test]
fn test_float_equality_for_set_semantics() {
    // For database semantics, we need NaN == NaN
    let nan1 = ScalarValue::Float(f64::NAN);
    let nan2 = ScalarValue::Float(f64::NAN);

    assert_eq!(nan1, nan2);

    // Same bit pattern floats should be equal
    let f1 = ScalarValue::Float(2.5);
    let f2 = ScalarValue::Float(2.5);
    assert_eq!(f1, f2);
}

#[test]
fn test_values_can_be_hashed() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(ScalarValue::Int(42));
    set.insert(ScalarValue::Int(42)); // Duplicate
    set.insert(ScalarValue::Int(43));

    assert_eq!(set.len(), 2);

    // Test with floats including NaN
    let mut float_set = HashSet::new();
    float_set.insert(ScalarValue::Float(2.5));
    float_set.insert(ScalarValue::Float(2.5)); // Duplicate
    float_set.insert(ScalarValue::Float(f64::NAN));
    float_set.insert(ScalarValue::Float(f64::NAN)); // Duplicate NaN

    assert_eq!(float_set.len(), 2); // 2.5 and NaN
}

#[test]
fn test_is_type() {
    let int_val = ScalarValue::Int(42);
    assert!(int_val.is_type(&ScalarType::Int));
    assert!(!int_val.is_type(&ScalarType::Float));
}

// Consolidated tests from user_defined_test.rs

#[test]
fn test_user_defined_types_are_distinct_from_builtin_types() {
    // Define two user-defined types, both backed by Int
    let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
    let supplier_id_type = ScalarType::user_defined("SupplierId", ScalarType::Int);

    // These types should NOT be equal even though they have the same representation
    assert_ne!(widget_id_type, supplier_id_type);

    // They should also not equal the built-in Int type
    assert_ne!(widget_id_type, ScalarType::Int);
    assert_ne!(supplier_id_type, ScalarType::Int);
}

#[test]
fn test_user_defined_values_are_type_safe() {
    let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
    let supplier_id_type = ScalarType::user_defined("SupplierId", ScalarType::Int);

    // Create values with the same underlying Int value (5)
    let widget_5 = ScalarValue::user_defined(widget_id_type.clone(), ScalarValue::Int(5));
    let supplier_5 = ScalarValue::user_defined(supplier_id_type.clone(), ScalarValue::Int(5));

    // These should NOT be equal - different types!
    assert_ne!(widget_5, supplier_5);

    // Values should also not equal raw Int(5)
    assert_ne!(widget_5, ScalarValue::Int(5));
    assert_ne!(supplier_5, ScalarValue::Int(5));
}

#[test]
fn test_user_defined_values_of_same_type_are_equal() {
    let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);

    let widget_5_a = ScalarValue::user_defined(widget_id_type.clone(), ScalarValue::Int(5));
    let widget_5_b = ScalarValue::user_defined(widget_id_type.clone(), ScalarValue::Int(5));
    let widget_7 = ScalarValue::user_defined(widget_id_type.clone(), ScalarValue::Int(7));

    assert_eq!(widget_5_a, widget_5_b);
    assert_ne!(widget_5_a, widget_7);
}

#[test]
fn test_user_defined_types_have_names() {
    let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);

    assert_eq!(widget_id_type.name(), "WidgetId");
}

#[test]
fn test_user_defined_values_carry_their_type() {
    let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
    let widget_5 = ScalarValue::user_defined(widget_id_type.clone(), ScalarValue::Int(5));

    assert_eq!(widget_5.scalar_type(), widget_id_type);
    assert_ne!(widget_5.scalar_type(), ScalarType::Int);
}

#[test]
fn test_possrep_selector_constructs_value() {
    let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);

    // Selector: construct a WidgetId from an Int
    let widget = ScalarValue::select(&widget_id_type, ScalarValue::Int(42)).unwrap();

    assert_eq!(widget.scalar_type(), widget_id_type);
}

#[test]
fn test_possrep_observer_extracts_representation() {
    let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
    let widget = ScalarValue::select(&widget_id_type, ScalarValue::Int(42)).unwrap();

    // Observer: extract the underlying Int value
    let underlying = widget.observer().unwrap();

    assert_eq!(underlying, ScalarValue::Int(42));
}

#[test]
fn test_nested_user_defined_types() {
    let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
    let special_widget_type = ScalarType::user_defined("SpecialWidgetId", widget_id_type.clone());

    let widget = ScalarValue::select(&widget_id_type, ScalarValue::Int(42)).unwrap();
    let special_widget = ScalarValue::select(&special_widget_type, widget.clone()).unwrap();

    assert_ne!(special_widget.scalar_type(), widget_id_type);
    assert_eq!(special_widget.scalar_type(), special_widget_type);
}

#[test]
fn test_type_safety_prevents_wrong_representation() {
    let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);

    // Should fail: trying to construct WidgetId from String
    let result = ScalarValue::select(
        &widget_id_type,
        ScalarValue::String("not an int".to_string()),
    );

    assert!(result.is_err());
}

#[test]
fn test_user_defined_types_serialize() {
    let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
    let widget = ScalarValue::select(&widget_id_type, ScalarValue::Int(42)).unwrap();

    let serialized = serde_json::to_string(&widget).unwrap();
    let deserialized: ScalarValue = serde_json::from_str(&serialized).unwrap();

    assert_eq!(widget, deserialized);
    assert_eq!(deserialized.scalar_type(), widget_id_type);
}

#[test]
fn test_user_defined_values_can_be_hashed() {
    use std::collections::HashSet;

    let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
    let supplier_id_type = ScalarType::user_defined("SupplierId", ScalarType::Int);

    let mut set = HashSet::new();
    set.insert(ScalarValue::select(&widget_id_type, ScalarValue::Int(5)).unwrap());
    set.insert(ScalarValue::select(&widget_id_type, ScalarValue::Int(5)).unwrap()); // Duplicate
    set.insert(ScalarValue::select(&supplier_id_type, ScalarValue::Int(5)).unwrap()); // Different type
    set.insert(ScalarValue::Int(5)); // Raw Int

    // Should have 3 distinct values:
    // - WidgetId(5)
    // - SupplierId(5)
    // - Int(5)
    assert_eq!(set.len(), 3);
}

#[test]
fn test_user_defined_type_names_must_be_unique() {
    let type1 = ScalarType::user_defined("MyType", ScalarType::Int);
    let type2 = ScalarType::user_defined("MyType", ScalarType::String);

    // This test documents that types with the same name but different
    // representations are distinct, as `PartialEq` is structural.
    assert_ne!(type1, type2);
    assert_eq!(type1.name(), type2.name());
}

// Bytes tests
#[test]
fn test_bytes_equality() {
    let b1 = ScalarValue::Bytes(vec![]);
    let b2 = ScalarValue::Bytes(vec![]);
    let b3 = ScalarValue::Bytes(vec![1, 2, 3]);
    let b4 = ScalarValue::Bytes(vec![1, 2, 3]);

    assert_eq!(b1, b2);
    assert_eq!(b3, b4);
    assert_ne!(b1, b3);
}

#[test]
fn test_bytes_hashing() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(ScalarValue::Bytes(vec![1, 2, 3]));
    set.insert(ScalarValue::Bytes(vec![1, 2, 3]));
    set.insert(ScalarValue::Bytes(vec![]));

    assert_eq!(set.len(), 2);
}

#[test]
fn test_bytes_serialization() {
    let val = ScalarValue::Bytes(vec![1, 2, 3, 255]);
    let serialized = serde_json::to_string(&val).unwrap();
    let deserialized: ScalarValue = serde_json::from_str(&serialized).unwrap();
    assert_eq!(val, deserialized);
}

// Relation value tests (RVAs)
#[test]
fn test_relation_value_equality() {
    use crate::tuple;
    use crate::types::{RelationType, TupleType};
    use crate::values::Relation;

    let heading = TupleType::new().with_attribute("a", ScalarType::Int);
    let rel_type = RelationType::new(heading);

    let mut rel1 = Relation::new(rel_type.clone());
    rel1.insert(tuple! { a: 1i64 }).unwrap();

    let mut rel2 = Relation::new(rel_type);
    rel2.insert(tuple! { a: 1i64 }).unwrap();

    let val1 = ScalarValue::Relation(rel1);
    let val2 = ScalarValue::Relation(rel2);

    assert_eq!(val1, val2);
}

#[test]
fn test_relation_value_hashing() {
    use crate::tuple;
    use crate::types::{RelationType, TupleType};
    use crate::values::Relation;
    use std::collections::HashSet;

    let heading = TupleType::new().with_attribute("a", ScalarType::Int);
    let rel_type = RelationType::new(heading);

    let mut rel1 = Relation::new(rel_type.clone());
    rel1.insert(tuple! { a: 1i64 }).unwrap();

    let mut rel2 = Relation::new(rel_type);
    rel2.insert(tuple! { a: 1i64 }).unwrap();

    let mut set = HashSet::new();
    set.insert(ScalarValue::Relation(rel1));
    set.insert(ScalarValue::Relation(rel2)); // Duplicate

    assert_eq!(set.len(), 1);
}

#[test]
fn test_relation_value_serialization() {
    use crate::tuple;
    use crate::types::{RelationType, TupleType};
    use crate::values::Relation;

    let heading = TupleType::new().with_attribute("a", ScalarType::Int);
    let rel_type = RelationType::new(heading);

    let mut rel = Relation::new(rel_type);
    rel.insert(tuple! { a: 1i64 }).unwrap();
    rel.insert(tuple! { a: 2i64 }).unwrap();

    let val = ScalarValue::Relation(rel);
    let serialized = serde_json::to_string(&val).unwrap();
    let deserialized: ScalarValue = serde_json::from_str(&serialized).unwrap();

    assert_eq!(val, deserialized);
}

#[test]
fn test_nested_relation_value() {
    use crate::tuple;
    use crate::types::{RelationType, TupleType};
    use crate::values::Relation;

    // Inner relation: {a: Int}
    let inner_heading = TupleType::new().with_attribute("a", ScalarType::Int);
    let inner_rel_type = RelationType::new(inner_heading.clone());

    let mut inner_rel = Relation::new(inner_rel_type.clone());
    inner_rel.insert(tuple! { a: 42i64 }).unwrap();

    // Outer relation: {id: Int, data: Relation}
    let outer_heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("data", ScalarType::Relation(Box::new(inner_rel_type)));

    let outer_rel_type = RelationType::new(outer_heading);
    let mut outer_rel = Relation::new(outer_rel_type);

    outer_rel
        .insert(tuple! {
            id: 1i64,
            data: ScalarValue::Relation(inner_rel)
        })
        .unwrap();

    assert_eq!(outer_rel.cardinality(), 1);

    // Verify we can retrieve the RVA
    let tuple = outer_rel.tuples().next().unwrap();
    let data = tuple.get("data").unwrap();

    match data {
        ScalarValue::Relation(rel) => {
            assert_eq!(rel.cardinality(), 1);
            assert_eq!(rel.degree(), 1);

            let inner_tuple = rel.tuples().next().unwrap();
            assert_eq!(inner_tuple.get("a"), Some(&ScalarValue::Int(42)));
        }
        _ => panic!("Expected relation value"),
    }
}
