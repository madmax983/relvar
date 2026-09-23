use relvar_core::types::{RelationType, ScalarType, TupleType};

#[test]
fn test_user_defined_type_cmp_coverage() {
    let type1 = ScalarType::user_defined("A", ScalarType::Int);
    let type2 = ScalarType::user_defined("B", ScalarType::Int);
    let type3 = ScalarType::user_defined("A", ScalarType::Float);

    // Different names
    assert!(type1 < type2);

    // Same name, different representation (Int vs Float)
    assert!(type1 < type3);
}

#[test]
fn test_relation_type_cmp_coverage() {
    let t_type1 = TupleType::new().with_attribute("a", ScalarType::Int);
    let t_type2 = TupleType::new()
        .with_attribute("a", ScalarType::Int)
        .with_attribute("b", ScalarType::Int);
    let t_type3 = TupleType::new().with_attribute("z", ScalarType::Int);

    let rel_type1 = ScalarType::Relation(Box::new(RelationType::new(t_type1)));
    let rel_type2 = ScalarType::Relation(Box::new(RelationType::new(t_type2)));
    let rel_type3 = ScalarType::Relation(Box::new(RelationType::new(t_type3)));

    // Degree comparison
    assert!(rel_type1 < rel_type2); // degree 1 vs 2

    // Attributes comparison (degree equal, names different)
    assert!(rel_type1 < rel_type3); // "a" < "z"
}
