use super::*;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};

fn create_employee_relation() -> Relation {
    let heading = TupleType::new()
        .with_attribute("emp_id".to_string(), ScalarType::Int)
        .with_attribute("name".to_string(), ScalarType::String)
        .with_attribute("dept_id".to_string(), ScalarType::Int);

    Relation::new(RelationType::new(heading))
}

#[test]
fn test_candidate_key_creation() {
    let key = CandidateKey::new(vec!["emp_id".to_string()]).unwrap();
    assert_eq!(key.attributes(), &["emp_id"]);
}

#[test]
fn test_empty_key_rejected() {
    let result = CandidateKey::new(vec![]);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), KeyConstraintError::EmptyKey));
}

#[test]
fn test_key_uniqueness_satisfied() {
    let mut relation = create_employee_relation();
    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
        .unwrap();
    relation
        .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
        .unwrap();

    let key = CandidateKey::new(vec!["emp_id".to_string()]).unwrap();
    assert!(key.is_satisfied_by(&relation).unwrap());
}

#[test]
fn test_key_uniqueness_violated() {
    let mut relation = create_employee_relation();
    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
        .unwrap();
    relation
        .insert(tuple! { emp_id: 1i64, name: "Bob", dept_id: 20i64 })
        .unwrap();

    let key = CandidateKey::new(vec!["emp_id".to_string()]).unwrap();
    assert!(!key.is_satisfied_by(&relation).unwrap());
}

#[test]
fn test_composite_key() {
    let mut relation = create_employee_relation();
    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
        .unwrap();
    relation
        .insert(tuple! { emp_id: 1i64, name: "Bob", dept_id: 20i64 })
        .unwrap();

    let key = CandidateKey::new(vec!["emp_id".to_string(), "dept_id".to_string()]).unwrap();
    assert!(key.is_satisfied_by(&relation).unwrap());
}

#[test]
fn test_would_violate_on_insert() {
    let mut relation = create_employee_relation();
    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
        .unwrap();

    let key = CandidateKey::new(vec!["emp_id".to_string()]).unwrap();

    let new_tuple = tuple! { emp_id: 1i64, name: "Bob", dept_id: 20i64 };
    assert!(key.would_violate(&relation, &new_tuple).unwrap());

    let new_tuple2 = tuple! { emp_id: 2i64, name: "Charlie", dept_id: 30i64 };
    assert!(!key.would_violate(&relation, &new_tuple2).unwrap());
}

#[test]
fn test_primary_key() {
    let pk = PrimaryKey::new(vec!["emp_id".to_string()]).unwrap();
    assert_eq!(pk.attributes(), &["emp_id"]);

    let mut relation = create_employee_relation();
    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
        .unwrap();
    relation
        .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
        .unwrap();

    assert!(pk.is_satisfied_by(&relation).unwrap());
}

#[test]
fn test_key_constraints_with_primary_key() {
    let pk = PrimaryKey::new(vec!["emp_id".to_string()]).unwrap();
    let constraints = KeyConstraints::new().with_primary_key(pk);

    let mut relation = create_employee_relation();
    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
        .unwrap();
    relation
        .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
        .unwrap();

    assert!(constraints.are_satisfied_by(&relation).unwrap());
}

#[test]
fn test_key_constraints_violation_detection() {
    let pk = PrimaryKey::new(vec!["emp_id".to_string()]).unwrap();
    let constraints = KeyConstraints::new().with_primary_key(pk);

    let mut relation = create_employee_relation();
    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
        .unwrap();

    let new_tuple = tuple! { emp_id: 1i64, name: "Bob", dept_id: 20i64 };
    let violation = constraints
        .would_violate_on_insert(&relation, &new_tuple)
        .unwrap();

    assert!(violation.is_some());
    assert_eq!(violation.unwrap(), vec!["emp_id"]);
}

#[test]
fn test_multiple_candidate_keys() {
    let heading = TupleType::new()
        .with_attribute("emp_id".to_string(), ScalarType::Int)
        .with_attribute("email".to_string(), ScalarType::String)
        .with_attribute("name".to_string(), ScalarType::String);

    let mut relation = Relation::new(RelationType::new(heading));
    relation
        .insert(tuple! { emp_id: 1i64, email: "alice@example.com", name: "Alice" })
        .unwrap();
    relation
        .insert(tuple! { emp_id: 2i64, email: "bob@example.com", name: "Bob" })
        .unwrap();

    let pk = PrimaryKey::new(vec!["emp_id".to_string()]).unwrap();
    let ck = CandidateKey::new(vec!["email".to_string()]).unwrap();

    let constraints = KeyConstraints::new()
        .with_primary_key(pk)
        .with_candidate_key(ck);

    assert!(constraints.are_satisfied_by(&relation).unwrap());

    // Try to insert with duplicate email
    let new_tuple = tuple! { emp_id: 3i64, email: "alice@example.com", name: "Alice2" };
    let violation = constraints
        .would_violate_on_insert(&relation, &new_tuple)
        .unwrap();
    assert!(violation.is_some());
}

#[test]
fn test_invalid_key_attributes() {
    let key = CandidateKey::new(vec!["nonexistent".to_string()]).unwrap();
    let relation = create_employee_relation();

    let result = key.is_satisfied_by(&relation);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        KeyConstraintError::InvalidKeyAttributes(_)
    ));
}

#[test]
fn test_extract_key_value_missing_attribute() {
    let heading = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
    let relation = Relation::new(RelationType::new(heading));

    let key = CandidateKey::new(vec!["id".to_string()]).unwrap();

    // Tuple missing "id"
    let bad_tuple = tuple! { name: "Alice" };

    // This should returns Err instead of panicking
    let result = key.would_violate(&relation, &bad_tuple);

    assert!(result.is_err());
    assert!(
        matches!(result.unwrap_err(), KeyConstraintError::TupleMissingAttribute(attr) if attr == "id")
    );
}
