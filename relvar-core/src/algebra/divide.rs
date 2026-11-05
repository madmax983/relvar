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
use crate::algebra::ProjectError;
use crate::values::Relation;
use alloc::string::String;
use alloc::vec::Vec;
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
    /// # Edge Case: Divisor Over the Whole Heading
    ///
    /// If the divisor's heading equals the dividend's, there are no remainder
    /// attributes (A = {}) and the result is `TABLE_DEE` when every divisor
    /// tuple is in the dividend, else `TABLE_DUM` (RM Proscription 5: the
    /// degree-zero case is handled, not rejected).
    ///
    /// This is Date's Small Divide, `r1{A} MINUS ((r1{A} JOIN r2) MINUS r1){A}`.
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
    pub fn divide(&self, divisor: &Relation) -> Result<Relation, DivideError> {
        let dividend_heading = self.relation_type().heading();
        let divisor_heading = divisor.relation_type().heading();

        // Validate divisor is compatible with dividend
        validate_division_compatibility(dividend_heading, divisor_heading)?;

        // Compute remainder attributes (dividend - divisor)
        let remainder_attrs = compute_remainder_attributes(dividend_heading, divisor_heading);

        // If divisor is empty, return projection onto remainder
        if divisor.is_empty() {
            return project_onto_attrs(self, &remainder_attrs);
        }

        // Project dividend onto remainder to get candidate tuples
        let candidates = project_onto_attrs(self, &remainder_attrs)?;

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
        let dividend_type = dividend_heading
            .get_attribute_type(attr_name)
            .ok_or_else(|| DivideError::MissingAttribute(attr_name.clone()))?;
        if Some(dividend_type) != divisor_heading.get_attribute_type(attr_name) {
            return Err(DivideError::TypeMismatch(attr_name.clone()));
        }
    }
    Ok(())
}

/// Compute remainder attributes (dividend - divisor).
///
/// The remainder may be empty (divisor over the dividend's whole heading);
/// the quotient is then `TABLE_DEE` or `TABLE_DUM`.
fn compute_remainder_attributes<'a>(
    dividend_heading: &'a crate::types::TupleType,
    divisor_heading: &crate::types::TupleType,
) -> Vec<&'a str> {
    // Optimization: Collect &str references directly from the dividend heading
    // instead of cloning Strings. This avoids an intermediate Vec<String>
    // allocation and cloning operation before projecting.
    dividend_heading
        .attribute_names()
        .filter(|attr| !divisor_heading.has_attribute(attr))
        .map(|s| s.as_str())
        .collect()
}

