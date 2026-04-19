//! Experimental Relational Time Series Analysis.
//!
//! This module demonstrates how to implement time series operations like
//! Moving Averages using pure Relational Algebra primitives (Self-Join,
//! Rename, Summarize).
//!
//! # Philosophy
//!
//! Traditional databases use specialized window functions (`OVER PARTITION BY`)
//! or imperative loops for time series. In a pure relational model, a window
//! is simply a set of tuples related by a time condition.
//!
//! By joining a relation with itself on `time between (current_time - window) and current_time`,
//! we form groups that represent the rolling window, which can then be aggregated
//! using standard summation operators.

use relvar_core::algebra::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::values::{Relation, ScalarValue};

fn generate_safe_suffix(heading: &relvar_core::types::TupleType) -> String {
    let mut prev_attr_suffix = "_prev".to_string();
    let mut suffix_idx = 1;
    loop {
        let mut collision = false;
        for (attr_name, _) in heading.attributes().iter() {
            let candidate_name = format!("{}{}", attr_name, prev_attr_suffix);
            if heading.has_attribute(&candidate_name) {
                collision = true;
                break;
            }
        }
        if !collision {
            break;
        }
        prev_attr_suffix = format!("_prev_{}", suffix_idx);
        suffix_idx += 1;
    }
    prev_attr_suffix
}

fn prepare_self_join(relation: &Relation, prev_attr_suffix: &str) -> Relation {
    let original_heading = relation.relation_type().heading();
    let mut rename_map = Vec::new();

    for (attr_name, _) in original_heading.attributes().iter() {
        rename_map.push((
            attr_name.as_str(),
            format!("{}{}", attr_name, prev_attr_suffix),
        ));
    }

    let rename_slice: Vec<(&str, &str)> =
        rename_map.iter().map(|(k, v)| (*k, v.as_str())).collect();

    relation.rename(&rename_slice)
}

fn perform_time_window_join(
    relation: &Relation,
    r_prev: &Relation,
    time_attr: &str,
    prev_attr_suffix: &str,
    window_size: i64,
    partition_by: &[&str],
) -> Relation {
    let effective_window_start_offset = window_size - 1;
    let time_attr_prev = format!("{}{}", time_attr, prev_attr_suffix);

    let p_by: Vec<String> = partition_by.iter().map(|s| s.to_string()).collect();
    let p_by_prev: Vec<String> = partition_by
        .iter()
        .map(|s| format!("{}{}", s, prev_attr_suffix))
        .collect();
    let t_curr = time_attr.to_string();
    let t_prev = time_attr_prev.clone();

    relation.theta_join(r_prev, move |curr, prev| {
        for (p_curr_name, p_prev_name) in p_by.iter().zip(p_by_prev.iter()) {
            let val_curr = curr.get(p_curr_name);
            let val_prev = prev.get(p_prev_name);
            if val_curr != val_prev {
                return false;
            }
        }

        if let (Some(ScalarValue::Int(curr_time)), Some(ScalarValue::Int(prev_time))) =
            (curr.get(&t_curr), prev.get(&t_prev))
        {
            let window_start = curr_time - effective_window_start_offset;
            *prev_time >= window_start && *prev_time <= *curr_time
        } else {
            false
        }
    })
}

fn summarize_moving_average(
    joined: &Relation,
    original_heading: &relvar_core::types::TupleType,
    value_attr: &str,
    prev_attr_suffix: &str,
) -> Result<Relation, DatabaseError> {
    let group_by_attrs: Vec<&str> = original_heading
        .attributes()
        .keys()
        .map(|k| k.as_str())
        .collect();

    let value_attr_prev = format!("{}{}", value_attr, prev_attr_suffix);
    let avg_agg = Aggregation::avg("moving_avg", &value_attr_prev);

    joined
        .summarize(&group_by_attrs, &[avg_agg])
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
}

