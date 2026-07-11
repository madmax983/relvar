//! Tuple values for the relational model.
//!
//! A tuple is a set of attribute-value pairs that conforms to a tuple type.
//! This module provides the [`Tuple`] struct and the `tuple!` macro for
//! convenient tuple creation.
//!
//! # TTM Compliance
//!
//! - **Proscription 1**: No NULL values - all attributes must have values
//! - **Proscription 4**: No attribute ordering - attributes identified by name
//! - Tuple equality is based on attribute values, not physical identity
//!
//! # Example
//!
//! ```
//! use relvar_core::tuple;
//! use relvar_core::values::ScalarValue;
//!
//! // Create a tuple using the macro
//! let employee = tuple! {
//!     emp_id: 1i64,
//!     name: "Alice",
//!     active: true,
//! };
//!
//! assert_eq!(employee.degree(), 3);
//! assert_eq!(employee.get("name"), Some(&ScalarValue::String("Alice".to_string())));
//! ```

use crate::types::TupleType;
use crate::values::ScalarValue;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use thiserror::Error;

mod arc_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::sync::Arc;

    /// Helper for Serde to serialize an `Arc<T>` transparently.
    ///
    /// This exists because `Tuple` stores its `TupleType` in an `Arc` for performance,
    /// but the default Serde serialization would fail or create nested structures. This ensures
    /// the underlying type is serialized cleanly.
    ///
    /// # Errors
    /// Yields a Serde error if the inner value cannot be serialized.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, ScalarType};
    /// use std::sync::Arc;
    /// // Internally used by serde
    /// ```
    pub fn serialize<S, T>(val: &Arc<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
    {
        T::serialize(val, serializer)
    }

    /// Helper for Serde to deserialize directly into an `Arc<T>`.
    ///
    /// This exists to rehydrate a `Tuple` from disk seamlessly wrapped in an `Arc` for fast
    /// memory sharing, rather than allocating a new unshared heap pointer.
    ///
    /// # Errors
    /// Yields a Serde error if the inner value cannot be deserialized.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, ScalarType};
    /// use std::sync::Arc;
    /// // Internally used by serde
    /// ```
    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Arc<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        let val = T::deserialize(deserializer)?;
        Ok(Arc::new(val))
    }
}

/// Errors that can occur when creating or modifying tuples.
#[derive(Debug, Error)]
pub enum TupleError {
    /// An attribute name was provided that doesn't exist in the tuple type.
    #[error("Attribute '{0}' not found in tuple type")]
    AttributeNotFound(String),

    /// A value's type doesn't match the expected type for its attribute.
    #[error("Type mismatch for attribute '{0}': expected {1:?}, got {2:?}")]
    TypeMismatch(String, String, String),

    /// An attribute defined in the tuple type has no corresponding value.
    ///
    /// This enforces TTM Proscription 1 (no NULL values).
    #[error("Missing value for attribute '{0}'")]
    MissingValue(String),
}

/// A tuple value that conforms to a tuple type.
///
/// A tuple is an unordered set of attribute-value pairs. Each attribute
/// has a name and a scalar value. The tuple must conform to its tuple type,
/// meaning all attributes defined in the type must be present with values
/// of the correct types.
///
/// # No NULL Values
///
/// Per TTM Proscription 1, every attribute must have a value. Attempting to
/// create a tuple with missing attributes will result in an error.
///
/// # Equality
///
/// Two tuples are equal if and only if they have the same tuple type and
/// all their attribute values are equal. The order of insertion does not
/// affect equality.
///
/// # Examples
///
/// ```
/// use relvar_core::tuple;
/// use relvar_core::values::ScalarValue;
///
/// let person = tuple! {
///     id: 1i64,
///     name: "Alice",
///     age: 30i64,
/// };
///
/// // Access values by attribute name
/// assert_eq!(person.get("id"), Some(&ScalarValue::Int(1)));
///
/// // Use typed access
/// let age: i64 = person.get_typed("age").unwrap();
/// assert_eq!(age, 30);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tuple {
    /// The tuple type (heading) that this tuple conforms to.
    #[serde(with = "arc_serde")]
    tuple_type: Arc<TupleType>,
    /// The attribute values, keyed by attribute name.
    values: BTreeMap<String, ScalarValue>,
}

