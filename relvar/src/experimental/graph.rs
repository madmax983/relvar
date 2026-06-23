//! Relational Graph Analytics.
//!
//! This module demonstrates how graph algorithms can be implemented using
//! pure relational algebra operations. It provides a `Graph` abstraction
//! over node and edge relations and implements BFS and PageRank.
//!
//! # Example: Breadth-First Search (BFS)
//!
//! ```
//! use relvar::{Relation, RelationType, ScalarType, TupleType, ScalarValue, tuple};
//! use relvar::experimental::Graph;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // 1. Define Nodes: {id}
//! let node_heading = TupleType::new().with_attribute("id", ScalarType::Int);
//! let mut nodes = Relation::new(RelationType::new(node_heading));
//! nodes.insert(tuple! { id: 1i64 })?;
//! nodes.insert(tuple! { id: 2i64 })?;
//! nodes.insert(tuple! { id: 3i64 })?;
//!
//! // 2. Define Edges: {from, to}
//! let edge_heading = TupleType::new()
//!     .with_attribute("from", ScalarType::Int)
//!     .with_attribute("to", ScalarType::Int);
//! let mut edges = Relation::new(RelationType::new(edge_heading));
//! edges.insert(tuple! { from: 1i64, to: 2i64 })?;
//! edges.insert(tuple! { from: 2i64, to: 3i64 })?;
//!
//! // 3. Create Graph View
//! let graph = Graph::new(nodes, edges, "id", "from", "to");
//!
//! // 4. Run BFS from node 1
//! let distances = graph.bfs(ScalarValue::Int(1))?;
//!
//! // Result: 1->0, 2->1, 3->2
//! let t3 = distances.tuples().find(|t| t.get_typed::<i64>("id") == Some(3)).unwrap();
//! assert_eq!(t3.get_typed::<i64>("distance"), Some(2));
//! # Ok(())
//! # }
//! ```

use relvar_core::algebra::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue, Tuple};

/// A graph wrapper around relational data.
///
/// A graph consists of:
/// - A `nodes` relation containing at least a unique identifier attribute.
/// - An `edges` relation containing source and target node identifiers.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// // Note: This is a placeholder example
/// ```
pub struct Graph {
    nodes: Relation,
    edges: Relation,
    node_id_attr: String,
    from_attr: String,
    to_attr: String,
}

impl Graph {
    /// Creates a new Graph view over existing relations.
    ///
    /// # Arguments
    ///
    /// * `nodes` - The relation containing nodes.
    /// * `edges` - The relation containing edges.
    /// * `node_id_attr` - The attribute name for node IDs in the `nodes` relation.
    /// * `from_attr` - The attribute name for source node IDs in the `edges` relation.
    /// * `to_attr` - The attribute name for target node IDs in the `edges` relation.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// // Note: This is a placeholder example
    /// ```
    pub fn new(
        nodes: Relation,
        edges: Relation,
        node_id_attr: &str,
        from_attr: &str,
        to_attr: &str,
    ) -> Self {
        Self {
            nodes,
            edges,
            node_id_attr: node_id_attr.to_string(),
            from_attr: from_attr.to_string(),
            to_attr: to_attr.to_string(),
        }
    }

    /// Performs a Breadth-First Search (BFS) starting from a given node.
    ///
    /// Yields a relation with heading `(node_id, distance)` containing all
    /// reachable nodes and their shortest distance from the start node.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::AlgebraError` if internal relational operations (Join, Project, etc.) fail,
    /// typically due to schema mismatches or invalid attribute names.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// // Note: This is a placeholder example
    /// ```
    pub fn bfs(&self, start_node_id: ScalarValue) -> Result<Relation, DatabaseError> {
        let mut visited = self.initialize_bfs_visited(start_node_id)?;
        let mut frontier = visited.clone();

        loop {
            if frontier.is_empty() {
                break;
            }

            let new_frontier = self.compute_next_frontier(&visited, &frontier)?;

            if new_frontier.is_empty() {
                break;
            }

            visited = visited
                .union(&new_frontier)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
            frontier = new_frontier;
        }

        Ok(visited)
    }

