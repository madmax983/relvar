//! Group and ungroup operators for relation-valued attributes.
//!
//! The group operator creates relation-valued attributes (RVAs) by collecting
//! related tuples into nested relations. The ungroup operator is the inverse,
//! flattening RVAs back into regular attributes.
//!
//! # TTM Compliance
//!
//! - Relation-valued attributes are a key feature of the relational model
//! - Group and ungroup are inverses: `ungroup(group(R, attrs, name), name) ≡ R`
//! - Set semantics are maintained at all levels
//!
//! # Example
//!
//! ```
//! use relvar_core::types::{TupleType, RelationType, ScalarType};
//! use relvar_core::values::{Relation, ScalarValue};
//! use relvar_core::algebra::group::GroupOps;
//! use relvar_core::tuple;
//!
//! let heading = TupleType::new()
//!     .with_attribute("dept_id", ScalarType::Int)
//!     .with_attribute("emp_id", ScalarType::Int)
//!     .with_attribute("name", ScalarType::String);
//!
//! let mut relation = Relation::new(RelationType::new(heading));
//! relation.insert(tuple! { dept_id: 10i64, emp_id: 1i64, name: "Alice" }).unwrap();
//! relation.insert(tuple! { dept_id: 10i64, emp_id: 2i64, name: "Bob" }).unwrap();
//! relation.insert(tuple! { dept_id: 20i64, emp_id: 3i64, name: "Charlie" }).unwrap();
//!
//! // Group employees by department
//! let grouped = relation.group(&["emp_id", "name"], "employees").unwrap();
//! assert_eq!(grouped.cardinality(), 2);  // Two departments
//! assert_eq!(grouped.degree(), 2);  // dept_id and employees RVA
//! ```

use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, ScalarValue, Tuple};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during group operations.
#[derive(Debug, Error)]
pub enum GroupError {
    /// An attribute specified for grouping does not exist in the relation.
    #[error("Grouping attribute '{0}' does not exist in relation")]
    AttributeNotFound(String),

    /// No attributes were specified to group into the RVA.
    #[error("No attributes specified for grouping")]
    NoAttributesSpecified,

    /// Cannot group all attributes - at least one must remain as a grouping key.
    #[error("All attributes cannot be grouped (need at least one non-grouped attribute)")]
    AllAttributesGrouped,

    /// The name for the result RVA conflicts with an existing attribute.
    #[error("Result attribute '{0}' already exists")]
    ResultAttributeExists(String),

    /// Failed to construct a tuple during the grouping process.
    #[error("Failed to create grouped tuple: {0}")]
    TupleCreation(String),
}

/// Errors that can occur during ungroup operations.
#[derive(Debug, Error)]
pub enum UngroupError {
    /// The specified RVA attribute does not exist in the relation.
    #[error("Attribute '{0}' does not exist in relation")]
    AttributeNotFound(String),

    /// The specified attribute is not a relation-valued attribute.
    #[error("Attribute '{0}' is not a relation-valued attribute")]
    NotRelationValued(String),

    /// Failed to construct a tuple during the ungrouping process.
    #[error("Failed to create ungrouped tuple: {0}")]
    TupleCreation(String),
}

/// Trait providing group and ungroup operations for relations.
///
/// This trait defines methods for creating and flattening relation-valued
/// attributes (RVAs), which allow nested relations within tuples.
pub trait GroupOps {
    /// Groups specified attributes into a relation-valued attribute.
    ///
    /// This operator collects tuples with matching values on the non-grouped
    /// attributes and creates a nested relation (RVA) containing the grouped
    /// attribute values.
    ///
    /// # Arguments
    ///
    /// * `attrs_to_group` - The attribute names to collect into the RVA
    /// * `rva_name` - The name for the new relation-valued attribute
    ///
    /// # Returns
    ///
    /// A new relation where the grouped attributes have been replaced by a
    /// single relation-valued attribute containing nested relations.
    ///
    /// # Errors
    ///
    /// - [`GroupError::AttributeNotFound`] - A specified attribute doesn't exist
    /// - [`GroupError::NoAttributesSpecified`] - Empty attributes list
    /// - [`GroupError::AllAttributesGrouped`] - No grouping key attributes remain
    /// - [`GroupError::ResultAttributeExists`] - RVA name conflicts
    fn group(&self, attrs_to_group: &[&str], rva_name: &str) -> Result<Relation, GroupError>;

