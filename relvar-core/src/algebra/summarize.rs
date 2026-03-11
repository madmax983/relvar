//! Summarize operator for aggregation with grouping.
//!
//! The summarize operator computes aggregate values (count, sum, average, min, max)
//! over groups of tuples. It is similar to SQL's GROUP BY with aggregate functions.
//!
//! # TTM Compliance
//!
//! - Aggregation is performed on tuple values, not physical storage
//! - Result is a valid relation with the grouped and computed attributes
//! - Set semantics are maintained
//!
//! # Example
//!
//! ```
//! use relvar_core::types::{TupleType, RelationType, ScalarType};
//! use relvar_core::values::{Relation, ScalarValue};
//! use relvar_core::algebra::Aggregation;
//! use relvar_core::tuple;
//!
//! let heading = TupleType::new()
//!     .with_attribute("dept_id", ScalarType::Int)
//!     .with_attribute("salary", ScalarType::Int);
//!
//! let mut relation = Relation::new(RelationType::new(heading));
//! relation.insert(tuple! { dept_id: 10i64, salary: 50000i64 }).unwrap();
//! relation.insert(tuple! { dept_id: 10i64, salary: 60000i64 }).unwrap();
//! relation.insert(tuple! { dept_id: 20i64, salary: 70000i64 }).unwrap();
//!
//! // Summarize by department
//! let result = relation.summarize(
//!     &["dept_id"],
//!     &[
//!         Aggregation::count("emp_count"),
//!         Aggregation::sum("total_salary", "salary"),
//!     ],
//! ).unwrap();
//!
//! assert_eq!(result.cardinality(), 2);  // Two departments
//! ```

use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, ScalarValue, Tuple};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during summarize operations.
#[derive(Debug, Error)]
pub enum SummarizeError {
    /// A specified grouping attribute does not exist in the relation.
    #[error("Grouping attribute '{0}' does not exist in relation")]
    GroupingAttributeNotFound(String),

    /// The result attribute name conflicts with an existing attribute.
    #[error("Result attribute '{0}' already exists")]
    ResultAttributeExists(String),

    /// Failed to construct a tuple during the summarization process.
    #[error("Failed to create summarized tuple: {0}")]
    TupleCreation(String),

    /// An error occurred during aggregate computation.
    ///
    /// This can happen if the attribute being aggregated doesn't exist
    /// or has an incompatible type.
    #[error("Aggregation error: {0}")]
    AggregationError(String),
}

/// Specifies the type of aggregation function to apply.
///
/// Each variant represents a different aggregate computation that can
/// be performed over a group of tuples.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationFn {
    /// Counts the number of tuples in the group.
    Count,

    /// Computes the sum of an integer attribute across the group.
    ///
    /// The string parameter specifies the attribute name to sum.
    Sum(String),

    /// Computes the average of an integer attribute across the group.
    ///
    /// The string parameter specifies the attribute name to average.
    /// Result is a floating-point value.
    Avg(String),

    /// Finds the minimum value of an attribute in the group.
    ///
    /// The string parameter specifies the attribute name.
    Min(String),

    /// Finds the maximum value of an attribute in the group.
    ///
    /// The string parameter specifies the attribute name.
    Max(String),
}

/// Defines a single aggregation to compute in a summarize operation.
///
/// An aggregation specifies what to compute (the function), what to call
/// the result (the name), and what type the result will be.
///
/// # Example
///
/// ```
/// use relvar_core::types::ScalarType;
/// use relvar_core::algebra::Aggregation;
///
/// // Count all tuples, store in "total" as Int
/// let count_agg = Aggregation::count("total");
///
/// // Sum the "salary" attribute, store in "total_pay" as Int
/// let sum_agg = Aggregation::sum("total_pay", "salary");
///
/// // Average the "age" attribute, store in "avg_age" as Float
/// let avg_agg = Aggregation::avg("avg_age", "age");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aggregation {
    /// The name for the computed result attribute.
    pub result_name: String,

    /// The scalar type of the result value.
    pub result_type: ScalarType,

    /// The aggregation function to apply.
    pub function: AggregationFn,
}

