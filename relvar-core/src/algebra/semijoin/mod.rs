//! Semijoin and semidifference operators.
//!
//! These operators filter one relation based on the existence (or absence) of
//! matching tuples in another relation, matching on common attributes.
//!
//! - **Semijoin** (MATCHING) - Tuples from A that match some tuple in B
//! - **Semidifference** (NOT MATCHING / antijoin) - Tuples from A that match no tuple in B
//!
//! # TTM Compliance
//!
//! - RM Prescription 7: relational algebra completeness
//! - Formally: `A SEMIJOIN B = (A JOIN B) PROJECT {attributes of A}`
//! - Formally: `A SEMIDIFFERENCE B = A MINUS (A SEMIJOIN B)`
//! - Result heading always equals self's heading
//! - Set semantics maintained (no duplicates, no ordering)

use crate::values::{Relation, Tuple};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

/// A key for hash join that avoids allocating a Vec for the key.
/// It holds references to the tuple and the attributes to key on.
#[derive(Debug, Eq)]
struct SemijoinKey<'t, 'a> {
    tuple: &'t Tuple,
    attributes: &'a [String],
}

impl<'t, 'a> PartialEq for SemijoinKey<'t, 'a> {
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

impl<'t, 'a> Hash for SemijoinKey<'t, 'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for attr in self.attributes {
            if let Some(val) = self.tuple.get(attr) {
                val.hash(state);
            }
        }
    }
}

/// Finds common attribute names between two relations' headings.
fn common_attributes(a: &Relation, b: &Relation) -> Vec<String> {
    a.relation_type()
        .heading()
        .attribute_names()
        .filter(|attr| b.relation_type().heading().has_attribute(attr))
        .cloned()
        .collect()
}

impl Relation {
    /// Computes the semijoin of this relation with another (A MATCHING B).
    ///
    /// Returns tuples from this relation that have at least one matching
    /// tuple in `other` on their common attributes. The result heading
    /// is always this relation's heading.
    ///
    /// Formally: `A SEMIJOIN B = (A JOIN B) PROJECT {attributes of A}`
    ///
    /// # Arguments
    ///
    /// * `other` - The relation to match against
    ///
    /// # Returns
    ///
    /// A new relation with this relation's heading, containing only the
    /// tuples that have a match in `other`.
    ///
    /// # Behavior
    ///
    /// - If there are no common attributes and `other` is non-empty, returns `self`
    ///   (vacuous match, like Cartesian product projected back)
    /// - If `other` is empty, returns an empty relation
    /// - If headings are identical, equivalent to [`intersect()`](Self::intersect)
    ///
    /// # Complexity
    ///
    /// O(n + m) where n and m are the cardinalities of the two relations.
    /// Optimized using a hash-based lookup.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let emp_heading = TupleType::new()
    ///     .with_attribute("emp_id", ScalarType::Int)
    ///     .with_attribute("dept_id", ScalarType::Int);
    /// let mut employees = Relation::new(RelationType::new(emp_heading));
    /// employees.insert(tuple! { emp_id: 1i64, dept_id: 10i64 }).unwrap();
    /// employees.insert(tuple! { emp_id: 2i64, dept_id: 20i64 }).unwrap();
    ///
    /// let dept_heading = TupleType::new()
    ///     .with_attribute("dept_id", ScalarType::Int)
    ///     .with_attribute("dept_name", ScalarType::String);
    /// let mut departments = Relation::new(RelationType::new(dept_heading));
    /// departments.insert(tuple! { dept_id: 10i64, dept_name: "Engineering" }).unwrap();
    ///
    /// let result = employees.semijoin(&departments);
    /// assert_eq!(result.cardinality(), 1); // Only emp 1 matches
    /// ```
    pub fn semijoin(&self, other: &Relation) -> Self {
        let common_attrs = common_attributes(self, other);

        // If no common attributes, we have a degenerate case (Cartesian product projection)
        if common_attrs.is_empty() {
            if !other.is_empty() {
                // If B is not empty, A MATCHING B = A (vacuous match)
                return self.clone();
            } else {
                // If B is empty, A MATCHING B = {}
                return Relation::new(self.relation_type().clone());
            }
        }

        // Build HashSet of keys from other relation
        // We use SemijoinKey to avoid allocating Vec per tuple
        let mut other_keys: HashSet<SemijoinKey> = HashSet::with_capacity(other.cardinality());

        for tuple in other.tuples() {
            other_keys.insert(SemijoinKey {
                tuple,
                attributes: &common_attrs,
            });
        }

        // Optimization: Pass an iterator directly to `from_tuples_unchecked` instead of
        // collecting into an intermediate `Vec`. The tuples are guaranteed to be valid
        // since they come from `self`.
        Relation::from_tuples_unchecked(
            self.relation_type().clone(),
            self.tuples()
                .filter(|tuple| {
                    let key = SemijoinKey {
                        tuple,
                        attributes: &common_attrs,
                    };
                    other_keys.contains(&key)
                })
                .cloned(),
        )
    }