impl Tuple {
    /// Creates a new tuple from a type and values.
    ///
    /// Validates that all attributes defined in the tuple type have
    /// corresponding values and that all values match their expected types.
    ///
    /// # Arguments
    ///
    /// * `tuple_type` - The tuple type (heading) this tuple must conform to
    /// * `values` - A map of attribute names to scalar values. Can be HashMap or BTreeMap (via IntoIterator)
    ///
    /// # Errors
    ///
    /// - [`TupleError::MissingValue`] - An attribute has no value (violates Proscription 1)
    /// - [`TupleError::AttributeNotFound`] - A value is provided for an undefined attribute
    /// - [`TupleError::TypeMismatch`] - A value's type doesn't match the attribute type
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, ScalarType};
    /// use relvar_core::values::{Tuple, ScalarValue};
    /// use std::collections::HashMap;
    ///
    /// let tuple_type = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int)
    ///     .with_attribute("name", ScalarType::String);
    ///
    /// let mut values = HashMap::new();
    /// values.insert("id".to_string(), ScalarValue::Int(1));
    /// values.insert("name".to_string(), ScalarValue::String("Alice".to_string()));
    ///
    /// let tuple = Tuple::new(tuple_type, values).unwrap();
    /// assert_eq!(tuple.degree(), 2);
    /// ```
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::{Tuple, ScalarValue};
    /// use relvar_core::types::{TupleType, ScalarType};
    /// use std::collections::BTreeMap;
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int)
    ///     .with_attribute("name", ScalarType::String);
    ///
    /// let mut values = BTreeMap::new();
    /// values.insert("id".to_string(), ScalarValue::Int(1));
    /// values.insert("name".to_string(), ScalarValue::String("Alice".to_string()));
    ///
    /// let tuple = Tuple::new(heading, values).unwrap();
    /// assert_eq!(tuple.degree(), 2);
    /// ```
    pub fn new<T, M>(tuple_type: T, values: M) -> Result<Self, TupleError>
    where
        T: Into<Arc<TupleType>>,
        M: IntoIterator<Item = (String, ScalarValue)>,
    {
        let tuple_type: Arc<TupleType> = tuple_type.into();
        // pre-allocate if the iterator provides a size hint
        // However BTreeMap does not support with_capacity in std yet. So just create a new one.
        let mut values_map = BTreeMap::new();

        // Validate values during collection to avoid double iteration and allocations on invalid data
        for (attr_name, value) in values {
            if let Some(expected_type) = tuple_type.get_attribute_type(&attr_name) {
                if !value.is_type(expected_type) {
                    return Err(TupleError::TypeMismatch(
                        attr_name,
                        expected_type.name().to_string(),
                        value.scalar_type().name().to_string(),
                    ));
                }
                values_map.insert(attr_name, value);
            } else {
                return Err(TupleError::AttributeNotFound(attr_name));
            }
        }

        // Verify all attributes have values. Optimize by checking length first.
        if values_map.len() != tuple_type.degree() {
            for attr_name in tuple_type.attribute_names() {
                if !values_map.contains_key(attr_name) {
                    return Err(TupleError::MissingValue(attr_name.clone()));
                }
            }
        }

        Ok(Self {
            tuple_type,
            values: values_map,
        })
    }

    /// Creates a tuple skipping all validation checks.
    ///
    /// # Safety
    ///
    /// This function is safe in terms of memory safety (no undefined behavior),
    /// but the resulting `Tuple` may violate relational integrity if:
    /// - Attributes are missing
    /// - Extra attributes are present
    /// - Types do not match the `tuple_type`
    ///
    /// The caller must ensure that `values` perfectly matches `tuple_type`.
    pub(crate) fn new_unchecked(
        tuple_type: Arc<TupleType>,
        values: BTreeMap<String, ScalarValue>,
    ) -> Self {
        Self { tuple_type, values }
    }

    /// Get the tuple type
    /// # Examples
    ///
    /// ```
    /// use relvar_core::tuple;
    ///
    /// let t = tuple!{ id: 1i64 };
    /// assert_eq!(t.tuple_type().degree(), 1);
    /// ```
    pub fn tuple_type(&self) -> &TupleType {
        &self.tuple_type
    }

    /// Get a value by attribute name
    /// # Examples
    ///
    /// ```
    /// use relvar_core::tuple;
    /// use relvar_core::values::ScalarValue;
    ///
    /// let t = tuple!{ id: 1i64 };
    /// assert_eq!(t.get("id"), Some(&ScalarValue::Int(1)));
    /// assert_eq!(t.get("name"), None);
    /// ```
    pub fn get(&self, attr_name: &str) -> Option<&ScalarValue> {
        self.values.get(attr_name)
    }

    /// Get a typed value by attribute name.
    /// Extracts the value by reference to avoid allocating or cloning the `ScalarValue` enum wrapper.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::tuple;
    ///
    /// let t = tuple!{ id: 1i64, active: true };
    /// assert_eq!(t.get_typed::<i64>("id"), Some(1));
    /// assert_eq!(t.get_typed::<bool>("active"), Some(true));
    /// ```
    pub fn get_typed<T>(&self, attr_name: &str) -> Option<T>
    where
        T: for<'a> TryFrom<&'a ScalarValue>,
    {
        self.get(attr_name).and_then(|v| T::try_from(v).ok())
    }

    /// Get all attribute names
    /// # Examples
    ///
    /// ```
    /// use relvar_core::tuple;
    ///
    /// let t = tuple!{ a: 1i64, b: 2i64 };
    /// let names: Vec<_> = t.attribute_names().collect();
    /// assert_eq!(names, vec!["a", "b"]);
    /// ```
    pub fn attribute_names(&self) -> impl Iterator<Item = &String> {
        self.values.keys()
    }

    /// Get all values
    /// # Examples
    ///
    /// ```
    /// use relvar_core::tuple;
    /// use relvar_core::values::ScalarValue;
    ///
    /// let t = tuple!{ id: 1i64 };
    /// let map = t.values();
    /// assert_eq!(map.len(), 1);
    /// assert_eq!(map.get("id"), Some(&ScalarValue::Int(1)));
    /// ```
    pub fn values(&self) -> &BTreeMap<String, ScalarValue> {
        &self.values
    }

    /// Consumes the tuple and returns its values
    /// # Examples
    ///
    /// ```
    /// use relvar_core::tuple;
    /// use relvar_core::values::ScalarValue;
    ///
    /// let t = tuple!{ id: 1i64 };
    /// let map = t.into_values();
    /// assert_eq!(map.len(), 1);
    /// assert_eq!(map.get("id"), Some(&ScalarValue::Int(1)));
    /// ```
    pub fn into_values(self) -> BTreeMap<String, ScalarValue> {
        self.values
    }

    /// Get the degree (number of attributes)
    /// # Examples
    ///
    /// ```
    /// use relvar_core::tuple;
    ///
    /// let t = tuple!{ a: 1i64, b: 2i64 };
    /// assert_eq!(t.degree(), 2);
    /// ```
    pub fn degree(&self) -> usize {
        self.values.len()
    }

    /// Check if this tuple conforms to a given tuple type
    /// # Examples
    ///
    /// ```
    /// use relvar_core::tuple;
    /// use relvar_core::types::{TupleType, ScalarType};
    ///
    /// let t = tuple!{ id: 1i64 };
    ///
    /// let correct_heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// assert!(t.conforms_to(&correct_heading));
    ///
    /// let wrong_heading = TupleType::new().with_attribute("id", ScalarType::String);
    /// assert!(!t.conforms_to(&wrong_heading));
    /// ```
    pub fn conforms_to(&self, tuple_type: &TupleType) -> bool {
        // Check that all required attributes are present
        for attr_name in tuple_type.attribute_names() {
            if !self.values.contains_key(attr_name) {
                return false;
            }
        }

        // Check that all values match their types
        for (attr_name, value) in &self.values {
            if let Some(expected_type) = tuple_type.get_attribute_type(attr_name) {
                if !value.is_type(expected_type) {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }

    /// Set a value for an attribute (mutable)
    /// # Examples
    ///
    /// ```
    /// use relvar_core::tuple;
    /// use relvar_core::values::ScalarValue;
    ///
    /// let mut t = tuple!{ id: 1i64 };
    /// t.set("id".to_string(), ScalarValue::Int(2)).unwrap();
    ///
    /// assert_eq!(t.get_typed::<i64>("id"), Some(2));
    /// ```
    pub fn set(&mut self, attr_name: String, value: ScalarValue) -> Result<(), TupleError> {
        // Check if attribute exists in tuple type
        let expected_type = self
            .tuple_type
            .get_attribute_type(&attr_name)
            .ok_or_else(|| TupleError::AttributeNotFound(attr_name.clone()))?;

        // Check type matches
        if !value.is_type(expected_type) {
            return Err(TupleError::TypeMismatch(
                attr_name.clone(),
                expected_type.name().to_string(),
                value.scalar_type().name().to_string(),
            ));
        }

        self.values.insert(attr_name, value);
        Ok(())
    }
}

impl std::hash::Hash for Tuple {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the tuple type first
        self.tuple_type.hash(state);

        // Hash the count
        self.values.len().hash(state);

        // Since BTreeMap iterates in sorted order, we can hash directly
        // without collecting and sorting first.
        for (name, value) in &self.values {
            name.hash(state);
            value.hash(state);
        }
    }
}

/// Enables zero-copy extraction of string attributes from a Tuple.
/// This prevents unnecessary heap allocations when borrowing as `&str` is sufficient.
impl<'a> TryFrom<&'a ScalarValue> for &'a str {
    type Error = ();
    fn try_from(value: &'a ScalarValue) -> Result<Self, Self::Error> {
        match value {
            ScalarValue::String(v) => Ok(v.as_str()),
            _ => Err(()),
        }
    }
}

