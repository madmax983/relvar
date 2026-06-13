use relvar_core::algebra::{DivideError, ExtendError, IntersectError};
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

#[test]
fn test_divide_empty_remainder_error() {
    let heading = TupleType::new()
        .with_attribute("A".to_string(), ScalarType::Int)
        .with_attribute("B".to_string(), ScalarType::Int);
    let r1 = Relation::new(RelationType::new(heading.clone()));
    let r2 = Relation::new(RelationType::new(heading));

    let result = r1.divide(&r2);
    assert!(matches!(result, Err(DivideError::EmptyRemainder)));
}

#[test]
fn test_extend_type_mismatch() {
    let heading = TupleType::new().with_attribute("A".to_string(), ScalarType::Int);
    let mut r1 = Relation::new(RelationType::new(heading));
    r1.insert(tuple! { A: 1i64 }).unwrap();

    let result = r1.extend("B", ScalarType::Int, |_| {
        ScalarValue::String("not an int".to_string())
    });
    assert!(matches!(result, Err(ExtendError::TupleCreation(_))));
}

#[test]
fn test_extend_into_type_mismatch() {
    let heading = TupleType::new().with_attribute("A".to_string(), ScalarType::Int);
    let mut r1 = Relation::new(RelationType::new(heading));
    r1.insert(tuple! { A: 1i64 }).unwrap();

    let result = r1.extend_into("B", ScalarType::Int, |_| {
        ScalarValue::String("not an int".to_string())
    });
    assert!(matches!(result, Err(ExtendError::TupleCreation(_))));
}

#[test]
fn test_intersect_into_type_mismatch() {
    let h1 = TupleType::new().with_attribute("A".to_string(), ScalarType::Int);
    let h2 = TupleType::new().with_attribute("B".to_string(), ScalarType::Int);
    let r1 = Relation::new(RelationType::new(h1));
    let r2 = Relation::new(RelationType::new(h2));

    let result = r1.intersect_into(&r2);
    assert!(matches!(result, Err(IntersectError)));
}

use relvar_core::algebra::DifferenceError;

#[test]
fn test_difference_into_type_mismatch() {
    let h1 = TupleType::new().with_attribute("A".to_string(), ScalarType::Int);
    let h2 = TupleType::new().with_attribute("B".to_string(), ScalarType::Int);
    let r1 = Relation::new(RelationType::new(h1));
    let r2 = Relation::new(RelationType::new(h2));

    let result = r1.difference_into(&r2);
    assert!(matches!(result, Err(DifferenceError)));
}

#[test]
fn test_minus_into_type_mismatch() {
    let h1 = TupleType::new().with_attribute("A".to_string(), ScalarType::Int);
    let h2 = TupleType::new().with_attribute("B".to_string(), ScalarType::Int);
    let r1 = Relation::new(RelationType::new(h1));
    let r2 = Relation::new(RelationType::new(h2));

    let result = r1.minus_into(&r2);
    assert!(matches!(result, Err(DifferenceError)));
}

#[test]
fn test_difference_type_mismatch() {
    let h1 = TupleType::new().with_attribute("A".to_string(), ScalarType::Int);
    let h2 = TupleType::new().with_attribute("B".to_string(), ScalarType::Int);
    let r1 = Relation::new(RelationType::new(h1));
    let r2 = Relation::new(RelationType::new(h2));

    let result = r1.difference(&r2);
    assert!(matches!(result, Err(DifferenceError)));
}

#[test]
fn test_minus_type_mismatch() {
    let h1 = TupleType::new().with_attribute("A".to_string(), ScalarType::Int);
    let h2 = TupleType::new().with_attribute("B".to_string(), ScalarType::Int);
    let r1 = Relation::new(RelationType::new(h1));
    let r2 = Relation::new(RelationType::new(h2));

    let result = r1.minus(&r2);
    assert!(matches!(result, Err(DifferenceError)));
}

#[test]
fn test_divide_empty_dividend() {
    let dividend_heading = TupleType::new()
        .with_attribute("supplier_id", ScalarType::String)
        .with_attribute("part_id", ScalarType::String);
    let supplies = Relation::new(RelationType::new(dividend_heading)); // Empty

    let parts_heading = TupleType::new().with_attribute("part_id", ScalarType::String);
    let mut parts = Relation::new(RelationType::new(parts_heading));
    parts.insert(tuple! { part_id: "P1" }).unwrap();

    let result = supplies.divide(&parts).unwrap();
    assert_eq!(result.cardinality(), 0);
    assert!(result.is_empty());
}

#[test]
fn test_divide_error_missing_attribute() {
    let supplies_heading = TupleType::new()
        .with_attribute("supplier_id", ScalarType::String)
        .with_attribute("part_id", ScalarType::String);
    let supplies = Relation::new(RelationType::new(supplies_heading));

    let invalid_heading = TupleType::new()
        .with_attribute("part_id", ScalarType::String)
        .with_attribute("color", ScalarType::String);
    let invalid_divisor = Relation::new(RelationType::new(invalid_heading));

    let result = supplies.divide(&invalid_divisor);

    assert!(matches!(result, Err(DivideError::MissingAttribute(_))));
}

#[test]
fn test_divide_error_type_mismatch() {
    let supplies_heading = TupleType::new()
        .with_attribute("supplier_id", ScalarType::String)
        .with_attribute("part_id", ScalarType::String);
    let supplies = Relation::new(RelationType::new(supplies_heading));

    let mismatched_heading = TupleType::new().with_attribute("part_id", ScalarType::Int);
    let mismatched_divisor = Relation::new(RelationType::new(mismatched_heading));

    let result = supplies.divide(&mismatched_divisor);

    assert!(matches!(result, Err(DivideError::TypeMismatch(_))));
}
