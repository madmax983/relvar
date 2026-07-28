use super::*;

#[test]
fn test_scalar_types_can_be_compared_for_equality() {
    let int1 = ScalarType::Int;
    let int2 = ScalarType::Int;
    let float1 = ScalarType::Float;

    assert_eq!(int1, int2);
    assert_ne!(int1, float1);
}

#[test]
fn test_type_names_are_unique_identifiers() {
    let int_type = ScalarType::Int;
    let float_type = ScalarType::Float;
    let string_type = ScalarType::String;
    let bool_type = ScalarType::Bool;
    let bytes_type = ScalarType::Bytes;

    assert_eq!(int_type.name(), "Int");
    assert_eq!(float_type.name(), "Float");
    assert_eq!(string_type.name(), "String");
    assert_eq!(bool_type.name(), "Bool");
    assert_eq!(bytes_type.name(), "Bytes");

    // All names should be unique
    let names = [
        int_type.name(),
        float_type.name(),
        string_type.name(),
        bool_type.name(),
        bytes_type.name(),
    ];
    let unique_names: std::collections::HashSet<_> = names.iter().collect();
    assert_eq!(names.len(), unique_names.len());
}

#[test]
fn test_built_in_types_exist() {
    // Verify all built-in types can be constructed
    let _ = ScalarType::Int;
    let _ = ScalarType::Float;
    let _ = ScalarType::String;
    let _ = ScalarType::Bool;
    let _ = ScalarType::Bytes;
}

#[test]
fn test_scalar_types_implement_clone() {
    let int_type = ScalarType::Int;
    let cloned = int_type.clone();
    assert_eq!(int_type, cloned);
}

#[test]
fn test_scalar_types_can_be_hashed() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(ScalarType::Int);
    set.insert(ScalarType::Float);
    set.insert(ScalarType::Int); // Duplicate

    assert_eq!(set.len(), 2); // Only Int and Float
    assert!(set.contains(&ScalarType::Int));
    assert!(set.contains(&ScalarType::Float));
}

// Tests for Ord/PartialOrd implementations (for coverage)
#[test]
fn test_scalar_type_ord_basic_types() {
    use std::cmp::Ordering;

    // Test discriminant ordering: Int < Float < String < Bool < Bytes < Relation < UserDefined
    assert_eq!(ScalarType::Int.cmp(&ScalarType::Float), Ordering::Less);
    assert_eq!(ScalarType::Float.cmp(&ScalarType::String), Ordering::Less);
    assert_eq!(ScalarType::String.cmp(&ScalarType::Bool), Ordering::Less);
    assert_eq!(ScalarType::Bool.cmp(&ScalarType::Bytes), Ordering::Less);

    // Same types are equal
    assert_eq!(ScalarType::Int.cmp(&ScalarType::Int), Ordering::Equal);
    assert_eq!(ScalarType::Float.cmp(&ScalarType::Float), Ordering::Equal);
    assert_eq!(ScalarType::String.cmp(&ScalarType::String), Ordering::Equal);
    assert_eq!(ScalarType::Bool.cmp(&ScalarType::Bool), Ordering::Equal);
    assert_eq!(ScalarType::Bytes.cmp(&ScalarType::Bytes), Ordering::Equal);
}

#[test]
fn test_scalar_type_ord_relation_types() {
    use crate::types::{RelationType, TupleType};
    use std::cmp::Ordering;

    // Create relation types with different headings
    let heading_a = TupleType::new().with_attribute("a", ScalarType::Int);
    let heading_b = TupleType::new().with_attribute("b", ScalarType::Int);
    let heading_a_copy = TupleType::new().with_attribute("a", ScalarType::Int);

    let rel_type_a = ScalarType::Relation(Box::new(RelationType::new(heading_a)));
    let rel_type_b = ScalarType::Relation(Box::new(RelationType::new(heading_b)));
    let rel_type_a_copy = ScalarType::Relation(Box::new(RelationType::new(heading_a_copy)));

    // Same heading should be equal
    assert_eq!(rel_type_a.cmp(&rel_type_a_copy), Ordering::Equal);

    // Different headings should have consistent ordering
    let result = rel_type_a.cmp(&rel_type_b);
    assert_ne!(result, Ordering::Equal);

    // Ordering should be transitive and antisymmetric
    assert_eq!(rel_type_b.cmp(&rel_type_a), result.reverse());
}

