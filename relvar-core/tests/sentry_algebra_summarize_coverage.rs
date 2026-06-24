use relvar_core::algebra::{Aggregation, AggregationFn};
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;
use std::error::Error;

#[test]
fn test_summarize_avg_float_overflow() -> Result<(), Box<dyn Error>> {
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("val", ScalarType::Float),
    );
    let mut relation = Relation::new(rel_type);

    relation.insert(tuple! { id: 1i64, val: f64::MAX })?;
    relation.insert(tuple! { id: 2i64, val: f64::MAX })?;

    // Using avg_float
    let aggregations = vec![Aggregation {
        result_name: "avg_val".to_string(),
        result_type: ScalarType::Float,
        function: AggregationFn::Avg("val".to_string()),
    }];

    let result = relation.summarize(&[], &aggregations);

    assert!(result.is_err());
    if let Err(err) = result {
        assert!(err.to_string().contains("Float overflow in AVG"));
    } else {
        panic!("Expected Float overflow error");
    }
    Ok(())
}

#[test]
fn test_summarize_sum_float_overflow() -> Result<(), Box<dyn Error>> {
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("val", ScalarType::Float),
    );
    let mut relation = Relation::new(rel_type);

    relation.insert(tuple! { id: 1i64, val: f64::MAX })?;
    relation.insert(tuple! { id: 2i64, val: f64::MAX })?;

    let aggregations = vec![Aggregation::sum_float("sum_val", "val")];

    let result = relation.summarize(&[], &aggregations);

    assert!(result.is_err());
    if let Err(err) = result {
        assert!(err.to_string().contains("Float overflow in SUM"));
    } else {
        panic!("Expected Float overflow error");
    }
    Ok(())
}

#[test]
fn test_summarize_min_max_empty() -> Result<(), Box<dyn Error>> {
    let rel_type = RelationType::new(TupleType::new().with_attribute("val", ScalarType::Int));
    let relation = Relation::new(rel_type);

    let aggregations = vec![Aggregation::min("min_val", "val", ScalarType::Int)];

    let result = relation.summarize(&[], &aggregations);

    assert!(result.is_err());
    if let Err(err) = result {
        assert!(err.to_string().contains("Cannot compute MIN on empty set"));
    } else {
        panic!("Expected empty set error");
    }
    Ok(())
}

#[test]
fn test_summarize_avg_invalid_type() -> Result<(), Box<dyn Error>> {
    let rel_type = RelationType::new(TupleType::new().with_attribute("val", ScalarType::String));
    let mut relation = Relation::new(rel_type);
    relation.insert(tuple! { val: "abc".to_string() })?;

    let aggregations = vec![Aggregation {
        result_name: "avg_val".to_string(),
        result_type: ScalarType::Float,
        function: AggregationFn::Avg("val".to_string()),
    }];

    let result = relation.summarize(&[], &aggregations);
    assert!(result.is_err());
    if let Err(err) = result {
        assert!(err.to_string().contains("Avg requires Int or Float"));
    } else {
        panic!("Expected invalid type error");
    }
    Ok(())
}