/// Project relation onto specified attributes.
///
/// The remainder attributes are drawn from the dividend's own heading, so
/// the projection cannot fail; the error is mapped defensively.
fn project_onto_attrs(relation: &Relation, attrs: &[&str]) -> Result<Relation, DivideError> {
    relation.project(attrs).map_err(|err| match err {
        ProjectError::UnknownAttribute(name) => DivideError::MissingAttribute(name),
    })
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
    use alloc::collections::BTreeMap;
    use alloc::sync::Arc;

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
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::Relation;

    // Test 1: Basic division - S1 supplies P1,P2; S2 supplies only P1; divide by {P1,P2} returns only S1
    #[test]
    fn test_divide_basic() {
        // SUPPLIES relation: {supplier_id, part_id}
        let supplies_heading = TupleType::new()
            .with_attribute("supplier_id", ScalarType::String)
            .with_attribute("part_id", ScalarType::String);
        let supplies_type = RelationType::new(supplies_heading);
        let mut supplies = Relation::new(supplies_type);

        // S1 supplies P1 and P2
        supplies
            .insert(tuple! { supplier_id: "S1", part_id: "P1" })
            .unwrap();
        supplies
            .insert(tuple! { supplier_id: "S1", part_id: "P2" })
            .unwrap();

        // S2 supplies only P1
        supplies
            .insert(tuple! { supplier_id: "S2", part_id: "P1" })
            .unwrap();

        // S3 supplies P1 and P2
        supplies
            .insert(tuple! { supplier_id: "S3", part_id: "P1" })
            .unwrap();
        supplies
            .insert(tuple! { supplier_id: "S3", part_id: "P2" })
            .unwrap();

        // PARTS relation: {part_id} with P1 and P2
        let parts_heading = TupleType::new().with_attribute("part_id", ScalarType::String);
        let parts_type = RelationType::new(parts_heading);
        let mut parts = Relation::new(parts_type);
        parts.insert(tuple! { part_id: "P1" }).unwrap();
        parts.insert(tuple! { part_id: "P2" }).unwrap();

        // Divide: which suppliers supply ALL parts?
        let result = supplies.divide(&parts).unwrap();

        // Result should have heading {supplier_id}
        assert_eq!(result.degree(), 1);
        assert!(
            result
                .relation_type()
                .heading()
                .has_attribute("supplier_id")
        );

        // Result should contain S1 and S3 (both supply all parts)
        assert_eq!(result.cardinality(), 2);
        assert!(result.contains(&tuple! { supplier_id: "S1" }));
        assert!(result.contains(&tuple! { supplier_id: "S3" }));
        assert!(!result.contains(&tuple! { supplier_id: "S2" })); // S2 doesn't supply P2
    }

    // Test 2: Empty divisor - should return projection onto remainder attributes
    #[test]
    fn test_divide_empty_divisor() {
        let supplies_heading = TupleType::new()
            .with_attribute("supplier_id", ScalarType::String)
            .with_attribute("part_id", ScalarType::String);
        let supplies_type = RelationType::new(supplies_heading);
        let mut supplies = Relation::new(supplies_type);

        supplies
            .insert(tuple! { supplier_id: "S1", part_id: "P1" })
            .unwrap();
        supplies
            .insert(tuple! { supplier_id: "S1", part_id: "P2" })
            .unwrap();
        supplies
            .insert(tuple! { supplier_id: "S2", part_id: "P1" })
            .unwrap();

        // Empty divisor
        let parts_heading = TupleType::new().with_attribute("part_id", ScalarType::String);
        let parts_type = RelationType::new(parts_heading);
        let parts = Relation::new(parts_type);

        let result = supplies.divide(&parts).unwrap();

        // Should be equivalent to projection onto {supplier_id}
        assert_eq!(result.degree(), 1);
        assert_eq!(result.cardinality(), 2); // S1 and S2
        assert!(result.contains(&tuple! { supplier_id: "S1" }));
        assert!(result.contains(&tuple! { supplier_id: "S2" }));
    }

    // Test 3: Empty dividend - should return empty relation
    #[test]
    fn test_divide_empty_dividend() {
        let supplies_heading = TupleType::new()
            .with_attribute("supplier_id", ScalarType::String)
            .with_attribute("part_id", ScalarType::String);
        let supplies_type = RelationType::new(supplies_heading);
        let supplies = Relation::new(supplies_type); // Empty

        let parts_heading = TupleType::new().with_attribute("part_id", ScalarType::String);
        let parts_type = RelationType::new(parts_heading);
        let mut parts = Relation::new(parts_type);
        parts.insert(tuple! { part_id: "P1" }).unwrap();

        let result = supplies.divide(&parts).unwrap();

        assert_eq!(result.degree(), 1);
        assert_eq!(result.cardinality(), 0);
        assert!(result.is_empty());
    }

    // Test 4: No supplier supplies all parts - should return empty
    #[test]
    fn test_divide_no_matches() {
        let supplies_heading = TupleType::new()
            .with_attribute("supplier_id", ScalarType::String)
            .with_attribute("part_id", ScalarType::String);
        let supplies_type = RelationType::new(supplies_heading);
        let mut supplies = Relation::new(supplies_type);

        // S1 supplies only P1
        supplies
            .insert(tuple! { supplier_id: "S1", part_id: "P1" })
            .unwrap();

        // S2 supplies only P2
        supplies
            .insert(tuple! { supplier_id: "S2", part_id: "P2" })
            .unwrap();

        // Divisor has both P1 and P2
        let parts_heading = TupleType::new().with_attribute("part_id", ScalarType::String);
        let parts_type = RelationType::new(parts_heading);
        let mut parts = Relation::new(parts_type);
        parts.insert(tuple! { part_id: "P1" }).unwrap();
        parts.insert(tuple! { part_id: "P2" }).unwrap();

        let result = supplies.divide(&parts).unwrap();

        // No supplier supplies both parts
        assert_eq!(result.cardinality(), 0);
        assert!(result.is_empty());
    }

    // Test 5: All suppliers supply all parts
    #[test]
    fn test_divide_all_match() {
        let supplies_heading = TupleType::new()
            .with_attribute("supplier_id", ScalarType::String)
            .with_attribute("part_id", ScalarType::String);
        let supplies_type = RelationType::new(supplies_heading);
        let mut supplies = Relation::new(supplies_type);

        // Both S1 and S2 supply both P1 and P2
        supplies
            .insert(tuple! { supplier_id: "S1", part_id: "P1" })
            .unwrap();
        supplies
            .insert(tuple! { supplier_id: "S1", part_id: "P2" })
            .unwrap();
        supplies
            .insert(tuple! { supplier_id: "S2", part_id: "P1" })
            .unwrap();
        supplies
            .insert(tuple! { supplier_id: "S2", part_id: "P2" })
            .unwrap();

        let parts_heading = TupleType::new().with_attribute("part_id", ScalarType::String);
        let parts_type = RelationType::new(parts_heading);
        let mut parts = Relation::new(parts_type);
        parts.insert(tuple! { part_id: "P1" }).unwrap();
        parts.insert(tuple! { part_id: "P2" }).unwrap();

        let result = supplies.divide(&parts).unwrap();

        assert_eq!(result.cardinality(), 2);
        assert!(result.contains(&tuple! { supplier_id: "S1" }));
        assert!(result.contains(&tuple! { supplier_id: "S2" }));
    }

    // Test 6: Error - divisor has attribute not in dividend
    #[test]
    fn test_divide_error_missing_attribute() {
        let supplies_heading = TupleType::new()
            .with_attribute("supplier_id", ScalarType::String)
            .with_attribute("part_id", ScalarType::String);
        let supplies_type = RelationType::new(supplies_heading);
        let supplies = Relation::new(supplies_type);

        // Divisor has attribute not in dividend
        let invalid_heading = TupleType::new()
            .with_attribute("part_id", ScalarType::String)
            .with_attribute("color", ScalarType::String); // Not in supplies!
        let invalid_type = RelationType::new(invalid_heading);
        let invalid_divisor = Relation::new(invalid_type);

        let result = supplies.divide(&invalid_divisor);

        assert!(result.is_err());
        match result {
            Err(DivideError::MissingAttribute(attr)) => {
                assert_eq!(attr, "color");
            }
            _ => panic!("Expected MissingAttribute error"),
        }
    }

    // Test 7: Error - attribute type mismatch
    #[test]
    fn test_divide_error_type_mismatch() {
        let supplies_heading = TupleType::new()
            .with_attribute("supplier_id", ScalarType::String)
            .with_attribute("part_id", ScalarType::String);
        let supplies_type = RelationType::new(supplies_heading);
        let supplies = Relation::new(supplies_type);

        // Divisor has same attribute name but different type
        let mismatched_heading = TupleType::new().with_attribute("part_id", ScalarType::Int); // Should be String!
        let mismatched_type = RelationType::new(mismatched_heading);
        let mismatched_divisor = Relation::new(mismatched_type);

        let result = supplies.divide(&mismatched_divisor);

        assert!(result.is_err());
        match result {
            Err(DivideError::TypeMismatch(attr)) => {
                assert_eq!(attr, "part_id");
            }
            _ => panic!("Expected TypeMismatch error"),
        }
    }

    // Test 8: divisor heading equals dividend heading (no remainder
    // attributes). TTM: A = {} so the result is r1{} restricted by the
    // division condition: TABLE_DEE if every divisor tuple is in the
    // dividend, else TABLE_DUM.
    #[test]
    fn test_divide_divisor_with_full_heading_gives_dee_or_dum() {
        let heading = TupleType::new()
            .with_attribute("supplier_id", ScalarType::String)
            .with_attribute("part_id", ScalarType::String);
        let mut supplies = Relation::new(RelationType::new(heading.clone()));
        supplies
            .insert(tuple! { supplier_id: "S1", part_id: "P1" })
            .unwrap();
        supplies
            .insert(tuple! { supplier_id: "S2", part_id: "P2" })
            .unwrap();

        let mut contained = Relation::new(RelationType::new(heading.clone()));
        contained
            .insert(tuple! { supplier_id: "S1", part_id: "P1" })
            .unwrap();
        let dee = supplies.divide(&contained).unwrap();
        assert_eq!(dee.degree(), 0);
        assert_eq!(dee.cardinality(), 1);

        let mut not_contained = Relation::new(RelationType::new(heading.clone()));
        not_contained
            .insert(tuple! { supplier_id: "S9", part_id: "P1" })
            .unwrap();
        let dum = supplies.divide(&not_contained).unwrap();
        assert_eq!(dum.degree(), 0);
        assert_eq!(dum.cardinality(), 0);

        // Empty divisor: r1{} (TABLE_DEE, since the dividend is non-empty).
        let empty = Relation::new(RelationType::new(heading));
        assert_eq!(supplies.divide(&empty).unwrap().cardinality(), 1);
    }
}

