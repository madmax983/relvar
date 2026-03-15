//! Join operators for combining relations.
//!
//! This module implements the join operators from relational algebra:
//!
//! - **Natural Join** - Joins on common attributes, combining matching tuples
//! - **Theta Join** - Joins with an arbitrary predicate condition
//!
//! # TTM Compliance
//!
//! - Natural join matches on attribute names and types (not physical identifiers)
//! - Result heading is the union of both relation headings
//! - Duplicate tuples are automatically eliminated (set semantics)
//!
//! # Example
//!
//! ```
//! use relvar_core::types::{TupleType, RelationType, ScalarType};
//! use relvar_core::values::Relation;
//! use relvar_core::tuple;
//!
//! // Employees with dept_id
//! let emp_heading = TupleType::new()
//!     .with_attribute("emp_id", ScalarType::Int)
//!     .with_attribute("name", ScalarType::String)
//!     .with_attribute("dept_id", ScalarType::Int);
//!
//! let mut employees = Relation::new(RelationType::new(emp_heading));
//! employees.insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 }).unwrap();
//!
//! // Departments with dept_id
//! let dept_heading = TupleType::new()
//!     .with_attribute("dept_id", ScalarType::Int)
//!     .with_attribute("dept_name", ScalarType::String);
//!
//! let mut departments = Relation::new(RelationType::new(dept_heading));
//! departments.insert(tuple! { dept_id: 10i64, dept_name: "Engineering" }).unwrap();
//!
//! // Natural join on dept_id
//! let result = employees.join(&departments).unwrap();
//! assert_eq!(result.degree(), 4);  // emp_id, name, dept_id, dept_name
//! ```

use crate::error::DatabaseError;
use crate::types::{RelationType, TupleType};
use crate::values::{Relation, ScalarValue, Tuple};
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

impl Relation {
    /// Performs a natural join with another relation.
    ///
    /// The natural join combines tuples from two relations based on matching values
    /// in their common attributes (attributes with the same name and type). The
    /// result contains combined tuples where all common attributes have equal values.
    ///
    /// # Arguments
    ///
    /// * `other` - The relation to join with
    ///
    /// # Returns
    ///
    /// A new relation with the union of both headings. Each result tuple is a
    /// combination of tuples from both relations that match on common attributes.
    ///
    /// # Behavior
    ///
    /// - If there are no common attributes, produces a Cartesian product
    /// - Common attributes appear once in the result (not duplicated)
    /// - Tuples that don't match on common attributes are excluded
    /// - Result maintains set semantics (no duplicate tuples)
    ///
    /// # Complexity
    ///
    /// O(n + m) where n and m are the cardinalities of the two relations.
    /// This implementation uses a Hash Join algorithm, significantly outperforming
    /// the O(n * m) nested-loop join for large relations.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// // Employees
    /// let emp_heading = TupleType::new()
    ///     .with_attribute("emp_id", ScalarType::Int)
    ///     .with_attribute("dept_id", ScalarType::Int);
    ///
    /// let mut employees = Relation::new(RelationType::new(emp_heading));
    /// employees.insert(tuple! { emp_id: 1i64, dept_id: 10i64 }).unwrap();
    /// employees.insert(tuple! { emp_id: 2i64, dept_id: 20i64 }).unwrap();
    ///
    /// // Departments
    /// let dept_heading = TupleType::new()
    ///     .with_attribute("dept_id", ScalarType::Int)
    ///     .with_attribute("budget", ScalarType::Int);
    ///
    /// let mut departments = Relation::new(RelationType::new(dept_heading));
    /// departments.insert(tuple! { dept_id: 10i64, budget: 100000i64 }).unwrap();
    ///
    /// let result = employees.join(&departments).unwrap();
    /// assert_eq!(result.cardinality(), 1);  // Only emp 1 matches (dept 10)
    /// ```
    pub fn join(&self, other: &Relation) -> Result<Self, DatabaseError> {
        let common_attrs = compute_common_attributes(self, other);
        let result_heading = compute_natural_join_heading(self, other);

        let result_rel_type = RelationType::new(result_heading.clone());
        let result_heading_arc = Arc::new(result_heading);

        // Perform Hash Join
        let (build_rel, probe_rel) = determine_hash_join_sides(self, other);

        let joined_tuples =
            perform_hash_join(build_rel, probe_rel, &common_attrs, &result_heading_arc)?;

        Ok(Relation::from_tuples(result_rel_type, joined_tuples)?)
    }

