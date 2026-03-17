use relvar_core::types::{TupleType, ScalarType, RelationType};

#[test]
#[should_panic(expected = "Type nesting too deep")]
fn test_tuple_type_depth_limit() {
    let mut current_type = ScalarType::Int;
    for i in 0..relvar_core::types::MAX_TYPE_DEPTH {
        let heading = TupleType::new().with_attribute(format!("a{}", i), current_type);
        current_type = ScalarType::Relation(Box::new(RelationType::new(heading)));
    }

    // This should panic
    let _ = TupleType::new().with_attribute("deep", current_type);
}