    /// Alias for [`semijoin`](Self::semijoin) using Tutorial D naming (MATCHING).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let heading1 = TupleType::new().with_attribute("id", ScalarType::Int).with_attribute("name", ScalarType::String);
    /// let rel_type1 = RelationType::new(heading1);
    /// let mut all = Relation::new(rel_type1.clone());
    /// all.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
    /// all.insert(tuple! { id: 2i64, name: "Bob" }).unwrap();
    ///
    /// let heading2 = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let rel_type2 = RelationType::new(heading2);
    /// let mut subset = Relation::new(rel_type2);
    /// subset.insert(tuple! { id: 2i64 }).unwrap();
    ///
    /// let result = all.matching(&subset);
    /// assert_eq!(result.cardinality(), 1);
    /// ```
    pub fn matching(&self, other: &Relation) -> Self {
        self.semijoin(other)
    }

    /// Computes the semijoin of this relation with another (A MATCHING B), consuming this relation.
    ///
    /// This is an optimized version of [`semijoin`](Self::semijoin) that avoids cloning
    /// tuples by modifying the `Relation` in place.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::{Relation, ScalarValue};
    /// use relvar_core::tuple;
    ///
    /// let heading1 = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int)
    ///     .with_attribute("name", ScalarType::String);
    /// let rel_type1 = RelationType::new(heading1);
    ///
    /// let heading2 = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int)
    ///     .with_attribute("role", ScalarType::String);
    /// let rel_type2 = RelationType::new(heading2);
    ///
    /// let mut employees = Relation::new(rel_type1);
    /// employees.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
    /// employees.insert(tuple! { id: 2i64, name: "Bob" }).unwrap();
    ///
    /// let mut roles = Relation::new(rel_type2);
    /// roles.insert(tuple! { id: 1i64, role: "Admin" }).unwrap();
    ///
    /// let result = employees.semijoin_into(&roles);
    /// assert_eq!(result.cardinality(), 1); // Only emp 1 matches
    /// ```
    pub fn semijoin_into(self, other: &Relation) -> Self {
        let common_attrs = common_attributes(&self, other);

        // If no common attributes, we have a degenerate case (Cartesian product projection)
        if common_attrs.is_empty() {
            if !other.is_empty() {
                // If B is not empty, A MATCHING B = A (vacuous match)
                return self;
            } else {
                // If B is empty, A MATCHING B = {}
                return Relation::new(self.relation_type().clone());
            }
        }

        // Build HashSet of keys from other relation
        // We use SemijoinKey to avoid allocating Vec per tuple
        let mut other_keys: HashSet<SemijoinKey> = HashSet::with_capacity(other.cardinality());

        for tuple in other.tuples() {
            other_keys.insert(SemijoinKey {
                tuple,
                attributes: &common_attrs,
            });
        }

        // Use restrict_into to filter in-place without cloning tuples
        self.restrict_into(|tuple| {
            let key = SemijoinKey {
                tuple,
                attributes: &common_attrs,
            };
            other_keys.contains(&key)
        })
    }

