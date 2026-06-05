use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;
use relvar_core::tuple;

#[test]
fn test_project_optimization() {
    let heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String)
        .with_attribute("dept_id", ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
        .unwrap();
    relation
        .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
        .unwrap();

    let result = relation.project(&["emp_id", "name"]);

    assert_eq!(result.cardinality(), 2);
    assert_eq!(result.degree(), 2);
    assert!(result.contains(&tuple! { emp_id: 1i64, name: "Alice" }));
    assert!(result.contains(&tuple! { emp_id: 2i64, name: "Bob" }));
}