#[cfg(test)]
mod tests_additional {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_divide_error_missing_attribute_internal() {
        let dividend_heading = TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int);
        let dividend = Relation::new(RelationType::new(dividend_heading));

        let divisor_heading = TupleType::new().with_attribute("c", ScalarType::Int);
        let divisor = Relation::new(RelationType::new(divisor_heading));

        assert!(matches!(
            dividend.divide(&divisor),
            Err(DivideError::MissingAttribute(attr)) if attr == "c"
        ));
    }

    #[test]
    fn test_divide_error_type_mismatch_internal() {
        let dividend_heading = TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int);
        let dividend = Relation::new(RelationType::new(dividend_heading));

        let divisor_heading = TupleType::new().with_attribute("b", ScalarType::String);
        let divisor = Relation::new(RelationType::new(divisor_heading));

        assert!(matches!(
            dividend.divide(&divisor),
            Err(DivideError::TypeMismatch(attr)) if attr == "b"
        ));
    }

    #[test]
    fn test_divide_empty_remainder_of_empty_dividend_is_dum_internal() {
        let dividend_heading = TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int);
        let dividend = Relation::new(RelationType::new(dividend_heading));

        let divisor_heading = TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int);
        let divisor = Relation::new(RelationType::new(divisor_heading));

        let result = dividend.divide(&divisor).unwrap();
        assert_eq!(result.degree(), 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_divide_empty_dividend_internal() {
        let dividend_heading = TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int);
        let dividend = Relation::new(RelationType::new(dividend_heading));

        let divisor_heading = TupleType::new().with_attribute("b", ScalarType::Int);
        let mut divisor = Relation::new(RelationType::new(divisor_heading));
        divisor.insert(tuple! {b: 1i64}).unwrap();

        let result = dividend.divide(&divisor).unwrap();
        assert_eq!(result.cardinality(), 0);
    }

    #[test]
    fn test_divide_empty_divisor_internal() {
        let dividend_heading = TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int);
        let mut dividend = Relation::new(RelationType::new(dividend_heading));
        dividend.insert(tuple! {a: 1i64, b: 2i64}).unwrap();
        dividend.insert(tuple! {a: 2i64, b: 3i64}).unwrap();

        let divisor_heading = TupleType::new().with_attribute("b", ScalarType::Int);
        let divisor = Relation::new(RelationType::new(divisor_heading));

        let result = dividend.divide(&divisor).unwrap();
        assert_eq!(result.cardinality(), 2);
    }
}