impl Aggregation {
    /// Creates a COUNT aggregation.
    ///
    /// Counts the number of tuples in each group. The result type is always
    /// [`ScalarType::Int`].
    ///
    /// # Arguments
    ///
    /// * `result_name` - The name for the count result attribute
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::algebra::Aggregation;
    ///
    /// let agg = Aggregation::count("num_employees");
    /// ```
    pub fn count(result_name: &str) -> Self {
        Self {
            result_name: result_name.to_string(),
            result_type: ScalarType::Int,
            function: AggregationFn::Count,
        }
    }

    /// Creates a SUM aggregation for integer values.
    ///
    /// Computes the sum of an integer attribute across all tuples in each group.
    /// The result type is [`ScalarType::Int`].
    ///
    /// # Arguments
    ///
    /// * `result_name` - The name for the sum result attribute
    /// * `attr_name` - The attribute to sum (must be Int type)
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::algebra::Aggregation;
    ///
    /// let agg = Aggregation::sum("total_salary", "salary");
    /// ```
    pub fn sum(result_name: &str, attr_name: &str) -> Self {
        Self {
            result_name: result_name.to_string(),
            result_type: ScalarType::Int,
            function: AggregationFn::Sum(attr_name.to_string()),
        }
    }

    /// Creates a SUM aggregation for floating-point values.
    ///
    /// Computes the sum of a float attribute across all tuples in each group.
    /// The result type is [`ScalarType::Float`].
    ///
    /// # Arguments
    ///
    /// * `result_name` - The name for the sum result attribute
    /// * `attr_name` - The attribute to sum (must be Float type)
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::algebra::Aggregation;
    ///
    /// let agg = Aggregation::sum_float("total_price", "price");
    /// ```
    pub fn sum_float(result_name: &str, attr_name: &str) -> Self {
        Self {
            result_name: result_name.to_string(),
            result_type: ScalarType::Float,
            function: AggregationFn::Sum(attr_name.to_string()),
        }
    }

    /// Creates an AVG (average) aggregation.
    ///
    /// Computes the arithmetic mean of an integer attribute across all tuples
    /// in each group. The result type is [`ScalarType::Float`].
    ///
    /// # Arguments
    ///
    /// * `result_name` - The name for the average result attribute
    /// * `attr_name` - The attribute to average (must be Int type)
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::algebra::Aggregation;
    ///
    /// let agg = Aggregation::avg("avg_salary", "salary");
    /// ```
    pub fn avg(result_name: &str, attr_name: &str) -> Self {
        Self {
            result_name: result_name.to_string(),
            result_type: ScalarType::Float,
            function: AggregationFn::Avg(attr_name.to_string()),
        }
    }

    /// Creates a MIN aggregation.
    ///
    /// Finds the minimum value of an attribute across all tuples in each group.
    /// The result type must be specified and should match the attribute type.
    ///
    /// # Arguments
    ///
    /// * `result_name` - The name for the minimum result attribute
    /// * `attr_name` - The attribute to find the minimum of
    /// * `result_type` - The type of the result (should match the attribute type)
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::ScalarType;
    /// use relvar_core::algebra::Aggregation;
    ///
    /// let agg = Aggregation::min("lowest_salary", "salary", ScalarType::Int);
    /// ```
    pub fn min(result_name: &str, attr_name: &str, result_type: ScalarType) -> Self {
        Self {
            result_name: result_name.to_string(),
            result_type,
            function: AggregationFn::Min(attr_name.to_string()),
        }
    }