    /// Alias for [`semijoin_into`](Self::semijoin_into) using the terminology from Date's Tutorial D.
    /// This method is identical in behavior to `semijoin_into`. It filters the current relation
    /// to retain only those tuples that have a matching tuple in the `other` relation over their
    /// common attributes.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::{tuple, Relation, RelationType, ScalarType, TupleType};
    ///
    /// let rel_type1 = RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("id", ScalarType::Int)
    ///         .with_attribute("name", ScalarType::String)
    /// );
    ///
    /// let rel_type2 = RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("id", ScalarType::Int)
    ///         .with_attribute("role", ScalarType::String)
    /// );
    ///
    /// let mut employees = Relation::new(rel_type1);
    /// employees.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
    /// employees.insert(tuple! { id: 2i64, name: "Bob" }).unwrap();
    ///
    /// let mut roles = Relation::new(rel_type2);
    /// roles.insert(tuple! { id: 1i64, role: "Admin" }).unwrap();
    ///
    /// let result = employees.matching_into(&roles);
    /// assert_eq!(result.cardinality(), 1); // Only emp 1 matches
    /// ```
    pub fn matching_into(self, other: &Relation) -> Self {
        self.semijoin_into(other)
    }

    /// Computes the semidifference of this relation with another (A NOT MATCHING B).
    ///
    /// Returns tuples from this relation that have NO matching tuple in
    /// `other` on their common attributes. The result heading is always
    /// this relation's heading. Also known as antijoin.
    ///
    /// Formally: `A SEMIDIFFERENCE B = A MINUS (A SEMIJOIN B)`
    ///
    /// # Arguments
    ///
    /// * `other` - The relation to match against
    ///
    /// # Returns
    ///
    /// A new relation with this relation's heading, containing only the
    /// tuples that have NO match in `other`.
    ///
    /// # Behavior
    ///
    /// - If there are no common attributes and `other` is non-empty, returns
    ///   an empty relation (all tuples vacuously match, so none remain)
    /// - If `other` is empty, returns `self` (no tuples to exclude)
    /// - If headings are identical, equivalent to [`difference()`](Self::difference)
    ///
    /// # Complexity
    ///
    /// O(n + m) where n and m are the cardinalities of the two relations.
    /// Optimized using a hash-based lookup.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let emp_heading = TupleType::new()
    ///     .with_attribute("emp_id", ScalarType::Int)
    ///     .with_attribute("dept_id", ScalarType::Int);
    /// let mut employees = Relation::new(RelationType::new(emp_heading));
    /// employees.insert(tuple! { emp_id: 1i64, dept_id: 10i64 }).unwrap();
    /// employees.insert(tuple! { emp_id: 2i64, dept_id: 20i64 }).unwrap();
    ///
    /// let dept_heading = TupleType::new()
    ///     .with_attribute("dept_id", ScalarType::Int)
    ///     .with_attribute("dept_name", ScalarType::String);
    /// let mut departments = Relation::new(RelationType::new(dept_heading));
    /// departments.insert(tuple! { dept_id: 10i64, dept_name: "Engineering" }).unwrap();
    ///
    /// let result = employees.semidifference(&departments);
    /// assert_eq!(result.cardinality(), 1); // Only emp 2 (no matching dept)
    /// ```
    pub fn semidifference(&self, other: &Relation) -> Self {
        let common_attrs = common_attributes(self, other);

        // Degenerate case handling
        if common_attrs.is_empty() {
            if !other.is_empty() {
                // If B is not empty, A MATCHING B = A, so A MINUS A = {}
                return Relation::new(self.relation_type().clone());
            } else {
                // If B is empty, A MATCHING B = {}, so A MINUS {} = A
                return self.clone();
            }
        }

        // Build HashSet of keys from other relation
        let mut other_keys: HashSet<SemijoinKey> = HashSet::with_capacity(other.cardinality());

        for tuple in other.tuples() {
            other_keys.insert(SemijoinKey {
                tuple,
                attributes: &common_attrs,
            });
        }

        // Optimization: Pass an iterator directly to `from_tuples_unchecked` instead of
        // collecting into an intermediate `Vec`. The tuples are guaranteed to be valid
        // since they come from `self`.
        Relation::from_tuples_unchecked(
            self.relation_type().clone(),
            self.tuples()
                .filter(|tuple| {
                    let key = SemijoinKey {
                        tuple,
                        attributes: &common_attrs,
                    };
                    !other_keys.contains(&key)
                })
                .cloned(),
        )
    }