#[cfg(test)]
mod small_divide_properties {
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::Relation;
    use alloc::collections::BTreeSet;
    use alloc::vec::Vec;
    use proptest::prelude::*;

    fn dividend(pairs: &BTreeSet<(i64, i64)>) -> Relation {
        let heading = TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int);
        let mut r = Relation::new(RelationType::new(heading));
        for (a, b) in pairs {
            r.insert(tuple! { a: *a, b: *b }).unwrap();
        }
        r
    }

    fn divisor(bs: &BTreeSet<i64>) -> Relation {
        let heading = TupleType::new().with_attribute("b", ScalarType::Int);
        let mut r = Relation::new(RelationType::new(heading));
        for b in bs {
            r.insert(tuple! { b: *b }).unwrap();
        }
        r
    }

    /// Date's Small Divide: `r1{A} MINUS ((r1{A} JOIN r2) MINUS r1){A}`.
    fn small_divide(r1: &Relation, r2: &Relation, a: &[&str]) -> Relation {
        let r1_a = r1.project(a).unwrap();
        let missing = r1_a
            .join(r2)
            .unwrap()
            .difference(r1)
            .unwrap()
            .project(a)
            .unwrap();
        r1_a.difference(&missing).unwrap()
    }

    proptest! {
        /// DIVIDEBY equals the Small Divide definition, including an empty
        /// divisor (result r1{A}).
        #[test]
        fn divide_matches_small_divide(
            pairs in prop::collection::btree_set((0i64..4, 0i64..4), 0..12),
            bs in prop::collection::btree_set(0i64..4, 0..4),
        ) {
            let (r1, r2) = (dividend(&pairs), divisor(&bs));
            prop_assert_eq!(r1.divide(&r2).unwrap(), small_divide(&r1, &r2, &["a"]));
        }

        /// The same with a divisor over the dividend's whole heading (A = {}).
        #[test]
        fn divide_by_full_heading_matches_small_divide(
            pairs in prop::collection::btree_set((0i64..3, 0i64..3), 0..6),
            picks in prop::collection::btree_set((0i64..3, 0i64..3), 0..3),
        ) {
            let r1 = dividend(&pairs);
            let r2 = dividend(&picks);
            let none: Vec<&str> = Vec::new();
            prop_assert_eq!(r1.divide(&r2).unwrap(), small_divide(&r1, &r2, &none));
        }
    }
}
