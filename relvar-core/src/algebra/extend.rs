//! Extend operator for adding computed attributes.
//!
//! The extend operator adds a new computed attribute to each tuple in a relation.
//! The new attribute's value is computed from the existing attribute values
//! using a user-provided function.
//!
//! # TTM Compliance
//!
//! - Result is a valid relation with an extended heading
//! - New attribute must not conflict with existing attribute names
//! - Set semantics are maintained
//!
//! # Example
//!
//! ```
//! use relvar_core::types::{TupleType, RelationType, ScalarType};
//! use relvar_core::values::{Relation, ScalarValue};
//! use relvar_core::tuple;
//!
//! let heading = TupleType::new()
//!     .with_attribute("price", ScalarType::Int)
//!     .with_attribute("quantity", ScalarType::Int);
//!
//! let mut relation = Relation::new(RelationType::new(heading));
//! relation.insert(tuple! { price: 10i64, quantity: 5i64 }).unwrap();
//!
//! // Add a computed "total" attribute
//! let result = relation.extend("total", ScalarType::Int, |t| {
//!     let price = t.get_typed::<i64>("price").unwrap();
//!     let quantity = t.get_typed::<i64>("quantity").unwrap();
//!     ScalarValue::Int(price * quantity)
//! }).unwrap();
//!
//! assert_eq!(result.degree(), 3);  // price, quantity, total
//! ```

use crate::values::{Relation, ScalarValue, Tuple};
use thiserror::Error;

/// Errors that can occur during extend operations.
#[derive(Debug, Error)]
pub enum ExtendError {
    /// The new attribute name conflicts with an existing attribute.
    ///
    /// Each attribute in a relation must have a unique name. Use rename
    /// first if you need to replace an existing attribute.
    #[error("Attribute '{0}' already exists in relation")]
    AttributeExists(String),

    /// Failed to create a tuple with the extended attributes.
    ///
    /// This can occur if the computed value type doesn't match the
    /// declared result type.
    #[error("Failed to create extended tuple: {0}")]
    TupleCreation(String),
}

impl Relation {
    /// Extends the relation with a new computed attribute.
    ///
    /// This operator adds a new attribute to each tuple, where the value
    /// is computed from the tuple's existing attribute values using the
    /// provided function.
    ///
    /// # Arguments
    ///
    /// * `attr_name` - The name for the new attribute (must not already exist)
    /// * `attr_type` - The scalar type of the new attribute
    /// * `compute` - A function that computes the new attribute value from
    ///   each tuple
    ///
    /// # Returns
    ///
    /// A new relation with the additional computed attribute.
    ///
    /// # Errors
    ///
    /// Returns [`ExtendError::AttributeExists`] if an attribute with the
    /// given name already exists in the relation.
    pub fn extend<F>(
        &self,
        attr_name: &str,
        attr_type: crate::types::ScalarType,
        compute: F,
    ) -> Result<Relation, ExtendError>
    where
        F: Fn(&Tuple) -> ScalarValue,
    {
        // Check if attribute already exists
        if self.relation_type().has_attribute(attr_name) {
            return Err(ExtendError::AttributeExists(attr_name.to_string()));
        }

        // Create new heading with the additional attribute
        let mut new_heading = self.relation_type().tuple_type().clone();
        new_heading = new_heading.with_attribute(attr_name.to_string(), attr_type);

        let new_rel_type = crate::types::RelationType::new(new_heading.clone());
        let new_heading_arc = std::sync::Arc::new(new_heading);

        // Create extended tuples
        let mut extended_tuples = Vec::new();
        for tuple in self.tuples() {
            let mut new_values = tuple.values().clone();
            let computed_value = compute(tuple);
            new_values.insert(attr_name.to_string(), computed_value);

            let extended_tuple = Tuple::new(new_heading_arc.clone(), new_values)
                .map_err(|e| ExtendError::TupleCreation(e.to_string()))?;
            extended_tuples.push(extended_tuple);
        }

        Ok(Relation::from_tuples(new_rel_type, extended_tuples)
            .expect("Extended tuples should conform to new relation type"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_extend_adds_computed_attribute() {
        let heading = TupleType::new()
            .with_attribute("price".to_string(), ScalarType::Int)
            .with_attribute("quantity".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { price: 10i64, quantity: 5i64 })
            .unwrap();
        relation
            .insert(tuple! { price: 20i64, quantity: 3i64 })
            .unwrap();

        // Extend with total = price * quantity
        let result = relation
            .extend("total", ScalarType::Int, |t| {
                let price = t.get_typed::<i64>("price").unwrap();
                let quantity = t.get_typed::<i64>("quantity").unwrap();
                ScalarValue::Int(price * quantity)
            })
            .unwrap();

        assert_eq!(result.cardinality(), 2);
        assert_eq!(result.degree(), 3);
        assert!(result.relation_type().has_attribute("total"));

        // Check computed values
        for tuple in result.tuples() {
            let price = tuple.get_typed::<i64>("price").unwrap();
            let quantity = tuple.get_typed::<i64>("quantity").unwrap();
            let total = tuple.get_typed::<i64>("total").unwrap();
            assert_eq!(total, price * quantity);
        }
    }

    #[test]
    fn test_extend_fails_if_attribute_exists() {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice" })
            .unwrap();

        // Try to extend with an attribute that already exists
        let result = relation.extend("name", ScalarType::String, |t| {
            t.get("name").unwrap().clone()
        });

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ExtendError::AttributeExists(_)
        ));
    }

    #[test]
    fn test_extend_on_empty_relation() {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("salary".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let relation = Relation::new(rel_type);

        let result = relation
            .extend("bonus", ScalarType::Int, |t| {
                let salary = t.get_typed::<i64>("salary").unwrap();
                ScalarValue::Int(salary / 10)
            })
            .unwrap();

        assert_eq!(result.cardinality(), 0);
        assert_eq!(result.degree(), 3);
        assert!(result.relation_type().has_attribute("bonus"));
    }

    #[test]
    fn test_extend_with_string_concatenation() {
        let heading = TupleType::new()
            .with_attribute("first_name".to_string(), ScalarType::String)
            .with_attribute("last_name".to_string(), ScalarType::String);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { first_name: "Alice", last_name: "Smith" })
            .unwrap();
        relation
            .insert(tuple! { first_name: "Bob", last_name: "Jones" })
            .unwrap();

        let result = relation
            .extend("full_name", ScalarType::String, |t| {
                let first = t.get_typed::<String>("first_name").unwrap();
                let last = t.get_typed::<String>("last_name").unwrap();
                ScalarValue::String(format!("{} {}", first, last))
            })
            .unwrap();

        assert_eq!(result.cardinality(), 2);
        assert_eq!(result.degree(), 3);

        for tuple in result.tuples() {
            let first = tuple.get_typed::<String>("first_name").unwrap();
            let last = tuple.get_typed::<String>("last_name").unwrap();
            let full = tuple.get_typed::<String>("full_name").unwrap();
            assert_eq!(full, format!("{} {}", first, last));
        }
    }
}