    /// Ungroups a relation-valued attribute back into regular attributes.
    ///
    /// This is the inverse of [`group`](Self::group). It flattens a nested
    /// relation by combining each inner tuple with its parent tuple's
    /// non-RVA attributes.
    ///
    /// # Arguments
    ///
    /// * `rva_name` - The name of the relation-valued attribute to flatten
    ///
    /// # Returns
    ///
    /// A new relation with the RVA replaced by its constituent attributes.
    ///
    /// # Errors
    ///
    /// - [`UngroupError::AttributeNotFound`] - The attribute doesn't exist
    /// - [`UngroupError::NotRelationValued`] - The attribute is not an RVA
    fn ungroup(&self, rva_name: &str) -> Result<Relation, UngroupError>;
}

impl GroupOps for Relation {
    fn group(&self, attrs_to_group: &[&str], rva_name: &str) -> Result<Relation, GroupError> {
        if attrs_to_group.is_empty() {
            return Err(GroupError::NoAttributesSpecified);
        }

        // Validate all attributes exist
        for attr in attrs_to_group {
            if !self.relation_type().has_attribute(attr) {
                return Err(GroupError::AttributeNotFound(attr.to_string()));
            }
        }

        // Check that we're not grouping all attributes
        if attrs_to_group.len() == self.relation_type().degree() {
            return Err(GroupError::AllAttributesGrouped);
        }

        // Determine grouping attributes (the ones NOT being grouped into RVA)
        let all_attrs: Vec<String> = self
            .relation_type()
            .tuple_type()
            .attribute_names()
            .map(|s| s.to_string())
            .collect();
        let grouping_attrs: Vec<String> = all_attrs
            .iter()
            .filter(|attr| !attrs_to_group.contains(&attr.as_str()))
            .cloned()
            .collect();

        // Build result heading
        let mut result_heading = TupleType::new();

        // Add grouping attributes
        for attr in &grouping_attrs {
            let attr_type = self
                .relation_type()
                .tuple_type()
                .get_attribute_type(attr)
                .unwrap();
            result_heading = result_heading.with_attribute(attr.clone(), attr_type.clone());
        }

        // Check RVA name doesn't conflict
        if result_heading.has_attribute(rva_name) {
            return Err(GroupError::ResultAttributeExists(rva_name.to_string()));
        }

        // Build RVA heading (from attributes being grouped)
        let mut rva_heading = TupleType::new();
        for attr in attrs_to_group {
            let attr_type = self
                .relation_type()
                .tuple_type()
                .get_attribute_type(attr)
                .unwrap();
            rva_heading = rva_heading.with_attribute(attr.to_string(), attr_type.clone());
        }

        // Add RVA to result heading
        result_heading = result_heading.with_attribute(
            rva_name.to_string(),
            ScalarType::Relation(Box::new(RelationType::new(rva_heading.clone()))),
        );

        // Group tuples
        let mut groups: HashMap<Vec<ScalarValue>, Vec<Tuple>> = HashMap::new();

        for tuple in self.tuples() {
            // Extract grouping key
            let key: Vec<ScalarValue> = grouping_attrs
                .iter()
                .map(|attr| tuple.get(attr).unwrap().clone())
                .collect();

            // Extract grouped attributes for RVA
            let mut rva_values = HashMap::new();
            for attr in attrs_to_group {
                rva_values.insert(attr.to_string(), tuple.get(attr).unwrap().clone());
            }

            let rva_tuple = Tuple::new(rva_heading.clone(), rva_values)
                .map_err(|e| GroupError::TupleCreation(e.to_string()))?;

            groups.entry(key).or_default().push(rva_tuple);
        }

        // Build result tuples
        let mut result_tuples = Vec::new();
        for (key, rva_tuples) in groups {
            let mut values = HashMap::new();

            // Add grouping attribute values
            for (i, attr) in grouping_attrs.iter().enumerate() {
                values.insert(attr.clone(), key[i].clone());
            }

            // Create RVA relation
            let rva_relation =
                Relation::from_tuples(RelationType::new(rva_heading.clone()), rva_tuples)
                    .map_err(|e| GroupError::TupleCreation(e.to_string()))?;
            values.insert(rva_name.to_string(), ScalarValue::Relation(rva_relation));

            let tuple = Tuple::new(result_heading.clone(), values)
                .map_err(|e| GroupError::TupleCreation(e.to_string()))?;
            result_tuples.push(tuple);
        }

        Ok(
            Relation::from_tuples(RelationType::new(result_heading), result_tuples)
                .expect("Grouped tuples should conform to result relation type"),
        )
    }

