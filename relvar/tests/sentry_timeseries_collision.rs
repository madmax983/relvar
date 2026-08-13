use relvar::experimental::timeseries::moving_average;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;
use relvar_core::tuple;

#[test]
fn test_sentry_timeseries_collision() {
    let heading = TupleType::new()
        .with_attribute("time".to_string(), ScalarType::Int)
        .with_attribute("value".to_string(), ScalarType::Float)
        // Add an attribute that would collide with the generated prefix logic if not careful
        .with_attribute("prev_value".to_string(), ScalarType::Float);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! {
            time: 1i64,
            value: 10.0,
            prev_value: 0.0
        })
        .unwrap();

    relation
        .insert(tuple! {
            time: 2i64,
            value: 20.0,
            prev_value: 10.0
        })
        .unwrap();

    // Since we're directly invoking moving_average, it shouldn't panic
    // if the collision logic correctly generates a unique suffix.
    let result = moving_average(&relation, "time", "value", 2, &[]).unwrap();
    assert_eq!(result.cardinality(), 2);

    let tuple = result
        .tuples()
        .find(|t| t.get_typed::<i64>("time") == Some(2))
        .unwrap();

    assert_eq!(tuple.get_typed::<f64>("moving_avg"), Some(15.0));
}
