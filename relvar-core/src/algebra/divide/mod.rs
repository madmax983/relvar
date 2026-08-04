//! The relational division (÷) operator.
//!
//! This module implements the `Divide` algebraic operation. Relational division is analogous
//! to integer division but operates on sets. It is commonly used to answer queries involving
//! "for all" conditions (e.g., "Find all suppliers who supply *all* parts").
//!
//! # Examples
//!
//! ```
//! use relvar_core::types::{RelationType, TupleType, ScalarType};
//! use relvar_core::values::{Relation, Tuple};
//! use relvar_core::tuple;
//!
//! // "Completed Tasks" - employee_id, task_id
//! let dividend_heading = TupleType::new()
//!     .with_attribute("employee_id", ScalarType::Int)
//!     .with_attribute("task_id", ScalarType::String);
//!
//! let mut completed = Relation::new(RelationType::new(dividend_heading.clone()));
//! completed.insert(Tuple::new(dividend_heading.clone(), vec![("employee_id".to_string(), relvar_core::values::ScalarValue::Int(1)), ("task_id".to_string(), relvar_core::values::ScalarValue::String("A".to_string()))]).unwrap()).unwrap();
//! completed.insert(Tuple::new(dividend_heading.clone(), vec![("employee_id".to_string(), relvar_core::values::ScalarValue::Int(1)), ("task_id".to_string(), relvar_core::values::ScalarValue::String("B".to_string()))]).unwrap()).unwrap();
//! completed.insert(Tuple::new(dividend_heading.clone(), vec![("employee_id".to_string(), relvar_core::values::ScalarValue::Int(2)), ("task_id".to_string(), relvar_core::values::ScalarValue::String("A".to_string()))]).unwrap()).unwrap();
//!
//! // "Required Tasks" - task_id
//! let divisor_heading = TupleType::new()
//!     .with_attribute("task_id", ScalarType::String);
//!
//! let mut required = Relation::new(RelationType::new(divisor_heading.clone()));
//! required.insert(Tuple::new(divisor_heading.clone(), vec![("task_id".to_string(), relvar_core::values::ScalarValue::String("A".to_string()))]).unwrap()).unwrap();
//! required.insert(Tuple::new(divisor_heading.clone(), vec![("task_id".to_string(), relvar_core::values::ScalarValue::String("B".to_string()))]).unwrap()).unwrap();
//!
//! // Who completed ALL required tasks?
//! let result = completed.divide(&required).unwrap();
//!
//! // Only employee 1 completed both A and B
//! assert_eq!(result.cardinality(), 1);
//! assert!(result.contains(&Tuple::new(result.relation_type().tuple_type().clone(), vec![("employee_id".to_string(), relvar_core::values::ScalarValue::Int(1))]).unwrap()));
//! ```
use crate::values::Relation;
use thiserror::Error;

/// Errors that can occur during relational division.
#[derive(Debug, Error)]
pub enum DivideError {
    /// An attribute in the divisor is not found in the dividend.
    #[error("Divisor attribute '{0}' not found in dividend")]
    MissingAttribute(String),

    /// Type mismatch for a common attribute.
    #[error("Type mismatch for attribute '{0}'")]
    TypeMismatch(String),

    /// The divisor heading equals the dividend heading (no remainder attributes).
    #[error("Divisor heading cannot equal dividend heading (no remainder attributes)")]
    EmptyRemainder,
}