    /// Performs a theta join with another relation using an arbitrary predicate.
    ///
    /// The theta join (θ-join) is a more general form of join that combines
    /// tuples based on any arbitrary predicate, not just equality on common
    /// attributes. This allows for comparisons like greater-than, less-than,
    /// or complex multi-attribute conditions.
    ///
    /// # Arguments
    ///
    /// * `other` - The relation to join with
    /// * `predicate` - A function that takes references to tuples from both
    ///   relations and returns `true` if they should be combined
    ///
    /// # Returns
    ///
    /// A new relation with the union of both headings. Each result tuple is a
    /// combination of tuples from both relations for which the predicate
    /// returns `true`.
    ///
    /// # Behavior
    ///
    /// - Attributes from both relations are combined, subject to the collision rule below.
    /// - **Attribute Collision Warning:** If relations have common attribute names,
    ///   the attribute from the *second* relation is silently dropped from the result
    ///   heading, and its values are discarded. The first relation's attribute takes
    ///   precedence. This is a known limitation; ensure attributes are uniquely named
    ///   before joining.
    /// - Result maintains set semantics.
    ///
    /// # Complexity
    ///
    /// O(n * m) where n and m are the cardinalities of the two relations.
    ///
    /// # Example: Standard Usage
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::{Relation, ScalarValue};
    /// use relvar_core::tuple;
    ///
    /// // Employees with salaries
    /// let emp_heading = TupleType::new()
    ///     .with_attribute("emp_id", ScalarType::Int)
    ///     .with_attribute("salary", ScalarType::Int);
    ///
    /// let mut employees = Relation::new(RelationType::new(emp_heading));
    /// employees.insert(tuple! { emp_id: 1i64, salary: 50000i64 }).unwrap();
    /// employees.insert(tuple! { emp_id: 2i64, salary: 75000i64 }).unwrap();
    ///
    /// // Departments with minimum salary requirements
    /// let dept_heading = TupleType::new()
    ///     .with_attribute("dept_id", ScalarType::Int)
    ///     .with_attribute("min_salary", ScalarType::Int);
    ///
    /// let mut departments = Relation::new(RelationType::new(dept_heading));
    /// departments.insert(tuple! { dept_id: 10i64, min_salary: 60000i64 }).unwrap();
    ///
    /// // Join employees to departments where salary meets minimum
    /// let result = employees.theta_join(&departments, |emp, dept| {
    ///     let salary = emp.get_typed::<i64>("salary").unwrap();
    ///     let min_sal = dept.get_typed::<i64>("min_salary").unwrap();
    ///     salary >= min_sal
    /// });
    /// // Only employee 2 (salary 75000) qualifies for dept 10 (min 60000)
    /// assert_eq!(result.cardinality(), 1);
    /// ```
    ///
    /// # Example: Safe Usage (Avoiding Collisions)
    ///
    /// To avoid silent attribute collisions when both relations share attribute names
    /// (e.g., both have `id`), rename the attributes before joining:
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    ///
    /// let mut r1 = Relation::new(rel_type.clone());
    /// r1.insert(tuple! { id: 1i64 }).unwrap();
    ///
    /// let mut r2 = Relation::new(rel_type);
    /// r2.insert(tuple! { id: 2i64 }).unwrap();
    ///
    /// // DANGEROUS: r1.theta_join(&r2, ...) will drop r2's "id" attribute!
    ///
    /// // SAFE: Rename r2's attribute first
    /// let r2_renamed = r2.rename(&[("id", "r2_id")]);
    ///
    /// let result = r1.theta_join(&r2_renamed, |t1, t2| true);
    ///
    /// // Result now has both: "id" (from r1) and "r2_id" (from r2)
    /// assert!(result.relation_type().has_attribute("id"));
    /// assert!(result.relation_type().has_attribute("r2_id"));
    /// ```
    pub fn theta_join<F>(&self, other: &Relation, predicate: F) -> Self
    where
        F: Fn(&Tuple, &Tuple) -> bool,
    {
        // Build result heading (union of both headings, but must handle conflicts)
        // Note: In a production system, we'd want to handle conflicts more explicitly
        let result_heading = compute_natural_join_heading(self, other);

        let result_rel_type = RelationType::new(result_heading.clone());
        let result_heading_arc = Arc::new(result_heading);

        // Perform theta join
        let joined_tuples = compute_theta_join_tuples(self, other, predicate, &result_heading_arc);

        Relation::from_tuples(result_rel_type, joined_tuples)
            .expect("Joined tuples should conform to result relation type")
    }
}

