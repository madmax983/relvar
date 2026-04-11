#[cfg(test)]
mod tests {
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    #[test]
    #[should_panic(expected = "Type nesting too deep")]
    fn test_relation_type_new_nesting_too_deep() {
        let mut ty = ScalarType::Int;

        // Build up to MAX_TYPE_DEPTH - 1
        for i in 0..(relvar_core::types::MAX_TYPE_DEPTH - 2) {
            ty = ScalarType::user_defined(format!("Type{}", i), ty);
        }

        let heading = TupleType::new().with_attribute("attr", ty);

        // This should panic inside RelationType::new because:
        // heading.depth() + 1 > MAX_TYPE_DEPTH
        let _ = RelationType::new(heading);
    }
}
