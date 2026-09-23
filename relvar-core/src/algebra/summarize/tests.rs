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
