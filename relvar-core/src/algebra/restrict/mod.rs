//! Restrict operator for tuple selection.
//!
//! The restrict operator (σ in relational algebra, commonly called "selection" or "WHERE"
//! in SQL) filters tuples from a relation based on a predicate function.
//!
//! # TTM Compliance
//!
//! - Result maintains set semantics (no duplicate tuples)
//! - Result is a valid relation with the same heading as the input
//! - Predicate operates on tuple values only, not on physical identifiers
//!
//! # Example
//!
//! ```
//! use relvar_core::types::{TupleType, RelationType, ScalarType};
//! use relvar_core::values::{Relation, ScalarValue};
//! use relvar_core::tuple;
//!
//! let heading = TupleType::new()
//!     .with_attribute("emp_id", ScalarType::Int)
//!     .with_attribute("name", ScalarType::String)
//!     .with_attribute("dept_id", ScalarType::Int);
//!
//! let mut relation = Relation::new(RelationType::new(heading));
//! relation.insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 }).unwrap();
//! relation.insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 }).unwrap();
//!
//! // Restrict to employees in department 10
//! let dept10 = relation.restrict(|t| {
//!     t.get_typed::<i64>("dept_id").unwrap() == 10
//! });
//! assert_eq!(dept10.cardinality(), 1);
//! ```

use crate::values::{Relation, Tuple};

impl Relation {
    /// Restricts this relation by filtering tuples with a predicate.
    ///
    /// This is the relational algebra σ (sigma) operator, commonly known as
    /// "selection" in relational algebra or "WHERE" in SQL. It produces a new
    /// relation containing only the tuples that satisfy the given predicate.
    ///
    /// # Arguments
    ///
    /// * `predicate` - A function that takes a tuple reference and returns `true`
    ///   if the tuple should be included in the result
    ///
    /// # Returns
    ///
    /// A new relation with the same heading, containing only tuples for which
    /// the predicate returns `true`.
    ///
    /// # Behavior
    ///
    /// - The result has the same heading (type) as the input relation
    /// - Tuples for which the predicate returns `false` are excluded
    /// - The predicate is evaluated once per tuple
    /// - An empty predicate result yields an empty relation
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::{Relation, ScalarValue};
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("emp_id", ScalarType::Int)
    ///     .with_attribute("salary", ScalarType::Int);
    ///
    /// let mut relation = Relation::new(RelationType::new(heading));
    /// relation.insert(tuple! { emp_id: 1i64, salary: 50000i64 }).unwrap();
    /// relation.insert(tuple! { emp_id: 2i64, salary: 75000i64 }).unwrap();
    ///
    /// // Find employees with salary > 60000
    /// let high_earners = relation.restrict(|t| {
    ///     t.get_typed::<i64>("salary").unwrap() > 60000
    /// });
    /// assert_eq!(high_earners.cardinality(), 1);
    /// ```
    pub fn restrict<F>(&self, predicate: F) -> Self
    where
        F: Fn(&Tuple) -> bool,
    {
        // Filter tuples and create relation without redundant checks
        // Safety: source tuples are from a valid relation
        Relation::from_tuples_unchecked(
            self.relation_type().clone(),
            self.tuples().filter(|tuple| predicate(tuple)).cloned(),
        )
    }
}

#[cfg(test)]
mod tests;
