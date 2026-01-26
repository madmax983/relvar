use crate::types::ScalarType;
use serde::{Deserialize, Serialize};

/// A scalar value with its type.
/// Per Date's relational model, all values carry their type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScalarValue {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Bytes(Vec<u8>),
    Relation(crate::values::Relation),
}

impl ScalarValue {
    /// Returns the type of this value
    pub fn scalar_type(&self) -> ScalarType {
        match self {
            ScalarValue::Int(_) => ScalarType::Int,
            ScalarValue::Float(_) => ScalarType::Float,
            ScalarValue::String(_) => ScalarType::String,
            ScalarValue::Bool(_) => ScalarType::Bool,
            ScalarValue::Bytes(_) => ScalarType::Bytes,
            ScalarValue::Relation(rel) => {
                ScalarType::Relation(Box::new(rel.relation_type().clone()))
            }
        }
    }

    /// Checks if this value is of the given type
    pub fn is_type(&self, ty: &ScalarType) -> bool {
        &self.scalar_type() == ty
    }
}

// Custom PartialEq implementation for ScalarValue
// Note: Float comparison uses bit equality, which is appropriate for
// database values (we want NaN == NaN for set semantics)
impl PartialEq for ScalarValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ScalarValue::Int(a), ScalarValue::Int(b)) => a == b,
            (ScalarValue::Float(a), ScalarValue::Float(b)) => a.to_bits() == b.to_bits(),
            (ScalarValue::String(a), ScalarValue::String(b)) => a == b,
            (ScalarValue::Bool(a), ScalarValue::Bool(b)) => a == b,
            (ScalarValue::Bytes(a), ScalarValue::Bytes(b)) => a == b,
            (ScalarValue::Relation(a), ScalarValue::Relation(b)) => a == b,
            _ => false,
        }
    }
}

// Custom Eq implementation for ScalarValue
impl Eq for ScalarValue {}

// Custom Hash implementation
impl std::hash::Hash for ScalarValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            ScalarValue::Int(v) => {
                0u8.hash(state);
                v.hash(state);
            }
            ScalarValue::Float(v) => {
                1u8.hash(state);
                v.to_bits().hash(state);
            }
            ScalarValue::String(v) => {
                2u8.hash(state);
                v.hash(state);
            }
            ScalarValue::Bool(v) => {
                3u8.hash(state);
                v.hash(state);
            }
            ScalarValue::Bytes(v) => {
                4u8.hash(state);
                v.hash(state);
            }
            ScalarValue::Relation(v) => {
                5u8.hash(state);
                v.hash(state);
            }
        }
    }
}

// Custom PartialOrd implementation for MIN/MAX operations
impl PartialOrd for ScalarValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// Custom Ord implementation for use in BTreeMap
// We order by type first, then by value within type
impl Ord for ScalarValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        // Helper to get type ordering
        fn type_order(val: &ScalarValue) -> u8 {
            match val {
                ScalarValue::Int(_) => 0,
                ScalarValue::Float(_) => 1,
                ScalarValue::String(_) => 2,
                ScalarValue::Bool(_) => 3,
                ScalarValue::Bytes(_) => 4,
                ScalarValue::Relation(_) => 5,
            }
        }

        // Compare types first
        match type_order(self).cmp(&type_order(other)) {
            Ordering::Equal => {
                // Same type: order by value
                match (self, other) {
                    (ScalarValue::Int(a), ScalarValue::Int(b)) => a.cmp(b),
                    (ScalarValue::Float(a), ScalarValue::Float(b)) => {
                        // For floats, use bit ordering (treats NaN consistently)
                        a.to_bits().cmp(&b.to_bits())
                    }
                    (ScalarValue::String(a), ScalarValue::String(b)) => a.cmp(b),
                    (ScalarValue::Bool(a), ScalarValue::Bool(b)) => a.cmp(b),
                    (ScalarValue::Bytes(a), ScalarValue::Bytes(b)) => a.cmp(b),
                    (ScalarValue::Relation(a), ScalarValue::Relation(b)) => {
                        // For relations, order by cardinality first, then degree
                        match a.cardinality().cmp(&b.cardinality()) {
                            Ordering::Equal => a.degree().cmp(&b.degree()),
                            other => other,
                        }
                    }
                    _ => unreachable!("Type orders are equal but types don't match"),
                }
            }
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_values_carry_their_type() {
        let int_val = ScalarValue::Int(42);
        let float_val = ScalarValue::Float(3.5);
        let string_val = ScalarValue::String("hello".to_string());
        let bool_val = ScalarValue::Bool(true);
        let bytes_val = ScalarValue::Bytes(vec![1, 2, 3]);

        assert_eq!(int_val.scalar_type(), ScalarType::Int);
        assert_eq!(float_val.scalar_type(), ScalarType::Float);
        assert_eq!(string_val.scalar_type(), ScalarType::String);
        assert_eq!(bool_val.scalar_type(), ScalarType::Bool);
        assert_eq!(bytes_val.scalar_type(), ScalarType::Bytes);
    }

    #[test]
    fn test_values_of_same_type_can_be_compared() {
        let int1 = ScalarValue::Int(42);
        let int2 = ScalarValue::Int(42);
        let int3 = ScalarValue::Int(43);

        assert_eq!(int1, int2);
        assert_ne!(int1, int3);

        let str1 = ScalarValue::String("hello".to_string());
        let str2 = ScalarValue::String("hello".to_string());
        let str3 = ScalarValue::String("world".to_string());

        assert_eq!(str1, str2);
        assert_ne!(str1, str3);
    }

    #[test]
    fn test_values_of_different_types_are_not_equal() {
        let int_val = ScalarValue::Int(42);
        let float_val = ScalarValue::Float(42.0);

        assert_ne!(int_val, float_val);
    }

    #[test]
    fn test_values_can_be_serialized_and_deserialized() {
        let original = ScalarValue::Int(42);
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: ScalarValue = serde_json::from_str(&serialized).unwrap();

        assert_eq!(original, deserialized);

        let original = ScalarValue::String("test".to_string());
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: ScalarValue = serde_json::from_str(&serialized).unwrap();

        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_float_equality_for_set_semantics() {
        // For database semantics, we need NaN == NaN
        let nan1 = ScalarValue::Float(f64::NAN);
        let nan2 = ScalarValue::Float(f64::NAN);

        assert_eq!(nan1, nan2);

        // Same bit pattern floats should be equal
        let f1 = ScalarValue::Float(2.5);
        let f2 = ScalarValue::Float(2.5);
        assert_eq!(f1, f2);
    }

    #[test]
    fn test_values_can_be_hashed() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(ScalarValue::Int(42));
        set.insert(ScalarValue::Int(42)); // Duplicate
        set.insert(ScalarValue::Int(43));

        assert_eq!(set.len(), 2);

        // Test with floats including NaN
        let mut float_set = HashSet::new();
        float_set.insert(ScalarValue::Float(2.5));
        float_set.insert(ScalarValue::Float(2.5)); // Duplicate
        float_set.insert(ScalarValue::Float(f64::NAN));
        float_set.insert(ScalarValue::Float(f64::NAN)); // Duplicate NaN

        assert_eq!(float_set.len(), 2); // 2.5 and NaN
    }

    #[test]
    fn test_is_type() {
        let int_val = ScalarValue::Int(42);
        assert!(int_val.is_type(&ScalarType::Int));
        assert!(!int_val.is_type(&ScalarType::Float));
    }
}
