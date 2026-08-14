//! Relational Graph Neural Network (GNN)
//!
//! This module demonstrates how a Graph Neural Network (specifically, message passing)
//! can be modeled and executed using purely relational algebra operations.
//!
//! # Concept
//!
//! A basic message passing layer involves three inputs:
//! - **Features**: Relation `(node_id: Int, in_idx: Int, value: Float)`.
//! - **Edges**: Relation `(src: Int, dst: Int)`.
//! - **Weights**: Relation `(in_idx: Int, out_idx: Int, weight: Float)`.
//!
//! We compute the next layer's node embeddings via relational operations:
//! 1. Rename `node_id` to `src` in `Features`.
//! 2. Join `Features` with `Edges` on `src`.
//! 3. Summarize by `(dst, in_idx)` to sum the neighbor features (Message Passing).
//! 4. Rename `dst` to `node_id`.
//! 5. Join the aggregated features with `Weights` on `in_idx`.
//! 6. Extend to multiply `agg_value` by `weight`.
//! 7. Summarize by `(node_id, out_idx)` to sum the products (Linear Transformation).
//! 8. Rename to the standard `Features` schema: `(node_id, out_idx -> in_idx, sum_product -> value)`.

use relvar_core::{
    algebra::{Aggregation, AggregationFn},
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue, Tuple},
};

/// A Relational Graph Neural Network.
///
/// The `GraphNeuralNetwork` structurally organizes message passing over a graph modeled as standard relations.
/// It operates purely via relational algebra combinations: joins, summaries, and extends.
///
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType, tuple};
/// use relvar::experimental::gnn::GraphNeuralNetwork;
///
/// // 1. Features: Node 1 has a value of 1.0, Node 3 has a value of 2.0.
/// let mut features = Relation::new(RelationType::new(
///     TupleType::new()
///         .with_attribute("node_id", ScalarType::Int)
///         .with_attribute("in_idx", ScalarType::Int)
///         .with_attribute("value", ScalarType::Float)
/// ));
/// features.insert(tuple! { node_id: 1i64, in_idx: 0i64, value: 1.0 }).unwrap();
/// features.insert(tuple! { node_id: 3i64, in_idx: 0i64, value: 2.0 }).unwrap();
///
/// // 2. Edges: Both Node 1 and Node 3 point to Node 2.
/// let mut edges = Relation::new(RelationType::new(
///     TupleType::new()
///         .with_attribute("src", ScalarType::Int)
///         .with_attribute("dst", ScalarType::Int)
/// ));
/// edges.insert(tuple! { src: 1i64, dst: 2i64 }).unwrap();
/// edges.insert(tuple! { src: 3i64, dst: 2i64 }).unwrap();
///
/// // 3. Weights: Transformation weight from in_idx 0 to out_idx 0 is 0.5.
/// let mut weights = Relation::new(RelationType::new(
///     TupleType::new()
///         .with_attribute("in_idx", ScalarType::Int)
///         .with_attribute("out_idx", ScalarType::Int)
///         .with_attribute("weight", ScalarType::Float)
/// ));
/// weights.insert(tuple! { in_idx: 0i64, out_idx: 0i64, weight: 0.5 }).unwrap();
///
/// // Perform the forward pass.
/// let output = GraphNeuralNetwork::forward(&features, &edges, &weights).unwrap();
///
/// // Node 2 receives aggregated sum (1.0 + 2.0) = 3.0.
/// // Applied weight (3.0 * 0.5) = 1.5.
/// assert_eq!(output.cardinality(), 1);
/// assert!(output.contains(&tuple! { node_id: 2i64, in_idx: 0i64, value: 1.5 }));
/// ```
pub struct GraphNeuralNetwork;