/// Helper to compute common attributes between two relations.
fn compute_common_attributes(left: &Relation, right: &Relation) -> Vec<String> {
    left.relation_type()
        .heading()
        .attribute_names()
        .filter(|attr| right.relation_type().heading().has_attribute(attr))
        .cloned()
        .collect()
}

/// Helper to compute the heading for a natural join (or theta join).
///
/// Result is the union of attributes from both relations.
/// Attributes present in both are included only once (from the left relation).
fn compute_natural_join_heading(left: &Relation, right: &Relation) -> TupleType {
    let mut result_heading = TupleType::new();

    // Add all attributes from left
    for (attr_name, attr_type) in left.relation_type().heading().attributes() {
        result_heading = result_heading.with_attribute(attr_name, attr_type.clone());
    }

    // Add attributes from right that aren't already in result
    for (attr_name, attr_type) in right.relation_type().heading().attributes() {
        if !result_heading.has_attribute(attr_name) {
            result_heading = result_heading.with_attribute(attr_name, attr_type.clone());
        }
    }

    result_heading
}

/// Helper to build the hash map for the join operation (Build Phase) - Single Attribute Optimization.
fn build_join_map_single<'a>(
    build_rel: &'a Relation,
    attr: &str,
) -> Result<HashMap<&'a ScalarValue, Vec<&'a Tuple>>, DatabaseError> {
    let mut build_map: HashMap<&ScalarValue, Vec<&Tuple>> =
        HashMap::with_capacity(build_rel.cardinality());

    for tuple in build_rel.tuples() {
        let val = tuple.get(attr).ok_or_else(|| {
            DatabaseError::AttributeNotFound(attr.to_string(), "build relation".to_string())
        })?;
        build_map.entry(val).or_default().push(tuple);
    }
    Ok(build_map)
}

/// Helper to probe the hash map and combine tuples (Probe Phase) - Single Attribute Optimization.
fn probe_and_combine_single<'a>(
    probe_rel: &'a Relation,
    build_map: &HashMap<&'a ScalarValue, Vec<&'a Tuple>>,
    attr: &str,
    result_heading: &Arc<TupleType>,
) -> Result<Vec<Tuple>, DatabaseError> {
    // Optimization: Pre-allocate capacity based on the larger relation.
    // This avoids resizing allocations in the hot loop.
    let mut joined_tuples = Vec::with_capacity(probe_rel.cardinality());

    for probe_tuple in probe_rel.tuples() {
        let val = probe_tuple.get(attr).ok_or_else(|| {
            DatabaseError::AttributeNotFound(attr.to_string(), "probe relation".to_string())
        })?;

        if let Some(matching_tuples) = build_map.get(val) {
            for build_tuple in matching_tuples {
                joined_tuples.push(combine_tuples(build_tuple, probe_tuple, result_heading)?);
            }
        }
    }
    Ok(joined_tuples)
}

