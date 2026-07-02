use relvar_core::algebra::{Aggregation, AggregationFn, ExtendError, GroupError, SummarizeError};
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue, Tuple};

#[test]
fn test_extend_into_computation_mismatch() {
    let t_type = TupleType::new().with_attribute("id", ScalarType::Int);
    let rel_type = RelationType::new(t_type.clone());
    let tuple = Tuple::new(t_type, vec![("id".to_string(), ScalarValue::Int(1))]).unwrap();
    let rel = Relation::from_tuples(rel_type, vec![tuple]).unwrap();

    let err = rel
        .extend_into("new_attr", ScalarType::Int, |_t| {
            ScalarValue::String("not an int".to_string())
        })
        .unwrap_err();

    assert!(matches!(err, ExtendError::TupleCreation(_)));
}

#[test]
fn test_extend_into_attribute_exists() {
    let t_type = TupleType::new().with_attribute("id", ScalarType::Int);
    let rel_type = RelationType::new(t_type.clone());
    let rel = Relation::new(rel_type);

    let err = rel
        .extend_into("id", ScalarType::Int, |_t| ScalarValue::Int(2))
        .unwrap_err();
    assert!(matches!(err, ExtendError::AttributeExists(_)));
}

#[test]
fn test_rename_into() {
    let t_type = TupleType::new().with_attribute("id", ScalarType::Int);
    let rel_type = RelationType::new(t_type.clone());
    let tuple = Tuple::new(t_type, vec![("id".to_string(), ScalarValue::Int(1))]).unwrap();
    let rel = Relation::from_tuples(rel_type, vec![tuple]).unwrap();

    let renamed = rel.rename_into(&[("id", "new_id")]);
    assert!(renamed.relation_type().has_attribute("new_id"));
}

#[test]
fn test_project_ordering() {
    let t_type = TupleType::new()
        .with_attribute("z", ScalarType::Int)
        .with_attribute("a", ScalarType::Int)
        .with_attribute("m", ScalarType::Int);
    let rel_type = RelationType::new(t_type.clone());
    let tuple = Tuple::new(
        t_type,
        vec![
            ("z".to_string(), ScalarValue::Int(1)),
            ("a".to_string(), ScalarValue::Int(2)),
            ("m".to_string(), ScalarValue::Int(3)),
        ],
    )
    .unwrap();
    let rel = Relation::from_tuples(rel_type, vec![tuple]).unwrap();

    let projected = rel.project(&["z", "a"]);
    assert_eq!(projected.relation_type().heading().degree(), 2);
}

#[test]
fn test_intersect_into() {
    let t_type = TupleType::new().with_attribute("id", ScalarType::Int);
    let rel_type = RelationType::new(t_type.clone());
    let tuple1 = Tuple::new(
        t_type.clone(),
        vec![("id".to_string(), ScalarValue::Int(1))],
    )
    .unwrap();
    let tuple2 = Tuple::new(
        t_type.clone(),
        vec![("id".to_string(), ScalarValue::Int(2))],
    )
    .unwrap();

    let rel1 =
        Relation::from_tuples(rel_type.clone(), vec![tuple1.clone(), tuple2.clone()]).unwrap();
    let rel2 = Relation::from_tuples(rel_type.clone(), vec![tuple2.clone()]).unwrap();

    let intersected = rel1.intersect_into(&rel2).unwrap();
    assert_eq!(intersected.cardinality(), 1);
}

#[test]
fn test_union_into() {
    let t_type = TupleType::new().with_attribute("id", ScalarType::Int);
    let rel_type = RelationType::new(t_type.clone());
    let tuple1 = Tuple::new(
        t_type.clone(),
        vec![("id".to_string(), ScalarValue::Int(1))],
    )
    .unwrap();
    let tuple2 = Tuple::new(
        t_type.clone(),
        vec![("id".to_string(), ScalarValue::Int(2))],
    )
    .unwrap();

    let rel1 = Relation::from_tuples(rel_type.clone(), vec![tuple1.clone()]).unwrap();
    let rel2 = Relation::from_tuples(rel_type.clone(), vec![tuple2.clone()]).unwrap();

    let unioned = rel1.union_into(&rel2).unwrap();
    assert_eq!(unioned.cardinality(), 2);
}

