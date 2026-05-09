use relvar_core::types::ScalarType;

#[test]
fn test_scalar_type_ordering() {
    let int = ScalarType::Int;
    let float = ScalarType::Float;

    assert!(int < float);
    assert!(float > int);
}

#[test]
fn test_scalar_type_cmp_relation_types_equal_degree_different_types() {
    use relvar_core::types::{RelationType, TupleType};

    // relation A with int
    let rel_a = ScalarType::Relation(Box::new(RelationType::new(
        TupleType::new().with_attribute("a", ScalarType::Int),
    )));

    // relation B with float
    let rel_b = ScalarType::Relation(Box::new(RelationType::new(
        TupleType::new().with_attribute("a", ScalarType::Float),
    )));

    // Int is < Float
    assert!(rel_a < rel_b);
    assert!(rel_b > rel_a);
}

#[test]
fn test_scalar_type_cmp_relation_types_different_names() {
    use relvar_core::types::{RelationType, TupleType};

    // relation A with a
    let rel_a = ScalarType::Relation(Box::new(RelationType::new(
        TupleType::new().with_attribute("a", ScalarType::Int),
    )));

    // relation B with b
    let rel_b = ScalarType::Relation(Box::new(RelationType::new(
        TupleType::new().with_attribute("b", ScalarType::Int),
    )));

    // "a" is < "b"
    assert!(rel_a < rel_b);
    assert!(rel_b > rel_a);
}

#[test]
fn test_scalar_type_user_defined_ordering() {
    let u1 = ScalarType::user_defined("A", ScalarType::Int);
    let u2 = ScalarType::user_defined("B", ScalarType::Int);
    let u3 = ScalarType::user_defined("A", ScalarType::Float);

    assert!(u1 < u2);
    assert!(u1 < u3);
    assert!(u2 > u3); // Name "B" > "A"
}
