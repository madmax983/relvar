//! Transitive Closure (TCLOSE) operator implementation.
//!
//! TCLOSE computes the transitive closure of a binary relation.
//! Given a relation R with attributes (A, B) representing edges in a graph,
//! TCLOSE(R) returns a relation containing all pairs (x, y) such that
//! there is a path from x to y in the graph.
//!
//! This operator is a classic example of a recursive query that cannot be expressed
//! in standard relational algebra without recursion (e.g., SQL's WITH RECURSIVE).
//!
//! # Algorithm
//!
//! This implementation uses a semi-naive evaluation strategy:
//! 1. Initialize `result` and `delta` with the input relation.
//! 2. In each iteration:
//!    - Compute `new_paths` by joining `delta` with the original relation.
//!    - Compute `new_tuples = new_paths MINUS result`.
//!    - If `new_tuples` is empty, terminate.
//!    - Update `result = result UNION new_tuples`.
//!    - Update `delta = new_tuples`.

use crate::error::DatabaseError;
use crate::values::Relation;

impl Relation {
    /// Computes the transitive closure of a binary relation.
    ///
    /// The transitive closure operator (TCLOSE) finds all reachable paths in a graph
    /// represented by a binary relation. If tuple `(x, y)` exists (meaning there is
    /// an edge from x to y), and `(y, z)` exists, TCLOSE will infer `(x, z)`.
    ///
    /// # Requirements
    ///
    /// - The relation must be **binary** (exactly two attributes).
    /// - Both attributes must have the **same type** (homogenous).
    /// - The attributes represent the "from" and "to" nodes of a directed graph.
    ///
    /// # Arguments
    ///
    /// * `from_attr` - The attribute name representing the source node (start of edge).
    /// * `to_attr` - The attribute name representing the target node (end of edge).
    ///
    /// # Returns
    ///
    /// A new relation containing the original edges plus all inferred transitive paths.
    ///
    /// # Complexity
    ///
    /// Uses a semi-naive algorithm that iteratively joins the relation with itself
    /// until no new paths are found. Performance depends on the graph's diameter and density.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// // Define a graph of direct flights: Origin -> Destination
    /// let heading = TupleType::new()
    ///     .with_attribute("origin", ScalarType::String)
    ///     .with_attribute("dest", ScalarType::String);
    ///
    /// let mut flights = Relation::new(RelationType::new(heading));
    ///
    /// // Direct flights: NY -> London -> Paris
    /// flights.insert(tuple! { origin: "NY", dest: "London" }).unwrap();
    /// flights.insert(tuple! { origin: "London", dest: "Paris" }).unwrap();
    ///
    /// // Compute all reachable destinations (transitive closure)
    /// let reachable = flights.tclose("origin", "dest").unwrap();
    ///
    /// // Result contains 3 tuples:
    /// // 1. NY -> London (original)
    /// // 2. London -> Paris (original)
    /// // 3. NY -> Paris (inferred)
    /// assert_eq!(reachable.cardinality(), 3);
    /// assert!(reachable.contains(&tuple! { origin: "NY", dest: "Paris" }));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError` if:
    /// - The relation is not binary (degree != 2).
    /// - The specified attributes do not exist.
    /// - The attributes have different types.
    /// - An algebraic operation fails.
    pub fn tclose(&self, from_attr: &str, to_attr: &str) -> Result<Relation, DatabaseError> {
        // 1. Validation
        if self.degree() != 2 {
            return Err(DatabaseError::AlgebraError(format!(
                "TCLOSE requires a binary relation (degree 2), found degree {}",
                self.degree()
            )));
        }

        let heading = self.relation_type().heading();
        if !heading.has_attribute(from_attr) {
            return Err(DatabaseError::AttributeNotFound(
                from_attr.to_string(),
                "relation".to_string(),
            ));
        }
        if !heading.has_attribute(to_attr) {
            return Err(DatabaseError::AttributeNotFound(
                to_attr.to_string(),
                "relation".to_string(),
            ));
        }

        let from_type = heading.get_attribute_type(from_attr).unwrap();
        let to_type = heading.get_attribute_type(to_attr).unwrap();

        if from_type != to_type {
            return Err(DatabaseError::AlgebraError(format!(
                "Attributes {} and {} must have the same type for TCLOSE. Found {:?} and {:?}",
                from_attr, to_attr, from_type, to_type
            )));
        }

        // Semi-naive algorithm setup
        let mut r_total = self.clone();
        let mut r_delta = self.clone(); // Newly discovered paths

        // We need a unique temporary name for the join attribute.
        // We join `r_delta(from, to)` with `self(from, to)`.
        // The join condition is `r_delta.to == self.from`.
        // So we rename `r_delta.to` -> `temp` and `self.from` -> `temp`.
        // The result will be `(from, temp, to)`.
        // Then we project `(from, to)`.

        let temp_join_attr = format!("_TCLOSE_JOIN_{}_{}", from_attr, to_attr);

        // Rename mappings
        // r_delta: rename `to_attr` -> `temp_join_attr`
        let delta_mappings = vec![(to_attr, temp_join_attr.as_str())];

        // self: rename `from_attr` -> `temp_join_attr`
        let self_mappings = vec![(from_attr, temp_join_attr.as_str())];

        // Pre-compute renamed self (the graph edges) to avoid repeated work
        // self(from, to) -> self(temp, to)
        // Note: Relation::rename returns a new Relation.
        let edges = self.rename(&self_mappings);

        loop {
            // 2. Rename delta: r_delta(from, to) -> r_delta(from, temp)
            let delta_renamed = r_delta.rename(&delta_mappings);

            // 3. Join: r_delta(from, temp) JOIN edges(temp, to) -> (from, temp, to)
            let joined = delta_renamed.join(&edges)?;

            // 4. Project: keep (from, to), discard temp
            let new_paths = joined.project(&[from_attr, to_attr]);

            // 5. Difference: new_paths = new_paths MINUS r_total
            // This filters out paths we already know about.
            let new_unique_paths = new_paths
                .difference(&r_total)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // 6. Termination check
            if new_unique_paths.is_empty() {
                break;
            }

            // 7. Update accumulators
            // r_total = r_total UNION new_unique_paths
            r_total = r_total
                .union(&new_unique_paths)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // r_delta = new_unique_paths (only extend from newly found paths)
            r_delta = new_unique_paths;
        }

        Ok(r_total)
    }
}