/// A key for hash join that avoids allocating a Vec for the key.
/// It holds references to the tuple and the attributes to key on.
#[derive(Debug, Eq)]
struct JoinKey<'t, 'a> {
    tuple: &'t Tuple,
    attributes: &'a [String],
}

impl<'t, 'a> PartialEq for JoinKey<'t, 'a> {
    fn eq(&self, other: &Self) -> bool {
        // We assume attributes are the same (or same values) as this is used internally
        // with the same common_attrs slice.
        for (i, attr) in self.attributes.iter().enumerate() {
            let v1 = self.tuple.get(attr);
            let v2 = other.tuple.get(&other.attributes[i]);
            if v1 != v2 {
                return false;
            }
        }
        true
    }
}

impl<'t, 'a> Hash for JoinKey<'t, 'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for attr in self.attributes {
            if let Some(val) = self.tuple.get(attr) {
                val.hash(state);
            }
        }
    }
}

/// Helper to build the hash map for the join operation (Build Phase).
fn build_join_map<'t, 'a>(
    build_rel: &'t Relation,
    common_attrs: &'a [String],
) -> Result<HashMap<JoinKey<'t, 'a>, Vec<&'t Tuple>>, DatabaseError> {
    let mut build_map: HashMap<JoinKey<'t, 'a>, Vec<&Tuple>> =
        HashMap::with_capacity(build_rel.cardinality());

    for tuple in build_rel.tuples() {
        let key = JoinKey {
            tuple,
            attributes: common_attrs,
        };
        build_map.entry(key).or_default().push(tuple);
    }
    Ok(build_map)
}

/// Helper to probe the hash map and combine tuples (Probe Phase).
fn probe_and_combine<'t, 'a>(
    probe_rel: &'t Relation,
    build_map: &HashMap<JoinKey<'t, 'a>, Vec<&'t Tuple>>,
    common_attrs: &'a [String],
    result_heading: &Arc<TupleType>,
) -> Result<Vec<Tuple>, DatabaseError> {
    // Optimization: Pre-allocate capacity based on the larger relation.
    // This avoids resizing allocations in the hot loop.
    let mut joined_tuples = Vec::with_capacity(probe_rel.cardinality());

    for probe_tuple in probe_rel.tuples() {
        let key = JoinKey {
            tuple: probe_tuple,
            attributes: common_attrs,
        };

        if let Some(matching_tuples) = build_map.get(&key) {
            for build_tuple in matching_tuples {
                joined_tuples.push(combine_tuples(build_tuple, probe_tuple, result_heading)?);
            }
        }
    }
    Ok(joined_tuples)
}

/// Helper to determine which relation should be the build side (smaller) and which the probe side (larger).
/// Returns (build_rel, probe_rel).
fn determine_hash_join_sides<'a>(
    left: &'a Relation,
    right: &'a Relation,
) -> (&'a Relation, &'a Relation) {
    if left.cardinality() <= right.cardinality() {
        (left, right)
    } else {
        (right, left)
    }
}

/// Helper to perform the hash join logic, dispatching between single-attribute optimization and general case.
fn perform_hash_join(
    build_rel: &Relation,
    probe_rel: &Relation,
    common_attrs: &[String],
    result_heading: &Arc<TupleType>,
) -> Result<Vec<Tuple>, DatabaseError> {
    if common_attrs.len() == 1 {
        // Optimization for single-attribute joins
        let attr = &common_attrs[0];
        let build_map = build_join_map_single(build_rel, attr)?;
        probe_and_combine_single(probe_rel, &build_map, attr, result_heading)
    } else {
        let build_map = build_join_map(build_rel, common_attrs)?;
        probe_and_combine(probe_rel, &build_map, common_attrs, result_heading)
    }
}

