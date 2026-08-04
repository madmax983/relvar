use super::*;
use crate::tuple;

#[test]
fn test_group_creates_rva() {
    let heading = TupleType::new()
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("emp_id".to_string(), ScalarType::Int)
        .with_attribute("name".to_string(), ScalarType::String);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { dept_id: 10i64, emp_id: 1i64, name: "Alice" })
        .unwrap();
    relation
        .insert(tuple! { dept_id: 10i64, emp_id: 2i64, name: "Bob" })
        .unwrap();
    relation
        .insert(tuple! { dept_id: 20i64, emp_id: 3i64, name: "Charlie" })
        .unwrap();

    let result = relation.group(&["emp_id", "name"], "employees").unwrap();

    // Result should have 2 tuples (one per dept_id)
    assert_eq!(result.cardinality(), 2);
    // Result should have 2 attributes: dept_id and employees (RVA)
    assert_eq!(result.degree(), 2);

    // Check that employees is an RVA
    for tuple in result.tuples() {
        let employees = tuple.get("employees").unwrap();
        match employees {
            ScalarValue::Relation(emp_rel) => {
                // Each department should have the correct number of employees
                let dept_id = tuple.get_typed::<i64>("dept_id").unwrap();
                if dept_id == 10 {
                    assert_eq!(emp_rel.cardinality(), 2);
                } else if dept_id == 20 {
                    assert_eq!(emp_rel.cardinality(), 1);
                }
                // RVA should have 2 attributes: emp_id and name
                assert_eq!(emp_rel.degree(), 2);
            }
            _ => panic!("Expected relation-valued attribute"),
        }
    }
}

#[test]
fn test_ungroup_flattens_rva() {
    let heading = TupleType::new()
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("emp_id".to_string(), ScalarType::Int)
        .with_attribute("name".to_string(), ScalarType::String);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { dept_id: 10i64, emp_id: 1i64, name: "Alice" })
        .unwrap();
    relation
        .insert(tuple! { dept_id: 10i64, emp_id: 2i64, name: "Bob" })
        .unwrap();
    relation
        .insert(tuple! { dept_id: 20i64, emp_id: 3i64, name: "Charlie" })
        .unwrap();

    // Group first
    let grouped = relation.group(&["emp_id", "name"], "employees").unwrap();

    // Then ungroup
    let ungrouped = grouped.ungroup("employees").unwrap();

    // Should have same cardinality as original
    assert_eq!(ungrouped.cardinality(), 3);
    // Should have same degree as original
    assert_eq!(ungrouped.degree(), 3);
}

#[test]
fn test_group_then_ungroup_is_identity() {
    let heading = TupleType::new()
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("emp_id".to_string(), ScalarType::Int)
        .with_attribute("name".to_string(), ScalarType::String);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { dept_id: 10i64, emp_id: 1i64, name: "Alice" })
        .unwrap();
    relation
        .insert(tuple! { dept_id: 10i64, emp_id: 2i64, name: "Bob" })
        .unwrap();
    relation
        .insert(tuple! { dept_id: 20i64, emp_id: 3i64, name: "Charlie" })
        .unwrap();

    // Group then ungroup
    let grouped = relation.group(&["emp_id", "name"], "employees").unwrap();
    let result = grouped.ungroup("employees").unwrap();

    // Should be equivalent to original relation (same tuples, possibly different order)
    assert_eq!(result.cardinality(), relation.cardinality());
    assert_eq!(result.degree(), relation.degree());

    // All original tuples should be present
    for tuple in relation.tuples() {
        assert!(result.contains(tuple));
    }

    // All result tuples should be in original
    for tuple in result.tuples() {
        assert!(relation.contains(tuple));
    }
}

#[test]
fn test_group_empty_relation() {
    let heading = TupleType::new()
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("emp_id".to_string(), ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let relation = Relation::new(rel_type);

    let result = relation.group(&["emp_id"], "employees").unwrap();

    assert_eq!(result.cardinality(), 0);
    assert_eq!(result.degree(), 2); // dept_id and employees RVA
}

#[test]
fn test_group_single_attribute() {
    let heading = TupleType::new()
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("emp_id".to_string(), ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { dept_id: 10i64, emp_id: 1i64 })
        .unwrap();
    relation
        .insert(tuple! { dept_id: 10i64, emp_id: 2i64 })
        .unwrap();

    let result = relation.group(&["emp_id"], "employees").unwrap();

    assert_eq!(result.cardinality(), 1);
    assert_eq!(result.degree(), 2);
}

#[test]
fn test_group_attribute_not_found() {
    let heading = TupleType::new()
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("emp_id".to_string(), ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let relation = Relation::new(rel_type);

    let result = relation.group(&["nonexistent"], "employees");

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        GroupError::AttributeNotFound(_)
    ));
}

#[test]
fn test_group_all_attributes_fails() {
    let heading = TupleType::new()
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("emp_id".to_string(), ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let relation = Relation::new(rel_type);

    let result = relation.group(&["dept_id", "emp_id"], "employees");

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        GroupError::AllAttributesGrouped
    ));
}

#[test]
fn test_ungroup_attribute_not_found() {
    let heading = TupleType::new()
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("emp_id".to_string(), ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let relation = Relation::new(rel_type);

    let result = relation.ungroup("nonexistent");

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        UngroupError::AttributeNotFound(_)
    ));
}

#[test]
fn test_ungroup_not_relation_valued() {
    let heading = TupleType::new()
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("emp_id".to_string(), ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { dept_id: 10i64, emp_id: 1i64 })
        .unwrap();

    let result = relation.ungroup("dept_id");

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        UngroupError::NotRelationValued(_)
    ));
}

#[test]
fn test_multiple_grouping_keys() {
    let heading = TupleType::new()
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("location".to_string(), ScalarType::String)
        .with_attribute("emp_id".to_string(), ScalarType::Int)
        .with_attribute("name".to_string(), ScalarType::String);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { dept_id: 10i64, location: "NYC", emp_id: 1i64, name: "Alice" })
        .unwrap();
    relation
        .insert(tuple! { dept_id: 10i64, location: "NYC", emp_id: 2i64, name: "Bob" })
        .unwrap();
    relation
        .insert(tuple! { dept_id: 10i64, location: "LA", emp_id: 3i64, name: "Charlie" })
        .unwrap();
    relation
        .insert(tuple! { dept_id: 20i64, location: "NYC", emp_id: 4i64, name: "David" })
        .unwrap();

    let result = relation.group(&["emp_id", "name"], "employees").unwrap();

    // Should have 3 groups: (10, NYC), (10, LA), (20, NYC)
    assert_eq!(result.cardinality(), 3);
    assert_eq!(result.degree(), 3); // dept_id, location, employees
}
