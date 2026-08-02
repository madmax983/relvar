use super::*;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

// Test 1: Basic division - S1 supplies P1,P2; S2 supplies only P1; divide by {P1,P2} returns only S1
#[test]
fn test_divide_basic() {
    // SUPPLIES relation: {supplier_id, part_id}
    let supplies_heading = TupleType::new()
        .with_attribute("supplier_id", ScalarType::String)
        .with_attribute("part_id", ScalarType::String);
    let supplies_type = RelationType::new(supplies_heading);
    let mut supplies = Relation::new(supplies_type);

    // S1 supplies P1 and P2
    supplies
        .insert(tuple! { supplier_id: "S1", part_id: "P1" })
        .unwrap();
    supplies
        .insert(tuple! { supplier_id: "S1", part_id: "P2" })
        .unwrap();

    // S2 supplies only P1
    supplies
        .insert(tuple! { supplier_id: "S2", part_id: "P1" })
        .unwrap();

    // S3 supplies P1 and P2
    supplies
        .insert(tuple! { supplier_id: "S3", part_id: "P1" })
        .unwrap();
    supplies
        .insert(tuple! { supplier_id: "S3", part_id: "P2" })
        .unwrap();

    // PARTS relation: {part_id} with P1 and P2
    let parts_heading = TupleType::new().with_attribute("part_id", ScalarType::String);
    let parts_type = RelationType::new(parts_heading);
    let mut parts = Relation::new(parts_type);
    parts.insert(tuple! { part_id: "P1" }).unwrap();
    parts.insert(tuple! { part_id: "P2" }).unwrap();

    // Divide: which suppliers supply ALL parts?
    let result = supplies.divide(&parts).unwrap();

    // Result should have heading {supplier_id}
    assert_eq!(result.degree(), 1);
    assert!(
        result
            .relation_type()
            .heading()
            .has_attribute("supplier_id")
    );

    // Result should contain S1 and S3 (both supply all parts)
    assert_eq!(result.cardinality(), 2);
    assert!(result.contains(&tuple! { supplier_id: "S1" }));
    assert!(result.contains(&tuple! { supplier_id: "S3" }));
    assert!(!result.contains(&tuple! { supplier_id: "S2" })); // S2 doesn't supply P2
}

// Test 2: Empty divisor - should return projection onto remainder attributes
#[test]
fn test_divide_empty_divisor() {
    let supplies_heading = TupleType::new()
        .with_attribute("supplier_id", ScalarType::String)
        .with_attribute("part_id", ScalarType::String);
    let supplies_type = RelationType::new(supplies_heading);
    let mut supplies = Relation::new(supplies_type);

    supplies
        .insert(tuple! { supplier_id: "S1", part_id: "P1" })
        .unwrap();
    supplies
        .insert(tuple! { supplier_id: "S1", part_id: "P2" })
        .unwrap();
    supplies
        .insert(tuple! { supplier_id: "S2", part_id: "P1" })
        .unwrap();

    // Empty divisor
    let parts_heading = TupleType::new().with_attribute("part_id", ScalarType::String);
    let parts_type = RelationType::new(parts_heading);
    let parts = Relation::new(parts_type);

    let result = supplies.divide(&parts).unwrap();

    // Should be equivalent to projection onto {supplier_id}
    assert_eq!(result.degree(), 1);
    assert_eq!(result.cardinality(), 2); // S1 and S2
    assert!(result.contains(&tuple! { supplier_id: "S1" }));
    assert!(result.contains(&tuple! { supplier_id: "S2" }));
}

// Test 3: Empty dividend - should return empty relation
#[test]
fn test_divide_empty_dividend() {
    let supplies_heading = TupleType::new()
        .with_attribute("supplier_id", ScalarType::String)
        .with_attribute("part_id", ScalarType::String);
    let supplies_type = RelationType::new(supplies_heading);
    let supplies = Relation::new(supplies_type); // Empty

    let parts_heading = TupleType::new().with_attribute("part_id", ScalarType::String);
    let parts_type = RelationType::new(parts_heading);
    let mut parts = Relation::new(parts_type);
    parts.insert(tuple! { part_id: "P1" }).unwrap();

    let result = supplies.divide(&parts).unwrap();

    assert_eq!(result.degree(), 1);
    assert_eq!(result.cardinality(), 0);
    assert!(result.is_empty());
}