/// Helper to compute tuples for a theta join by iterating through the Cartesian product
/// and filtering with the predicate.
fn compute_theta_join_tuples<F>(
    left: &Relation,
    right: &Relation,
    predicate: F,
    result_heading: &Arc<TupleType>,
) -> Vec<Tuple>
where
    F: Fn(&Tuple, &Tuple) -> bool,
{
    // Optimization: Use `std::cmp::max` to pre-allocate an initial capacity
    // to reduce vector re-allocations during the cross product.
    let capacity = std::cmp::max(left.cardinality(), right.cardinality());
    let mut joined_tuples = Vec::with_capacity(capacity);

    for tuple1 in left.tuples() {
        for tuple2 in right.tuples() {
            if !predicate(tuple1, tuple2) {
                continue;
            }

            if let Ok(combined_tuple) = combine_tuples(tuple1, tuple2, result_heading) {
                joined_tuples.push(combined_tuple);
            }
        }
    }
    joined_tuples
}

/// Helper to combine two tuples into a single tuple.
///
/// Attributes from `primary` take precedence over `secondary` if there are collisions.
fn combine_tuples(
    primary: &Tuple,
    secondary: &Tuple,
    result_heading: &Arc<TupleType>,
) -> Result<Tuple, DatabaseError> {
    // Optimization: Use a merge-sort style iteration to combine values.
    // Since both BTreeMaps are sorted, we can iterate through them simultaneously
    // and build the new map in O(N) time without O(log N) insertions.
    let mut iter_p = primary.values().iter().peekable();
    let mut iter_s = secondary.values().iter().peekable();

    // Pre-allocate to avoid reallocations
    let mut values = Vec::with_capacity(primary.degree() + secondary.degree());

    loop {
        match (iter_p.peek(), iter_s.peek()) {
            (Some(&(k_p, v_p)), Some(&(k_s, v_s))) => {
                if k_p == k_s {
                    // Collision: Primary wins (as per doc)
                    // Consume both since they match
                    values.push((k_p, v_p));
                    iter_p.next();
                    iter_s.next();
                } else if k_p < k_s {
                    // Primary is smaller, take it
                    values.push((k_p, v_p));
                    iter_p.next();
                } else {
                    // Secondary is smaller, take it
                    values.push((k_s, v_s));
                    iter_s.next();
                }
            }
            (Some(&(k_p, v_p)), None) => {
                // Only primary remaining
                values.push((k_p, v_p));
                iter_p.next();
            }
            (None, Some(&(k_s, v_s))) => {
                // Only secondary remaining
                values.push((k_s, v_s));
                iter_s.next();
            }
            (None, None) => break,
        }
    }

    // Safety:
    // 1. Primary and secondary tuples are valid and conform to their headings.
    // 2. Result heading is the union of both headings.
    // 3. We combined values from both, respecting types.
    // 4. Therefore, the resulting map conforms to result_heading.
    //
    // BTreeMap::from_iter is efficient (O(N)) when input is already sorted.
    // We defer cloning to the iterator mapping step to avoid repeatedly cloning
    // discarded values or allocating strings inside the hot loop.
    let combined_values =
        BTreeMap::from_iter(values.into_iter().map(|(k, v)| (k.clone(), v.clone())));

    Ok(Tuple::new_unchecked(
        result_heading.clone(),
        combined_values,
    ))
}

#[cfg(test)]
mod tests {
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::{Relation, ScalarValue};

    #[test]
    fn test_natural_join_on_common_attributes() {
        // Employees: emp_id, name, dept_id
        let emp_heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("dept_id", ScalarType::Int);

        let emp_rel_type = RelationType::new(emp_heading);
        let mut employees = Relation::new(emp_rel_type);

        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        employees
            .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
            .unwrap();

        // Departments: dept_id, dept_name
        let dept_heading = TupleType::new()
            .with_attribute("dept_id", ScalarType::Int)
            .with_attribute("dept_name", ScalarType::String);

        let dept_rel_type = RelationType::new(dept_heading);
        let mut departments = Relation::new(dept_rel_type);

        departments
            .insert(tuple! { dept_id: 10i64, dept_name: "Engineering" })
            .unwrap();
        departments
            .insert(tuple! { dept_id: 20i64, dept_name: "Sales" })
            .unwrap();

        // Join on dept_id
        let result = employees.join(&departments).unwrap();

        assert_eq!(result.degree(), 4); // emp_id, name, dept_id, dept_name
        assert_eq!(result.cardinality(), 2);

        // Verify Alice is joined with Engineering
        let alice_result = tuple! {
            emp_id: 1i64,
            name: "Alice",
            dept_id: 10i64,
            dept_name: "Engineering"
        };
        assert!(result.contains(&alice_result));
    }

