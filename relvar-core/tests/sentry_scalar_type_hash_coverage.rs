use relvar_core::types::{RelationType, ScalarType, TupleType};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

#[test]
fn test_scalar_type_hash_coverage() {
    let int_type = ScalarType::Int;
    let float_type = ScalarType::Float;
    let string_type = ScalarType::String;
    let bool_type = ScalarType::Bool;
    let bytes_type = ScalarType::Bytes;

    let rel_type = ScalarType::Relation(Box::new(RelationType::new(TupleType::new())));

    let user_defined = ScalarType::user_defined("MyType", ScalarType::Int);

    // Hash all of them
    let _ = calculate_hash(&int_type);
    let _ = calculate_hash(&float_type);
    let _ = calculate_hash(&string_type);
    let _ = calculate_hash(&bool_type);
    let _ = calculate_hash(&bytes_type);
    let _ = calculate_hash(&rel_type);
    let _ = calculate_hash(&user_defined);
}