#[test]
fn test_group_ungroup_error_paths() {
    let inner_t_type = TupleType::new().with_attribute("val", ScalarType::Int);
    let inner_rel_type = RelationType::new(inner_t_type.clone());

    let outer_t_type = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute(
            "rva",
            ScalarType::Relation(Box::new(inner_rel_type.clone())),
        );
    let outer_rel_type = RelationType::new(outer_t_type.clone());

    let inner_tuple =
        Tuple::new(inner_t_type, vec![("val".to_string(), ScalarValue::Int(1))]).unwrap();
    let inner_rel = Relation::from_tuples(inner_rel_type, vec![inner_tuple]).unwrap();

    let outer_tuple = Tuple::new(
        outer_t_type.clone(),
        vec![
            ("id".to_string(), ScalarValue::Int(1)),
            ("rva".to_string(), ScalarValue::Relation(inner_rel)),
        ],
    )
    .unwrap();

    let rel = Relation::from_tuples(outer_rel_type, vec![outer_tuple]).unwrap();

    // Group attribute not found
    let err = rel.group(&["missing"], "new_rva").unwrap_err();
    assert!(matches!(err, GroupError::AttributeNotFound(_)));
}

#[test]
fn test_join_error_paths() {
    let t_type1 = TupleType::new().with_attribute("id", ScalarType::Int);
    let rel_type1 = RelationType::new(t_type1.clone());
    let tuple1 = Tuple::new(t_type1, vec![("id".to_string(), ScalarValue::Int(1))]).unwrap();
    let rel1 = Relation::from_tuples(rel_type1, vec![tuple1]).unwrap();

    // Theta join empty result because of condition
    let res = rel1.theta_join(&rel1, |_, _| false);
    assert_eq!(res.cardinality(), 0);
}

#[test]
fn test_summarize_avg_error() {
    let t_type = TupleType::new()
        .with_attribute("group_id", ScalarType::Int)
        .with_attribute("val", ScalarType::String);
    let rel_type = RelationType::new(t_type.clone());
    let tuple = Tuple::new(
        t_type,
        vec![
            ("group_id".to_string(), ScalarValue::Int(1)),
            (
                "val".to_string(),
                ScalarValue::String("not num".to_string()),
            ),
        ],
    )
    .unwrap();
    let rel = Relation::from_tuples(rel_type, vec![tuple]).unwrap();

    let agg = Aggregation {
        result_name: "avg_val".to_string(),
        result_type: ScalarType::Float,
        function: AggregationFn::Avg("val".to_string()),
    };

    let err = rel.summarize(&["group_id"], &[agg]).unwrap_err();
    assert!(matches!(err, SummarizeError::AggregationError(_)));

    // Empty sum extremum
    let empty_rel = Relation::new(rel.relation_type().clone());
    let agg2 = Aggregation {
        result_name: "max_val".to_string(),
        result_type: ScalarType::String,
        function: AggregationFn::Max("val".to_string()),
    };
    let res = empty_rel.summarize(&["group_id"], &[agg2]).unwrap();
    assert_eq!(res.cardinality(), 0);
}

// Intersect coverage test

#[test]
fn test_intersect_larger_self() {
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::Relation;

    let rel_type = RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int));

    let mut rel1 = Relation::new(rel_type.clone());
    rel1.insert(tuple! { x: 1i64 }).unwrap();
    rel1.insert(tuple! { x: 2i64 }).unwrap();
    rel1.insert(tuple! { x: 3i64 }).unwrap();

    let mut rel2 = Relation::new(rel_type.clone());
    rel2.insert(tuple! { x: 2i64 }).unwrap();

    // rel1 is larger than rel2
    let result = rel1.intersect(&rel2).unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_union_into_larger_self() {
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::Relation;

    let rel_type = RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int));

    let mut rel1 = Relation::new(rel_type.clone());
    rel1.insert(tuple! { x: 1i64 }).unwrap();
    rel1.insert(tuple! { x: 2i64 }).unwrap();
    rel1.insert(tuple! { x: 3i64 }).unwrap();

    let mut rel2 = Relation::new(rel_type.clone());
    rel2.insert(tuple! { x: 2i64 }).unwrap();
    rel2.insert(tuple! { x: 4i64 }).unwrap();

    // rel1 is larger than rel2
    let result = rel1.union_into(&rel2).unwrap();
    assert_eq!(result.cardinality(), 4);
}

