use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, ScalarValue};

#[test]
fn test_natural_join_on_common_attributes() {
    // Employees: emp_id, name, dept_id
    let emp_heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String)
        .with_attribute("dept_id", ScalarType::Int);

    let emp_rel_type = RelationType::new(emp_heading);
    let mut employees = Relation::new(emp_rel_type);

    employees
        .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
        .unwrap();
    employees
        .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
        .unwrap();

    // Departments: dept_id, dept_name
    let dept_heading = TupleType::new()
        .with_attribute("dept_id", ScalarType::Int)
        .with_attribute("dept_name", ScalarType::String);

    let dept_rel_type = RelationType::new(dept_heading);
    let mut departments = Relation::new(dept_rel_type);

    departments
        .insert(tuple! { dept_id: 10i64, dept_name: "Engineering" })
        .unwrap();
    departments
        .insert(tuple! { dept_id: 20i64, dept_name: "Sales" })
        .unwrap();

    // Join on dept_id
    let result = employees.join(&departments).unwrap();

    assert_eq!(result.degree(), 4); // emp_id, name, dept_id, dept_name
    assert_eq!(result.cardinality(), 2);

    // Verify Alice is joined with Engineering
    let alice_result = tuple! {
        emp_id: 1i64,
        name: "Alice",
        dept_id: 10i64,
        dept_name: "Engineering"
    };
    assert!(result.contains(&alice_result));
}

#[test]
fn test_natural_join_no_common_attributes_cartesian_product() {
    let rel1_heading = TupleType::new().with_attribute("a", ScalarType::Int);

    let rel1_type = RelationType::new(rel1_heading);
    let mut rel1 = Relation::new(rel1_type);

    rel1.insert(tuple! { a: 1i64 }).unwrap();
    rel1.insert(tuple! { a: 2i64 }).unwrap();

    let rel2_heading = TupleType::new().with_attribute("b", ScalarType::String);

    let rel2_type = RelationType::new(rel2_heading);
    let mut rel2 = Relation::new(rel2_type);

    rel2.insert(tuple! { b: "x" }).unwrap();
    rel2.insert(tuple! { b: "y" }).unwrap();

    let result = rel1.join(&rel2).unwrap();

    // Cartesian product: 2 x 2 = 4
    assert_eq!(result.cardinality(), 4);
    assert_eq!(result.degree(), 2); // a and b
}

#[test]
fn test_join_with_empty_relation() {
    let heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);

    let rel_type = RelationType::new(heading.clone());
    let mut rel1 = Relation::new(rel_type.clone());

    rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

    let rel2 = Relation::new(rel_type);

    let result = rel1.join(&rel2).unwrap();

    assert_eq!(result.cardinality(), 0);
    assert!(result.is_empty());
}

#[test]
fn test_self_join() {
    let heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice" })
        .unwrap();
    relation
        .insert(tuple! { emp_id: 2i64, name: "Bob" })
        .unwrap();

    let result = relation.join(&relation).unwrap();

    // Self-join on all attributes = original relation
    assert_eq!(result.cardinality(), 2);
    assert_eq!(result, relation);
}

#[test]
fn test_theta_join() {
    let emp_heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("salary", ScalarType::Int);

    let emp_rel_type = RelationType::new(emp_heading);
    let mut employees = Relation::new(emp_rel_type);

    employees
        .insert(tuple! { emp_id: 1i64, salary: 50000i64 })
        .unwrap();
    employees
        .insert(tuple! { emp_id: 2i64, salary: 75000i64 })
        .unwrap();

    let dept_heading = TupleType::new()
        .with_attribute("dept_id", ScalarType::Int)
        .with_attribute("min_salary", ScalarType::Int);

    let dept_rel_type = RelationType::new(dept_heading);
    let mut departments = Relation::new(dept_rel_type);

    departments
        .insert(tuple! { dept_id: 10i64, min_salary: 60000i64 })
        .unwrap();
    departments
        .insert(tuple! { dept_id: 20i64, min_salary: 40000i64 })
        .unwrap();

    // Join where employee salary >= department min_salary
    let result = employees.theta_join(&departments, |emp, dept| {
        if let (Some(ScalarValue::Int(salary)), Some(ScalarValue::Int(min_sal))) =
            (emp.get("salary"), dept.get("min_salary"))
        {
            salary >= min_sal
        } else {
            false
        }
    });

    // emp_id 1 (50000) matches dept 20 (40000)
    // emp_id 2 (75000) matches both dept 10 (60000) and dept 20 (40000)
    assert_eq!(result.cardinality(), 3);
}