    fn initialize_bfs_visited(
        &self,
        start_node_id: ScalarValue,
    ) -> Result<Relation, DatabaseError> {
        let result_heading = TupleType::new()
            .with_attribute(
                self.node_id_attr.clone(),
                self.nodes
                    .relation_type()
                    .heading()
                    .get_attribute_type(&self.node_id_attr)
                    .unwrap()
                    .clone(),
            )
            .with_attribute("distance", ScalarType::Int);

        let result_type = RelationType::new(result_heading.clone());
        let mut visited = Relation::new(result_type.clone());
        let mut start_tuple_map = std::collections::BTreeMap::new();
        start_tuple_map.insert(self.node_id_attr.clone(), start_node_id);
        start_tuple_map.insert("distance".to_string(), ScalarValue::Int(0));

        visited.insert(
            Tuple::new(result_heading, start_tuple_map)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?,
        )?;

        Ok(visited)
    }

    fn compute_next_frontier(
        &self,
        visited: &Relation,
        frontier: &Relation,
    ) -> Result<Relation, DatabaseError> {
        let rename_map = vec![(self.node_id_attr.as_str(), self.from_attr.as_str())];
        let frontier_renamed = frontier.rename(&rename_map);
        let joined = frontier_renamed.join(&self.edges)?;
        let projected = joined.project(&[self.to_attr.as_str(), "distance"]);

        let rename_back_map = vec![(self.to_attr.as_str(), self.node_id_attr.as_str())];
        let next_nodes = projected.rename(&rename_back_map);

        let next_nodes_inc = next_nodes
            .extend("new_distance", ScalarType::Int, |t| {
                let d = t.get_typed::<i64>("distance").unwrap();
                ScalarValue::Int(d + 1)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let next_frontier_candidates = next_nodes_inc
            .project(&[self.node_id_attr.as_str(), "new_distance"])
            .rename(&[("new_distance", "distance")]);

        let visited_ids = visited.project(&[self.node_id_attr.as_str()]);
        let candidate_ids = next_frontier_candidates.project(&[self.node_id_attr.as_str()]);
        let new_ids = candidate_ids
            .difference(&visited_ids)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        new_ids.join(&next_frontier_candidates)
    }

    /// Computes PageRank for all nodes in the graph.
    ///
    /// # Arguments
    ///
    /// * `iterations` - Number of iterations to run.
    /// * `damping_factor` - Probability of following a link (usually 0.85).
    ///
    /// # Returns
    ///
    /// A relation with heading `(node_id, rank)`.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::AlgebraError` if internal relational operations fail.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// // Note: This is a placeholder example
    /// ```
    pub fn pagerank(
        &self,
        iterations: usize,
        damping_factor: f64,
    ) -> Result<Relation, DatabaseError> {
        let num_nodes = self.nodes.cardinality();
        if num_nodes == 0 {
            return Ok(self.nodes.clone()); // Or empty relation with rank
        }

        let initial_rank = 1.0 / (num_nodes as f64);
        let mut ranks = self.initialize_pagerank_ranks(initial_rank)?;
        let out_degrees = self.compute_out_degrees()?;

        for _ in 0..iterations {
            ranks =
                self.compute_pagerank_iteration(&ranks, &out_degrees, num_nodes, damping_factor)?;
        }

        Ok(ranks)
    }

    fn initialize_pagerank_ranks(&self, initial_rank: f64) -> Result<Relation, DatabaseError> {
        self.nodes
            .project(&[self.node_id_attr.as_str()])
            .extend("rank", ScalarType::Float, move |_| {
                ScalarValue::Float(initial_rank)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    fn compute_out_degrees(&self) -> Result<Relation, DatabaseError> {
        self.edges
            .summarize(
                &[self.from_attr.as_str()],
                &[Aggregation::count("out_degree")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    fn compute_pagerank_iteration(
        &self,
        ranks: &Relation,
        out_degrees: &Relation,
        num_nodes: usize,
        damping_factor: f64,
    ) -> Result<Relation, DatabaseError> {
        let rename_map = vec![(self.node_id_attr.as_str(), self.from_attr.as_str())];
        let ranks_renamed = ranks.rename(&rename_map);
        let joined_edges = ranks_renamed.join(&self.edges)?;
        let with_degrees = joined_edges.join(out_degrees)?;

        let contributions = with_degrees
            .extend("contribution", ScalarType::Float, |t| {
                let r = t.get_typed::<f64>("rank").unwrap();
                let d = t.get_typed::<i64>("out_degree").unwrap();
                if d == 0 {
                    ScalarValue::Float(0.0)
                } else {
                    ScalarValue::Float(r / (d as f64))
                }
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let incoming = contributions.project(&[self.to_attr.as_str(), "contribution"]);
        let rename_back_map = vec![(self.to_attr.as_str(), self.node_id_attr.as_str())];
        let incoming_renamed = incoming.rename(&rename_back_map);

        let new_ranks_sum = incoming_renamed
            .summarize(
                &[self.node_id_attr.as_str()],
                &[Aggregation::sum_float("sum_rank", "contribution")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let all_node_ids = self.nodes.project(&[self.node_id_attr.as_str()]);
        let ranked_node_ids = new_ranks_sum.project(&[self.node_id_attr.as_str()]);
        let missing_nodes = all_node_ids
            .difference(&ranked_node_ids)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let missing_ranks = missing_nodes
            .extend("sum_rank", ScalarType::Float, |_| ScalarValue::Float(0.0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let total_ranks = new_ranks_sum
            .union(&missing_ranks)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let base_score = (1.0 - damping_factor) / (num_nodes as f64);

        let result = total_ranks
            .extend("new_rank_final", ScalarType::Float, move |t| {
                let sum_r = t.get_typed::<f64>("sum_rank").unwrap();
                ScalarValue::Float(base_score + damping_factor * sum_r)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&[self.node_id_attr.as_str(), "new_rank_final"])
            .rename(&[("new_rank_final", "rank")]);

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;

    #[test]
    fn test_bfs_simple_chain() {
        // 1 -> 2 -> 3
        let node_heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let mut nodes = Relation::new(RelationType::new(node_heading));
        nodes.insert(tuple! { id: 1i64 }).unwrap();
        nodes.insert(tuple! { id: 2i64 }).unwrap();
        nodes.insert(tuple! { id: 3i64 }).unwrap();

        let edge_heading = TupleType::new()
            .with_attribute("from", ScalarType::Int)
            .with_attribute("to", ScalarType::Int);
        let mut edges = Relation::new(RelationType::new(edge_heading));
        edges.insert(tuple! { from: 1i64, to: 2i64 }).unwrap();
        edges.insert(tuple! { from: 2i64, to: 3i64 }).unwrap();

        let graph = Graph::new(nodes, edges, "id", "from", "to");
        let result = graph.bfs(ScalarValue::Int(1)).unwrap();

        // 1: dist 0
        // 2: dist 1
        // 3: dist 2
        assert_eq!(result.cardinality(), 3);

        let t1 = result
            .tuples()
            .find(|t| t.get_typed::<i64>("id") == Some(1))
            .unwrap();
        assert_eq!(t1.get_typed::<i64>("distance"), Some(0));

        let t2 = result
            .tuples()
            .find(|t| t.get_typed::<i64>("id") == Some(2))
            .unwrap();
        assert_eq!(t2.get_typed::<i64>("distance"), Some(1));

        let t3 = result
            .tuples()
            .find(|t| t.get_typed::<i64>("id") == Some(3))
            .unwrap();
        assert_eq!(t3.get_typed::<i64>("distance"), Some(2));
    }

    #[test]
    fn test_pagerank_small() {
        // 1 <-> 2
        let node_heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let mut nodes = Relation::new(RelationType::new(node_heading));
        nodes.insert(tuple! { id: 1i64 }).unwrap();
        nodes.insert(tuple! { id: 2i64 }).unwrap();

        let edge_heading = TupleType::new()
            .with_attribute("from", ScalarType::Int)
            .with_attribute("to", ScalarType::Int);
        let mut edges = Relation::new(RelationType::new(edge_heading));
        edges.insert(tuple! { from: 1i64, to: 2i64 }).unwrap();
        edges.insert(tuple! { from: 2i64, to: 1i64 }).unwrap();

        let graph = Graph::new(nodes, edges, "id", "from", "to");

        // With d=0.85, 2 nodes cycle, they should converge to 0.5 each.
        let ranks = graph.pagerank(10, 0.85).unwrap();

        assert_eq!(ranks.cardinality(), 2);

        for t in ranks.tuples() {
            let r = t.get_typed::<f64>("rank").unwrap();
            assert!((r - 0.5).abs() < 0.01, "Rank {} should be approx 0.5", r);
        }
    }
}
