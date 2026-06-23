use relvar::tools::to_csv;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;

#[test]
fn test_csv_injection() {
    let heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);
    let mut relation = Relation::new(RelationType::new(heading));

    relation
        .insert(tuple! { id: 1i64, name: "=cmd|' /C calc'!A0" })
        .unwrap();
    relation
        .insert(tuple! { id: 2i64, name: "+SUM(1,1)" })
        .unwrap();
    relation
        .insert(tuple! { id: 3i64, name: "-SUM(1,1)" })
        .unwrap();
    relation
        .insert(tuple! { id: 4i64, name: "@SUM(1,1)" })
        .unwrap();
    relation
        .insert(tuple! { id: 5i64, name: "\tSUM(1,1)" })
        .unwrap();
    relation
        .insert(tuple! { id: 6i64, name: "\rSUM(1,1)" })
        .unwrap();
    relation
        .insert(tuple! { id: 7i64, name: "\nSUM(1,1)" })
        .unwrap();

    let csv = to_csv(&relation, ',').unwrap();
    println!("{}", csv);
    assert!(!csv.contains("\"=cmd"));
}
