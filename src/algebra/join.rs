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
//! use relvar::types::{TupleType, RelationType, ScalarType};
//! use relvar::values::Relation;
//! use relvar::tuple;
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
//! let result = employees.join(&departments);
//! assert_eq!(result.degree(), 4);  // emp_id, name, dept_id, dept_name
//! ```

use crate::types::{RelationType, TupleType};
use crate::values::{Relation, ScalarValue, Tuple};
use std::collections::HashMap;

/// Helper function to combine two tuples into one.
fn combine_tuples(tuple1: &Tuple, tuple2: &Tuple, result_heading: &TupleType) -> Tuple {
    let mut combined_values = HashMap::new();

    // Add all values from tuple1
    for (attr_name, value) in tuple1.values() {
        combined_values.insert(attr_name.clone(), value.clone());
    }

    // Add values from tuple2 that aren't common (common ones are already in)
    for (attr_name, value) in tuple2.values() {
        if !combined_values.contains_key(attr_name) {
            combined_values.insert(attr_name.clone(), value.clone());
        }
    }

    Tuple::new(result_heading.clone(), combined_values)
        .expect("Combined tuple should conform to result heading")
}

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
    /// O(n + m) where n and m are the cardinalities of the two relations (assuming
    /// common attributes exist). This uses a hash join implementation. If no common
    /// attributes exist (Cartesian product), complexity is O(n * m).
    ///
    /// # Example
    ///
    /// ```
    /// use relvar::types::{TupleType, RelationType, ScalarType};
    /// use relvar::values::Relation;
    /// use relvar::tuple;
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
    /// let result = employees.join(&departments);
    /// assert_eq!(result.cardinality(), 1);  // Only emp 1 matches (dept 10)
    /// ```
    pub fn join(&self, other: &Relation) -> Self {
        // Find common attributes
        let common_attrs: Vec<String> = self
            .relation_type()
            .heading()
            .attribute_names()
            .filter(|attr| other.relation_type().heading().has_attribute(attr))
            .cloned()
            .collect();

        // Build result heading (union of both headings)
        let mut result_heading = TupleType::new();

        // Add all attributes from self
        for (attr_name, attr_type) in self.relation_type().heading().attributes() {
            result_heading = result_heading.with_attribute(attr_name, attr_type.clone());
        }

        // Add attributes from other that aren't already in result
        for (attr_name, attr_type) in other.relation_type().heading().attributes() {
            if !result_heading.has_attribute(attr_name) {
                result_heading = result_heading.with_attribute(attr_name, attr_type.clone());
            }
        }

        let result_rel_type = RelationType::new(result_heading.clone());

        // Perform join
        let mut joined_tuples = Vec::new();

        if common_attrs.is_empty() {
            // Case 1: No common attributes -> Cartesian product (Nested Loop)
            for tuple1 in self.tuples() {
                for tuple2 in other.tuples() {
                    joined_tuples.push(combine_tuples(tuple1, tuple2, &result_heading));
                }
            }
        } else if common_attrs.len() == 1 {
            // Case 2: Single common attribute -> Optimized Hash Join (ScalarValue key)
            let attr_name = &common_attrs[0];
            let mut build_map: HashMap<ScalarValue, Vec<&Tuple>> = HashMap::new();

            // Build phase
            for tuple in other.tuples() {
                let key = tuple
                    .get(attr_name)
                    .expect("Tuple must have attribute defined in relation type")
                    .clone();
                build_map.entry(key).or_default().push(tuple);
            }

            // Probe phase
            for tuple1 in self.tuples() {
                let key = tuple1
                    .get(attr_name)
                    .expect("Tuple must have attribute defined in relation type");

                if let Some(matching_tuples) = build_map.get(key) {
                    for tuple2 in matching_tuples {
                        joined_tuples.push(combine_tuples(tuple1, tuple2, &result_heading));
                    }
                }
            }
        } else {
            // Case 3: Multiple common attributes -> Hash Join (Vec<ScalarValue> key)
            // Helper closure to extract composite key
            let get_key = |tuple: &Tuple| -> Vec<ScalarValue> {
                common_attrs
                    .iter()
                    .map(|attr| {
                        tuple
                            .get(attr)
                            .expect("Tuple must have attribute defined in relation type")
                            .clone()
                    })
                    .collect()
            };

            let mut build_map: HashMap<Vec<ScalarValue>, Vec<&Tuple>> = HashMap::new();

            // Build phase
            for tuple in other.tuples() {
                build_map.entry(get_key(tuple)).or_default().push(tuple);
            }

            // Probe phase
            for tuple1 in self.tuples() {
                if let Some(matching_tuples) = build_map.get(&get_key(tuple1)) {
                    for tuple2 in matching_tuples {
                        joined_tuples.push(combine_tuples(tuple1, tuple2, &result_heading));
                    }
                }
            }
        }

        Relation::from_tuples(result_rel_type, joined_tuples)
            .expect("Joined tuples should conform to result relation type")
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
    /// - All attribute pairs are combined (no automatic deduplication of common names)
    /// - If relations have common attribute names, the first relation's values
    ///   take precedence
    /// - Result maintains set semantics
    ///
    /// # Complexity
    ///
    /// O(n * m) where n and m are the cardinalities of the two relations.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar::types::{TupleType, RelationType, ScalarType};
    /// use relvar::values::{Relation, ScalarValue};
    /// use relvar::tuple;
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
    pub fn theta_join<F>(&self, other: &Relation, predicate: F) -> Self
    where
        F: Fn(&Tuple, &Tuple) -> bool,
    {
        // Build result heading (union of both headings, but must handle conflicts)
        let mut result_heading = TupleType::new();

        // Add all attributes from self
        for (attr_name, attr_type) in self.relation_type().heading().attributes() {
            result_heading = result_heading.with_attribute(attr_name, attr_type.clone());
        }

        // Add attributes from other, renaming if there's a conflict
        for (attr_name, attr_type) in other.relation_type().heading().attributes() {
            if !result_heading.has_attribute(attr_name) {
                result_heading = result_heading.with_attribute(attr_name, attr_type.clone());
            }
            // Note: In a production system, we'd want to handle conflicts more explicitly
        }

        let result_rel_type = RelationType::new(result_heading.clone());

        // Perform theta join
        let mut joined_tuples = Vec::new();

        for tuple1 in self.tuples() {
            for tuple2 in other.tuples() {
                if predicate(tuple1, tuple2) {
                    // Combine tuples
                    let mut combined_values = HashMap::new();

                    for (attr_name, value) in tuple1.values() {
                        combined_values.insert(attr_name.clone(), value.clone());
                    }

                    for (attr_name, value) in tuple2.values() {
                        if !combined_values.contains_key(attr_name) {
                            combined_values.insert(attr_name.clone(), value.clone());
                        }
                    }

                    if let Ok(combined_tuple) = Tuple::new(result_heading.clone(), combined_values)
                    {
                        joined_tuples.push(combined_tuple);
                    }
                }
            }
        }

        Relation::from_tuples(result_rel_type, joined_tuples)
            .expect("Joined tuples should conform to result relation type")
    }
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
        let result = employees.join(&departments);

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

        let result = rel1.join(&rel2);

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

        let result = rel1.join(&rel2);

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

        let result = relation.join(&relation);

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

        let result = rel1.join(&rel2);

        assert_eq!(result.cardinality(), 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_join_multi_attribute() {
        // Test composite key join (path 3)
        let heading1 = TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int)
            .with_attribute("c", ScalarType::Int);
        let mut rel1 = Relation::new(RelationType::new(heading1));
        rel1.insert(tuple! { a: 1i64, b: 1i64, c: 10i64 }).unwrap();
        rel1.insert(tuple! { a: 1i64, b: 2i64, c: 20i64 }).unwrap();

        let heading2 = TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int)
            .with_attribute("d", ScalarType::Int);
        let mut rel2 = Relation::new(RelationType::new(heading2));
        rel2.insert(tuple! { a: 1i64, b: 1i64, d: 100i64 }).unwrap();
        rel2.insert(tuple! { a: 2i64, b: 1i64, d: 200i64 }).unwrap();

        // Join on (a, b)
        let result = rel1.join(&rel2);

        // Should only match (1, 1)
        assert_eq!(result.cardinality(), 1);
        let tuple = result.tuples().next().unwrap();
        assert_eq!(tuple.get_typed::<i64>("a").unwrap(), 1);
        assert_eq!(tuple.get_typed::<i64>("b").unwrap(), 1);
        assert_eq!(tuple.get_typed::<i64>("c").unwrap(), 10);
        assert_eq!(tuple.get_typed::<i64>("d").unwrap(), 100);
    }
}
