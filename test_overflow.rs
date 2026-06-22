use relvar_core::{tuple, Relation, RelationType, TupleType, ScalarType, values::ScalarValue};

fn main() {
    let t = TupleType::new()
        .with_attribute("a", ScalarType::Int)
        .with_attribute("b", ScalarType::Relation(TupleType::new().with_attribute("c", ScalarType::Int)));
    let mut rel = Relation::new(RelationType::new(t)).unwrap();

    // Simulate high cardinality by injecting relations... (just a conceptual check)
}
