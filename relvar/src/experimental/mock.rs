//! Mock data generation module.
//!
//! This module provides tools to generate random relations for testing and prototyping.
//! It can automatically populate relations based on their schema (RelationType),
//! supporting both primitive types and nested structures.
//!
//! # Example
//!
//! ```
//! use relvar::experimental::mock::generate_mock_relation;
//! use relvar_core::types::{RelationType, TupleType, ScalarType};
//!
//! // 1. Define schema
//! let rel_type = RelationType::new(
//!     TupleType::new()
//!         .with_attribute("id", ScalarType::Int)
//!         .with_attribute("score", ScalarType::Float)
//!         .with_attribute("tag", ScalarType::String)
//! );
//!
//! // 2. Generate random data
//! let relation = generate_mock_relation(rel_type, 100, Some(42));
//!
//! assert_eq!(relation.cardinality(), 100);
//! ```

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use relvar_core::types::{RelationType, ScalarType};
use relvar_core::values::{Relation, ScalarValue, Tuple};
use std::collections::BTreeMap;

/// Generates a mock relation with random data.
///
/// Note: Since relations are sets, duplicate tuples will be ignored.
/// If the random generation produces duplicates, the final cardinality
/// might be less than `count`.
///
/// # Example
///
/// ```
/// use relvar::experimental::mock::generate_mock_relation;
/// use relvar_core::types::{RelationType, TupleType, ScalarType};
///
/// let rel_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("id", ScalarType::Int)
///         .with_attribute("name", ScalarType::String)
/// );
///
/// let relation = generate_mock_relation(rel_type, 10, Some(42));
///
/// assert_eq!(relation.cardinality(), 10);
/// ```
pub fn generate_mock_relation(
    relation_type: RelationType,
    count: usize,
    seed: Option<u64>,
) -> Relation {
    let mut relation = Relation::new(relation_type.clone());
    let mut rng = if let Some(seed) = seed {
        StdRng::seed_from_u64(seed)
    } else {
        StdRng::from_entropy()
    };

    for _ in 0..count {
        if let Some(tuple) = generate_tuple(&relation_type, &mut rng) {
            let _ = relation.insert(tuple);
        }
    }

    relation
}

fn generate_tuple(relation_type: &RelationType, rng: &mut StdRng) -> Option<Tuple> {
    let mut values = BTreeMap::new();

    for attr_name in relation_type.heading().attribute_names() {
        let scalar_type = relation_type.heading().get_attribute_type(attr_name)?;
        let value = generate_value(scalar_type, rng);
        values.insert(attr_name.clone(), value);
    }

    Tuple::new(relation_type.heading().clone(), values).ok()
}

fn generate_value(scalar_type: &ScalarType, rng: &mut StdRng) -> ScalarValue {
    match scalar_type {
        ScalarType::Int => ScalarValue::Int(rng.r#gen()),
        ScalarType::Float => ScalarValue::Float(rng.r#gen()),
        ScalarType::String => {
            // Generate a random string of length 5-15
            let len = rng.gen_range(5..=15);
            let s: String = (0..len)
                .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
                .collect();
            ScalarValue::String(s)
        }
        ScalarType::Bool => ScalarValue::Bool(rng.r#gen()),
        ScalarType::Bytes => {
            let len = rng.gen_range(1..=32);
            let bytes: Vec<u8> = (0..len).map(|_| rng.r#gen()).collect();
            ScalarValue::Bytes(bytes)
        }
        ScalarType::Relation(rel_type) => {
            // Nested relation: return empty for now to avoid infinite recursion
            ScalarValue::Relation(Relation::new(*rel_type.clone()))
        }
        ScalarType::UserDefined { representation, .. } => {
            // Recursively generate value for the representation
            let inner_value = generate_value(representation, rng);
            ScalarValue::UserDefined {
                type_def: scalar_type.clone(),
                value: Box::new(inner_value),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::types::TupleType;

    #[test]
    fn test_generate_mock_relation() {
        let rel_type = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("name", ScalarType::String)
                .with_attribute("active", ScalarType::Bool),
        );

        let relation = generate_mock_relation(rel_type, 50, Some(12345));

        assert_eq!(relation.cardinality(), 50);
        assert_eq!(relation.degree(), 3);
    }

    #[test]
    fn test_generate_all_types() {
        let nested_rel_type =
            RelationType::new(TupleType::new().with_attribute("inner", ScalarType::Int));

        let user_defined_type = ScalarType::UserDefined {
            name: "MyType".to_string(),
            representation: Box::new(ScalarType::Int),
        };

        let rel_type = RelationType::new(
            TupleType::new()
                .with_attribute("int_val", ScalarType::Int)
                .with_attribute("float_val", ScalarType::Float)
                .with_attribute("str_val", ScalarType::String)
                .with_attribute("bool_val", ScalarType::Bool)
                .with_attribute("bytes_val", ScalarType::Bytes)
                .with_attribute("rel_val", ScalarType::Relation(Box::new(nested_rel_type)))
                .with_attribute("user_val", user_defined_type),
        );

        let relation = generate_mock_relation(rel_type, 10, Some(42));

        assert_eq!(relation.degree(), 7);
        assert_eq!(relation.cardinality(), 10);
    }

    #[test]
    fn test_determinism() {
        let rel_type = RelationType::new(TupleType::new().with_attribute("val", ScalarType::Float));

        let rel1 = generate_mock_relation(rel_type.clone(), 10, Some(42));

        let rel2 = generate_mock_relation(rel_type, 10, Some(42));

        assert_eq!(rel1, rel2);
    }
}
