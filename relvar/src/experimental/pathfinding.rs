//! Relational Pathfinding (A* and Dijkstra)
//!
//! This module demonstrates how shortest-path algorithms like Dijkstra's
//! algorithm (and A* concepts) can be implemented using purely relational
//! algebra operations.
//!
//! # Concept
//!
//! We use a relational version of the Bellman-Ford / Shortest Path relaxation
//! approach. Relational algebra operates on sets, so instead of a priority queue
//! (which is procedural and ordered), we iteratively relax all edges in the
//! current "frontier" until the distances stop changing (reaching a fixpoint).
//!
//! - **Nodes**: Relation `(node_id: Int)`
//! - **Edges**: Relation `(from_node: Int, to_node: Int, cost: Float)`
//! - **State**: Relation `(node_id: Int, cost: Float, parent_id: Int)`

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue, Tuple},
};

/// A Relational Pathfinding Engine.
pub struct PathfindingEngine {
    /// The nodes in the graph. Schema: (node_id: Int)
    pub nodes: Relation,
    /// The directed edges. Schema: (from_node: Int, to_node: Int, cost: Float)
    pub edges: Relation,
}

impl PathfindingEngine {
    /// Creates a new PathfindingEngine.
    pub fn new(nodes: Relation, edges: Relation) -> Self {
        Self { nodes, edges }
    }

    /// Finds the shortest paths from the `start_node_id` to all other reachable nodes.
    ///
    /// This uses a relational relaxation approach (Bellman-Ford variant):
    /// 1. Start with the source node having cost 0.
    /// 2. Iteratively join current paths with edges to find new paths.
    /// 3. Calculate new costs and use `Summarize` with `Aggregation::min` to keep the best.
    /// 4. Stop when the paths relation stops changing.
    ///
    /// Returns a relation with schema: `(node_id: Int, path_cost: Float)`
    pub fn compute_shortest_paths(&self, start_node_id: i64) -> Result<Relation, DatabaseError> {
        // 1. Initialize State
        let state_heading = TupleType::new()
            .with_attribute("node_id".to_string(), ScalarType::Int)
            .with_attribute("path_cost".to_string(), ScalarType::Float);

        let mut current_state = Relation::new(RelationType::new(state_heading.clone()));
        current_state.insert(
            Tuple::new(
                state_heading,
                [
                    ("node_id".to_string(), ScalarValue::Int(start_node_id)),
                    ("path_cost".to_string(), ScalarValue::Float(0.0)),
                ]
                .into_iter()
                .collect::<std::collections::BTreeMap<String, ScalarValue>>(),
            )
            .unwrap(),
        )?;

        let max_iterations = self.nodes.cardinality();

        for _ in 0..max_iterations {
            let next_state = self.relax_edges(&current_state)?;

            let diff1 = next_state.difference(&current_state).unwrap();
            let diff2 = current_state.difference(&next_state).unwrap();

            if diff1.is_empty() && diff2.is_empty() {
                // Converged
                break;
            }

            current_state = next_state;
        }

        Ok(current_state)
    }

    fn relax_edges(&self, current_state: &Relation) -> Result<Relation, DatabaseError> {
        // We want to find: new_cost = current_state.path_cost + edges.cost

        // a. Rename current_state to match edges: (node_id -> from_node)
        let state_renamed =
            current_state.rename(&[("node_id", "from_node"), ("path_cost", "current_cost")]);

        // b. Join with edges
        let joined = state_renamed.join(&self.edges)?;

        // c. Calculate new cost
        let new_paths = joined
            .extend("new_cost", ScalarType::Float, |t| {
                let current = t.get_typed::<f64>("current_cost").unwrap();
                let cost = t.get_typed::<f64>("cost").unwrap();
                ScalarValue::Float(current + cost)
            })
            .unwrap();

        // d. Project to target schema: (node_id, path_cost)
        let new_paths_projected = new_paths
            .project(&["to_node", "new_cost"])
            .rename(&[("to_node", "node_id"), ("new_cost", "path_cost")]);

        // e. Union with existing state
        let all_paths = current_state.union(&new_paths_projected).unwrap();

        // f. Group by node_id and take MIN(path_cost)
        let best_paths = all_paths
            .summarize(
                &["node_id"],
                &[Aggregation::min(
                    "path_cost",
                    "path_cost",
                    ScalarType::Float,
                )],
            )
            .unwrap();

        Ok(best_paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;

    #[test]
    fn test_shortest_path() {
        let node_heading = TupleType::new().with_attribute("node_id", ScalarType::Int);
        let mut nodes = Relation::new(RelationType::new(node_heading));
        nodes.insert(tuple! { node_id: 1i64 }).unwrap();
        nodes.insert(tuple! { node_id: 2i64 }).unwrap();
        nodes.insert(tuple! { node_id: 3i64 }).unwrap();
        nodes.insert(tuple! { node_id: 4i64 }).unwrap();

        let edge_heading = TupleType::new()
            .with_attribute("from_node", ScalarType::Int)
            .with_attribute("to_node", ScalarType::Int)
            .with_attribute("cost", ScalarType::Float);
        let mut edges = Relation::new(RelationType::new(edge_heading));

        // 1 -> 2 (cost 1.0)
        edges
            .insert(tuple! { from_node: 1i64, to_node: 2i64, cost: 1.0f64 })
            .unwrap();
        // 2 -> 3 (cost 2.0)
        edges
            .insert(tuple! { from_node: 2i64, to_node: 3i64, cost: 2.0f64 })
            .unwrap();
        // 1 -> 3 (cost 4.0) - direct but slower
        edges
            .insert(tuple! { from_node: 1i64, to_node: 3i64, cost: 4.0f64 })
            .unwrap();
        // 3 -> 4 (cost 1.0)
        edges
            .insert(tuple! { from_node: 3i64, to_node: 4i64, cost: 1.0f64 })
            .unwrap();

        let engine = PathfindingEngine::new(nodes, edges);
        let paths = engine.compute_shortest_paths(1).unwrap();

        assert_eq!(paths.cardinality(), 4);

        let t3 = paths
            .tuples()
            .find(|t| t.get_typed::<i64>("node_id").unwrap() == 3)
            .unwrap();
        // Shortest path to 3 is 1->2->3 (cost 3.0), not direct (cost 4.0)
        assert_eq!(t3.get_typed::<f64>("path_cost").unwrap(), 3.0);

        let t4 = paths
            .tuples()
            .find(|t| t.get_typed::<i64>("node_id").unwrap() == 4)
            .unwrap();
        // Shortest path to 4 is 1->2->3->4 (cost 4.0)
        assert_eq!(t4.get_typed::<f64>("path_cost").unwrap(), 4.0);
    }
}
