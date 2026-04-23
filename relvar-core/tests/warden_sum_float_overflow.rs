use relvar_core::algebra::Aggregation;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;

#[test]
fn test_sum_float_overflow() {
    let heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("price", ScalarType::Float);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { id: 1i64, price: f64::MAX })
        .unwrap();
    relation
        .insert(tuple! { id: 2i64, price: f64::MAX })
        .unwrap();

    let sum_agg = Aggregation::sum_float("total_price", "price");
    let result = relation.summarize(&[], &[sum_agg]);

    assert!(result.is_err(), "Expected an error due to float overflow");
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Float overflow") || err_msg.contains("AggregationError"));
}

#[test]
fn test_avg_float_overflow() {
    let heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("price", ScalarType::Float);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { id: 1i64, price: f64::MAX })
        .unwrap();
    relation
        .insert(tuple! { id: 2i64, price: f64::MAX })
        .unwrap();

    let avg_agg = Aggregation::avg("avg_price", "price");
    let result = relation.summarize(&[], &[avg_agg]);

    assert!(result.is_err(), "Expected an error due to float overflow");
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Float overflow") || err_msg.contains("AggregationError"));
}
