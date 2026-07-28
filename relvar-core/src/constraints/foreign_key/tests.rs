use super::*;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};

fn create_department_relation() -> Relation {
    let heading = TupleType::new()
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("dept_name".to_string(), ScalarType::String);

    let mut relation = Relation::new(RelationType::new(heading));
    relation
        .insert(tuple! { dept_id: 10i64, dept_name: "Engineering" })
        .unwrap();
    relation
        .insert(tuple! { dept_id: 20i64, dept_name: "Sales" })
        .unwrap();
    relation
}

fn create_employee_relation() -> Relation {
    let heading = TupleType::new()
        .with_attribute("emp_id".to_string(), ScalarType::Int)
        .with_attribute("name".to_string(), ScalarType::String)
        .with_attribute("dept_id".to_string(), ScalarType::Int);

    Relation::new(RelationType::new(heading))
}

#[test]
fn test_foreign_key_creation() {
    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["dept_id".to_string()],
    )
    .unwrap();

    assert_eq!(fk.foreign_key_attributes(), &["dept_id"]);
    assert_eq!(fk.referenced_relation_name(), "DEPT");
    assert_eq!(fk.referenced_attributes(), &["dept_id"]);
}

#[test]
fn test_would_violate_on_insert_missing_attribute_error() {
    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();

    let dept_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    let depts = Relation::new(dept_type);

    let t = crate::tuple! { emp_id: 1i64 };

    let res = fk.would_violate_on_insert(&t, &depts);
    assert!(res.is_err());
    assert!(
        matches!(res.unwrap_err(), ForeignKeyError::MissingAttribute(attr) if attr == "dept_id")
    );
}

#[test]
fn test_would_violate_on_delete_missing_attribute_error() {
    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["id".to_string()],
    )
    .unwrap();

    let emp_type = RelationType::new(TupleType::new().with_attribute("dept_id", ScalarType::Int));
    let emps = Relation::new(emp_type);

    let t = crate::tuple! { dept_id: 1i64 }; // intentionally missing "id"

    let res = fk.would_violate_on_delete(&t, &emps);
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), ForeignKeyError::MissingAttribute(attr) if attr == "id"));
}

#[test]
fn test_empty_foreign_key_rejected() {
    let result = ForeignKey::new(vec![], "DEPT".to_string(), vec![]);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ForeignKeyError::EmptyForeignKey
    ));
}

#[test]
fn test_attribute_count_mismatch() {
    let result = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["dept_id".to_string(), "dept_name".to_string()],
    );

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ForeignKeyError::AttributeCountMismatch
    ));
}

#[test]
fn test_foreign_key_satisfied() {
    let departments = create_department_relation();
    let mut employees = create_employee_relation();

    employees
        .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
        .unwrap();
    employees
        .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
        .unwrap();

    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["dept_id".to_string()],
    )
    .unwrap();

    assert!(fk.is_satisfied_by(&employees, &departments).unwrap());
}

#[test]
fn test_foreign_key_violated() {
    let departments = create_department_relation();
    let mut employees = create_employee_relation();

    employees
        .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
        .unwrap();
    employees
        .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 99i64 })
        .unwrap(); // dept_id 99 doesn't exist

    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["dept_id".to_string()],
    )
    .unwrap();

    assert!(!fk.is_satisfied_by(&employees, &departments).unwrap());
}

#[test]
fn test_would_violate_on_insert() {
    let departments = create_department_relation();

    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["dept_id".to_string()],
    )
    .unwrap();

    // Valid insert
    let valid_tuple = tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 };
    assert!(
        !fk.would_violate_on_insert(&valid_tuple, &departments)
            .unwrap()
    );

    // Invalid insert
    let invalid_tuple = tuple! { emp_id: 2i64, name: "Bob", dept_id: 99i64 };
    assert!(
        fk.would_violate_on_insert(&invalid_tuple, &departments)
            .unwrap()
    );
}

#[test]
fn test_would_violate_on_delete() {
    let departments = create_department_relation();
    let mut employees = create_employee_relation();

    employees
        .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
        .unwrap();

    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["dept_id".to_string()],
    )
    .unwrap();

    // Get department tuple
    let dept_tuple = departments
        .tuples()
        .find(|t| t.get_typed::<i64>("dept_id").unwrap() == 10)
        .unwrap();

    // Deleting this department would violate FK
    assert!(fk.would_violate_on_delete(dept_tuple, &employees).unwrap());

    // Get unused department
    let unused_dept = departments
        .tuples()
        .find(|t| t.get_typed::<i64>("dept_id").unwrap() == 20)
        .unwrap();

    // Deleting this department wouldn't violate FK
    assert!(!fk.would_violate_on_delete(unused_dept, &employees).unwrap());
}