    fn ungroup(&self, rva_name: &str) -> Result<Relation, UngroupError> {
        // Check attribute exists
        if !self.relation_type().has_attribute(rva_name) {
            return Err(UngroupError::AttributeNotFound(rva_name.to_string()));
        }

        // Get the RVA type
        let rva_type = self
            .relation_type()
            .tuple_type()
            .get_attribute_type(rva_name)
            .unwrap();

        let rva_relation_type = match rva_type {
            ScalarType::Relation(rel_type) => rel_type.as_ref(),
            _ => return Err(UngroupError::NotRelationValued(rva_name.to_string())),
        };

        // Build result heading (non-RVA attributes + RVA's attributes)
        let mut result_heading = TupleType::new();

        // Add non-RVA attributes
        for attr_name in self.relation_type().tuple_type().attribute_names() {
            if attr_name != rva_name {
                let attr_type = self
                    .relation_type()
                    .tuple_type()
                    .get_attribute_type(attr_name)
                    .unwrap();
                result_heading =
                    result_heading.with_attribute(attr_name.to_string(), attr_type.clone());
            }
        }

        // Add RVA's attributes
        for attr_name in rva_relation_type.tuple_type().attribute_names() {
            let attr_type = rva_relation_type
                .tuple_type()
                .get_attribute_type(attr_name)
                .unwrap();
            result_heading =
                result_heading.with_attribute(attr_name.to_string(), attr_type.clone());
        }

        // Ungroup tuples
        let mut result_tuples = Vec::new();

        for tuple in self.tuples() {
            // Get the RVA relation
            let rva_relation = match tuple.get(rva_name).unwrap() {
                ScalarValue::Relation(rel) => rel,
                _ => return Err(UngroupError::NotRelationValued(rva_name.to_string())),
            };

            // For each tuple in the RVA, create a new tuple combining non-RVA and RVA attributes
            for rva_tuple in rva_relation.tuples() {
                let mut values = HashMap::new();

                // Add non-RVA attribute values
                for attr_name in self.relation_type().tuple_type().attribute_names() {
                    if attr_name != rva_name {
                        values.insert(attr_name.to_string(), tuple.get(attr_name).unwrap().clone());
                    }
                }

                // Add RVA tuple's attribute values
                for attr_name in rva_relation_type.tuple_type().attribute_names() {
                    values.insert(
                        attr_name.to_string(),
                        rva_tuple.get(attr_name).unwrap().clone(),
                    );
                }

                let result_tuple = Tuple::new(result_heading.clone(), values)
                    .map_err(|e| UngroupError::TupleCreation(e.to_string()))?;
                result_tuples.push(result_tuple);
            }
        }

        Ok(
            Relation::from_tuples(RelationType::new(result_heading), result_tuples)
                .expect("Ungrouped tuples should conform to result relation type"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;

    #[test]
    fn test_group_creates_rva() {
        let heading = TupleType::new()
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { dept_id: 10i64, emp_id: 1i64, name: "Alice" })
            .unwrap();
        relation
            .insert(tuple! { dept_id: 10i64, emp_id: 2i64, name: "Bob" })
            .unwrap();
        relation
            .insert(tuple! { dept_id: 20i64, emp_id: 3i64, name: "Charlie" })
            .unwrap();

        let result = relation.group(&["emp_id", "name"], "employees").unwrap();

        // Result should have 2 tuples (one per dept_id)
        assert_eq!(result.cardinality(), 2);
        // Result should have 2 attributes: dept_id and employees (RVA)
        assert_eq!(result.degree(), 2);

        // Check that employees is an RVA
        for tuple in result.tuples() {
            let employees = tuple.get("employees").unwrap();
            match employees {
                ScalarValue::Relation(emp_rel) => {
                    // Each department should have the correct number of employees
                    let dept_id = tuple.get_typed::<i64>("dept_id").unwrap();
                    if dept_id == 10 {
                        assert_eq!(emp_rel.cardinality(), 2);
                    } else if dept_id == 20 {
                        assert_eq!(emp_rel.cardinality(), 1);
                    }
                    // RVA should have 2 attributes: emp_id and name
                    assert_eq!(emp_rel.degree(), 2);
                }
                _ => panic!("Expected relation-valued attribute"),
            }
        }
    }

    #[test]
    fn test_ungroup_flattens_rva() {
        let heading = TupleType::new()
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { dept_id: 10i64, emp_id: 1i64, name: "Alice" })
            .unwrap();
        relation
            .insert(tuple! { dept_id: 10i64, emp_id: 2i64, name: "Bob" })
            .unwrap();
        relation
            .insert(tuple! { dept_id: 20i64, emp_id: 3i64, name: "Charlie" })
            .unwrap();

        // Group first
        let grouped = relation.group(&["emp_id", "name"], "employees").unwrap();

        // Then ungroup
        let ungrouped = grouped.ungroup("employees").unwrap();

        // Should have same cardinality as original
        assert_eq!(ungrouped.cardinality(), 3);
        // Should have same degree as original
        assert_eq!(ungrouped.degree(), 3);
    }

    #[test]
    fn test_group_then_ungroup_is_identity() {
        let heading = TupleType::new()
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { dept_id: 10i64, emp_id: 1i64, name: "Alice" })
            .unwrap();
        relation
            .insert(tuple! { dept_id: 10i64, emp_id: 2i64, name: "Bob" })
            .unwrap();
        relation
            .insert(tuple! { dept_id: 20i64, emp_id: 3i64, name: "Charlie" })
            .unwrap();

        // Group then ungroup
        let grouped = relation.group(&["emp_id", "name"], "employees").unwrap();
        let result = grouped.ungroup("employees").unwrap();

        // Should be equivalent to original relation (same tuples, possibly different order)
        assert_eq!(result.cardinality(), relation.cardinality());
        assert_eq!(result.degree(), relation.degree());

        // All original tuples should be present
        for tuple in relation.tuples() {
            assert!(result.contains(tuple));
        }

        // All result tuples should be in original
        for tuple in result.tuples() {
            assert!(relation.contains(tuple));
        }
    }

