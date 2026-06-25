use relvar_core::algebra::{Aggregation, SummarizeError};
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;

#[test]
fn test_warden_exploit_math_overflow_dos() {
    let heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("val", ScalarType::Float);

    let mut relation = Relation::new(RelationType::new(heading));

    // Insert tuples with MAX float values
    relation.insert(tuple! { id: 1i64, val: f64::MAX }).unwrap();
    relation.insert(tuple! { id: 2i64, val: f64::MAX }).unwrap();

    // Attempting to trigger an overflow
    let aggr = Aggregation::sum_float("sum_result", "val");
    let result = relation.summarize(&[], &[aggr]);

    assert!(result.is_err());

    // Check if it is a SummarizeError::AggregationError
    if let Err(e) = result {
        assert!(matches!(e, SummarizeError::AggregationError(_)));
    }
}