    #[test]
    fn test_natural_join_no_common_attributes_cartesian_product() {
        let rel1_heading = TupleType::new().with_attribute("a", ScalarType::Int);

        let rel1_type = RelationType::new(rel1_heading);
        let mut rel1 = Relation::new(rel1_type);

        rel1.insert(tuple! { a: 1i64 }).unwrap();
        rel1.insert(tuple! { a: 2i64 }).unwrap();

        let rel2_heading = TupleType::new().with_attribute("b", ScalarType::String);

        let rel2_type = RelationType::new(rel2_heading);
        let mut rel2 = Relation::new(rel2_type);

        rel2.insert(tuple! { b: "x" }).unwrap();
        rel2.insert(tuple! { b: "y" }).unwrap();

        let result = rel1.join(&rel2).unwrap();

        // Cartesian product: 2 x 2 = 4
        assert_eq!(result.cardinality(), 4);
        assert_eq!(result.degree(), 2); // a and b
    }

    #[test]
    fn test_join_with_empty_relation() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let rel_type = RelationType::new(heading.clone());
        let mut rel1 = Relation::new(rel_type.clone());

        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

        let rel2 = Relation::new(rel_type);

        let result = rel1.join(&rel2).unwrap();

        assert_eq!(result.cardinality(), 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_self_join() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice" })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, name: "Bob" })
            .unwrap();

        let result = relation.join(&relation).unwrap();

        // Self-join on all attributes = original relation
        assert_eq!(result.cardinality(), 2);
        assert_eq!(result, relation);
    }

    #[test]
    fn test_theta_join() {
        let emp_heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("salary", ScalarType::Int);

        let emp_rel_type = RelationType::new(emp_heading);
        let mut employees = Relation::new(emp_rel_type);

        employees
            .insert(tuple! { emp_id: 1i64, salary: 50000i64 })
            .unwrap();
        employees
            .insert(tuple! { emp_id: 2i64, salary: 75000i64 })
            .unwrap();

        let dept_heading = TupleType::new()
            .with_attribute("dept_id", ScalarType::Int)
            .with_attribute("min_salary", ScalarType::Int);

        let dept_rel_type = RelationType::new(dept_heading);
        let mut departments = Relation::new(dept_rel_type);

        departments
            .insert(tuple! { dept_id: 10i64, min_salary: 60000i64 })
            .unwrap();
        departments
            .insert(tuple! { dept_id: 20i64, min_salary: 40000i64 })
            .unwrap();

        // Join where employee salary >= department min_salary
        let result = employees.theta_join(&departments, |emp, dept| {
            if let (Some(ScalarValue::Int(salary)), Some(ScalarValue::Int(min_sal))) =
                (emp.get("salary"), dept.get("min_salary"))
            {
                salary >= min_sal
            } else {
                false
            }
        });

        // emp_id 1 (50000) matches dept 20 (40000)
        // emp_id 2 (75000) matches both dept 10 (60000) and dept 20 (40000)
        assert_eq!(result.cardinality(), 3);
    }

