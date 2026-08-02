use super::*;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};

#[test]
fn test_extend_adds_computed_attribute() {
    let heading = TupleType::new()
        .with_attribute("price".to_string(), ScalarType::Int)
        .with_attribute("quantity".to_string(), ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { price: 10i64, quantity: 5i64 })
        .unwrap();
    relation
        .insert(tuple! { price: 20i64, quantity: 3i64 })
        .unwrap();

    // Extend with total = price * quantity
    let result = relation
        .extend("total", ScalarType::Int, |t| {
            let price = t.get_typed::<i64>("price").unwrap();
            let quantity = t.get_typed::<i64>("quantity").unwrap();
            ScalarValue::Int(price * quantity)
        })
        .unwrap();

    assert_eq!(result.cardinality(), 2);
    assert_eq!(result.degree(), 3);
    assert!(result.relation_type().has_attribute("total"));

    // Check computed values
    for tuple in result.tuples() {
        let price = tuple.get_typed::<i64>("price").unwrap();
        let quantity = tuple.get_typed::<i64>("quantity").unwrap();
        let total = tuple.get_typed::<i64>("total").unwrap();
        assert_eq!(total, price * quantity);
    }
}

#[test]
fn test_extend_fails_if_attribute_exists() {
    let heading = TupleType::new()
        .with_attribute("emp_id".to_string(), ScalarType::Int)
        .with_attribute("name".to_string(), ScalarType::String);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice" })
        .unwrap();

    // Try to extend with an attribute that already exists
    let result = relation.extend("name", ScalarType::String, |t| {
        t.get("name").unwrap().clone()
    });

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ExtendError::AttributeExists(_)
    ));
}

#[test]
fn test_extend_on_empty_relation() {
    let heading = TupleType::new()
        .with_attribute("emp_id".to_string(), ScalarType::Int)
        .with_attribute("salary".to_string(), ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let relation = Relation::new(rel_type);

    let result = relation
        .extend("bonus", ScalarType::Int, |t| {
            let salary = t.get_typed::<i64>("salary").unwrap();
            ScalarValue::Int(salary / 10)
        })
        .unwrap();

    assert_eq!(result.cardinality(), 0);
    assert_eq!(result.degree(), 3);
    assert!(result.relation_type().has_attribute("bonus"));
}

#[test]
fn test_extend_with_string_concatenation() {
    let heading = TupleType::new()
        .with_attribute("first_name".to_string(), ScalarType::String)
        .with_attribute("last_name".to_string(), ScalarType::String);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { first_name: "Alice", last_name: "Smith" })
        .unwrap();
    relation
        .insert(tuple! { first_name: "Bob", last_name: "Jones" })
        .unwrap();

    let result = relation
        .extend("full_name", ScalarType::String, |t| {
            let first = t.get_typed::<String>("first_name").unwrap();
            let last = t.get_typed::<String>("last_name").unwrap();
            ScalarValue::String(format!("{} {}", first, last))
        })
        .unwrap();

    assert_eq!(result.cardinality(), 2);
    assert_eq!(result.degree(), 3);

    for tuple in result.tuples() {
        let first = tuple.get_typed::<String>("first_name").unwrap();
        let last = tuple.get_typed::<String>("last_name").unwrap();
        let full = tuple.get_typed::<String>("full_name").unwrap();
        assert_eq!(full, format!("{} {}", first, last));
    }
}

#[test]
fn test_extend_type_mismatch_fails() {
    let heading = TupleType::new().with_attribute("emp_id".to_string(), ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation.insert(tuple! { emp_id: 1i64 }).unwrap();

    // Try to extend with a type mismatch
    // Expected: String, Computed: Int
    let result = relation.extend("name_length", ScalarType::String, |_| {
        // Return an Int instead of String
        ScalarValue::Int(10)
    });

    assert!(result.is_err());
    match result.unwrap_err() {
        ExtendError::TupleCreation(msg) => {
            assert!(msg.contains("Type mismatch"));
            assert!(msg.contains("expected String"));
            assert!(msg.contains("got Int"));
        }
        _ => panic!("Expected TupleCreation error"),
    }
}

#[test]
fn test_extend_into_adds_computed_attribute() {
    let heading = TupleType::new()
        .with_attribute("price".to_string(), ScalarType::Int)
        .with_attribute("quantity".to_string(), ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { price: 10i64, quantity: 5i64 })
        .unwrap();
    relation
        .insert(tuple! { price: 20i64, quantity: 3i64 })
        .unwrap();

    let result = relation
        .extend_into("total", ScalarType::Int, |t| {
            let price = t.get_typed::<i64>("price").unwrap();
            let quantity = t.get_typed::<i64>("quantity").unwrap();
            crate::values::ScalarValue::Int(price * quantity)
        })
        .unwrap();

    assert_eq!(result.degree(), 3);
    assert_eq!(result.cardinality(), 2);

    let mut found_50 = false;
    let mut found_60 = false;

    for tuple in result.tuples() {
        if let Some(crate::values::ScalarValue::Int(total)) = tuple.get("total") {
            if *total == 50 {
                found_50 = true;
            }
            if *total == 60 {
                found_60 = true;
            }
        }
    }

    assert!(found_50 && found_60);
}
