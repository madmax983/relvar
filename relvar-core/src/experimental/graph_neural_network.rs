use crate::algebra::summarize::Aggregation;
use crate::error::DatabaseError;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// A Graph Neural Network (GNN) modeled purely with relational algebra.
///
/// Models message passing over a graph using Joins and Aggregation to
/// simulate neighbor feature aggregation and linear transformations.
pub struct GraphNeuralNetwork {
    /// Edges relation (source: Int, target: Int)
    edges: Relation,
    /// Node features relation (node_id: Int, feature_val: Float)
    /// (For simplicity, this example assumes a scalar feature per node,
    ///  but could easily be extended to multi-dimensional vectors).
    features: Relation,
    /// Weights relation for transformation (weight_val: Float)
    weights: Relation,
}

impl GraphNeuralNetwork {
    /// Creates a new Relational GNN.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let edges_heading = TupleType::new()
            .with_attribute("source", ScalarType::Int)
            .with_attribute("target", ScalarType::Int);

        let features_heading = TupleType::new()
            .with_attribute("node_id", ScalarType::Int)
            .with_attribute("feature_val", ScalarType::Float);

        let weights_heading = TupleType::new().with_attribute("weight_val", ScalarType::Float);

        Self {
            edges: Relation::new(RelationType::new(edges_heading)),
            features: Relation::new(RelationType::new(features_heading)),
            weights: Relation::new(RelationType::new(weights_heading)),
        }
    }

    /// Adds an edge to the graph.
    pub fn add_edge(&mut self, source: i64, target: i64) -> Result<(), DatabaseError> {
        let _ = self
            .edges
            .insert(tuple! { source: source, target: target })?;
        Ok(())
    }

    /// Adds or updates a node feature.
    pub fn set_feature(&mut self, node_id: i64, feature_val: f64) -> Result<(), DatabaseError> {
        let t = tuple! { node_id: node_id, feature_val: feature_val };
        // Remove existing feature entry if any
        let existing = self
            .features
            .restrict(|tuple| tuple.get_typed::<i64>("node_id").unwrap() == node_id);
        self.features = self
            .features
            .clone()
            .difference_into(&existing)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let _ = self.features.insert(t)?;
        Ok(())
    }

    /// Sets the model weights.
    pub fn set_weights(&mut self, weight_val: f64) -> Result<(), DatabaseError> {
        self.weights = Relation::new(self.weights.relation_type().clone());
        let _ = self.weights.insert(tuple! { weight_val: weight_val })?;
        Ok(())
    }

    /// Performs one layer of Message Passing.
    /// Returns a new relation representing the updated node features.
    pub fn forward_pass(&self) -> Result<Relation, DatabaseError> {
        // 1. Join Edges with Source Features to get messages.
        // Rename features: (node_id -> source, feature_val -> msg_val)
        let source_features = self
            .features
            .rename(&[("node_id", "source"), ("feature_val", "msg_val")]);
        let messages = self.edges.join(&source_features)?;

        // 2. Aggregate messages for each target node.
        // Group by 'target', SUM('msg_val') -> 'agg_val'
        let aggs = [Aggregation::sum_float("agg_val", "msg_val")];
        let aggregated = messages
            .summarize(&["target"], &aggs)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Linear Transformation (apply weights).
        // Since weights is a single-tuple relation with 'weight_val', cross join will distribute it.
        // Then extend to compute the final transformed value.
        let transformed = aggregated
            .join(&self.weights)?
            .extend("new_feature", ScalarType::Float, |t| {
                let agg = t.get_typed::<f64>("agg_val").unwrap_or(0.0);
                let w = t.get_typed::<f64>("weight_val").unwrap_or(0.0);
                // In a real GNN, this might involve an activation function like ReLU here.
                crate::values::ScalarValue::Float(agg * w)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Project back to the expected features schema: (node_id, feature_val)
        let final_features = transformed
            .rename(&[("target", "node_id"), ("new_feature", "feature_val")])
            .project(&["node_id", "feature_val"]);

        Ok(final_features)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gnn_message_passing() {
        let mut gnn = GraphNeuralNetwork::new();

        // Graph structure: 1 -> 2, 3 -> 2
        gnn.add_edge(1, 2).unwrap();
        gnn.add_edge(3, 2).unwrap();

        // Initial features
        gnn.set_feature(1, 10.0).unwrap();
        gnn.set_feature(2, 5.0).unwrap(); // node 2 gets ignored in message source, receives messages
        gnn.set_feature(3, 20.0).unwrap();

        // Set weight
        gnn.set_weights(0.5).unwrap();

        let next_layer_features = gnn.forward_pass().unwrap();

        // Node 2 should receive: (10.0 + 20.0) * 0.5 = 15.0
        assert_eq!(next_layer_features.cardinality(), 1);
        let tuple = next_layer_features.tuples().next().unwrap();
        assert_eq!(tuple.get_typed::<i64>("node_id").unwrap(), 2);
        assert_eq!(tuple.get_typed::<f64>("feature_val").unwrap(), 15.0);
    }
}
