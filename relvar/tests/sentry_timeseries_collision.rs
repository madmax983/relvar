use relvar::experimental::timeseries::moving_average;
use relvar::tuple;
use relvar::{Relation, RelationType, ScalarType, TupleType};

#[test]
#[should_panic(expected = "Bug: moving_average collision returns incorrect result")]
fn test_moving_average_attribute_collision() {
    // This test demonstrates a vulnerability in `moving_average`.
    // The implementation renames attributes by appending "_prev".
    // If the input relation ALREADY has an attribute with the "_prev" suffix,
    // the join operation will have a naming collision.
    //
    // Specifically:
    // Input: (time, value, value_prev)
    // Renamed (r_prev): (time_prev, value_prev, value_prev_prev)
    //
    // Join: Input JOIN r_prev
    // Common attribute: value_prev
    //
    // `theta_join` behavior: drops the attribute from the second relation (r_prev).
    // Result has `value_prev` from Input (the CURRENT row's value_prev).
    //
    // Aggregation: averages `value_prev`.
    // It intends to average the PREVIOUS row's `value` (which was renamed to `value_prev`).
    // But due to the collision, it averages the CURRENT row's `value_prev`.

    let mut rel = Relation::new(RelationType::new(
        TupleType::new()
            .with_attribute("time", ScalarType::Int)
            .with_attribute("value", ScalarType::Float)
            .with_attribute("value_prev", ScalarType::Float), // Collision bait
    ));

    // Time 1: value=10, value_prev=100
    rel.insert(tuple! { time: 1, value: 10.0, value_prev: 100.0 })
        .unwrap();
    // Time 2: value=20, value_prev=200
    rel.insert(tuple! { time: 2, value: 20.0, value_prev: 200.0 })
        .unwrap();

    // Compute moving average of "value" with window=2.
    // Expected at Time 2: avg(value@T1, value@T2) = avg(10, 20) = 15.0
    //
    // Actual execution logic:
    // 1. r_prev created. T1 renamed: time_prev=1, value_prev=10, value_prev_prev=100.
    // 2. Join (T2 from Input, T1 from r_prev).
    //    Input T2: time=2, value=20, value_prev=200.
    //    r_prev T1: time_prev=1, value_prev=10, ...
    //    Collision on `value_prev`. `theta_join` keeps Input's `value_prev` (200).
    //    Joined Tuple: time=2, value=20, value_prev=200, time_prev=1, ...
    // 3. Aggregate `value_prev`.
    //    It averages 200 (from T2) and ... (T2 join T2 -> value_prev=200).
    //    So it averages 200s. Result ~200.
    //
    // If correct: should be 15.0.
    let res = moving_average(&rel, "time", "value", 2, &[]).unwrap();

    let t2 = res
        .tuples()
        .find(|t| t.get_typed::<i64>("time") == Some(2))
        .unwrap();

    let avg = t2.get_typed::<f64>("moving_avg").unwrap();

    // Check if we hit the bug
    if (avg - 15.0).abs() < 0.001 {
        // Test passed (bug fixed or logic handles it)
        // This path will cause the test to FAIL (because #[should_panic] expects a panic),
        // which alerts us that the bug has been fixed and we can remove the attribute.
    } else if (avg - 200.0).abs() < 0.001 {
        // Bug confirmed
        panic!("Bug: moving_average collision returns incorrect result");
    } else {
        panic!("Unexpected result: {}", avg);
    }
}
