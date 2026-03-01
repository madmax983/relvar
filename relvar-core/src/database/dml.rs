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

    let kept_tuples: Vec<Tuple> = current_relation
        .into_iter()
        .filter(|tuple| !predicate(tuple))
        .collect();

    let delete_count = initial_cardinality - kept_tuples.len();

    let new_relation = Relation::from_tuples(relation_type, kept_tuples)?;

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

    let new_relation = Relation::from_tuples(relation_type, tuples)?;

    Ok((new_relation, update_count))
}