    /// Creates a MAX aggregation.
    ///
    /// Finds the maximum value of an attribute across all tuples in each group.
    /// The result type must be specified and should match the attribute type.
    ///
    /// # Arguments
    ///
    /// * `result_name` - The name for the maximum result attribute
    /// * `attr_name` - The attribute to find the maximum of
    /// * `result_type` - The type of the result (should match the attribute type)
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::ScalarType;
    /// use relvar_core::algebra::Aggregation;
    ///
    /// let agg = Aggregation::max("highest_salary", "salary", ScalarType::Int);
    /// ```
    pub fn max(result_name: &str, attr_name: &str, result_type: ScalarType) -> Self {
        Self {
            result_name: result_name.to_string(),
            result_type,
            function: AggregationFn::Max(attr_name.to_string()),
        }
    }

    fn compute(&self, tuples: &[&Tuple]) -> Result<ScalarValue, SummarizeError> {
        match &self.function {
            AggregationFn::Count => self.compute_count(tuples),
            AggregationFn::Sum(attr_name) => self.compute_sum(attr_name, tuples),
            AggregationFn::Avg(attr_name) => self.compute_avg(attr_name, tuples),
            AggregationFn::Min(attr_name) => {
                self.compute_extremum(attr_name, tuples, |a, b| a < b, "MIN")
            }
            AggregationFn::Max(attr_name) => {
                self.compute_extremum(attr_name, tuples, |a, b| a > b, "MAX")
            }
        }
    }

    fn compute_count(&self, tuples: &[&Tuple]) -> Result<ScalarValue, SummarizeError> {
        Ok(ScalarValue::Int(tuples.len() as i64))
    }

    fn compute_sum(
        &self,
        attr_name: &str,
        tuples: &[&Tuple],
    ) -> Result<ScalarValue, SummarizeError> {
        if self.result_type == ScalarType::Float {
            let mut sum = 0.0;
            for tuple in tuples {
                let value = tuple.get_typed::<f64>(attr_name).ok_or_else(|| {
                    SummarizeError::AggregationError(format!(
                        "Failed to get attribute {} as f64",
                        attr_name
                    ))
                })?;
                sum += value;
            }
            Ok(ScalarValue::Float(sum))
        } else {
            let mut sum = 0i64;
            for tuple in tuples {
                let value = tuple.get_typed::<i64>(attr_name).ok_or_else(|| {
                    SummarizeError::AggregationError(format!(
                        "Failed to get attribute {} as i64",
                        attr_name
                    ))
                })?;
                sum = sum.checked_add(value).ok_or_else(|| {
                    SummarizeError::AggregationError("Integer overflow in SUM".to_string())
                })?;
            }
            Ok(ScalarValue::Int(sum))
        }
    }

    fn compute_avg(
        &self,
        attr_name: &str,
        tuples: &[&Tuple],
    ) -> Result<ScalarValue, SummarizeError> {
        if tuples.is_empty() {
            return Ok(ScalarValue::Float(0.0));
        }

        // Check the type of the first tuple to decide implementation
        // We assume all tuples have the same type (enforced by Relation)
        let first_val = tuples[0].get(attr_name).ok_or_else(|| {
            SummarizeError::AggregationError(format!("Attribute {} not found", attr_name))
        })?;

        match first_val.scalar_type() {
            ScalarType::Int => {
                let mut sum = 0i128;
                for tuple in tuples {
                    let value = tuple.get_typed::<i64>(attr_name).ok_or_else(|| {
                        SummarizeError::AggregationError(format!(
                            "Failed to get attribute {} as i64",
                            attr_name
                        ))
                    })?;
                    sum = sum.checked_add(value as i128).ok_or_else(|| {
                        SummarizeError::AggregationError("Integer overflow in AVG".to_string())
                    })?;
                }
                let avg = sum as f64 / tuples.len() as f64;
                Ok(ScalarValue::Float(avg))
            }
            ScalarType::Float => {
                let mut sum = 0.0;
                for tuple in tuples {
                    let value = tuple.get_typed::<f64>(attr_name).ok_or_else(|| {
                        SummarizeError::AggregationError(format!(
                            "Failed to get attribute {} as f64",
                            attr_name
                        ))
                    })?;
                    sum += value;
                }
                let avg = sum / tuples.len() as f64;
                Ok(ScalarValue::Float(avg))
            }
            _ => Err(SummarizeError::AggregationError(format!(
                "Avg requires Int or Float attribute, got {:?}",
                first_val.scalar_type()
            ))),
        }
    }

