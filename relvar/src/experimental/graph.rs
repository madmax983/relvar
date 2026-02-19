//! Relational Graph Analytics.
//!
//! This module demonstrates how graph algorithms can be implemented using
//! pure relational algebra operations. It provides a `Graph` abstraction
//! over node and edge relations and implements BFS and PageRank.

use relvar_core::algebra::summarize::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue, Tuple};

/// A graph wrapper around relational data.
///
/// A graph consists of:
/// - A `nodes` relation containing at least a unique identifier attribute.
/// - An `edges` relation containing source and target node identifiers.
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
    /// Returns a relation with heading `(node_id, distance)` containing all
    /// reachable nodes and their shortest distance from the start node.
    pub fn bfs(&self, start_node_id: ScalarValue) -> Result<Relation, DatabaseError> {
        // 1. Initialize result schema: (node_id, distance)
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

        // 2. Create initial visited set (just the start node at distance 0)
        let mut visited = Relation::new(result_type.clone());
        let mut start_tuple_map = std::collections::BTreeMap::new();
        start_tuple_map.insert(self.node_id_attr.clone(), start_node_id);
        start_tuple_map.insert("distance".to_string(), ScalarValue::Int(0));

        visited.insert(
            Tuple::new(result_heading, start_tuple_map)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?,
        )?;

        // 3. Frontier is the set of newly visited nodes to expand
        let mut frontier = visited.clone();

        // 4. Loop until frontier is empty
        loop {
            if frontier.is_empty() {
                break;
            }

            // JOIN frontier with edges
            // frontier: (node_id, distance)
            // edges: (from, to)
            // Join condition: frontier.node_id = edges.from

            // Rename frontier.node_id -> from_attr to enable natural join
            let rename_map = vec![(self.node_id_attr.as_str(), self.from_attr.as_str())];
            let frontier_renamed = frontier.rename(&rename_map);

            // Join: (from, distance, to)
            let joined = frontier_renamed.join(&self.edges)?;

            // Project: (to, distance)
            let projected = joined.project(&[self.to_attr.as_str(), "distance"]);

            // Rename: to -> node_id
            let rename_back_map = vec![(self.to_attr.as_str(), self.node_id_attr.as_str())];
            let next_nodes = projected.rename(&rename_back_map);

            // Extend: distance = distance + 1
            let next_nodes_inc = next_nodes
                .extend("new_distance", ScalarType::Int, |t| {
                    let d = t.get_typed::<i64>("distance").unwrap();
                    ScalarValue::Int(d + 1)
                })
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // Project out old distance and rename new_distance -> distance
            let next_frontier_candidates = next_nodes_inc
                .project(&[self.node_id_attr.as_str(), "new_distance"])
                .rename(&[("new_distance", "distance")]);

            // Filter out already visited nodes
            // Note: Difference requires exact tuple match.
            // Since visited contains (node, dist), and we might have found a shorter path (impossible in BFS)
            // or longer path to same node. We only care about nodes NOT in visited regardless of distance.
            // However, relation difference is strict.
            // A better way is:
            // new_nodes = candidates.project(node_id) MINUS visited.project(node_id)
            // Then join back to get distances? No, that's complex.

            // Simpler: Just subtract visited.
            // In BFS, if we see a node again, it's at a distance >= current.
            // We only want strict new nodes.
            // But `difference` includes distance in comparison.
            // If visited has (A, 0) and candidates has (A, 1), difference keeps (A, 1).
            // We don't want that. We want to exclude A entirely.

            // Correct approach:
            // 1. Get visited nodes (just IDs)
            let visited_ids = visited.project(&[self.node_id_attr.as_str()]);
            // 2. Get candidate nodes (just IDs)
            let candidate_ids = next_frontier_candidates.project(&[self.node_id_attr.as_str()]);
            // 3. New IDs = candidate IDs MINUS visited IDs
            let new_ids = candidate_ids
                .difference(&visited_ids)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // 4. Reconstruct frontier with distances
            // We join new_ids with next_frontier_candidates to get the distances back.
            // natural join on node_id
            let new_frontier = new_ids.join(&next_frontier_candidates)?;

            // But wait, what if `next_frontier_candidates` has duplicates of the same node (from multiple parents)?
            // BFS level-order guarantees we process them together.
            // We should deduplicate by taking the one with min distance?
            // In unweighted BFS, they all have the same distance (current_depth + 1).
            // So simple projection handles deduplication (Relations are sets).

            if new_frontier.is_empty() {
                break;
            }

            // Update visited
            visited = visited
                .union(&new_frontier)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
            frontier = new_frontier;
        }

        Ok(visited)
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

        // 1. Initialize ranks: (node_id, rank)
        // We can't easily iterate and insert. Let's use Extend on `nodes`.
        let mut ranks = self
            .nodes
            .project(&[self.node_id_attr.as_str()])
            .extend("rank", ScalarType::Float, move |_| {
                ScalarValue::Float(initial_rank)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 2. Precompute out-degrees: (from_node, out_degree)
        // Group edges by from_attr, count(*).
        // Summarize requires an aggregation function.
        let out_degrees = self
            .edges
            .summarize(
                &[self.from_attr.as_str()],
                &[Aggregation::count("out_degree")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // We also need to handle dangling nodes (sink nodes with no outgoing edges).
        // They accumulate rank but don't distribute it. In standard PageRank, they distribute to everyone.
        // For this simplified implementation, we'll ignore that distribution (rank sink).

        for _ in 0..iterations {
            // Join ranks with edges to distribute score
            // ranks: (node_id, rank)
            // edges: (from, to)
            // out_degrees: (from, out_degree)

            // Prepare ranks for join: rename node_id -> from
            let rename_map = vec![(self.node_id_attr.as_str(), self.from_attr.as_str())];
            let ranks_renamed = ranks.rename(&rename_map);

            // Join with edges
            let joined_edges = ranks_renamed.join(&self.edges)?;

            // Join with out_degrees to get divisor
            let with_degrees = joined_edges.join(&out_degrees)?;

            // Calculate contribution: rank / out_degree
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

            // Project: (to, contribution)
            let incoming = contributions.project(&[self.to_attr.as_str(), "contribution"]);

            // Rename: to -> node_id
            let rename_back_map = vec![(self.to_attr.as_str(), self.node_id_attr.as_str())];
            let incoming_renamed = incoming.rename(&rename_back_map);

            // Summarize: Sum contributions for each node
            let new_ranks_sum = incoming_renamed
                .summarize(
                    &[self.node_id_attr.as_str()],
                    &[Aggregation::sum_float("sum_rank", "contribution")],
                )
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // Note: `new_ranks_sum` only contains nodes that have INCOMING edges.
            // Nodes with no incoming edges (sources) will be missing.
            // We must RIGHT JOIN with original nodes? Or Union with missing nodes?
            // Relvar doesn't have Right Join.
            // We can take `nodes` project `node_id`, difference `new_ranks_sum` project `node_id`,
            // extend with 0.0, and Union.

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

            // Apply damping factor: (1 - d) / N + d * sum_rank
            let base_score = (1.0 - damping_factor) / (num_nodes as f64);

            ranks = total_ranks
                .extend("new_rank_final", ScalarType::Float, move |t| {
                    let sum_r = t.get_typed::<f64>("sum_rank").unwrap();
                    ScalarValue::Float(base_score + damping_factor * sum_r)
                })
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
                .project(&[self.node_id_attr.as_str(), "new_rank_final"])
                .rename(&[("new_rank_final", "rank")]);
        }

        Ok(ranks)
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
