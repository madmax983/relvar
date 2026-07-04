use crate::persistent_engine::*;
use relvar_core::types::{ScalarType, TupleType};

pub(crate) fn test_rel_type() -> RelationType {
    RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    )
}