    fn compute_extremum<F>(
        &self,
        attr_name: &str,
        tuples: &[&Tuple],
        compare: F,
        op_name: &str,
    ) -> Result<ScalarValue, SummarizeError>
    where
        F: Fn(&ScalarValue, &ScalarValue) -> bool,
    {
        if tuples.is_empty() {
            return Err(SummarizeError::AggregationError(format!(
                "Cannot compute {} on empty set",
                op_name
            )));
        }
        let first = tuples[0].get(attr_name).ok_or_else(|| {
            SummarizeError::AggregationError(format!("Attribute {} not found", attr_name))
        })?;
        let mut extremum = first.clone();

        for tuple in tuples.iter().skip(1) {
            let value = tuple.get(attr_name).ok_or_else(|| {
                SummarizeError::AggregationError(format!("Attribute {} not found", attr_name))
            })?;
            if compare(value, &extremum) {
                extremum = value.clone();
            }
        }
        Ok(extremum)
    }
}

impl Relation {
    /// Summarizes the relation by computing aggregations over groups.
    ///
    /// This operator groups tuples by the specified attributes and computes
    /// aggregate values (count, sum, avg, min, max, or custom) for each group.
    /// It is similar to SQL's `GROUP BY` clause with aggregate functions.
    ///
    /// # Arguments
    ///
    /// * `group_by` - Attribute names to group by. If empty, all tuples are
    ///   treated as a single group.
    /// * `aggregations` - The aggregation functions to compute for each group
    ///
    /// # Returns
    ///
    /// A new relation with the grouping attributes plus the computed aggregate
    /// attributes. There is one tuple per distinct combination of grouping
    /// attribute values.
    ///
    /// # Errors
    ///
    /// - [`SummarizeError::GroupingAttributeNotFound`] - A grouping attribute
    ///   doesn't exist
    /// - [`SummarizeError::ResultAttributeExists`] - An aggregation result name
    ///   conflicts with a grouping attribute
    /// - [`SummarizeError::AggregationError`] - An aggregation failed (e.g.,
    ///   MIN/MAX on empty set)
    ///
    /// # Performance
    ///
    /// Pre-allocates the `result_tuples` vector using `Vec::with_capacity(groups.len())`.
    /// Because the exact number of output tuples is known after grouping the input,
    /// this prevents dynamic heap reallocations when constructing the resulting relation.
    pub fn summarize(
        &self,
        group_by: &[&str],
        aggregations: &[Aggregation],
    ) -> Result<Relation, SummarizeError> {
        self.validate_grouping_attributes(group_by)?;
        let result_heading = self.build_result_heading(group_by, aggregations)?;
        let result_rel_type = RelationType::new(result_heading.clone());
        let result_heading_arc = std::sync::Arc::new(result_heading);

        let groups = self.group_tuples(group_by);

        // Compute aggregations for each group
        let mut result_tuples = Vec::with_capacity(groups.len());
        for (key, group_tuples) in groups {
            let mut values = std::collections::BTreeMap::new();

            // Add grouping attribute values
            for (i, attr) in group_by.iter().enumerate() {
                values.insert(attr.to_string(), (*key[i]).clone());
            }

            // Compute aggregations
            for agg in aggregations {
                let agg_value = agg.compute(&group_tuples)?;
                values.insert(agg.result_name.clone(), agg_value);
            }

            // Using new_unchecked avoids O(N) validation per tuple where N is degree,
            // since we've already validated the grouping and aggregation attributes
            let tuple = Tuple::new_unchecked(result_heading_arc.clone(), values);
            result_tuples.push(tuple);
        }

        Ok(Relation::from_tuples(result_rel_type, result_tuples)
            .expect("Summarized tuples should conform to result relation type"))
    }