#[test]
fn test_scalar_type_ord_relation_types_different_degrees() {
    use crate::types::{RelationType, TupleType};
    use std::cmp::Ordering;

    // Different degree headings
    let heading_1 = TupleType::new().with_attribute("a", ScalarType::Int);
    let heading_2 = TupleType::new()
        .with_attribute("a", ScalarType::Int)
        .with_attribute("b", ScalarType::Int);

    let rel_type_1 = ScalarType::Relation(Box::new(RelationType::new(heading_1)));
    let rel_type_2 = ScalarType::Relation(Box::new(RelationType::new(heading_2)));

    // Different degrees should have consistent ordering
    let result = rel_type_1.cmp(&rel_type_2);
    assert_ne!(result, Ordering::Equal);
    assert_eq!(rel_type_2.cmp(&rel_type_1), result.reverse());
}

#[test]
fn test_scalar_type_ord_user_defined_by_name() {
    use std::cmp::Ordering;

    let widget_id = ScalarType::user_defined("WidgetId", ScalarType::Int);
    let supplier_id = ScalarType::user_defined("SupplierId", ScalarType::Int);
    let widget_id_copy = ScalarType::user_defined("WidgetId", ScalarType::Int);

    // Same name and representation should be equal
    assert_eq!(widget_id.cmp(&widget_id_copy), Ordering::Equal);

    // Different names should have consistent ordering
    let result = widget_id.cmp(&supplier_id);
    assert_ne!(result, Ordering::Equal);
    assert_eq!(supplier_id.cmp(&widget_id), result.reverse());
}

#[test]
fn test_scalar_type_ord_user_defined_by_representation() {
    use std::cmp::Ordering;

    // Same name but different representation
    let widget_id_int = ScalarType::user_defined("WidgetId", ScalarType::Int);
    let widget_id_string = ScalarType::user_defined("WidgetId", ScalarType::String);

    // Different representations should have consistent ordering
    let result = widget_id_int.cmp(&widget_id_string);
    assert_ne!(result, Ordering::Equal);
    assert_eq!(widget_id_string.cmp(&widget_id_int), result.reverse());
}

#[test]
fn test_scalar_type_ord_mixed_variants() {
    use crate::types::{RelationType, TupleType};
    use std::cmp::Ordering;

    let int_type = ScalarType::Int;
    let relation_type = ScalarType::Relation(Box::new(RelationType::new(
        TupleType::new().with_attribute("a", ScalarType::Int),
    )));
    let user_type = ScalarType::user_defined("CustomType", ScalarType::Int);

    // Relation comes after basic types
    assert_eq!(int_type.cmp(&relation_type), Ordering::Less);

    // UserDefined comes after Relation
    assert_eq!(relation_type.cmp(&user_type), Ordering::Less);

    // Transitive property
    assert_eq!(int_type.cmp(&user_type), Ordering::Less);
}

#[test]
fn test_scalar_type_partial_ord_consistency() {
    // PartialOrd should be consistent with Ord
    let int_type = ScalarType::Int;
    let float_type = ScalarType::Float;

    assert_eq!(
        int_type.partial_cmp(&float_type),
        Some(int_type.cmp(&float_type))
    );
    assert_eq!(
        int_type.partial_cmp(&int_type),
        Some(std::cmp::Ordering::Equal)
    );
}

#[test]
fn test_scalar_type_ord_reflexivity() {
    // x.cmp(x) == Equal (reflexivity)
    let types = vec![
        ScalarType::Int,
        ScalarType::Float,
        ScalarType::String,
        ScalarType::Bool,
        ScalarType::Bytes,
        ScalarType::user_defined("Test", ScalarType::Int),
    ];

    for ty in types {
        assert_eq!(ty.cmp(&ty), std::cmp::Ordering::Equal);
    }
}

#[test]
fn test_scalar_type_ord_transitivity() {
    use std::cmp::Ordering;

    // If a < b and b < c, then a < c (transitivity)
    let a = ScalarType::Int;
    let b = ScalarType::Float;
    let c = ScalarType::String;

    assert_eq!(a.cmp(&b), Ordering::Less);
    assert_eq!(b.cmp(&c), Ordering::Less);
    assert_eq!(a.cmp(&c), Ordering::Less);
}

#[test]
fn test_scalar_type_can_be_sorted() {
    // Practical test: should be able to sort a Vec of ScalarTypes
    let mut types = [
        ScalarType::String,
        ScalarType::Int,
        ScalarType::Float,
        ScalarType::Bool,
        ScalarType::Bytes,
    ];

    types.sort();

    // Should be sorted by discriminant order
    assert_eq!(types[0], ScalarType::Int);
    assert_eq!(types[1], ScalarType::Float);
    assert_eq!(types[2], ScalarType::String);
    assert_eq!(types[3], ScalarType::Bool);
    assert_eq!(types[4], ScalarType::Bytes);
}
