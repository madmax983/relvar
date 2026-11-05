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

    // TTM: SUMMARIZE r PER (r{}) has one tuple per tuple of r{}; r{} is
    // TABLE_DUM when r is empty, so the result is empty.
    assert_eq!(result.cardinality(), 0);
    assert_eq!(result.degree(), 1);
    assert!(result.relation_type().has_attribute("total_count"));
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

    // Average on empty relation (no grouping): no groups, so no tuples
    // (and no fabricated 0.0 average).
    let result = relation
        .summarize(&[], &[Aggregation::avg("avg_salary", "salary")])
        .unwrap();
    assert_eq!(result.cardinality(), 0);
    assert_eq!(
        result
            .relation_type()
            .heading()
            .get_attribute_type("avg_salary"),
        Some(&ScalarType::Float)
    );
}

#[test]
fn test_min_on_empty_set() {
    let heading = TupleType::new().with_attribute("salary".to_string(), ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let empty_rel = Relation::new(rel_type);

    // MIN over an empty relation: there are no groups, so MIN is never
    // applied to an empty set and the result is empty.
    let result = empty_rel
        .summarize(
            &[],
            &[Aggregation::min("min_salary", "salary", ScalarType::Int)],
        )
        .unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_max_on_empty_set() {
    let heading = TupleType::new().with_attribute("salary".to_string(), ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let empty_rel = Relation::new(rel_type);

    // MAX over an empty relation: no groups, empty result.
    let result = empty_rel
        .summarize(
            &[],
            &[Aggregation::max("max_salary", "salary", ScalarType::Int)],
        )
        .unwrap();
    assert!(result.is_empty());
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

fn salaries() -> Relation {
    let heading = TupleType::new()
        .with_attribute("dept", ScalarType::String)
        .with_attribute("salary", ScalarType::Int)
        .with_attribute("bonus", ScalarType::Float);
    let mut relation = Relation::new(RelationType::new(heading));
    relation
        .insert(tuple! { dept: "IT", salary: 10i64, bonus: 1.5 })
        .unwrap();
    relation
        .insert(tuple! { dept: "IT", salary: 20i64, bonus: 2.5 })
        .unwrap();
    relation
        .insert(tuple! { dept: "HR", salary: 30i64, bonus: 0.5 })
        .unwrap();
    relation
}

/// A caller-declared MIN result type that differs from the attribute type
/// used to produce tuples that violate their own heading.
#[test]
fn test_summarize_rejects_min_result_type_mismatch() {
    let result = salaries().summarize(
        &["dept"],
        &[Aggregation::min("m", "salary", ScalarType::String)],
    );
    assert!(
        matches!(
            result,
            Err(SummarizeError::ResultTypeMismatch { ref result_name, ref declared, ref expected })
                if result_name == "m"
                    && *declared == ScalarType::String
                    && *expected == ScalarType::Int
        ),
        "expected ResultTypeMismatch, got {result:?}"
    );
}

#[test]
fn test_summarize_rejects_max_result_type_mismatch() {
    let result = salaries().summarize(&[], &[Aggregation::max("m", "bonus", ScalarType::Int)]);
    assert!(matches!(
        result,
        Err(SummarizeError::ResultTypeMismatch { .. })
    ));
}

/// Public fields let callers build an aggregation with any result type;
/// summarize must still check it against what the function produces.
#[test]
fn test_summarize_rejects_count_declared_with_wrong_type() {
    let agg = Aggregation {
        result_name: "n".to_string(),
        result_type: ScalarType::String,
        function: AggregationFn::Count,
    };
    let result = salaries().summarize(&["dept"], &[agg]);
    assert!(matches!(
        result,
        Err(SummarizeError::ResultTypeMismatch { .. })
    ));
}

/// SUM over a Float attribute produces a Float, so the Int constructor is
/// ill-typed for it (use `sum_float`).
#[test]
fn test_summarize_rejects_int_sum_over_float_attribute() {
    let result = salaries().summarize(&["dept"], &[Aggregation::sum("s", "bonus")]);
    assert!(matches!(
        result,
        Err(SummarizeError::ResultTypeMismatch { .. })
    ));
    let ok = salaries()
        .summarize(&["dept"], &[Aggregation::sum_float("s", "bonus")])
        .unwrap();
    assert_eq!(ok.cardinality(), 2);
}

/// Type errors are static: they are reported even when there is no tuple
/// to aggregate.
#[test]
fn test_summarize_rejects_sum_over_string_even_when_empty() {
    let empty = Relation::new(salaries().relation_type().clone());
    let result = empty.summarize(&["dept"], &[Aggregation::sum("s", "dept")]);
    assert!(matches!(result, Err(SummarizeError::AggregationError(_))));
}

#[test]
fn test_summarize_rejects_unknown_aggregated_attribute_even_when_empty() {
    let empty = Relation::new(salaries().relation_type().clone());
    let result = empty.summarize(&["dept"], &[Aggregation::max("m", "nope", ScalarType::Int)]);
    assert!(matches!(result, Err(SummarizeError::AggregationError(_))));
}

/// Every result tuple conforms to the result heading (closure).
#[test]
fn test_summarize_result_tuples_conform_to_heading() {
    let result = salaries()
        .summarize(
            &["dept"],
            &[
                Aggregation::count("n"),
                Aggregation::sum("s", "salary"),
                Aggregation::avg("a", "salary"),
                Aggregation::min("lo", "bonus", ScalarType::Float),
                Aggregation::max("hi", "salary", ScalarType::Int),
            ],
        )
        .unwrap();
    let heading = result.relation_type().heading();
    assert_eq!(result.cardinality(), 2);
    for t in result.tuples() {
        assert!(t.conforms_to(heading));
        assert_eq!(t.tuple_type(), heading);
    }
}

/// `0.0` and `-0.0` are distinct values, and the sum of a group whose only
/// value is `-0.0` is `-0.0` (the IEEE additive identity is `-0.0`), so SUM
/// and AVG must not turn it into `0.0`. Found by `tests/ttm_refinement.rs`.
#[test]
fn test_sum_and_avg_of_negative_zero_are_negative_zero() {
    let heading = TupleType::new()
        .with_attribute("k", ScalarType::Int)
        .with_attribute("v", ScalarType::Float);
    let mut relation = Relation::new(RelationType::new(heading));
    relation.insert(tuple! { k: 1i64, v: -0.0f64 }).unwrap();

    let result = relation
        .summarize(
            &[],
            &[Aggregation::sum_float("s", "v"), Aggregation::avg("m", "v")],
        )
        .unwrap();

    let t = result.tuples().next().unwrap();
    assert_eq!(t.get("s"), Some(&ScalarValue::Float(-0.0)));
    assert_eq!(t.get("m"), Some(&ScalarValue::Float(-0.0)));
}