impl GraphNeuralNetwork {
    /// Computes a single forward pass of the GNN.
    ///
    /// This applies a relational interpretation of Message Passing:
    /// 1. Joins the feature vectors with edge relationships.
    /// 2. Summarizes the values across incoming edges (aggregation).
    /// 3. Computes the linear transformation using a join with layer weights.
    ///
    /// # Arguments
    /// * `features` - Relation with schema `(node_id: Int, in_idx: Int, value: Float)`
    /// * `edges` - Relation with schema `(src: Int, dst: Int)`
    /// * `weights` - Relation with schema `(in_idx: Int, out_idx: Int, weight: Float)`
    ///
    /// # Returns
    /// * A new Relation with schema `(node_id: Int, in_idx: Int, value: Float)` representing
    ///   the computed output features.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType, tuple};
    /// use relvar::experimental::gnn::GraphNeuralNetwork;
    ///
    /// // Minimal example for a single node passing a value along an edge.
    /// let mut features = Relation::new(RelationType::new(TupleType::new()
    ///     .with_attribute("node_id", ScalarType::Int).with_attribute("in_idx", ScalarType::Int).with_attribute("value", ScalarType::Float)));
    /// features.insert(tuple! { node_id: 10i64, in_idx: 0i64, value: 5.0 }).unwrap();
    ///
    /// let mut edges = Relation::new(RelationType::new(TupleType::new()
    ///     .with_attribute("src", ScalarType::Int).with_attribute("dst", ScalarType::Int)));
    /// edges.insert(tuple! { src: 10i64, dst: 20i64 }).unwrap();
    ///
    /// let mut weights = Relation::new(RelationType::new(TupleType::new()
    ///     .with_attribute("in_idx", ScalarType::Int).with_attribute("out_idx", ScalarType::Int).with_attribute("weight", ScalarType::Float)));
    /// weights.insert(tuple! { in_idx: 0i64, out_idx: 0i64, weight: 2.0 }).unwrap();
    ///
    /// // Compute the forward step.
    /// let output = GraphNeuralNetwork::forward(&features, &edges, &weights).unwrap();
    ///
    /// // Node 20 aggregated value 5.0 * weight 2.0 = 10.0
    /// assert!(output.contains(&tuple! { node_id: 20i64, in_idx: 0i64, value: 10.0 }));
    /// ```
    pub fn forward(
        features: &Relation,
        edges: &Relation,
        weights: &Relation,
    ) -> Result<Relation, DatabaseError> {
        // Step 1: Prepare features for message passing. Rename 'node_id' to 'src'.
        let features_src = features.rename(&[("node_id", "src")]);

        // Step 2: Message Passing - Propagate features along edges.
        let messages = edges
            .join(&features_src)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Step 3: Aggregate messages for each destination node.
        let aggregated = messages
            .summarize(
                &["dst", "in_idx"],
                &[Aggregation {
                    result_name: "agg_value".to_string(),
                    result_type: ScalarType::Float,
                    function: AggregationFn::Sum("value".to_string()),
                }],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Step 4: Rename back to 'node_id' for linear transformation.
        let agg_renamed = aggregated.rename(&[("dst", "node_id")]);

        // Step 5: Linear Transformation - Join aggregated features with layer weights.
        let transformed = agg_renamed
            .join(weights)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Step 6: Multiply aggregated values by weights.
        let multiplied = transformed
            .extend("product", ScalarType::Float, |t: &Tuple| {
                let v = t.get_typed::<f64>("agg_value").unwrap();
                let w = t.get_typed::<f64>("weight").unwrap();
                ScalarValue::Float(v * w)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Step 7: Sum the products over 'out_idx' to complete the matrix multiplication.
        let out_features = multiplied
            .summarize(
                &["node_id", "out_idx"],
                &[Aggregation {
                    result_name: "sum_product".to_string(),
                    result_type: ScalarType::Float,
                    function: AggregationFn::Sum("product".to_string()),
                }],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Step 8: Standardize the schema for the next layer.
        let final_features =
            out_features.rename(&[("out_idx", "in_idx"), ("sum_product", "value")]);

        Ok(final_features)
    }
}

#[allow(dead_code)]
#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::{
        tuple,
        types::{RelationType, ScalarType, TupleType},
    };

    #[test]
    fn test_gnn_forward() {
        // Setup Features: (node_id: Int, in_idx: Int, value: Float)
        let features_type = RelationType::new(
            TupleType::new()
                .with_attribute("node_id", ScalarType::Int)
                .with_attribute("in_idx", ScalarType::Int)
                .with_attribute("value", ScalarType::Float),
        );
        let mut features = Relation::new(features_type);
        // Node 1 has feature 1.0 at index 0
        features
            .insert(tuple! { node_id: 1i64, in_idx: 0i64, value: 1.0 })
            .unwrap();
        // Node 3 has feature 2.0 at index 0
        features
            .insert(tuple! { node_id: 3i64, in_idx: 0i64, value: 2.0 })
            .unwrap();

        // Setup Edges: (src: Int, dst: Int)
        let edges_type = RelationType::new(
            TupleType::new()
                .with_attribute("src", ScalarType::Int)
                .with_attribute("dst", ScalarType::Int),
        );
        let mut edges = Relation::new(edges_type);
        // Edge: 1 -> 2
        edges.insert(tuple! { src: 1i64, dst: 2i64 }).unwrap();
        // Edge: 3 -> 2
        edges.insert(tuple! { src: 3i64, dst: 2i64 }).unwrap();

        // Setup Weights: (in_idx: Int, out_idx: Int, weight: Float)
        let weights_type = RelationType::new(
            TupleType::new()
                .with_attribute("in_idx", ScalarType::Int)
                .with_attribute("out_idx", ScalarType::Int)
                .with_attribute("weight", ScalarType::Float),
        );
        let mut weights = Relation::new(weights_type);
        // Weight from in_idx=0 to out_idx=0 is 0.5
        weights
            .insert(tuple! { in_idx: 0i64, out_idx: 0i64, weight: 0.5 })
            .unwrap();

        // Run Forward Pass
        let output = GraphNeuralNetwork::forward(&features, &edges, &weights).unwrap();

        // Node 2 receives messages from Node 1 (1.0) and Node 3 (2.0).
        // Sum = 3.0.
        // Multiply by weight 0.5 -> 1.5.
        // Output for Node 2 at out_idx (renamed to in_idx) 0 should be 1.5.

        assert_eq!(output.cardinality(), 1);
        let expected_tuple = tuple! { node_id: 2i64, in_idx: 0i64, value: 1.5 };
        assert!(output.contains(&expected_tuple));
    }
}