/// Computes a Simple Moving Average (SMA) over a time window.
///
/// # Arguments
///
/// * `relation` - The input relation containing time series data.
/// * `time_attr` - The attribute representing the time step (must be Int).
/// * `value_attr` - The attribute to average (must be Int or Float).
/// * `window_size` - The size of the moving window (inclusive of current row).
/// * `partition_by` - Optional list of attributes to partition the series by (e.g., "symbol").
///
/// # Returns
///
/// A new relation with the original attributes plus a `moving_avg` attribute.
///
/// # Examples
///
/// ```
/// use relvar::{tuple, Relation, RelationType, TupleType, ScalarType};
/// use relvar::experimental::timeseries::moving_average;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Create a relation with (time, value)
/// let mut rel = Relation::new(RelationType::new(
///     TupleType::new()
///         .with_attribute("time", ScalarType::Int)
///         .with_attribute("value", ScalarType::Float)
/// ));
///
/// rel.insert(tuple! { time: 1, value: 10.0 })?;
/// rel.insert(tuple! { time: 2, value: 20.0 })?;
/// rel.insert(tuple! { time: 3, value: 30.0 })?;
///
/// // Compute 2-period moving average
/// let result = moving_average(&rel, "time", "value", 2, &[])?;
///
/// // Result at time 3: avg(20, 30) = 25.0
/// let t3 = result.tuples().find(|t| t.get_typed::<i64>("time") == Some(3)).unwrap();
/// assert_eq!(t3.get_typed::<f64>("moving_avg"), Some(25.0));
/// # Ok(())
/// # }
/// ```
///
/// # Known Issues
///
/// * **Attribute Naming Collision**: This function works by self-joining the relation. To do this,
///   it renames the attributes of the "previous" relation by appending `_prev` to them.
///   If your input relation already contains attributes ending in `_prev` that would conflict
///   with these generated names, the function will fail (panic or return error depending on the exact conflict).
///   **Workaround**: Ensure input attributes do not end with `_prev`.
pub fn moving_average(
    relation: &Relation,
    time_attr: &str,
    value_attr: &str,
    window_size: i64,
    partition_by: &[&str],
) -> Result<Relation, DatabaseError> {
    let original_heading = relation.relation_type().heading();
    let prev_attr_suffix = generate_safe_suffix(original_heading);
    let r_prev = prepare_self_join(relation, &prev_attr_suffix);
    let joined = perform_time_window_join(
        relation,
        &r_prev,
        time_attr,
        &prev_attr_suffix,
        window_size,
        partition_by,
    );
    summarize_moving_average(&joined, original_heading, value_attr, &prev_attr_suffix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_moving_average_simple() {
        let mut rel = Relation::new(RelationType::new(
            TupleType::new()
                .with_attribute("time", ScalarType::Int)
                .with_attribute("value", ScalarType::Float),
        ));

        rel.insert(tuple! { time: 1, value: 10.0 }).unwrap();
        rel.insert(tuple! { time: 2, value: 20.0 }).unwrap();
        rel.insert(tuple! { time: 3, value: 30.0 }).unwrap();
        rel.insert(tuple! { time: 4, value: 40.0 }).unwrap();

        // Window = 2
        // T1: avg(10) = 10
        // T2: avg(10, 20) = 15
        // T3: avg(20, 30) = 25
        // T4: avg(30, 40) = 35
        let res = moving_average(&rel, "time", "value", 2, &[]).unwrap();

        assert_eq!(res.cardinality(), 4);

        // Check T3
        let t3 = res
            .tuples()
            .find(|t| t.get_typed::<i64>("time") == Some(3))
            .unwrap();
        assert_eq!(t3.get_typed::<f64>("moving_avg").unwrap(), 25.0);
    }

    #[test]
    fn test_moving_average_partitioned() {
        let mut rel = Relation::new(RelationType::new(
            TupleType::new()
                .with_attribute("symbol", ScalarType::String)
                .with_attribute("time", ScalarType::Int)
                .with_attribute("value", ScalarType::Float),
        ));

        // AAPL
        rel.insert(tuple! { symbol: "AAPL", time: 1, value: 100.0 })
            .unwrap();
        rel.insert(tuple! { symbol: "AAPL", time: 2, value: 110.0 })
            .unwrap();

        // GOOGL
        rel.insert(tuple! { symbol: "GOOGL", time: 1, value: 200.0 })
            .unwrap();
        rel.insert(tuple! { symbol: "GOOGL", time: 2, value: 200.0 })
            .unwrap(); // Flat

        // Window = 2
        let res = moving_average(&rel, "time", "value", 2, &["symbol"]).unwrap();

        // AAPL T2: avg(100, 110) = 105
        let aapl_t2 = res
            .tuples()
            .find(|t| {
                t.get_typed::<String>("symbol").as_deref() == Some("AAPL")
                    && t.get_typed::<i64>("time") == Some(2)
            })
            .unwrap();
        assert_eq!(aapl_t2.get_typed::<f64>("moving_avg").unwrap(), 105.0);

        // GOOGL T2: avg(200, 200) = 200
        let googl_t2 = res
            .tuples()
            .find(|t| {
                t.get_typed::<String>("symbol").as_deref() == Some("GOOGL")
                    && t.get_typed::<i64>("time") == Some(2)
            })
            .unwrap();
        assert_eq!(googl_t2.get_typed::<f64>("moving_avg").unwrap(), 200.0);
    }

    #[test]
    fn test_moving_average_window_larger_than_history() {
        let mut rel = Relation::new(RelationType::new(
            TupleType::new()
                .with_attribute("time", ScalarType::Int)
                .with_attribute("value", ScalarType::Float),
        ));

        rel.insert(tuple! { time: 1, value: 10.0 }).unwrap();

        // Window = 5
        let res = moving_average(&rel, "time", "value", 5, &[]).unwrap();

        let t1 = res.tuples().next().unwrap();
        assert_eq!(t1.get_typed::<f64>("moving_avg").unwrap(), 10.0);
    }
}
