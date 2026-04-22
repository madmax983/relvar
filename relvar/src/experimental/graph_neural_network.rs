//! Relational Graph Neural Network (GNN) Message Passing
//!
//! This module demonstrates how to implement a basic Graph Neural Network
//! message passing algorithm using purely relational algebra.
//!
//! # Concept
//!
//! A Graph Neural Network aggregates features from neighboring nodes to update
//! the feature representation of each node. This process can be modeled naturally
//! with relational joins and summaries.
//!
//! - **Nodes/Features**: `(node_id: Int, feature_idx: Int, value: Float)`
//! - **Edges**: `(source_id: Int, target_id: Int)`
//!
//! The message passing step consists of:
//! 1. Join `Edges` and `Features` on `source_id = node_id` to get messages.
//! 2. Rename `target_id` to `node_id`.
//! 3. Summarize by `(node_id, feature_idx)` to compute the sum of neighbor features.
//! 4. In a real GNN, this would be followed by a neural network layer (e.g., matrix multiplication).
//!    For simplicity, we only demonstrate the relational message aggregation step here.

use relvar_core::{algebra::Aggregation, error::DatabaseError, values::Relation};

/// A Relational Graph Neural Network message passing step.
///
/// # Examples
///
/// ```
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::values::Relation;
/// use relvar::experimental::graph_neural_network::RelationalGNN;
/// // Note: This is a placeholder example
/// ```
pub struct RelationalGNN {
    /// Schema: (node_id: Int, feature_idx: Int, value: Float)
    pub node_features: Relation,
    /// Schema: (source_id: Int, target_id: Int)
    pub edges: Relation,
}

impl RelationalGNN {
    /// Creates a new Relational GNN representation.
    pub fn new(node_features: Relation, edges: Relation) -> Self {
        Self {
            node_features,
            edges,
        }
    }

    /// Performs one step of message passing (neighborhood aggregation).
    ///
    /// Returns a new relation of updated node features.
    /// Schema: (node_id: Int, feature_idx: Int, value: Float)
    pub fn aggregate_messages(&self) -> Result<Relation, DatabaseError> {
        // 1. Rename node_features to match edges source_id
        let features_renamed = self.node_features.rename(&[("node_id", "source_id")]);

        // 2. Join to propagate features along edges
        // Result schema: (source_id, target_id, feature_idx, value)
        let messages = self.edges.join(&features_renamed)?;

        // 3. Rename target_id back to node_id, and drop source_id
        let messages_at_target = messages
            .project(&["target_id", "feature_idx", "value"])
            .rename(&[("target_id", "node_id")]);

        // 4. Aggregate messages for each node and feature_idx using SUM
        let aggregated_features = messages_at_target
            .summarize(
                &["node_id", "feature_idx"],
                &[Aggregation::sum_float("agg_value", "value")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Rename agg_value back to value to match the original feature schema
        let updated_features = aggregated_features.rename(&[("agg_value", "value")]);

        Ok(updated_features)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    fn setup_schema() -> (RelationType, RelationType) {
        let features_type = RelationType::new(
            TupleType::new()
                .with_attribute("node_id", ScalarType::Int)
                .with_attribute("feature_idx", ScalarType::Int)
                .with_attribute("value", ScalarType::Float),
        );
        let edges_type = RelationType::new(
            TupleType::new()
                .with_attribute("source_id", ScalarType::Int)
                .with_attribute("target_id", ScalarType::Int),
        );
        (features_type, edges_type)
    }

    #[test]
    fn test_gnn_message_passing() {
        let (features_type, edges_type) = setup_schema();

        let mut features = Relation::new(features_type);
        let mut edges = Relation::new(edges_type);

        // Node Features (1 feature per node, index 0)
        // Node 1: [1.0]
        // Node 2: [2.0]
        // Node 3: [3.0]
        features
            .insert(tuple! { node_id: 1i64, feature_idx: 0i64, value: 1.0f64 })
            .unwrap();
        features
            .insert(tuple! { node_id: 2i64, feature_idx: 0i64, value: 2.0f64 })
            .unwrap();
        features
            .insert(tuple! { node_id: 3i64, feature_idx: 0i64, value: 3.0f64 })
            .unwrap();

        // Edges
        // 1 -> 2
        // 1 -> 3
        // 2 -> 3
        edges
            .insert(tuple! { source_id: 1i64, target_id: 2i64 })
            .unwrap();
        edges
            .insert(tuple! { source_id: 1i64, target_id: 3i64 })
            .unwrap();
        edges
            .insert(tuple! { source_id: 2i64, target_id: 3i64 })
            .unwrap();

        let gnn = RelationalGNN::new(features, edges);

        let updated = gnn.aggregate_messages().unwrap();

        // Check the aggregated features
        // Node 1 receives no messages
        // Node 2 receives from 1: [1.0]
        // Node 3 receives from 1 and 2: [1.0 + 2.0] = [3.0]

        let mut results: Vec<_> = updated
            .tuples()
            .map(|t| {
                (
                    t.get_typed::<i64>("node_id").unwrap(),
                    t.get_typed::<i64>("feature_idx").unwrap(),
                    t.get_typed::<f64>("value").unwrap(),
                )
            })
            .collect();
        results.sort_by_key(|r| r.0);

        assert_eq!(results.len(), 2);

        // Node 2, Feature 0, Sum = 1.0
        assert_eq!(results[0], (2, 0, 1.0));

        // Node 3, Feature 0, Sum = 3.0
        assert_eq!(results[1], (3, 0, 3.0));
    }
}