    #[test]
    fn test_join_no_matches() {
        let rel1_heading = TupleType::new().with_attribute("dept_id", ScalarType::Int);

        let rel1_type = RelationType::new(rel1_heading);
        let mut rel1 = Relation::new(rel1_type);

        rel1.insert(tuple! { dept_id: 10i64 }).unwrap();

        let rel2_heading = TupleType::new().with_attribute("dept_id", ScalarType::Int);

        let rel2_type = RelationType::new(rel2_heading);
        let mut rel2 = Relation::new(rel2_type);

        rel2.insert(tuple! { dept_id: 20i64 }).unwrap();

        let result = rel1.join(&rel2).unwrap();

        assert_eq!(result.cardinality(), 0);
        assert!(result.is_empty());
    }

    /// Verifies the documented behavior that theta_join drops conflicting attributes
    /// from the second relation.
    #[test]
    fn test_theta_join_attribute_collision_resolution() {
        // Relation 1: id=1
        let heading1 = TupleType::new().with_attribute("id", ScalarType::Int);
        let mut rel1 = Relation::new(RelationType::new(heading1));
        rel1.insert(tuple! { id: 1i64 }).unwrap();

        // Relation 2: id=2 (SAME ATTRIBUTE NAME)
        let heading2 = TupleType::new().with_attribute("id", ScalarType::Int);
        let mut rel2 = Relation::new(RelationType::new(heading2));
        rel2.insert(tuple! { id: 2i64 }).unwrap();

        // Theta join with a predicate that is always true
        // If this were a proper Cartesian product (renaming aside), we'd expect
        // something like (id_left: 1, id_right: 2).
        //
        // Current behavior: The second 'id' is dropped.
        let result = rel1.theta_join(&rel2, |_, _| true);

        // Assert that the result only has degree 1 (from rel1), not 2
        assert_eq!(
            result.degree(),
            1,
            "Expected colliding attribute to be dropped per current behavior"
        );

        // Assert that the value preserved is from the first relation (id=1)
        let tuple = result.tuples().next().expect("Should have one tuple");
        assert_eq!(
            tuple.get_typed::<i64>("id").expect("id attribute missing"),
            1
        );
    }

    /// Verifies behavior when colliding attributes have DIFFERENT types.
    ///
    /// Even if types mismatch, the second attribute is dropped, and the result
    /// retains the type and value of the first attribute. This ensures no
    /// type confusion or panic occurs during tuple construction.
    #[test]
    fn test_theta_join_colliding_attributes_different_types() {
        // Relation 1: id=1 (Int)
        let heading1 = TupleType::new().with_attribute("id", ScalarType::Int);
        let mut rel1 = Relation::new(RelationType::new(heading1));
        rel1.insert(tuple! { id: 1i64 }).unwrap();

        // Relation 2: id="2" (String) - Same name, different type
        let heading2 = TupleType::new().with_attribute("id", ScalarType::String);
        let mut rel2 = Relation::new(RelationType::new(heading2));
        rel2.insert(tuple! { id: "2" }).unwrap();

        // Theta join
        let result = rel1.theta_join(&rel2, |_, _| true);

        // Result should have 'id' as Int (from rel1)
        assert_eq!(result.degree(), 1);
        let tuple = result.tuples().next().unwrap();

        // Should be able to get as Int
        assert_eq!(tuple.get_typed::<i64>("id").unwrap(), 1);

        // Should NOT be able to get as String
        assert!(tuple.get_typed::<String>("id").is_none());

        // Verify type in heading
        assert_eq!(
            result.relation_type().heading().get_attribute_type("id"),
            Some(&ScalarType::Int)
        );
    }

    #[test]
    fn test_join_swap_optimization() {
        // Test case 1: Left is smaller (no swap)
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let mut small = Relation::new(RelationType::new(heading.clone()));
        small.insert(tuple! { id: 1i64 }).unwrap();

        let mut large = Relation::new(RelationType::new(heading.clone()));
        for i in 0..10 {
            large.insert(tuple! { id: i as i64 }).unwrap();
        }

        let result1 = small.join(&large).unwrap();
        assert_eq!(result1.cardinality(), 1);

        // Test case 2: Right is smaller (swap)
        let result2 = large.join(&small).unwrap();
        assert_eq!(result2.cardinality(), 1);
    }
}
