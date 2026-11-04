use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;

#[test]
fn should_project_into_all_attributes() {
    let heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type.clone());

    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice" })
        .unwrap();
    relation
        .insert(tuple! { emp_id: 2i64, name: "Bob" })
        .unwrap();

    let result = relation.project_into(&["emp_id", "name"]).unwrap();

    assert_eq!(result.degree(), 2);
    assert_eq!(result.cardinality(), 2);
}

#[test]
fn should_project_with_ordering_greater_coverage() {
    let heading = TupleType::new()
        .with_attribute("a", ScalarType::Int)
        .with_attribute("z", ScalarType::Int);

    let mut relation = Relation::new(RelationType::new(heading));

    relation.insert(tuple! { a: 1i64, z: 2i64 }).unwrap();

    // Project out "a", leaving "z"
    let result = relation.project(&["z"]).unwrap();
    assert_eq!(result.degree(), 1);

    // Test project_into as well
    let mut relation2 = Relation::new(RelationType::new(
        TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("z", ScalarType::Int),
    ));
    relation2.insert(tuple! { a: 1i64, z: 2i64 }).unwrap();
    let result2 = relation2.project_into(&["z"]).unwrap();
    assert_eq!(result2.degree(), 1);
}

#[test]
fn should_project_with_ordering_greater_coverage_more() {
    let heading = TupleType::new()
        .with_attribute("a", ScalarType::Int)
        .with_attribute("b", ScalarType::Int)
        .with_attribute("z", ScalarType::Int);

    let mut relation = Relation::new(RelationType::new(heading));

    relation
        .insert(tuple! { a: 1i64, b: 2i64, z: 3i64 })
        .unwrap();

    // Project out "a", leaving "b" and "z" - "b" is > "a"
    let result = relation.project(&["b", "z"]).unwrap();
    assert_eq!(result.degree(), 2);

    // Test project_into as well
    let mut relation2 = Relation::new(RelationType::new(
        TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int)
            .with_attribute("z", ScalarType::Int),
    ));
    relation2
        .insert(tuple! { a: 1i64, b: 2i64, z: 3i64 })
        .unwrap();
    let result2 = relation2.project_into(&["b", "z"]).unwrap();
    assert_eq!(result2.degree(), 2);
}

#[test]
fn should_project_with_ordering_greater() {
    let heading = TupleType::new()
        .with_attribute("c", ScalarType::Int)
        .with_attribute("d", ScalarType::Int);

    let mut relation = Relation::new(RelationType::new(heading));
    relation.insert(tuple! { c: 1i64, d: 2i64 }).unwrap();

    // TTM: projecting on "a" (not in the heading) is a type error.
    assert!(relation.project(&["a", "d"]).is_err());
    let result = relation.project(&["d"]).unwrap();
    assert_eq!(result.degree(), 1);

    // Now testing project_into
    let mut relation2 = Relation::new(RelationType::new(
        TupleType::new()
            .with_attribute("c", ScalarType::Int)
            .with_attribute("d", ScalarType::Int),
    ));
    relation2.insert(tuple! { c: 1i64, d: 2i64 }).unwrap();
    assert!(relation2.clone().project_into(&["a", "d"]).is_err());
    let result2 = relation2.project_into(&["d"]).unwrap();
    assert_eq!(result2.degree(), 1);
}

#[test]
fn should_project_with_ordering_less() {
    let heading = TupleType::new()
        .with_attribute("a", ScalarType::Int)
        .with_attribute("b", ScalarType::Int);

    let mut relation = Relation::new(RelationType::new(heading));
    relation.insert(tuple! { a: 1i64, b: 2i64 }).unwrap();

    assert!(relation.project(&["c", "a"]).is_err());
    let result = relation.project(&["a"]).unwrap();
    assert_eq!(result.degree(), 1);

    let mut relation2 = Relation::new(RelationType::new(
        TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int),
    ));
    relation2.insert(tuple! { a: 1i64, b: 2i64 }).unwrap();
    assert!(relation2.clone().project_into(&["c", "a"]).is_err());
    let result2 = relation2.project_into(&["a"]).unwrap();
    assert_eq!(result2.degree(), 1);
}

#[test]
fn should_project_with_ordering_less_owned() {
    let heading = TupleType::new()
        .with_attribute("c", ScalarType::Int)
        .with_attribute("d", ScalarType::Int);

    let mut relation = Relation::new(RelationType::new(heading));
    relation.insert(tuple! { c: 1i64, d: 2i64 }).unwrap();

    assert!(relation.clone().project_into(&["a", "b"]).is_err());
    let result = relation.project_into(&[]).unwrap();
    assert_eq!(result.degree(), 0);
}

#[test]
fn should_project_with_ordering_greater_for_project_tuple_values() {
    let heading = TupleType::new()
        .with_attribute("z", ScalarType::Int)
        .with_attribute("a", ScalarType::Int); // "a" comes before "z" lexicographically
    let mut relation = Relation::new(RelationType::new(heading));

    relation.insert(tuple! { z: 1i64, a: 2i64 }).unwrap();

    let result = relation.project(&["z"]).unwrap(); // Heading of result will just have "z"
    // "a" is less than "z", so `t_attr.cmp(h_attr)` where t_attr="a", h_attr="z" will be Less
    assert_eq!(result.degree(), 1);
}