/// Helper macro for creating tuples
#[macro_export]
macro_rules! tuple {
    ($($name:ident: $value:expr),* $(,)?) => {{
        use std::collections::HashMap;
        let mut type_builder = $crate::types::TupleType::new();
        let mut values = HashMap::new();

        $(
            let value = $crate::values::ScalarValue::from($value);
            type_builder = type_builder.with_attribute(stringify!($name), value.scalar_type());
            values.insert(stringify!($name).to_string(), value);
        )*

        $crate::values::Tuple::new(type_builder, values).unwrap()
    }};
}

// Implement From for common types to ScalarValue
impl From<i64> for ScalarValue {
    fn from(v: i64) -> Self {
        ScalarValue::Int(v)
    }
}

impl From<f64> for ScalarValue {
    fn from(v: f64) -> Self {
        ScalarValue::Float(v)
    }
}

impl From<String> for ScalarValue {
    fn from(v: String) -> Self {
        ScalarValue::String(v)
    }
}

impl From<&str> for ScalarValue {
    fn from(v: &str) -> Self {
        ScalarValue::String(v.to_string())
    }
}

impl From<bool> for ScalarValue {
    fn from(v: bool) -> Self {
        ScalarValue::Bool(v)
    }
}

impl From<Vec<u8>> for ScalarValue {
    fn from(v: Vec<u8>) -> Self {
        ScalarValue::Bytes(v)
    }
}