#[test]
fn test_intersect_into_larger_self_uncovered() {
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::Relation;

    let rel_type = RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int));

    let mut rel1 = Relation::new(rel_type.clone());
    rel1.insert(tuple! { x: 1i64 }).unwrap();
    rel1.insert(tuple! { x: 2i64 }).unwrap();
    rel1.insert(tuple! { x: 3i64 }).unwrap();

    let mut rel2 = Relation::new(rel_type.clone());
    rel2.insert(tuple! { x: 2i64 }).unwrap();

    // rel1 is larger than rel2, testing intersect_into where self is larger
    let result = rel1.intersect_into(&rel2).unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_intersect_into_type_mismatch() {
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::Relation;

    let type1 = TupleType::new().with_attribute("x", ScalarType::Int);
    let rel1 = Relation::new(RelationType::new(type1));

    let type2 = TupleType::new().with_attribute("x", ScalarType::String);
    let rel2 = Relation::new(RelationType::new(type2));

    let result = rel1.intersect_into(&rel2);
    assert!(result.is_err());
}

#[test]
fn test_union_into_type_mismatch() {
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use relvar_core::values::Relation;

    let type1 = TupleType::new().with_attribute("x", ScalarType::Int);
    let rel1 = Relation::new(RelationType::new(type1));

    let type2 = TupleType::new().with_attribute("x", ScalarType::String);
    let rel2 = Relation::new(RelationType::new(type2));

    let result = rel1.union_into(&rel2);
    assert!(result.is_err());
}

#[test]
fn test_intersect_optimization_empty() {
    let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    let rel_type = RelationType::new(heading);
    let r1 = Relation::new(rel_type.clone());
    let r2 = Relation::new(rel_type);

    let result = r1.intersect(&r2).unwrap();
    assert_eq!(result.cardinality(), 0);
}

#[test]
fn test_difference_optimization_empty() {
    let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    let rel_type = RelationType::new(heading);
    let r1 = Relation::new(rel_type.clone());
    let r2 = Relation::new(rel_type);

    let result = r1.difference(&r2).unwrap();
    assert_eq!(result.cardinality(), 0);
}

#[test]
fn test_union_optimization_empty() {
    let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    let rel_type = RelationType::new(heading);
    let r1 = Relation::new(rel_type.clone());
    let r2 = Relation::new(rel_type);

    let result = r1.union(&r2).unwrap();
    assert_eq!(result.cardinality(), 0);
}

#[test]
fn test_group_no_attributes_specified() {
    let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    let r = Relation::new(RelationType::new(heading));

    let result = r.group(&[], "my_rva");
    assert!(matches!(result, Err(GroupError::NoAttributesSpecified)));
}

#[test]
fn test_difference_optimization_empty_again() {
    let heading1 = TupleType::new().with_attribute("id", ScalarType::Int);
    let heading2 = TupleType::new().with_attribute("id", ScalarType::String);
    let r1 = Relation::new(RelationType::new(heading1));
    let r2 = Relation::new(RelationType::new(heading2));

    // type mismatch in difference
    let result = r1.clone().difference(&r2);
    assert!(matches!(result, Err(relvar_core::algebra::DifferenceError)));

    let result = r1.difference_into(&r2);
    assert!(matches!(result, Err(relvar_core::algebra::DifferenceError)));
}

#[test]
fn test_semijoin_edge_cases() {
    // cover empty returns
    let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    let rel_type = RelationType::new(heading);
    let mut r1 = Relation::new(rel_type.clone());
    r1.insert(relvar_core::tuple! { id: 1i64 }).unwrap();
    let r_empty = Relation::new(rel_type);

    let result = r1.clone().semijoin_into(&r_empty);
    assert_eq!(result.cardinality(), 0);

    let result = r1.clone().semidifference_into(&r_empty);
    assert_eq!(result.cardinality(), 1);

    let result = r_empty.clone().semidifference_into(&r1);
    assert_eq!(result.cardinality(), 0);
}