#[cfg(test)]
mod tests {
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::Relation;

    #[test]
    fn test_tclose_simple_chain() {
        // A -> B -> C
        let heading = TupleType::new()
            .with_attribute("start", ScalarType::Int)
            .with_attribute("end", ScalarType::Int);
        let mut r = Relation::new(RelationType::new(heading));

        r.insert(tuple! { start: 1i64, end: 2i64 }).unwrap();
        r.insert(tuple! { start: 2i64, end: 3i64 }).unwrap();

        let closure = r.tclose("start", "end").unwrap();

        // Expected: (1,2), (2,3), (1,3)
        assert_eq!(closure.cardinality(), 3);
        assert!(closure.contains(&tuple! { start: 1i64, end: 3i64 }));
    }

    #[test]
    fn test_tclose_cycle() {
        // A -> B -> A
        let heading = TupleType::new()
            .with_attribute("n1", ScalarType::String)
            .with_attribute("n2", ScalarType::String);
        let mut r = Relation::new(RelationType::new(heading));

        r.insert(tuple! { n1: "A", n2: "B" }).unwrap();
        r.insert(tuple! { n1: "B", n2: "A" }).unwrap();

        let closure = r.tclose("n1", "n2").unwrap();

        // Expected:
        // (A, B) - initial
        // (B, A) - initial
        // (A, A) - via A->B->A
        // (B, B) - via B->A->B
        assert_eq!(closure.cardinality(), 4);
        assert!(closure.contains(&tuple! { n1: "A", n2: "A" }));
        assert!(closure.contains(&tuple! { n1: "B", n2: "B" }));
    }

    #[test]
    fn test_tclose_disconnected() {
        // 1->2, 3->4
        let heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int);
        let mut r = Relation::new(RelationType::new(heading));

        r.insert(tuple! { x: 1i64, y: 2i64 }).unwrap();
        r.insert(tuple! { x: 3i64, y: 4i64 }).unwrap();

        let closure = r.tclose("x", "y").unwrap();

        // No new paths
        assert_eq!(closure.cardinality(), 2);
    }

    #[test]
    fn test_tclose_invalid_degree() {
        let heading = TupleType::new().with_attribute("x", ScalarType::Int);
        let r = Relation::new(RelationType::new(heading));

        assert!(r.tclose("x", "x").is_err());
    }

    #[test]
    fn test_tclose_type_mismatch() {
        let heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::String);
        let mut r = Relation::new(RelationType::new(heading));
        r.insert(tuple! { x: 1i64, y: "a" }).unwrap();

        assert!(r.tclose("x", "y").is_err());
    }
}