impl Relation {
    /// Relational division operator (÷)
    ///
    /// TTM: RM Prescription 7 - Relational algebra completeness (8/8 operators).
    ///
    /// Derives all tuples from the dividend's remainder attributes such that
    /// for every tuple in the divisor, the extended tuple exists in the dividend.
    ///
    /// Mathematical definition: R1 DIVIDEBY R2 returns all tuples t from
    /// (R1.attributes - R2.attributes) such that for all s in R2, (t ∪ s) ∈ R1.
    ///
    /// Result heading: R1.attributes - R2.attributes (set difference)
    ///
    /// # Edge Case: Empty Divisor
    ///
    /// If the divisor (R2) is empty, the result contains **all unique combinations** of the
    /// remainder attributes found in the dividend (R1).
    ///
    /// *Reasoning:* The condition "for all s in R2, (t ∪ s) ∈ R1" is trivially true because
    /// there are no `s` in R2. Thus, every candidate `t` (from the projection of R1 onto
    /// the remainder attributes) satisfies the condition.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// // SUPPLIES(supplier_id, part_id)
    /// let supplies_heading = TupleType::new()
    ///     .with_attribute("supplier_id", ScalarType::String)
    ///     .with_attribute("part_id", ScalarType::String);
    /// let mut supplies = Relation::new(RelationType::new(supplies_heading));
    ///
    /// // S1 supplies P1, P2
    /// supplies.insert(tuple! { supplier_id: "S1", part_id: "P1" }).unwrap();
    /// supplies.insert(tuple! { supplier_id: "S1", part_id: "P2" }).unwrap();
    /// // S2 supplies only P1
    /// supplies.insert(tuple! { supplier_id: "S2", part_id: "P1" }).unwrap();
    /// // S3 supplies P1, P2
    /// supplies.insert(tuple! { supplier_id: "S3", part_id: "P1" }).unwrap();
    /// supplies.insert(tuple! { supplier_id: "S3", part_id: "P2" }).unwrap();
    ///
    /// // PARTS(part_id): P1, P2
    /// let parts_heading = TupleType::new()
    ///     .with_attribute("part_id", ScalarType::String);
    /// let mut parts = Relation::new(RelationType::new(parts_heading));
    /// parts.insert(tuple! { part_id: "P1" }).unwrap();
    /// parts.insert(tuple! { part_id: "P2" }).unwrap();
    ///
    /// // Find suppliers who supply ALL parts
    /// let result = supplies.divide(&parts).unwrap();
    ///
    /// // Result: {S1, S3}
    /// assert_eq!(result.cardinality(), 2);
    /// assert!(result.contains(&tuple! { supplier_id: "S1" }));
    /// assert!(result.contains(&tuple! { supplier_id: "S3" }));
    /// assert!(!result.contains(&tuple! { supplier_id: "S2" }));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `Err` if:
    /// - Divisor has attributes not in dividend
    /// - Attribute types don't match
    /// - Divisor heading equals dividend heading (no remainder)
    pub fn divide(&self, divisor: &Relation) -> Result<Relation, DivideError> {
        let dividend_heading = self.relation_type().heading();
        let divisor_heading = divisor.relation_type().heading();

        // Validate divisor is compatible with dividend
        validate_division_compatibility(dividend_heading, divisor_heading)?;

        // Compute remainder attributes (dividend - divisor)
        let remainder_attrs = compute_remainder_attributes(dividend_heading, divisor_heading)?;

        // If divisor is empty, return projection onto remainder
        if divisor.is_empty() {
            return Ok(project_onto_attrs(self, &remainder_attrs));
        }

        // Project dividend onto remainder to get candidate tuples
        let candidates = project_onto_attrs(self, &remainder_attrs);

        // Filter candidates: keep only those where ALL divisor tuples
        // have a matching extended tuple in the dividend
        let result = filter_matching_candidates(candidates, divisor, self, dividend_heading);

        Ok(result)
    }
}

/// Validate that divisor attributes are a proper subset of dividend attributes
/// and that types match.
fn validate_division_compatibility(
    dividend_heading: &crate::types::TupleType,
    divisor_heading: &crate::types::TupleType,
) -> Result<(), DivideError> {
    for attr_name in divisor_heading.attribute_names() {
        // Check if attribute exists in dividend
        match dividend_heading.get_attribute_type(attr_name) {
            None => {
                return Err(DivideError::MissingAttribute(attr_name.clone()));
            }
            Some(dividend_type) => {
                // Check if types match
                let divisor_type = divisor_heading.get_attribute_type(attr_name).unwrap();
                if dividend_type != divisor_type {
                    return Err(DivideError::TypeMismatch(attr_name.clone()));
                }
            }
        }
    }
    Ok(())
}

/// Compute remainder attributes (dividend - divisor).
/// Returns error if remainder is empty.
fn compute_remainder_attributes<'a>(
    dividend_heading: &'a crate::types::TupleType,
    divisor_heading: &crate::types::TupleType,
) -> Result<Vec<&'a str>, DivideError> {
    // Optimization: Collect &str references directly from the dividend heading
    // instead of cloning Strings. This avoids an intermediate Vec<String>
    // allocation and cloning operation before projecting.
    let remainder_attrs: Vec<&'a str> = dividend_heading
        .attribute_names()
        .filter(|attr| !divisor_heading.has_attribute(attr))
        .map(|s| s.as_str())
        .collect();

    if remainder_attrs.is_empty() {
        return Err(DivideError::EmptyRemainder);
    }

    Ok(remainder_attrs)
}

/// Project relation onto specified attributes.
fn project_onto_attrs(relation: &Relation, attrs: &[&str]) -> Relation {
    relation.project(attrs)
}

/// Filter candidate tuples, keeping only those where ALL divisor tuples
/// can be found when extended with the candidate.
///
/// **Optimization Details**: This function consumes the `candidates` relation
/// and uses `.restrict_into(...)` to filter the set of tuples in-place.
/// Because `candidates` was constructed as an intermediate projection just prior
/// to this step, owning it allows us to utilize the internal `HashSet::retain()`
/// mechanism. This acts as a zero-cost abstraction, completely bypassing
/// the intermediate `.collect::<Vec<_>>()` heap allocation that would otherwise
/// occur during the filtering pipeline.
fn filter_matching_candidates(
    candidates: Relation,
    divisor: &Relation,
    dividend: &Relation,
    dividend_heading: &crate::types::TupleType,
) -> Relation {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    // Clone dividend_heading once outside the loop to avoid repeated clones
    let dividend_heading_arc = Arc::new(dividend_heading.clone());

    candidates.restrict_into(|candidate| {
        // Check if ALL divisor tuples match when extended with this candidate
        divisor.tuples().all(|divisor_tuple| {
            // Extend candidate with divisor tuple using iterator-based approach
            let extended_values: BTreeMap<_, _> = candidate
                .values()
                .iter()
                .chain(divisor_tuple.values())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            // Create extended tuple
            let extended_tuple =
                crate::values::Tuple::new_unchecked(dividend_heading_arc.clone(), extended_values);

            // Check if this extended tuple exists in the dividend
            dividend.contains(&extended_tuple)
        })
    })
}

#[cfg(test)]
mod tests;
