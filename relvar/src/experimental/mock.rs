//! Mock data generation module.
//!
//! This module provides tools to generate random relations for testing and prototyping.
//! It can automatically populate relations based on their schema (RelationType),
//! supporting both primitive types and nested structures.
//!
//! # Example
//!
//! ```
//! use relvar::experimental::mock::MockRelation;
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
//! let relation = MockRelation::new(rel_type)
//!     .count(100)       // Generate 100 tuples
//!     .seed(42)         // Use fixed seed for deterministic results
//!     .generate();
//!
//! assert_eq!(relation.cardinality(), 100);
//! ```

use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use relvar_core::types::{RelationType, ScalarType};
use relvar_core::values::{Relation, ScalarValue, Tuple};
use std::collections::BTreeMap;

/// A builder for generating mock relations with random data.
///
/// # Examples
///
/// ```
/// use relvar::experimental::mock::MockRelation;
/// use relvar_core::types::{RelationType, TupleType, ScalarType};
///
/// let rel_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("id", ScalarType::Int)
///         .with_attribute("name", ScalarType::String)
/// );
///
/// let relation = MockRelation::new(rel_type)
///     .count(10)
///     .seed(42)
///     .generate();
///
/// assert_eq!(relation.cardinality(), 10);
/// ```
pub struct MockRelation {
    relation_type: RelationType,
    count: usize,
    seed: Option<u64>,
}

impl MockRelation {
    /// Create a new MockRelation builder for the given relation type.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::mock::MockRelation;
    /// // Note: This is a placeholder example
    /// ```
    pub fn new(relation_type: RelationType) -> Self {
        Self {
            relation_type,
            count: 0,
            seed: None,
        }
    }

    /// Set the number of tuples to generate.
    ///
    /// Note: Since relations are sets, duplicate tuples will be ignored.
    /// If the random generation produces duplicates, the final cardinality
    /// might be less than `count`.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::mock::MockRelation;
    /// // Note: This is a placeholder example
    /// ```
    pub fn count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    /// Set the random seed for deterministic generation.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::mock::MockRelation;
    /// // Note: This is a placeholder example
    /// ```
    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Generate the relation.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::mock::MockRelation;
    /// // Note: This is a placeholder example
    /// ```
    pub fn generate(self) -> Relation {
        let mut relation = Relation::new(self.relation_type.clone());
        let mut rng = if let Some(seed) = self.seed {
            StdRng::seed_from_u64(seed)
        } else {
            StdRng::from_rng(&mut rand::rng())
        };

        for _ in 0..self.count {
            if let Some(tuple) = self.generate_tuple(&mut rng) {
                let _ = relation.insert(tuple);
            }
        }

        relation
    }

    fn generate_tuple(&self, rng: &mut StdRng) -> Option<Tuple> {
        let mut values = BTreeMap::new();

        for attr_name in self.relation_type.heading().attribute_names() {
            let scalar_type = self.relation_type.heading().get_attribute_type(attr_name)?;
            let value = self.generate_value(scalar_type, rng);
            values.insert(attr_name.clone(), value);
        }

        Tuple::new(self.relation_type.heading().clone(), values).ok()
    }

    fn generate_value(&self, scalar_type: &ScalarType, rng: &mut StdRng) -> ScalarValue {
        match scalar_type {
            ScalarType::Int => ScalarValue::Int(rng.random()),
            ScalarType::Float => ScalarValue::Float(rng.random()),
            ScalarType::String => {
                // Generate a random string of length 5-15
                let len = rng.random_range(5..=15);
                let s: String = (0..len)
                    .map(|_| rng.sample(rand::distr::Alphanumeric) as char)
                    .collect();
                ScalarValue::String(s)
            }
            ScalarType::Bool => ScalarValue::Bool(rng.random()),
            ScalarType::Bytes => {
                let len = rng.random_range(1..=32);
                let bytes: Vec<u8> = (0..len).map(|_| rng.random()).collect();
                ScalarValue::Bytes(bytes)
            }
            ScalarType::Relation(rel_type) => {
                // Nested relation: return empty for now to avoid infinite recursion
                ScalarValue::Relation(Relation::new(*rel_type.clone()))
            }
            ScalarType::UserDefined { representation, .. } => {
                // Recursively generate value for the representation
                let inner_value = self.generate_value(representation, rng);
                ScalarValue::UserDefined {
                    type_def: scalar_type.clone(),
                    value: Box::new(inner_value),
                }
            }
        }
    }
}

#[allow(dead_code)]
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

        let relation = MockRelation::new(rel_type)
            .count(50)
            .seed(12345) // Deterministic
            .generate();

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

        let relation = MockRelation::new(rel_type).count(10).seed(42).generate();

        assert_eq!(relation.degree(), 7);
        assert_eq!(relation.cardinality(), 10);
    }

    #[test]
    fn test_determinism() {
        let rel_type = RelationType::new(TupleType::new().with_attribute("val", ScalarType::Float));

        let rel1 = MockRelation::new(rel_type.clone())
            .count(10)
            .seed(42)
            .generate();

        let rel2 = MockRelation::new(rel_type).count(10).seed(42).generate();

        assert_eq!(rel1, rel2);
    }
}
