use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation};

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

    let result = relation.project_into(&["emp_id", "name"]);

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
    let result = relation.project(&["z"]);
    assert_eq!(result.degree(), 1);

    // Test project_into as well
    let mut relation2 = Relation::new(RelationType::new(
        TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("z", ScalarType::Int),
    ));
    relation2.insert(tuple! { a: 1i64, z: 2i64 }).unwrap();
    let result2 = relation2.project_into(&["z"]);
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
    let result = relation.project(&["b", "z"]);
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
    let result2 = relation2.project_into(&["b", "z"]);
    assert_eq!(result2.degree(), 2);
}

#[test]
fn should_project_with_ordering_greater() {
    let heading = TupleType::new()
        .with_attribute("c", ScalarType::Int)
        .with_attribute("d", ScalarType::Int);

    let mut relation = Relation::new(RelationType::new(heading));
    relation.insert(tuple! { c: 1i64, d: 2i64 }).unwrap();

    // Projecting "a" which comes before "c" in the heading attributes
    // However, it ignores non-existent attributes silently
    let result = relation.project(&["a", "d"]);
    assert_eq!(result.degree(), 1); // Only "d" is projected

    // Now testing project_into
    let mut relation2 = Relation::new(RelationType::new(
        TupleType::new()
            .with_attribute("c", ScalarType::Int)
            .with_attribute("d", ScalarType::Int),
    ));
    relation2.insert(tuple! { c: 1i64, d: 2i64 }).unwrap();
    let result2 = relation2.project_into(&["a", "d"]);
    assert_eq!(result2.degree(), 1); // Only "d" is projected
}

#[test]
fn should_project_with_ordering_less() {
    let heading = TupleType::new()
        .with_attribute("a", ScalarType::Int)
        .with_attribute("b", ScalarType::Int);

    let mut relation = Relation::new(RelationType::new(heading));
    relation.insert(tuple! { a: 1i64, b: 2i64 }).unwrap();

    let result = relation.project(&["c", "a"]);
    assert_eq!(result.degree(), 1);

    let mut relation2 = Relation::new(RelationType::new(
        TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int),
    ));
    relation2.insert(tuple! { a: 1i64, b: 2i64 }).unwrap();
    let result2 = relation2.project_into(&["c", "a"]);
    assert_eq!(result2.degree(), 1);
}

#[test]
fn should_project_with_ordering_less_owned() {
    let heading = TupleType::new()
        .with_attribute("c", ScalarType::Int)
        .with_attribute("d", ScalarType::Int);

    let mut relation = Relation::new(RelationType::new(heading));
    relation.insert(tuple! { c: 1i64, d: 2i64 }).unwrap();

    let result = relation.project_into(&["a", "b"]);
    assert_eq!(result.degree(), 0);
}
