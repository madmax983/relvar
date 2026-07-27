use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;

#[test]
fn test_difference_into_type_mismatch() {
    let type1 = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);
    let rel_type1 = RelationType::new(type1);
    let rel1 = Relation::new(rel_type1);

    let type2 = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("salary", ScalarType::Float);

    let rel_type2 = RelationType::new(type2);
    let rel2 = Relation::new(rel_type2);

    let result = rel1.difference_into(&rel2);
    assert!(result.is_err());
}

#[test]
fn test_intersect_into_type_mismatch() {
    let type1 = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);
    let rel_type1 = RelationType::new(type1);
    let rel1 = Relation::new(rel_type1);

    let type2 = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("salary", ScalarType::Float);

    let rel_type2 = RelationType::new(type2);
    let rel2 = Relation::new(rel_type2);

    let result = rel1.intersect_into(&rel2);
    assert!(result.is_err());
}

#[test]
fn test_union_into_type_mismatch() {
    let type1 = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);
    let rel_type1 = RelationType::new(type1);
    let rel1 = Relation::new(rel_type1);

    let type2 = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("salary", ScalarType::Float);

    let rel_type2 = RelationType::new(type2);
    let rel2 = Relation::new(rel_type2);

    let result = rel1.union_into(&rel2);
    assert!(result.is_err());
}
