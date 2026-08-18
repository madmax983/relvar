//! Relational Graph Neural Network in Relational Algebra.
//!
//! This module implements a Message Passing Neural Network layer using purely relational algebra.

use crate::algebra::Aggregation;
use crate::error::DatabaseError;
use crate::types::ScalarType;
use crate::values::{Relation, ScalarValue, Tuple};

/// Evaluates one Message Passing Neural Network (MPNN) layer purely using relational algebra.
///
/// `edges` must have attributes: `source` (Int), `target` (Int).
/// `node_features` must have attributes: `node` (Int), `feature_id` (Int), `value` (Float).
/// `weights` must have attributes: `in_feature_id` (Int), `out_feature_id` (Int), `weight` (Float).
///
/// # Examples
/// ```
/// use relvar_core::tuple;
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::values::Relation;
/// use relvar_core::experimental::graph_neural_network::mpnn_layer;
///
/// // Create a simple line graph: 1 -> 2
/// let edge_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("source", ScalarType::Int)
///         .with_attribute("target", ScalarType::Int)
/// );
/// let mut edges = Relation::new(edge_type);
/// edges.insert(tuple! { source: 1i64, target: 2i64 }).unwrap();
///
/// // Create node features
/// let node_feature_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("node", ScalarType::Int)
///         .with_attribute("feature_id", ScalarType::Int)
///         .with_attribute("value", ScalarType::Float)
/// );
/// let mut node_features = Relation::new(node_feature_type);
/// node_features.insert(tuple! { node: 1i64, feature_id: 1i64, value: 0.5f64 }).unwrap();
/// node_features.insert(tuple! { node: 2i64, feature_id: 1i64, value: 0.2f64 }).unwrap();
///
/// // Create weights matrix (1 in feature, 1 out feature)
/// let weights_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("in_feature_id", ScalarType::Int)
///         .with_attribute("out_feature_id", ScalarType::Int)
///         .with_attribute("weight", ScalarType::Float)
/// );
/// let mut weights = Relation::new(weights_type);
/// weights.insert(tuple! { in_feature_id: 1i64, out_feature_id: 1i64, weight: 2.0f64 }).unwrap();
///
/// let result = mpnn_layer(&edges, &node_features, &weights).unwrap();
/// assert_eq!(result.cardinality(), 1); // only node 2 gets a message
/// ```
pub fn mpnn_layer(
    edges: &Relation,
    node_features: &Relation,
    weights: &Relation,
) -> Result<Relation, DatabaseError> {
    // 1. Message Passing: For each edge (source, target), get the features of the source.
    // Rename `node_features.node` to `source` to join with edges.
    let source_features = node_features.rename(&[("node", "source")]);
    let messages = edges
        .join(&source_features)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Rename `target` to `node` so we have messages per node
    let messages = messages.rename(&[("target", "node")]);

    // Project away the source to avoid grouping issues.
    let messages = messages.project(&["node", "feature_id", "value"]);

    // 2. Aggregation: Sum messages per (node, feature_id)
    let aggregated = messages
        .summarize(
            &["node", "feature_id"],
            &[Aggregation::sum_float("agg_value", "value")],
        )
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Rename `feature_id` to `in_feature_id` to join with weights.
    let agg_to_join = aggregated.rename(&[("feature_id", "in_feature_id")]);

    // 3. Update (Linear Transformation): Join with weights and multiply.
    let weighted = agg_to_join
        .join(weights)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
    let multiplied = weighted
        .extend("out_value", ScalarType::Float, |t: &Tuple| {
            let agg_value = t.get_typed::<f64>("agg_value").unwrap_or(0.0);
            let weight = t.get_typed::<f64>("weight").unwrap_or(0.0);
            ScalarValue::Float(agg_value * weight)
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Project and group by (node, out_feature_id) to compute final sum.
    let final_features = multiplied
        .summarize(
            &["node", "out_feature_id"],
            &[Aggregation::sum_float("value", "out_value")],
        )
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Rename `out_feature_id` back to `feature_id`.
    Ok(final_features.rename(&[("out_feature_id", "feature_id")]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_mpnn_layer() {
        let edge_type = RelationType::new(
            TupleType::new()
                .with_attribute("source", ScalarType::Int)
                .with_attribute("target", ScalarType::Int),
        );
        let mut edges = Relation::new(edge_type);
        edges.insert(tuple! { source: 1i64, target: 2i64 }).unwrap();
        edges.insert(tuple! { source: 1i64, target: 3i64 }).unwrap();

        let node_feature_type = RelationType::new(
            TupleType::new()
                .with_attribute("node", ScalarType::Int)
                .with_attribute("feature_id", ScalarType::Int)
                .with_attribute("value", ScalarType::Float),
        );
        let mut node_features = Relation::new(node_feature_type);
        // Node 1 has feature_id 1 = 0.5
        node_features
            .insert(tuple! { node: 1i64, feature_id: 1i64, value: 0.5f64 })
            .unwrap();
        // Node 2 has feature_id 1 = 0.2
        node_features
            .insert(tuple! { node: 2i64, feature_id: 1i64, value: 0.2f64 })
            .unwrap();
        // Node 3 has feature_id 1 = 0.3
        node_features
            .insert(tuple! { node: 3i64, feature_id: 1i64, value: 0.3f64 })
            .unwrap();

        let weights_type = RelationType::new(
            TupleType::new()
                .with_attribute("in_feature_id", ScalarType::Int)
                .with_attribute("out_feature_id", ScalarType::Int)
                .with_attribute("weight", ScalarType::Float),
        );
        let mut weights = Relation::new(weights_type);
        // Transform feature_id 1 into feature_id 2 with weight 2.0
        weights
            .insert(tuple! { in_feature_id: 1i64, out_feature_id: 2i64, weight: 2.0f64 })
            .unwrap();

        let result = mpnn_layer(&edges, &node_features, &weights).unwrap();

        // Node 1 sends its feature (0.5) to Node 2 and Node 3.
        // The weight transforms feature 1 into feature 2 and multiplies by 2.0.
        // Output for Node 2: 0.5 * 2.0 = 1.0 (feature 2)
        // Output for Node 3: 0.5 * 2.0 = 1.0 (feature 2)

        assert_eq!(result.cardinality(), 2);

        let node2_tuple = result
            .restrict(|t| t.get_typed::<i64>("node").unwrap() == 2)
            .tuples()
            .next()
            .unwrap()
            .clone();
        assert_eq!(node2_tuple.get_typed::<i64>("feature_id").unwrap(), 2);
        assert_eq!(node2_tuple.get_typed::<f64>("value").unwrap(), 1.0);

        let node3_tuple = result
            .restrict(|t| t.get_typed::<i64>("node").unwrap() == 3)
            .tuples()
            .next()
            .unwrap()
            .clone();
        assert_eq!(node3_tuple.get_typed::<i64>("feature_id").unwrap(), 2);
        assert_eq!(node3_tuple.get_typed::<f64>("value").unwrap(), 1.0);
    }
}