// Test 4: No supplier supplies all parts - should return empty
#[test]
fn test_divide_no_matches() {
    let supplies_heading = TupleType::new()
        .with_attribute("supplier_id", ScalarType::String)
        .with_attribute("part_id", ScalarType::String);
    let supplies_type = RelationType::new(supplies_heading);
    let mut supplies = Relation::new(supplies_type);

    // S1 supplies only P1
    supplies
        .insert(tuple! { supplier_id: "S1", part_id: "P1" })
        .unwrap();

    // S2 supplies only P2
    supplies
        .insert(tuple! { supplier_id: "S2", part_id: "P2" })
        .unwrap();

    // Divisor has both P1 and P2
    let parts_heading = TupleType::new().with_attribute("part_id", ScalarType::String);
    let parts_type = RelationType::new(parts_heading);
    let mut parts = Relation::new(parts_type);
    parts.insert(tuple! { part_id: "P1" }).unwrap();
    parts.insert(tuple! { part_id: "P2" }).unwrap();

    let result = supplies.divide(&parts).unwrap();

    // No supplier supplies both parts
    assert_eq!(result.cardinality(), 0);
    assert!(result.is_empty());
}

// Test 5: All suppliers supply all parts
#[test]
fn test_divide_all_match() {
    let supplies_heading = TupleType::new()
        .with_attribute("supplier_id", ScalarType::String)
        .with_attribute("part_id", ScalarType::String);
    let supplies_type = RelationType::new(supplies_heading);
    let mut supplies = Relation::new(supplies_type);

    // Both S1 and S2 supply both P1 and P2
    supplies
        .insert(tuple! { supplier_id: "S1", part_id: "P1" })
        .unwrap();
    supplies
        .insert(tuple! { supplier_id: "S1", part_id: "P2" })
        .unwrap();
    supplies
        .insert(tuple! { supplier_id: "S2", part_id: "P1" })
        .unwrap();
    supplies
        .insert(tuple! { supplier_id: "S2", part_id: "P2" })
        .unwrap();

    let parts_heading = TupleType::new().with_attribute("part_id", ScalarType::String);
    let parts_type = RelationType::new(parts_heading);
    let mut parts = Relation::new(parts_type);
    parts.insert(tuple! { part_id: "P1" }).unwrap();
    parts.insert(tuple! { part_id: "P2" }).unwrap();

    let result = supplies.divide(&parts).unwrap();

    assert_eq!(result.cardinality(), 2);
    assert!(result.contains(&tuple! { supplier_id: "S1" }));
    assert!(result.contains(&tuple! { supplier_id: "S2" }));
}

// Test 6: Error - divisor has attribute not in dividend
#[test]
fn test_divide_error_missing_attribute() {
    let supplies_heading = TupleType::new()
        .with_attribute("supplier_id", ScalarType::String)
        .with_attribute("part_id", ScalarType::String);
    let supplies_type = RelationType::new(supplies_heading);
    let supplies = Relation::new(supplies_type);

    // Divisor has attribute not in dividend
    let invalid_heading = TupleType::new()
        .with_attribute("part_id", ScalarType::String)
        .with_attribute("color", ScalarType::String); // Not in supplies!
    let invalid_type = RelationType::new(invalid_heading);
    let invalid_divisor = Relation::new(invalid_type);

    let result = supplies.divide(&invalid_divisor);

    assert!(result.is_err());
    match result {
        Err(DivideError::MissingAttribute(attr)) => {
            assert_eq!(attr, "color");
        }
        _ => panic!("Expected MissingAttribute error"),
    }
}

// Test 7: Error - attribute type mismatch
#[test]
fn test_divide_error_type_mismatch() {
    let supplies_heading = TupleType::new()
        .with_attribute("supplier_id", ScalarType::String)
        .with_attribute("part_id", ScalarType::String);
    let supplies_type = RelationType::new(supplies_heading);
    let supplies = Relation::new(supplies_type);

    // Divisor has same attribute name but different type
    let mismatched_heading = TupleType::new().with_attribute("part_id", ScalarType::Int); // Should be String!
    let mismatched_type = RelationType::new(mismatched_heading);
    let mismatched_divisor = Relation::new(mismatched_type);

    let result = supplies.divide(&mismatched_divisor);

    assert!(result.is_err());
    match result {
        Err(DivideError::TypeMismatch(attr)) => {
            assert_eq!(attr, "part_id");
        }
        _ => panic!("Expected TypeMismatch error"),
    }
}

// Test 8: Error - divisor heading equals dividend heading (no remainder)
#[test]
fn test_divide_error_empty_remainder() {
    let supplies_heading = TupleType::new()
        .with_attribute("supplier_id", ScalarType::String)
        .with_attribute("part_id", ScalarType::String);
    let supplies_type = RelationType::new(supplies_heading.clone());
    let supplies = Relation::new(supplies_type);

    // Divisor has same heading as dividend
    let divisor_type = RelationType::new(supplies_heading);
    let divisor = Relation::new(divisor_type);

    let result = supplies.divide(&divisor);

    assert!(result.is_err());
    match result {
        Err(DivideError::EmptyRemainder) => {
            // Expected
        }
        _ => panic!("Expected EmptyRemainder error"),
    }
}