    /// Alias for [`semidifference`](Self::semidifference) using Tutorial D naming (NOT MATCHING).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let heading1 = TupleType::new().with_attribute("id", ScalarType::Int).with_attribute("name", ScalarType::String);
    /// let rel_type1 = RelationType::new(heading1);
    /// let mut all = Relation::new(rel_type1.clone());
    /// all.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
    /// all.insert(tuple! { id: 2i64, name: "Bob" }).unwrap();
    ///
    /// let heading2 = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let rel_type2 = RelationType::new(heading2);
    /// let mut subset = Relation::new(rel_type2);
    /// subset.insert(tuple! { id: 2i64 }).unwrap();
    ///
    /// let result = all.not_matching(&subset);
    /// assert_eq!(result.cardinality(), 1);
    /// ```
    pub fn not_matching(&self, other: &Relation) -> Self {
        self.semidifference(other)
    }

    /// Computes the semidifference of this relation with another (A NOT MATCHING B),
    /// consuming this relation.
    ///
    /// This is an optimized version of [`semidifference`](Self::semidifference) that
    /// avoids cloning tuples by modifying the `Relation` in place.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::{Relation, ScalarValue};
    /// use relvar_core::tuple;
    ///
    /// let heading1 = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int)
    ///     .with_attribute("name", ScalarType::String);
    /// let rel_type1 = RelationType::new(heading1);
    ///
    /// let heading2 = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int)
    ///     .with_attribute("role", ScalarType::String);
    /// let rel_type2 = RelationType::new(heading2);
    ///
    /// let mut employees = Relation::new(rel_type1);
    /// employees.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
    /// employees.insert(tuple! { id: 2i64, name: "Bob" }).unwrap();
    ///
    /// let mut roles = Relation::new(rel_type2);
    /// roles.insert(tuple! { id: 1i64, role: "Admin" }).unwrap();
    ///
    /// let result = employees.semidifference_into(&roles);
    /// assert_eq!(result.cardinality(), 1); // Bob (id 2) has no role
    /// ```
    pub fn semidifference_into(self, other: &Relation) -> Self {
        let common_attrs = common_attributes(&self, other);

        // Degenerate case handling
        if common_attrs.is_empty() {
            if !other.is_empty() {
                // If B is not empty, A MATCHING B = A, so A MINUS A = {}
                return Relation::new(self.relation_type().clone());
            } else {
                // If B is empty, A MATCHING B = {}, so A MINUS {} = A
                return self;
            }
        }

        // Build HashSet of keys from other relation
        let mut other_keys: HashSet<SemijoinKey> = HashSet::with_capacity(other.cardinality());

        for tuple in other.tuples() {
            other_keys.insert(SemijoinKey {
                tuple,
                attributes: &common_attrs,
            });
        }

        // Use restrict_into to filter in-place without cloning tuples
        self.restrict_into(|tuple| {
            let key = SemijoinKey {
                tuple,
                attributes: &common_attrs,
            };
            !other_keys.contains(&key)
        })
    }

    /// Alias for [`semidifference_into`](Self::semidifference_into) using the terminology from Date's Tutorial D.
    /// This method is identical in behavior to `semidifference_into`. It filters the current relation
    /// to retain only those tuples that have NO matching tuple in the `other` relation over their
    /// common attributes.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::{tuple, Relation, RelationType, ScalarType, TupleType};
    ///
    /// let rel_type1 = RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("id", ScalarType::Int)
    ///         .with_attribute("name", ScalarType::String)
    /// );
    ///
    /// let rel_type2 = RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("id", ScalarType::Int)
    ///         .with_attribute("role", ScalarType::String)
    /// );
    ///
    /// let mut employees = Relation::new(rel_type1);
    /// employees.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
    /// employees.insert(tuple! { id: 2i64, name: "Bob" }).unwrap();
    ///
    /// let mut roles = Relation::new(rel_type2);
    /// roles.insert(tuple! { id: 1i64, role: "Admin" }).unwrap();
    ///
    /// let result = employees.not_matching_into(&roles);
    /// assert_eq!(result.cardinality(), 1); // Only emp 2 has no matching role
    /// ```
    pub fn not_matching_into(self, other: &Relation) -> Self {
        self.semidifference_into(other)
    }
}

#[cfg(test)]
#[cfg(test)]
mod tests;
