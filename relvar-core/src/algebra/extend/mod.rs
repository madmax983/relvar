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

/// The boundaries of extending knowledge.
///
/// The `extend` operator breathes new life into a relation by generating new attributes
/// from existing data. However, the universe has rules.
///
/// # Recovery
/// - **AttributeExists:** You cannot overwrite an existing attribute. Relational attributes are immutable facts. If you want to replace an attribute, you must first `rename` the old one or `project` it away.
/// - **TupleCreation:** The closure you provided returned a `ScalarValue` that doesn't match the `ScalarType` you promised in the `extend` signature. Ensure your types align.
///
/// # Examples
///
/// ```
/// use relvar_core::algebra::ExtendError;
///
/// // The user attempted to create a new attribute that collides with an existing one.
/// // To recover, they should either pick a new name, or `project` away the old attribute.
/// let error = ExtendError::AttributeExists("id".to_string());
/// assert!(error.to_string().contains("Attribute 'id' already exists"));
/// ```
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
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::{Relation, ScalarValue};
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    /// let mut rel = Relation::new(rel_type);
    /// rel.insert(tuple! { id: 1i64 }).unwrap();
    ///
    /// let extended = rel.extend("double_id", ScalarType::Int, |t| {
    ///     ScalarValue::Int(t.get_typed::<i64>("id").unwrap() * 2)
    /// }).unwrap();
    /// assert_eq!(extended.cardinality(), 1);
    /// ```
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
        // # Performance
        // We know exactly how many tuples will be produced because `extend` computes
        // exactly one new tuple for each input tuple. Pre-allocating the HashSet
        // avoids dynamic heap reallocations when collecting the results.
        // We iterate directly into a pre-sized HashSet and construct the relation
        // using `from_body_unchecked` to avoid any redundant internal reallocation or
        // O(N) validation overhead.
        let mut extended_tuples = std::collections::HashSet::with_capacity(self.cardinality());
        for tuple in self.tuples() {
            extended_tuples.insert(create_extended_tuple(
                tuple,
                attr_name,
                &attr_type,
                &new_heading_arc,
                &compute,
            )?);
        }

        // Safety:
        // We guarantee that extended_tuples conform to new_rel_type because:
        // - new_rel_type uses new_heading
        // - Tuples are created with new_heading in create_extended_tuple
        // - create_extended_tuple ensures the computed value type matches
        Ok(Relation::from_body_unchecked(new_rel_type, extended_tuples))
    }

    /// Computes the extension of this relation, consuming it.
    ///
    /// This is an optimized version of `extend` that avoids O(N) tuple clones for the
    /// relation by consuming it.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::{Relation, ScalarValue};
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    /// let mut rel = Relation::new(rel_type);
    /// rel.insert(tuple! { id: 1i64 }).unwrap();
    ///
    /// // `extend_into` consumes `rel`, taking ownership of its tuples
    /// // instead of cloning them.
    /// let extended = rel.extend_into("double_id", ScalarType::Int, |t| {
    ///     ScalarValue::Int(t.get_typed::<i64>("id").unwrap() * 2)
    /// }).unwrap();
    /// assert_eq!(extended.cardinality(), 1);
    /// ```
    pub fn extend_into<F>(
        self,
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

        let mut extended_tuples = std::collections::HashSet::with_capacity(self.cardinality());

        // Take ownership of the tuples from self
        for tuple in self.into_iter() {
            extended_tuples.insert(create_extended_tuple_owned(
                tuple,
                attr_name,
                &attr_type,
                &new_heading_arc,
                &compute,
            )?);
        }

        Ok(Relation::from_body_unchecked(new_rel_type, extended_tuples))
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

/// Helper function to create a single extended tuple by taking ownership of the source tuple.
fn create_extended_tuple_owned<F>(
    tuple: Tuple,
    attr_name: &str,
    attr_type: &crate::types::ScalarType,
    new_heading: &std::sync::Arc<crate::types::TupleType>,
    compute: &F,
) -> Result<Tuple, ExtendError>
where
    F: Fn(&Tuple) -> ScalarValue,
{
    let computed_value = compute(&tuple);

    if !computed_value.is_type(attr_type) {
        return Err(ExtendError::TupleCreation(format!(
            "Type mismatch for attribute '{}': expected {}, got {}",
            attr_name,
            attr_type.name(),
            computed_value.scalar_type().name()
        )));
    }

    let mut new_values = tuple.into_values();
    new_values.insert(attr_name.to_string(), computed_value);

    Ok(Tuple::new_unchecked(new_heading.clone(), new_values))
}

#[cfg(test)]
mod tests;
