use crate::error::DatabaseError;
use crate::values::{Relation, Tuple};

pub(crate) fn compute_relation_after_delete<F>(
    current_relation: Relation,
    predicate: F,
) -> Result<(Relation, usize), DatabaseError>
where
    F: Fn(&Tuple) -> bool,
{
    let initial_cardinality = current_relation.cardinality();
    let relation_type = current_relation.relation_type().clone();

    // Optimization: Pass the iterator directly to `from_tuples_unchecked`
    // instead of collecting into an intermediate `Vec`. Since the source
    // relation is valid, the filtered tuples are also guaranteed to be valid,
    // allowing us to bypass redundant type checking.
    let new_relation = Relation::from_tuples_unchecked(
        relation_type,
        current_relation
            .into_iter()
            .filter(|tuple| !predicate(tuple)),
    );

    let delete_count = initial_cardinality - new_relation.cardinality();

    Ok((new_relation, delete_count))
}

pub(crate) fn compute_relation_after_update<F, U>(
    current_relation: Relation,
    predicate: F,
    updater: U,
) -> Result<(Relation, usize), DatabaseError>
where
    F: Fn(&Tuple) -> bool,
    U: Fn(&Tuple) -> Tuple,
{
    let relation_type = current_relation.relation_type().clone();
    let expected_type = relation_type.tuple_type().clone();
    let initial_cardinality = current_relation.cardinality();

    let (tuples, update_count) = current_relation.into_iter().try_fold(
        (Vec::with_capacity(initial_cardinality), 0),
        |(mut acc, count), tuple| {
            if !predicate(&tuple) {
                acc.push(tuple);
                return Ok((acc, count));
            }

            let updated_tuple = updater(&tuple);

            if !updated_tuple.conforms_to(&expected_type) {
                return Err(DatabaseError::TupleMismatch);
            }

            acc.push(updated_tuple);
            Ok((acc, count + 1))
        },
    )?;

    // Optimization: The updated tuples are already validated via `conforms_to`
    // inside the `try_fold` loop. We can safely use `from_tuples_unchecked`
    // to build the new relation, avoiding a second O(N*M) validation pass.
    let new_relation = Relation::from_tuples_unchecked(relation_type, tuples);

    Ok((new_relation, update_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    fn test_relation_type() -> RelationType {
        let tuple_type = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);
        RelationType::new(tuple_type)
    }

    fn test_relation() -> Relation {
        let rel_type = test_relation_type();
        let mut relation = Relation::new(rel_type);
        relation.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
        relation.insert(tuple! { id: 2i64, name: "Bob" }).unwrap();
        relation
            .insert(tuple! { id: 3i64, name: "Charlie" })
            .unwrap();
        relation
    }

    #[test]
    fn should_delete_matching_tuples() {
        let relation = test_relation();
        let (new_relation, delete_count) =
            compute_relation_after_delete(relation, |t| t.get_typed::<i64>("id").unwrap() == 2)
                .unwrap();

        assert_eq!(delete_count, 1);
        assert_eq!(new_relation.cardinality(), 2);
        assert!(!new_relation.contains(&tuple! { id: 2i64, name: "Bob" }));
        assert!(new_relation.contains(&tuple! { id: 1i64, name: "Alice" }));
        assert!(new_relation.contains(&tuple! { id: 3i64, name: "Charlie" }));
    }

    #[test]
    fn should_return_zero_deletes_when_no_match() {
        let relation = test_relation();
        let (new_relation, delete_count) =
            compute_relation_after_delete(relation, |t| t.get_typed::<i64>("id").unwrap() == 99)
                .unwrap();

        assert_eq!(delete_count, 0);
        assert_eq!(new_relation.cardinality(), 3);
    }

    #[test]
    fn should_update_matching_tuples() {
        let relation = test_relation();
        let (new_relation, update_count) = compute_relation_after_update(
            relation,
            |t| t.get_typed::<i64>("id").unwrap() == 2,
            |t| tuple! { id: t.get_typed::<i64>("id").unwrap(), name: "Robert" },
        )
        .unwrap();

        assert_eq!(update_count, 1);
        assert_eq!(new_relation.cardinality(), 3);
        assert!(!new_relation.contains(&tuple! { id: 2i64, name: "Bob" }));
        assert!(new_relation.contains(&tuple! { id: 2i64, name: "Robert" }));
        assert!(new_relation.contains(&tuple! { id: 1i64, name: "Alice" }));
    }

    #[test]
    fn should_return_zero_updates_when_no_match() {
        let relation = test_relation();
        let (new_relation, update_count) = compute_relation_after_update(
            relation,
            |t| t.get_typed::<i64>("id").unwrap() == 99,
            |t| t.clone(),
        )
        .unwrap();

        assert_eq!(update_count, 0);
        assert_eq!(new_relation.cardinality(), 3);
    }

    #[test]
    fn should_return_error_when_updater_creates_invalid_tuple() {
        let relation = test_relation();

        let result = compute_relation_after_update(
            relation,
            |t| t.get_typed::<i64>("id").unwrap() == 2,
            |_t| tuple! { id: 2i64, age: 30i64 }, // Invalid tuple type
        );

        assert!(matches!(result, Err(DatabaseError::TupleMismatch)));
    }
}
