#[cfg(test)]
mod tests {
    use relvar_core::types::{ScalarType, TupleType};

    #[test]
    #[should_panic(expected = "Type nesting too deep")]
    fn test_tuple_type_with_attribute_nesting_too_deep() {
        let mut ty = ScalarType::Int;

        // Build up to exactly MAX_TYPE_DEPTH
        // ScalarType::Int is depth 1
        for i in 0..(relvar_core::types::MAX_TYPE_DEPTH - 1) {
            ty = ScalarType::user_defined(format!("Type{}", i), ty);
        }

        // ty.depth() is now MAX_TYPE_DEPTH
        assert_eq!(ty.depth(), relvar_core::types::MAX_TYPE_DEPTH);

        // This should panic inside `with_attribute` because:
        // ty.depth() + 1 = MAX_TYPE_DEPTH + 1 > MAX_TYPE_DEPTH
        let _ = TupleType::new().with_attribute("attr", ty);
    }

    #[test]
    fn test_tuple_type_default() {
        let t = TupleType::default();
        assert_eq!(t.degree(), 0);
    }
}
