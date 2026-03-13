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
    /// is computed from the tuple's existing attribute values using a user-provided
    /// function.
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
        new_heading = new_heading.with_attribute(attr_name.to_string(), attr_type.clone());

        let new_rel_type = crate::types::RelationType::new(new_heading.clone());
        let new_heading_arc = std::sync::Arc::new(new_heading);

        // Create extended tuples
        //
        // Performance: We avoid collecting into an intermediate Vec<Tuple> by folding directly
        // into a Relation via `from_tuples_unchecked`. We explicitly constructed each tuple to
        // conform to `new_heading_arc` and verified the computed attribute's type. This bypasses
        // the O(N) validation overhead per tuple inside `from_tuples` and eliminates the intermediate heap allocation.
        let mut body = std::collections::HashSet::with_capacity(self.cardinality());
        for tuple in self.tuples() {
            let extended_tuple =
                create_extended_tuple(tuple, attr_name, &attr_type, &new_heading_arc, &compute)?;
            body.insert(extended_tuple);
        }

        // Safety: We guarantee that extended tuples conform to new_rel_type because:
        // - new_rel_type uses new_heading
        // - Tuples are created with new_heading and the new attribute's type is verified
        Ok(Relation::from_tuples_unchecked(new_rel_type, body))
    }
}

/// Helper function to create a single extended tuple.
fn create_extended_tuple<F>(
    tuple: &Tuple,
    attr_name: &str,
    attr_type: &crate::types::ScalarType,
    new_heading: &std::sync::Arc<crate::types::TupleType>,
    compute: &F,
) -> Result<Tuple, ExtendError>
where
    F: Fn(&Tuple) -> ScalarValue,
{
    let computed_value = compute(tuple);

    // Manual type check for the new value
    // We only need to check this one value because the existing values
    // come from a valid tuple and are guaranteed to match the rest of the heading.
    if !computed_value.is_type(attr_type) {
        return Err(ExtendError::TupleCreation(format!(
            "Type mismatch for attribute '{}': expected {}, got {}",
            attr_name,
            attr_type.name(),
            computed_value.scalar_type().name()
        )));
    }

    let mut new_values = tuple.values().clone();
    new_values.insert(attr_name.to_string(), computed_value);

    // Safety: We verified the new value's type above, and existing values
    // are known to be valid because they come from a valid Tuple.
    // Using new_unchecked avoids O(N) validation per tuple where N is degree.
    Ok(Tuple::new_unchecked(new_heading.clone(), new_values))
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

    #[test]
    fn test_extend_type_mismatch_fails() {
        let heading = TupleType::new().with_attribute("emp_id".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation.insert(tuple! { emp_id: 1i64 }).unwrap();

        // Try to extend with a type mismatch
        // Expected: String, Computed: Int
        let result = relation.extend("name_length", ScalarType::String, |_| {
            // Return an Int instead of String
            ScalarValue::Int(10)
        });

        assert!(result.is_err());
        match result.unwrap_err() {
            ExtendError::TupleCreation(msg) => {
                assert!(msg.contains("Type mismatch"));
                assert!(msg.contains("expected String"));
                assert!(msg.contains("got Int"));
            }
            _ => panic!("Expected TupleCreation error"),
        }
    }
}