#[test]
fn test_composite_foreign_key() {
    let heading = TupleType::new()
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("location".to_string(), ScalarType::String)
        .with_attribute("dept_name".to_string(), ScalarType::String);

    let mut departments = Relation::new(RelationType::new(heading));
    departments
        .insert(tuple! { dept_id: 10i64, location: "NYC", dept_name: "Engineering" })
        .unwrap();

    let emp_heading = TupleType::new()
        .with_attribute("emp_id".to_string(), ScalarType::Int)
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("dept_location".to_string(), ScalarType::String);

    let mut employees = Relation::new(RelationType::new(emp_heading));
    employees
        .insert(tuple! { emp_id: 1i64, dept_id: 10i64, dept_location: "NYC" })
        .unwrap();

    let fk = ForeignKey::new(
        vec!["dept_id".to_string(), "dept_location".to_string()],
        "DEPT".to_string(),
        vec!["dept_id".to_string(), "location".to_string()],
    )
    .unwrap();

    assert!(fk.is_satisfied_by(&employees, &departments).unwrap());
}

#[test]
fn test_foreign_key_constraints_collection() {
    let fk1 = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["dept_id".to_string()],
    )
    .unwrap();

    let fk2 = ForeignKey::new(
        vec!["manager_id".to_string()],
        "EMP".to_string(),
        vec!["emp_id".to_string()],
    )
    .unwrap();

    let constraints = ForeignKeyConstraints::new()
        .with_foreign_key(fk1)
        .with_foreign_key(fk2);

    assert_eq!(constraints.foreign_keys().len(), 2);
}

#[test]
fn test_invalid_foreign_key_attributes() {
    let departments = create_department_relation();
    let employees = create_employee_relation();

    let fk = ForeignKey::new(
        vec!["nonexistent".to_string()],
        "DEPT".to_string(),
        vec!["dept_id".to_string()],
    )
    .unwrap();

    let result = fk.is_satisfied_by(&employees, &departments);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ForeignKeyError::InvalidForeignKeyAttributes(_)
    ));
}

#[test]
fn test_invalid_referenced_attributes() {
    let departments = create_department_relation();
    let employees = create_employee_relation();

    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["nonexistent".to_string()],
    )
    .unwrap();

    let result = fk.is_satisfied_by(&employees, &departments);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ForeignKeyError::InvalidReferencedAttributes(_)
    ));
}

#[test]
fn test_foreign_key_type_mismatch_int_string() {
    // Referenced relation has String ID
    let dept_heading = TupleType::new()
        .with_attribute("dept_id", ScalarType::String)
        .with_attribute("dept_name", ScalarType::String);
    let mut departments = Relation::new(RelationType::new(dept_heading));
    departments
        .insert(tuple! { dept_id: "10", dept_name: "Engineering" })
        .unwrap();

    // Referencing relation has Int ID
    let emp_heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("dept_id", ScalarType::Int);
    let mut employees = Relation::new(RelationType::new(emp_heading));
    employees
        .insert(tuple! { emp_id: 1i64, dept_id: 10i64 })
        .unwrap();

    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["dept_id".to_string()],
    )
    .unwrap();

    // Should fail because Int(10) != String("10")
    // Currently returns Ok(false) (Violation), but ideally should be Err(TypeMismatch)
    // For now, we assert the current behavior (Violation) to prevent regression/undefined behavior
    assert!(!fk.is_satisfied_by(&employees, &departments).unwrap());
}

#[test]
fn test_foreign_key_type_mismatch_int_float() {
    // Referenced relation has Float ID
    let dept_heading = TupleType::new()
        .with_attribute("dept_id", ScalarType::Float)
        .with_attribute("dept_name", ScalarType::String);
    let mut departments = Relation::new(RelationType::new(dept_heading));
    departments
        .insert(tuple! { dept_id: 10.0, dept_name: "Engineering" })
        .unwrap();

    // Referencing relation has Int ID
    let emp_heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("dept_id", ScalarType::Int);
    let mut employees = Relation::new(RelationType::new(emp_heading));
    employees
        .insert(tuple! { emp_id: 1i64, dept_id: 10i64 })
        .unwrap();

    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["dept_id".to_string()],
    )
    .unwrap();

    // Should fail because Int(10) != Float(10.0)
    assert!(!fk.is_satisfied_by(&employees, &departments).unwrap());
}

#[test]
fn test_foreign_key_float_strictness() {
    // Referenced relation has Float ID with 0.0
    let dept_heading = TupleType::new().with_attribute("dept_id", ScalarType::Float);
    let mut departments = Relation::new(RelationType::new(dept_heading));
    departments.insert(tuple! { dept_id: 0.0 }).unwrap();

    // Referencing relation has Float ID with -0.0
    let emp_heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("dept_id", ScalarType::Float);
    let mut employees = Relation::new(RelationType::new(emp_heading));
    employees
        .insert(tuple! { emp_id: 1i64, dept_id: -0.0 })
        .unwrap();

    let fk = ForeignKey::new(
        vec!["dept_id".to_string()],
        "DEPT".to_string(),
        vec!["dept_id".to_string()],
    )
    .unwrap();

    // Should fail because -0.0 != 0.0 in ScalarValue equality logic
    // This confirms strict bit-pattern matching for FKs on floats
    assert!(!fk.is_satisfied_by(&employees, &departments).unwrap());
}