#[test]
fn test_join_no_matches() {
    let rel1_heading = TupleType::new().with_attribute("dept_id", ScalarType::Int);

    let rel1_type = RelationType::new(rel1_heading);
    let mut rel1 = Relation::new(rel1_type);

    rel1.insert(tuple! { dept_id: 10i64 }).unwrap();

    let rel2_heading = TupleType::new().with_attribute("dept_id", ScalarType::Int);

    let rel2_type = RelationType::new(rel2_heading);
    let mut rel2 = Relation::new(rel2_type);

    rel2.insert(tuple! { dept_id: 20i64 }).unwrap();

    let result = rel1.join(&rel2).unwrap();

    assert_eq!(result.cardinality(), 0);
    assert!(result.is_empty());
}

/// Verifies the documented behavior that theta_join drops conflicting attributes
/// from the second relation.
#[test]
fn test_theta_join_attribute_collision_resolution() {
    // Relation 1: id=1
    let heading1 = TupleType::new().with_attribute("id", ScalarType::Int);
    let mut rel1 = Relation::new(RelationType::new(heading1));
    rel1.insert(tuple! { id: 1i64 }).unwrap();

    // Relation 2: id=2 (SAME ATTRIBUTE NAME)
    let heading2 = TupleType::new().with_attribute("id", ScalarType::Int);
    let mut rel2 = Relation::new(RelationType::new(heading2));
    rel2.insert(tuple! { id: 2i64 }).unwrap();

    // Theta join with a predicate that is always true
    // If this were a proper Cartesian product (renaming aside), we'd expect
    // something like (id_left: 1, id_right: 2).
    //
    // Current behavior: The second 'id' is dropped.
    let result = rel1.theta_join(&rel2, |_, _| true);

    // Assert that the result only has degree 1 (from rel1), not 2
    assert_eq!(
        result.degree(),
        1,
        "Expected colliding attribute to be dropped per current behavior"
    );

    // Assert that the value preserved is from the first relation (id=1)
    let tuple = result.tuples().next().expect("Should have one tuple");
    assert_eq!(
        tuple.get_typed::<i64>("id").expect("id attribute missing"),
        1
    );
}

/// Verifies behavior when colliding attributes have DIFFERENT types.
///
/// Even if types mismatch, the second attribute is dropped, and the result
/// retains the type and value of the first attribute. This ensures no
/// type confusion or panic occurs during tuple construction.
#[test]
fn test_theta_join_colliding_attributes_different_types() {
    // Relation 1: id=1 (Int)
    let heading1 = TupleType::new().with_attribute("id", ScalarType::Int);
    let mut rel1 = Relation::new(RelationType::new(heading1));
    rel1.insert(tuple! { id: 1i64 }).unwrap();

    // Relation 2: id="2" (String) - Same name, different type
    let heading2 = TupleType::new().with_attribute("id", ScalarType::String);
    let mut rel2 = Relation::new(RelationType::new(heading2));
    rel2.insert(tuple! { id: "2" }).unwrap();

    // Theta join
    let result = rel1.theta_join(&rel2, |_, _| true);

    // Result should have 'id' as Int (from rel1)
    assert_eq!(result.degree(), 1);
    let tuple = result.tuples().next().unwrap();

    // Should be able to get as Int
    assert_eq!(tuple.get_typed::<i64>("id").unwrap(), 1);

    // Should NOT be able to get as String
    assert!(tuple.get_typed::<String>("id").is_none());

    // Verify type in heading
    assert_eq!(
        result.relation_type().heading().get_attribute_type("id"),
        Some(&ScalarType::Int)
    );
}

#[test]
fn test_join_swap_optimization() {
    // Test case 1: Left is smaller (no swap)
    let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    let mut small = Relation::new(RelationType::new(heading.clone()));
    small.insert(tuple! { id: 1i64 }).unwrap();

    let mut large = Relation::new(RelationType::new(heading.clone()));
    for i in 0..10 {
        large.insert(tuple! { id: i as i64 }).unwrap();
    }

    let result1 = small.join(&large).unwrap();
    assert_eq!(result1.cardinality(), 1);

    // Test case 2: Right is smaller (swap)
    let result2 = large.join(&small).unwrap();
    assert_eq!(result2.cardinality(), 1);
}
