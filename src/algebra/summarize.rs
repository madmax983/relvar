use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, ScalarValue, Tuple};
use std::collections::{BTreeMap, HashMap};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SummarizeError {
    #[error("Grouping attribute '{0}' does not exist in relation")]
    GroupingAttributeNotFound(String),
    #[error("Result attribute '{0}' already exists")]
    ResultAttributeExists(String),
    #[error("Failed to create summarized tuple: {0}")]
    TupleCreation(String),
    #[error("Aggregation error: {0}")]
    AggregationError(String),
}

/// Aggregation function type
pub enum AggregationFn {
    Count,
    Sum(String), // attribute name to sum
    Avg(String), // attribute name to average
    Min(String), // attribute name for minimum
    Max(String), // attribute name for maximum
    #[allow(clippy::type_complexity)]
    Custom(Box<dyn Fn(&[&Tuple]) -> ScalarValue>),
}

pub struct Aggregation {
    pub result_name: String,
    pub result_type: ScalarType,
    pub function: AggregationFn,
}

impl Aggregation {
    pub fn count(result_name: &str) -> Self {
        Self {
            result_name: result_name.to_string(),
            result_type: ScalarType::Int,
            function: AggregationFn::Count,
        }
    }

    pub fn sum(result_name: &str, attr_name: &str) -> Self {
        Self {
            result_name: result_name.to_string(),
            result_type: ScalarType::Int,
            function: AggregationFn::Sum(attr_name.to_string()),
        }
    }

    pub fn avg(result_name: &str, attr_name: &str) -> Self {
        Self {
            result_name: result_name.to_string(),
            result_type: ScalarType::Float,
            function: AggregationFn::Avg(attr_name.to_string()),
        }
    }

    pub fn min(result_name: &str, attr_name: &str, result_type: ScalarType) -> Self {
        Self {
            result_name: result_name.to_string(),
            result_type,
            function: AggregationFn::Min(attr_name.to_string()),
        }
    }

    pub fn max(result_name: &str, attr_name: &str, result_type: ScalarType) -> Self {
        Self {
            result_name: result_name.to_string(),
            result_type,
            function: AggregationFn::Max(attr_name.to_string()),
        }
    }

    fn compute(&self, tuples: &[&Tuple]) -> Result<ScalarValue, SummarizeError> {
        match &self.function {
            AggregationFn::Count => Ok(ScalarValue::Int(tuples.len() as i64)),
            AggregationFn::Sum(attr_name) => {
                let mut sum = 0i64;
                for tuple in tuples {
                    let value = tuple.get_typed::<i64>(attr_name).ok_or_else(|| {
                        SummarizeError::AggregationError(format!(
                            "Failed to get attribute {} as i64",
                            attr_name
                        ))
                    })?;
                    sum += value;
                }
                Ok(ScalarValue::Int(sum))
            }
            AggregationFn::Avg(attr_name) => {
                if tuples.is_empty() {
                    return Ok(ScalarValue::Float(0.0));
                }
                let mut sum = 0i64;
                for tuple in tuples {
                    let value = tuple.get_typed::<i64>(attr_name).ok_or_else(|| {
                        SummarizeError::AggregationError(format!(
                            "Failed to get attribute {} as i64",
                            attr_name
                        ))
                    })?;
                    sum += value;
                }
                let avg = sum as f64 / tuples.len() as f64;
                Ok(ScalarValue::Float(avg))
            }
            AggregationFn::Min(attr_name) => {
                if tuples.is_empty() {
                    return Err(SummarizeError::AggregationError(
                        "Cannot compute MIN on empty set".to_string(),
                    ));
                }
                let first = tuples[0].get(attr_name).ok_or_else(|| {
                    SummarizeError::AggregationError(format!("Attribute {} not found", attr_name))
                })?;
                let mut min_value = first.clone();

                for tuple in tuples.iter().skip(1) {
                    let value = tuple.get(attr_name).ok_or_else(|| {
                        SummarizeError::AggregationError(format!(
                            "Attribute {} not found",
                            attr_name
                        ))
                    })?;
                    if value < &min_value {
                        min_value = value.clone();
                    }
                }
                Ok(min_value)
            }
            AggregationFn::Max(attr_name) => {
                if tuples.is_empty() {
                    return Err(SummarizeError::AggregationError(
                        "Cannot compute MAX on empty set".to_string(),
                    ));
                }
                let first = tuples[0].get(attr_name).ok_or_else(|| {
                    SummarizeError::AggregationError(format!("Attribute {} not found", attr_name))
                })?;
                let mut max_value = first.clone();

                for tuple in tuples.iter().skip(1) {
                    let value = tuple.get(attr_name).ok_or_else(|| {
                        SummarizeError::AggregationError(format!(
                            "Attribute {} not found",
                            attr_name
                        ))
                    })?;
                    if value > &max_value {
                        max_value = value.clone();
                    }
                }
                Ok(max_value)
            }
            AggregationFn::Custom(f) => Ok(f(tuples)),
        }
    }
}

pub trait SummarizeOps {
    /// Summarize the relation with aggregations, optionally grouped by attributes
    fn summarize(
        &self,
        group_by: &[&str],
        aggregations: &[Aggregation],
    ) -> Result<Relation, SummarizeError>;
}

impl SummarizeOps for Relation {
    fn summarize(
        &self,
        group_by: &[&str],
        aggregations: &[Aggregation],
    ) -> Result<Relation, SummarizeError> {
        // Validate grouping attributes exist
        for attr in group_by {
            if !self.relation_type().has_attribute(attr) {
                return Err(SummarizeError::GroupingAttributeNotFound(attr.to_string()));
            }
        }

        // Build result heading
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

        // Group tuples
        let groups = if group_by.is_empty() {
            // No grouping - all tuples in one group
            let mut map = HashMap::new();
            let all_tuples: Vec<&Tuple> = self.tuples().collect();
            map.insert(Vec::new(), all_tuples);
            map
        } else {
            // Group by specified attributes
            let mut groups: HashMap<Vec<ScalarValue>, Vec<&Tuple>> = HashMap::new();
            for tuple in self.tuples() {
                let key: Vec<ScalarValue> = group_by
                    .iter()
                    .map(|attr| tuple.get(attr).unwrap().clone())
                    .collect();
                groups.entry(key).or_default().push(tuple);
            }
            groups
        };

        // Compute aggregations for each group
        let mut result_tuples = Vec::new();
        for (key, group_tuples) in groups {
            let mut values = BTreeMap::new();

            // Add grouping attribute values
            for (i, attr) in group_by.iter().enumerate() {
                values.insert(attr.to_string(), key[i].clone());
            }

            // Compute aggregations
            for agg in aggregations {
                let agg_value = agg.compute(&group_tuples)?;
                values.insert(agg.result_name.clone(), agg_value);
            }

            let tuple = Tuple::new(result_heading.clone(), values)
                .map_err(|e| SummarizeError::TupleCreation(e.to_string()))?;
            result_tuples.push(tuple);
        }

        Ok(
            Relation::from_tuples(RelationType::new(result_heading), result_tuples)
                .expect("Summarized tuples should conform to result relation type"),
        )
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
}