    fn validate_grouping_attributes(&self, group_by: &[&str]) -> Result<(), SummarizeError> {
        for attr in group_by {
            if !self.relation_type().has_attribute(attr) {
                return Err(SummarizeError::GroupingAttributeNotFound(attr.to_string()));
            }
        }
        Ok(())
    }

    fn build_result_heading(
        &self,
        group_by: &[&str],
        aggregations: &[Aggregation],
    ) -> Result<TupleType, SummarizeError> {
        let mut result_heading = TupleType::new();

        // Add grouping attributes
        for attr in group_by {
            let attr_type = self
                .relation_type()
                .tuple_type()
                .get_attribute_type(attr)
                .unwrap();
            result_heading = result_heading.with_attribute(attr.to_string(), attr_type.clone());
        }

        // Add aggregation result attributes
        for agg in aggregations {
            if result_heading.has_attribute(&agg.result_name) {
                return Err(SummarizeError::ResultAttributeExists(
                    agg.result_name.clone(),
                ));
            }
            result_heading =
                result_heading.with_attribute(agg.result_name.clone(), agg.result_type.clone());
        }
        Ok(result_heading)
    }

    fn group_tuples<'a>(
        &'a self,
        group_by: &[&str],
    ) -> HashMap<Vec<&'a ScalarValue>, Vec<&'a Tuple>> {
        if group_by.is_empty() {
            // No grouping - all tuples in one group
            let mut map = HashMap::new();
            let all_tuples: Vec<&Tuple> = self.tuples().collect();
            map.insert(Vec::new(), all_tuples);
            map
        } else {
            // Group by specified attributes
            let mut groups: HashMap<Vec<&'a ScalarValue>, Vec<&Tuple>> = HashMap::new();
            for tuple in self.tuples() {
                let key: Vec<&ScalarValue> = group_by
                    .iter()
                    .map(|attr| tuple.get(attr).unwrap())
                    .collect();
                groups.entry(key).or_default().push(tuple);
            }
            groups
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;

    #[test]
    fn test_summarize_count() {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice" })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, name: "Bob" })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 3i64, name: "Charlie" })
            .unwrap();

        let result = relation
            .summarize(&[], &[Aggregation::count("total_count")])
            .unwrap();

        assert_eq!(result.cardinality(), 1);
        assert_eq!(result.degree(), 1);

        let tuple = result.tuples().next().unwrap();
        assert_eq!(tuple.get_typed::<i64>("total_count").unwrap(), 3);
    }

    #[test]
    fn test_summarize_sum() {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("salary".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, salary: 50000i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, salary: 60000i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 3i64, salary: 70000i64 })
            .unwrap();

        let result = relation
            .summarize(&[], &[Aggregation::sum("total_salary", "salary")])
            .unwrap();

        assert_eq!(result.cardinality(), 1);
        let tuple = result.tuples().next().unwrap();
        assert_eq!(tuple.get_typed::<i64>("total_salary").unwrap(), 180000);
    }

    #[test]
    fn test_summarize_avg() {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("salary".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, salary: 50000i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, salary: 60000i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 3i64, salary: 70000i64 })
            .unwrap();

        let result = relation
            .summarize(&[], &[Aggregation::avg("avg_salary", "salary")])
            .unwrap();

        assert_eq!(result.cardinality(), 1);
        let tuple = result.tuples().next().unwrap();
        assert_eq!(tuple.get_typed::<f64>("avg_salary").unwrap(), 60000.0);
    }

    #[test]
    fn test_summarize_min_max() {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("salary".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, salary: 50000i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, salary: 60000i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 3i64, salary: 70000i64 })
            .unwrap();

        let result = relation
            .summarize(
                &[],
                &[
                    Aggregation::min("min_salary", "salary", ScalarType::Int),
                    Aggregation::max("max_salary", "salary", ScalarType::Int),
                ],
            )
            .unwrap();

        assert_eq!(result.cardinality(), 1);
        let tuple = result.tuples().next().unwrap();
        assert_eq!(tuple.get_typed::<i64>("min_salary").unwrap(), 50000);
        assert_eq!(tuple.get_typed::<i64>("max_salary").unwrap(), 70000);
    }

    #[test]
    fn test_summarize_with_grouping() {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("salary".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, dept_id: 10i64, salary: 50000i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, dept_id: 10i64, salary: 60000i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 3i64, dept_id: 20i64, salary: 70000i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 4i64, dept_id: 20i64, salary: 80000i64 })
            .unwrap();

        let result = relation
            .summarize(
                &["dept_id"],
                &[
                    Aggregation::count("emp_count"),
                    Aggregation::sum("total_salary", "salary"),
                    Aggregation::avg("avg_salary", "salary"),
                ],
            )
            .unwrap();

        assert_eq!(result.cardinality(), 2);
        assert_eq!(result.degree(), 4);

        // Check dept_id 10
        let dept10: Vec<_> = result
            .tuples()
            .filter(|t| t.get_typed::<i64>("dept_id").unwrap() == 10)
            .collect();
        assert_eq!(dept10.len(), 1);
        let dept10_tuple = dept10[0];
        assert_eq!(dept10_tuple.get_typed::<i64>("emp_count").unwrap(), 2);
        assert_eq!(
            dept10_tuple.get_typed::<i64>("total_salary").unwrap(),
            110000
        );
        assert_eq!(
            dept10_tuple.get_typed::<f64>("avg_salary").unwrap(),
            55000.0
        );

        // Check dept_id 20
        let dept20: Vec<_> = result
            .tuples()
            .filter(|t| t.get_typed::<i64>("dept_id").unwrap() == 20)
            .collect();
        assert_eq!(dept20.len(), 1);
        let dept20_tuple = dept20[0];
        assert_eq!(dept20_tuple.get_typed::<i64>("emp_count").unwrap(), 2);
        assert_eq!(
            dept20_tuple.get_typed::<i64>("total_salary").unwrap(),
            150000
        );
        assert_eq!(
            dept20_tuple.get_typed::<f64>("avg_salary").unwrap(),
            75000.0
        );
    }

    #[test]
    fn test_summarize_multiple_grouping_attributes() {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("location".to_string(), ScalarType::String)
            .with_attribute("salary".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, dept_id: 10i64, location: "NYC", salary: 50000i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, dept_id: 10i64, location: "NYC", salary: 60000i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 3i64, dept_id: 10i64, location: "LA", salary: 55000i64 })
            .unwrap();

        let result = relation
            .summarize(&["dept_id", "location"], &[Aggregation::count("emp_count")])
            .unwrap();

        assert_eq!(result.cardinality(), 2);
        assert_eq!(result.degree(), 3);
    }

    #[test]
    fn test_summarize_empty_relation() {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("salary".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let relation = Relation::new(rel_type);

        let result = relation
            .summarize(&[], &[Aggregation::count("total_count")])
            .unwrap();

        assert_eq!(result.cardinality(), 1);
        let tuple = result.tuples().next().unwrap();
        assert_eq!(tuple.get_typed::<i64>("total_count").unwrap(), 0);
    }

    #[test]
    fn test_summarize_grouping_attribute_not_found() {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("salary".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let relation = Relation::new(rel_type);

        let result = relation.summarize(&["dept_id"], &[Aggregation::count("total_count")]);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SummarizeError::GroupingAttributeNotFound(_)
        ));
    }

    #[test]
    fn test_summarize_result_attribute_exists() {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("salary".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);
        relation
            .insert(tuple! { emp_id: 1i64, salary: 50000i64 })
            .unwrap();

        // Result name conflicts with grouping attribute
        let result = relation.summarize(&["emp_id"], &[Aggregation::count("emp_id")]);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SummarizeError::ResultAttributeExists(_)
        ));
    }

    #[test]
    fn test_sum_attribute_not_found() {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);
        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice" })
            .unwrap();

        // Try to sum a non-existent attribute
        let result = relation.summarize(&[], &[Aggregation::sum("total", "salary")]);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SummarizeError::AggregationError(_)
        ));
    }

    #[test]
    fn test_avg_attribute_not_found() {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);
        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice" })
            .unwrap();

        // Try to average a non-existent attribute
        let result = relation.summarize(&[], &[Aggregation::avg("average", "salary")]);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SummarizeError::AggregationError(_)
        ));
    }

    #[test]
    fn test_avg_empty_tuples() {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("salary".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let relation = Relation::new(rel_type);

        // Average on empty relation (with grouping)
        let result = relation.summarize(&[], &[Aggregation::avg("avg_salary", "salary")]);

        // This should succeed with 0.0
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.cardinality(), 1);
        let tuple = result.tuples().next().unwrap();
        assert_eq!(tuple.get_typed::<f64>("avg_salary").unwrap(), 0.0);
    }

    #[test]
    fn test_min_on_empty_set() {
        let heading = TupleType::new().with_attribute("salary".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let empty_rel = Relation::new(rel_type);

        // MIN on empty set should error
        let result = empty_rel.summarize(
            &[],
            &[Aggregation::min("min_salary", "salary", ScalarType::Int)],
        );

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SummarizeError::AggregationError(_)
        ));
    }

    #[test]
    fn test_max_on_empty_set() {
        let heading = TupleType::new().with_attribute("salary".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let empty_rel = Relation::new(rel_type);

        // MAX on empty set should error
        let result = empty_rel.summarize(
            &[],
            &[Aggregation::max("max_salary", "salary", ScalarType::Int)],
        );

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SummarizeError::AggregationError(_)
        ));
    }

    #[test]
    fn test_min_attribute_not_found() {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);
        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice" })
            .unwrap();

        // Try MIN on non-existent attribute
        let result = relation.summarize(
            &[],
            &[Aggregation::min("min_salary", "salary", ScalarType::Int)],
        );

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SummarizeError::AggregationError(_)
        ));
    }

    #[test]
    fn test_max_attribute_not_found() {
        let heading = TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);
        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice" })
            .unwrap();

        // Try MAX on non-existent attribute
        let result = relation.summarize(
            &[],
            &[Aggregation::max("max_salary", "salary", ScalarType::Int)],
        );

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SummarizeError::AggregationError(_)
        ));
    }

    #[test]
    fn test_min_with_multiple_tuples() {
        let heading = TupleType::new()
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("salary".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { dept_id: 10i64, salary: 70000i64 })
            .unwrap();
        relation
            .insert(tuple! { dept_id: 10i64, salary: 50000i64 })
            .unwrap();
        relation
            .insert(tuple! { dept_id: 10i64, salary: 60000i64 })
            .unwrap();

        let result = relation
            .summarize(
                &["dept_id"],
                &[Aggregation::min("min_salary", "salary", ScalarType::Int)],
            )
            .unwrap();

        assert_eq!(result.cardinality(), 1);
        let tuple = result.tuples().next().unwrap();
        assert_eq!(tuple.get_typed::<i64>("min_salary").unwrap(), 50000);
    }

    #[test]
    fn test_max_with_multiple_tuples() {
        let heading = TupleType::new()
            .with_attribute("dept_id".to_string(), ScalarType::Int)
            .with_attribute("salary".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { dept_id: 10i64, salary: 50000i64 })
            .unwrap();
        relation
            .insert(tuple! { dept_id: 10i64, salary: 70000i64 })
            .unwrap();
        relation
            .insert(tuple! { dept_id: 10i64, salary: 60000i64 })
            .unwrap();

        let result = relation
            .summarize(
                &["dept_id"],
                &[Aggregation::max("max_salary", "salary", ScalarType::Int)],
            )
            .unwrap();

        assert_eq!(result.cardinality(), 1);
        let tuple = result.tuples().next().unwrap();
        assert_eq!(tuple.get_typed::<i64>("max_salary").unwrap(), 70000);
    }

    #[test]
    fn test_min_max_with_strings() {
        let heading = TupleType::new().with_attribute("name".to_string(), ScalarType::String);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation.insert(tuple! { name: "Charlie" }).unwrap();
        relation.insert(tuple! { name: "Alice" }).unwrap();
        relation.insert(tuple! { name: "Bob" }).unwrap();

        let result = relation
            .summarize(
                &[],
                &[
                    Aggregation::min("min_name", "name", ScalarType::String),
                    Aggregation::max("max_name", "name", ScalarType::String),
                ],
            )
            .unwrap();

        assert_eq!(result.cardinality(), 1);
        let tuple = result.tuples().next().unwrap();
        assert_eq!(tuple.get_typed::<String>("min_name").unwrap(), "Alice");
        assert_eq!(tuple.get_typed::<String>("max_name").unwrap(), "Charlie");
    }
}