    #[test]
    fn test_group_empty_relation() {
        let heading = TupleType::new()
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("emp_id".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let relation = Relation::new(rel_type);

        let result = relation.group(&["emp_id"], "employees").unwrap();

        assert_eq!(result.cardinality(), 0);
        assert_eq!(result.degree(), 2); // dept_id and employees RVA
    }

    #[test]
    fn test_group_single_attribute() {
        let heading = TupleType::new()
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("emp_id".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { dept_id: 10i64, emp_id: 1i64 })
            .unwrap();
        relation
            .insert(tuple! { dept_id: 10i64, emp_id: 2i64 })
            .unwrap();

        let result = relation.group(&["emp_id"], "employees").unwrap();

        assert_eq!(result.cardinality(), 1);
        assert_eq!(result.degree(), 2);
    }

    #[test]
    fn test_group_attribute_not_found() {
        let heading = TupleType::new()
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("emp_id".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let relation = Relation::new(rel_type);

        let result = relation.group(&["nonexistent"], "employees");

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            GroupError::AttributeNotFound(_)
        ));
    }

    #[test]
    fn test_group_all_attributes_fails() {
        let heading = TupleType::new()
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("emp_id".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let relation = Relation::new(rel_type);

        let result = relation.group(&["dept_id", "emp_id"], "employees");

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            GroupError::AllAttributesGrouped
        ));
    }

    #[test]
    fn test_ungroup_attribute_not_found() {
        let heading = TupleType::new()
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("emp_id".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let relation = Relation::new(rel_type);

        let result = relation.ungroup("nonexistent");

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            UngroupError::AttributeNotFound(_)
        ));
    }

    #[test]
    fn test_ungroup_not_relation_valued() {
        let heading = TupleType::new()
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("emp_id".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { dept_id: 10i64, emp_id: 1i64 })
            .unwrap();

        let result = relation.ungroup("dept_id");

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            UngroupError::NotRelationValued(_)
        ));
    }

    #[test]
    fn test_multiple_grouping_keys() {
        let heading = TupleType::new()
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("location".to_string(), ScalarType::String)
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { dept_id: 10i64, location: "NYC", emp_id: 1i64, name: "Alice" })
            .unwrap();
        relation
            .insert(tuple! { dept_id: 10i64, location: "NYC", emp_id: 2i64, name: "Bob" })
            .unwrap();
        relation
            .insert(tuple! { dept_id: 10i64, location: "LA", emp_id: 3i64, name: "Charlie" })
            .unwrap();
        relation
            .insert(tuple! { dept_id: 20i64, location: "NYC", emp_id: 4i64, name: "David" })
            .unwrap();

        let result = relation.group(&["emp_id", "name"], "employees").unwrap();

        // Should have 3 groups: (10, NYC), (10, LA), (20, NYC)
        assert_eq!(result.cardinality(), 3);
        assert_eq!(result.degree(), 3); // dept_id, location, employees
    }
}
