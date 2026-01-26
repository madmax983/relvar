use crate::values::Relation;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DivideError {
    #[error("Divisor attribute '{0}' not found in dividend")]
    MissingAttribute(String),
    #[error("Type mismatch for attribute '{0}'")]
    TypeMismatch(String),
    #[error("Divisor heading cannot equal dividend heading (no remainder attributes)")]
    EmptyRemainder,
}

/// Relational division operator (÷)
///
/// TTM: RM Prescription 7 - Relational algebra completeness (8/8 operators).
///
/// Returns all tuples from the dividend's remainder attributes such that
/// for every tuple in the divisor, the extended tuple exists in the dividend.
///
/// Mathematical definition: R1 DIVIDEBY R2 returns all tuples t from
/// (R1.attributes - R2.attributes) such that for all s in R2, (t ∪ s) ∈ R1.
///
/// Result heading: R1.attributes - R2.attributes (set difference)
///
/// # Examples
///
/// ```
/// // SUPPLIES(supplier_id, part_id):
/// // S1 supplies: P1, P2
/// // S2 supplies: P1
/// // S3 supplies: P1, P2
/// //
/// // PARTS(part_id): P1, P2
/// //
/// // SUPPLIES.divide(&PARTS) = {supplier_id}
/// // Result: {S1, S3} - suppliers who supply ALL parts
/// ```
///
/// # Errors
///
/// Returns `Err` if:
/// - Divisor has attributes not in dividend
/// - Attribute types don't match
/// - Divisor heading equals dividend heading (no remainder)
pub trait DivideOps {
    fn divide(&self, divisor: &Relation) -> Result<Relation, DivideError>;
}

impl DivideOps for Relation {
    fn divide(&self, divisor: &Relation) -> Result<Relation, DivideError> {
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
        let result_tuples =
            filter_matching_candidates(&candidates, divisor, self, dividend_heading);

        // Return result relation
        use crate::types::RelationType;
        let result_type = RelationType::new(candidates.relation_type().heading().clone());
        Ok(Relation::from_tuples(result_type, result_tuples)
            .expect("Result tuples should conform to result type"))
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
fn compute_remainder_attributes(
    dividend_heading: &crate::types::TupleType,
    divisor_heading: &crate::types::TupleType,
) -> Result<Vec<String>, DivideError> {
    let remainder_attrs: Vec<String> = dividend_heading
        .attribute_names()
        .filter(|attr| !divisor_heading.has_attribute(attr))
        .cloned()
        .collect();

    if remainder_attrs.is_empty() {
        return Err(DivideError::EmptyRemainder);
    }

    Ok(remainder_attrs)
}

/// Project relation onto specified attributes.
fn project_onto_attrs(relation: &Relation, attrs: &[String]) -> Relation {
    let attr_refs: Vec<&str> = attrs.iter().map(|s| s.as_str()).collect();
    relation.project(&attr_refs)
}

/// Filter candidate tuples, keeping only those where ALL divisor tuples
/// can be found when extended with the candidate.
fn filter_matching_candidates(
    candidates: &Relation,
    divisor: &Relation,
    dividend: &Relation,
    dividend_heading: &crate::types::TupleType,
) -> Vec<crate::values::Tuple> {
    use std::collections::BTreeMap;

    candidates
        .tuples()
        .filter(|candidate| {
            // Check if ALL divisor tuples match when extended with this candidate
            divisor.tuples().all(|divisor_tuple| {
                // Extend candidate with divisor tuple
                let mut extended_values = BTreeMap::new();

                // Add candidate attributes
                for attr_name in candidate.attribute_names() {
                    if let Some(value) = candidate.get(attr_name) {
                        extended_values.insert(attr_name.clone(), value.clone());
                    }
                }

                // Add divisor attributes
                for attr_name in divisor_tuple.attribute_names() {
                    if let Some(value) = divisor_tuple.get(attr_name) {
                        extended_values.insert(attr_name.clone(), value.clone());
                    }
                }

                // Create extended tuple
                let extended_tuple =
                    crate::values::Tuple::new(dividend_heading.clone(), extended_values)
                        .expect("Extended tuple should conform to dividend heading");

                // Check if this extended tuple exists in the dividend
                dividend.contains(&extended_tuple)
            })
        })
        .cloned()
        .collect()
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

    // Test 8: Error - divisor heading equals dividend heading (no remainder)
    #[test]
    fn test_divide_error_empty_remainder() {
        let supplies_heading = TupleType::new()
            .with_attribute("supplier_id", ScalarType::String)
            .with_attribute("part_id", ScalarType::String);
        let supplies_type = RelationType::new(supplies_heading.clone());
        let supplies = Relation::new(supplies_type);

        // Divisor has same heading as dividend
        let divisor_type = RelationType::new(supplies_heading);
        let divisor = Relation::new(divisor_type);

        let result = supplies.divide(&divisor);

        assert!(result.is_err());
        match result {
            Err(DivideError::EmptyRemainder) => {
                // Expected
            }
            _ => panic!("Expected EmptyRemainder error"),
        }
    }
}