#[cfg(test)]
mod overflow_tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::Relation;

    #[test]
    fn test_sum_overflow_returns_error() {
        let heading = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("amount".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { id: 1i64, amount: i64::MAX })
            .unwrap();
        relation.insert(tuple! { id: 2i64, amount: 1i64 }).unwrap();

        let result = relation.summarize(&[], &[Aggregation::sum("total", "amount")]);

        assert!(result.is_err(), "Expected overflow error, got Ok");
        match result {
            Err(SummarizeError::AggregationError(msg)) => {
                assert!(
                    msg.contains("overflow"),
                    "Expected overflow message, got: {}",
                    msg
                );
            }
            _ => panic!("Expected AggregationError, got {:?}", result),
        }
    }

    #[test]
    fn test_avg_handles_large_sums() {
        let heading = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("amount".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { id: 1i64, amount: i64::MAX })
            .unwrap();
        relation
            .insert(tuple! { id: 2i64, amount: i64::MAX })
            .unwrap();

        let result = relation.summarize(&[], &[Aggregation::avg("average", "amount")]);

        assert!(result.is_ok(), "Expected success on large sum average");
        let result = result.unwrap();
        let tuple = result.tuples().next().unwrap();
        // Average of MAX and MAX is MAX
        assert_eq!(tuple.get_typed::<f64>("average").unwrap(), i64::MAX as f64);
    }

    #[test]
    fn test_sum_float() {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("price", ScalarType::Float);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation.insert(tuple! { id: 1i64, price: 10.5 }).unwrap();
        relation.insert(tuple! { id: 2i64, price: 20.5 }).unwrap();

        // Use new helper for Float Sum
        let sum_agg = Aggregation::sum_float("total_price", "price");

        let result = relation.summarize(&[], &[sum_agg]);

        assert!(result.is_ok(), "Sum on Float failed: {:?}", result.err());

        let result = result.unwrap();
        assert_eq!(result.cardinality(), 1);
        let tuple = result.tuples().next().unwrap();
        assert_eq!(tuple.get_typed::<f64>("total_price").unwrap(), 31.0);
    }

    #[test]
    fn test_avg_float() {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("price", ScalarType::Float);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation.insert(tuple! { id: 1i64, price: 10.0 }).unwrap();
        relation.insert(tuple! { id: 2i64, price: 20.0 }).unwrap();

        // Avg helper sets result_type to Float
        let avg_agg = Aggregation::avg("avg_price", "price");

        let result = relation.summarize(&[], &[avg_agg]);

        assert!(result.is_ok(), "Avg on Float failed: {:?}", result.err());

        let result = result.unwrap();
        assert_eq!(result.cardinality(), 1);
        let tuple = result.tuples().next().unwrap();
        assert_eq!(tuple.get_typed::<f64>("avg_price").unwrap(), 15.0);
    }
}