// Implement TryFrom for extracting typed values
impl TryFrom<ScalarValue> for i64 {
    type Error = ();
    fn try_from(value: ScalarValue) -> Result<Self, Self::Error> {
        match value {
            ScalarValue::Int(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryFrom<ScalarValue> for f64 {
    type Error = ();
    fn try_from(value: ScalarValue) -> Result<Self, Self::Error> {
        match value {
            ScalarValue::Float(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryFrom<ScalarValue> for String {
    type Error = ();
    fn try_from(mut value: ScalarValue) -> Result<Self, Self::Error> {
        match value {
            ScalarValue::String(ref mut v) => Ok(std::mem::take(v)),
            _ => Err(()),
        }
    }
}

impl TryFrom<ScalarValue> for bool {
    type Error = ();
    fn try_from(value: ScalarValue) -> Result<Self, Self::Error> {
        match value {
            ScalarValue::Bool(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryFrom<ScalarValue> for Vec<u8> {
    type Error = ();
    fn try_from(mut value: ScalarValue) -> Result<Self, Self::Error> {
        match value {
            ScalarValue::Bytes(ref mut v) => Ok(std::mem::take(v)),
            _ => Err(()),
        }
    }
}

impl<'a> TryFrom<&'a ScalarValue> for i64 {
    type Error = ();
    fn try_from(value: &'a ScalarValue) -> Result<Self, Self::Error> {
        match value {
            ScalarValue::Int(v) => Ok(*v),
            _ => Err(()),
        }
    }
}

impl<'a> TryFrom<&'a ScalarValue> for f64 {
    type Error = ();
    fn try_from(value: &'a ScalarValue) -> Result<Self, Self::Error> {
        match value {
            ScalarValue::Float(v) => Ok(*v),
            _ => Err(()),
        }
    }
}

impl<'a> TryFrom<&'a ScalarValue> for String {
    type Error = ();
    fn try_from(value: &'a ScalarValue) -> Result<Self, Self::Error> {
        match value {
            ScalarValue::String(v) => Ok(v.clone()),
            _ => Err(()),
        }
    }
}

impl<'a> TryFrom<&'a ScalarValue> for bool {
    type Error = ();
    fn try_from(value: &'a ScalarValue) -> Result<Self, Self::Error> {
        match value {
            ScalarValue::Bool(v) => Ok(*v),
            _ => Err(()),
        }
    }
}

impl<'a> TryFrom<&'a ScalarValue> for Vec<u8> {
    type Error = ();
    fn try_from(value: &'a ScalarValue) -> Result<Self, Self::Error> {
        match value {
            ScalarValue::Bytes(v) => Ok(v.clone()),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests;
