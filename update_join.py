content = open("relvar-core/src/algebra/join.rs", "r").read()

old_code = """
/// Helper to combine two tuples into a single tuple.
///
/// Attributes from `primary` take precedence over `secondary` if there are collisions.
fn combine_tuples(
    primary: &Tuple,
    secondary: &Tuple,
    result_heading: &Arc<TupleType>,
) -> Result<Tuple, DatabaseError> {
    let values = merge_tuple_values(primary, secondary);

    // Safety:
    // 1. Primary and secondary tuples are valid and conform to their headings.
    // 2. Result heading is the union of both headings.
    // 3. We combined values from both, respecting types.
    // 4. Therefore, the resulting map conforms to result_heading.
    //
    // BTreeMap::from_iter is efficient (O(N)) when input is already sorted.
    // We defer cloning to the iterator mapping step to avoid repeatedly cloning
    // discarded values or allocating strings inside the hot loop.
    let combined_values = BTreeMap::from_iter(values);

    Ok(Tuple::new_unchecked(
        result_heading.clone(),
        combined_values,
    ))
}

/// Helper to merge values from two tuples.
///
/// Optimization: Uses a merge-sort style iteration to combine values.
/// Since both BTreeMaps are sorted, we can iterate through them simultaneously
/// and build the new map in O(N) time without O(log N) insertions.
fn merge_tuple_values<'a>(primary: &'a Tuple, secondary: &'a Tuple) -> Vec<(String, ScalarValue)> {
    let mut iter_p = primary.values().iter().peekable();
    let mut iter_s = secondary.values().iter().peekable();

    // Pre-allocate to avoid reallocations
    let mut values = Vec::with_capacity(primary.degree() + secondary.degree());

    loop {
        if !merge_next_values(&mut iter_p, &mut iter_s, &mut values) {
            break;
        }
    }
    values
}

/// Helper to merge the next value from the primary and secondary tuples.
/// Returns `true` if a value was merged, `false` if both iterators are empty.
fn merge_next_values<'a>(
    iter_p: &mut std::iter::Peekable<std::collections::btree_map::Iter<'a, String, ScalarValue>>,
    iter_s: &mut std::iter::Peekable<std::collections::btree_map::Iter<'a, String, ScalarValue>>,
    values: &mut Vec<(String, ScalarValue)>,
) -> bool {
    match (iter_p.peek(), iter_s.peek()) {
        (Some(&(k_p, v_p)), Some(&(k_s, v_s))) => {
            if k_p == k_s {
                // Collision: Primary wins (as per doc)
                // Consume both since they match
                values.push((k_p.clone(), v_p.clone()));
                iter_p.next();
                iter_s.next();
            } else if k_p < k_s {
                // Primary is smaller, take it
                values.push((k_p.clone(), v_p.clone()));
                iter_p.next();
            } else {
                // Secondary is smaller, take it
                values.push((k_s.clone(), v_s.clone()));
                iter_s.next();
            }
            true
        }
        (Some(&(k_p, v_p)), None) => {
            // Only primary remaining
            values.push((k_p.clone(), v_p.clone()));
            iter_p.next();
            true
        }
        (None, Some(&(k_s, v_s))) => {
            // Only secondary remaining
            values.push((k_s.clone(), v_s.clone()));
            iter_s.next();
            true
        }
        (None, None) => false,
    }
}
"""

new_code = """
/// Helper to combine two tuples into a single tuple.
///
/// Attributes from `primary` take precedence over `secondary` if there are collisions.
fn combine_tuples(
    primary: &Tuple,
    secondary: &Tuple,
    result_heading: &Arc<TupleType>,
) -> Result<Tuple, DatabaseError> {
    // Optimization: Uses a merge-sort style iteration to combine values.
    // Since both BTreeMaps are sorted, we can iterate through them simultaneously
    // and build the new map in O(N) time without O(log N) insertions.
    // By passing a custom Iterator to `BTreeMap::from_iter`, we avoid the intermediate
    // heap allocation of a `Vec`.
    let mut iter_p = primary.values().iter().peekable();
    let mut iter_s = secondary.values().iter().peekable();

    let iter = std::iter::from_fn(move || {
        match (iter_p.peek(), iter_s.peek()) {
            (Some(&(k_p, v_p)), Some(&(k_s, v_s))) => {
                if k_p == k_s {
                    // Collision: Primary wins (as per doc)
                    // Consume both since they match
                    let res = (k_p.clone(), v_p.clone());
                    iter_p.next();
                    iter_s.next();
                    Some(res)
                } else if k_p < k_s {
                    // Primary is smaller, take it
                    let res = (k_p.clone(), v_p.clone());
                    iter_p.next();
                    Some(res)
                } else {
                    // Secondary is smaller, take it
                    let res = (k_s.clone(), v_s.clone());
                    iter_s.next();
                    Some(res)
                }
            }
            (Some(&(k_p, v_p)), None) => {
                // Only primary remaining
                let res = (k_p.clone(), v_p.clone());
                iter_p.next();
                Some(res)
            }
            (None, Some(&(k_s, v_s))) => {
                // Only secondary remaining
                let res = (k_s.clone(), v_s.clone());
                iter_s.next();
                Some(res)
            }
            (None, None) => None,
        }
    });

    // Safety:
    // 1. Primary and secondary tuples are valid and conform to their headings.
    // 2. Result heading is the union of both headings.
    // 3. We combined values from both, respecting types.
    // 4. Therefore, the resulting map conforms to result_heading.
    //
    // BTreeMap::from_iter is efficient (O(N)) when input is already sorted.
    let combined_values = BTreeMap::from_iter(iter);

    Ok(Tuple::new_unchecked(
        result_heading.clone(),
        combined_values,
    ))
}
"""

if old_code in content:
    with open("relvar-core/src/algebra/join.rs", "w") as f:
        f.write(content.replace(old_code, new_code))
    print("Success")
else:
    print("Old code not found")
