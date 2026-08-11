//! Relational Neural Network
//!
//! This module demonstrates how a simple feedforward neural network can be modeled
//! and executed using purely relational algebra operations.
//!
//! # Concept
//!
//! - **Activations**: Relation `(layer: Int, node: Int, val: Float)`.
//! - **Weights**: Relation `(layer: Int, from_node: Int, to_node: Int, weight: Float)`.
//! - **Biases**: Relation `(layer: Int, node: Int, bias: Float)`.
//!
//! We compute the next layer's activations via relational operations:
//! 1. Join `activations` with `weights` on `(layer, node = from_node)`.
//! 2. Extend to multiply `val` by `weight`.
//! 3. Summarize by `(layer, to_node)` to sum the products.
//! 4. Join with `biases`.
//! 5. Extend to add `bias` and apply an activation function (e.g., ReLU).
//! 6. Restrict to project the final output as the new `activations` state.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A simple Relational Feedforward Neural Network.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// // Note: This is a placeholder example
/// ```
pub struct NeuralNetwork {
    /// The current state of node activations.
    /// Schema: (layer: Int, node: Int, val: Float)
    pub activations: Relation,
    /// The weights between layers.
    /// Schema: (layer: Int, from_node: Int, to_node: Int, weight: Float)
    pub weights: Relation,
    /// The biases for nodes.
    /// Schema: (layer: Int, node: Int, bias: Float)
    pub biases: Relation,
}

impl NeuralNetwork {
    /// Creates a new Relational Neural Network.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// // Note: This is a placeholder example
    /// ```
    pub fn new(activations: Relation, weights: Relation, biases: Relation) -> Self {
        Self {
            activations,
            weights,
            biases,
        }
    }

    /// Computes the forward pass for one layer.
    ///
    /// # Arguments
    ///
    /// * `current_layer_idx` - The index of the layer to propagate forward from.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// // Note: This is a placeholder example
    /// ```
    pub fn forward_layer(&mut self, current_layer_idx: i64) -> Result<(), DatabaseError> {
        // 1. Restrict to current layer activations
        let current_activations = self
            .activations
            .restrict(move |t| t.get_typed::<i64>("layer").unwrap_or(-1) == current_layer_idx);

        // 2. Restrict weights to current layer connections
        let current_weights = self
            .weights
            .restrict(move |t| t.get_typed::<i64>("layer").unwrap_or(-1) == current_layer_idx);

        // 3. Join activations with weights on (node = from_node)
        // Rename `node` in activations to `from_node` for natural join
        let acts_renamed = current_activations.rename(&[("node", "from_node")]);
        let joined = acts_renamed.join(&current_weights)?;

        // 4. Extend to multiply `val` * `weight`
        let with_products = joined
            .extend("product", ScalarType::Float, |t| {
                let val = t.get_typed::<f64>("val").unwrap_or(0.0);
                let weight = t.get_typed::<f64>("weight").unwrap_or(0.0);
                ScalarValue::Float(val * weight)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Summarize by `to_node` to get sum of products (pre-activation)
        // Also keep `layer` so we can naturally join with biases later. The next layer is `layer + 1`.
        // However, `current_weights` has `layer`, so grouping by `to_node` is fine, we just need the next layer idx.
        let next_layer_idx = current_layer_idx.saturating_add(1);

        let sum_products = with_products
            .summarize(
                &["to_node"],
                &[Aggregation::sum_float("sum_prod", "product")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .rename(&[("to_node", "node")])
            .extend("layer", ScalarType::Int, move |_| {
                ScalarValue::Int(next_layer_idx)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 6. Join with biases
        let joined_biases = sum_products.join(&self.biases)?;

        // 7. Extend to add bias and apply ReLU
        let new_activations = joined_biases
            .extend("val", ScalarType::Float, compute_relu_activation)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["layer", "node", "val"]);

        // 8. Union the new activations into the network state
        self.activations = self
            .activations
            .union(&new_activations)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(())
    }

    /// Computes the full forward pass through all layers.
    ///
    /// # Arguments
    ///
    /// * `num_layers` - The total number of layers to process.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// // Note: This is a placeholder example
    /// ```
    pub fn forward_pass(&mut self, num_layers: i64) -> Result<(), DatabaseError> {
        for i in 0..num_layers {
            self.forward_layer(i)?;
        }
        Ok(())
    }

    /// Retrieves the activations for a specific layer.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// // Note: This is a placeholder example
    /// ```
    pub fn get_layer_activations(&self, layer_idx: i64) -> Relation {
        self.activations
            .restrict(move |t| t.get_typed::<i64>("layer").unwrap_or(-1) == layer_idx)
    }
}

fn compute_relu_activation(t: &relvar_core::values::Tuple) -> ScalarValue {
    let sum_prod = t.get_typed::<f64>("sum_prod").unwrap_or(0.0);
    let bias = t.get_typed::<f64>("bias").unwrap_or(0.0);
    let x = sum_prod + bias;
    // ReLU activation
    let relu = if x > 0.0 { x } else { 0.0 };
    ScalarValue::Float(relu)
}
#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::{
        tuple,
        types::{RelationType, TupleType},
    };

    #[test]
    fn test_feedforward_network() {
        // 1. Setup Data Headings
        let acts_heading = TupleType::new()
            .with_attribute("layer", ScalarType::Int)
            .with_attribute("node", ScalarType::Int)
            .with_attribute("val", ScalarType::Float);

        let weights_heading = TupleType::new()
            .with_attribute("layer", ScalarType::Int)
            .with_attribute("from_node", ScalarType::Int)
            .with_attribute("to_node", ScalarType::Int)
            .with_attribute("weight", ScalarType::Float);

        let biases_heading = TupleType::new()
            .with_attribute("layer", ScalarType::Int)
            .with_attribute("node", ScalarType::Int)
            .with_attribute("bias", ScalarType::Float);

        let mut acts = Relation::new(RelationType::new(acts_heading));
        let mut weights = Relation::new(RelationType::new(weights_heading));
        let mut biases = Relation::new(RelationType::new(biases_heading));

        // 2. Initial State (Layer 0)
        // Two inputs: node 0 = 0.5, node 1 = 1.0
        acts.insert(tuple! { layer: 0i64, node: 0i64, val: 0.5f64 })
            .unwrap();
        acts.insert(tuple! { layer: 0i64, node: 1i64, val: 1.0f64 })
            .unwrap();

        // 3. Weights (Layer 0 -> Layer 1)
        // L0 N0 -> L1 N0: 0.2
        // L0 N1 -> L1 N0: 0.4
        // L0 N0 -> L1 N1: 0.6
        // L0 N1 -> L1 N1: 0.8
        weights
            .insert(tuple! { layer: 0i64, from_node: 0i64, to_node: 0i64, weight: 0.2f64 })
            .unwrap();
        weights
            .insert(tuple! { layer: 0i64, from_node: 1i64, to_node: 0i64, weight: 0.4f64 })
            .unwrap();
        weights
            .insert(tuple! { layer: 0i64, from_node: 0i64, to_node: 1i64, weight: 0.6f64 })
            .unwrap();
        weights
            .insert(tuple! { layer: 0i64, from_node: 1i64, to_node: 1i64, weight: 0.8f64 })
            .unwrap();

        // 4. Biases (for Layer 1)
        // L1 N0: 0.1
        // L1 N1: -0.5
        biases
            .insert(tuple! { layer: 1i64, node: 0i64, bias: 0.1f64 })
            .unwrap();
        biases
            .insert(tuple! { layer: 1i64, node: 1i64, bias: -0.5f64 })
            .unwrap();

        // 5. Initialize Network and Forward Pass
        let mut nn = NeuralNetwork::new(acts, weights, biases);
        nn.forward_layer(0).unwrap();

        // 6. Verify Results
        let l1_acts = nn.get_layer_activations(1);
        assert_eq!(l1_acts.cardinality(), 2);

        // Expected L1 N0 = ReLU((0.5 * 0.2) + (1.0 * 0.4) + 0.1) = ReLU(0.1 + 0.4 + 0.1) = 0.6
        // Expected L1 N1 = ReLU((0.5 * 0.6) + (1.0 * 0.8) - 0.5) = ReLU(0.3 + 0.8 - 0.5) = ReLU(0.6) = 0.6

        let mut acts_vec: Vec<_> = l1_acts.tuples().collect();
        acts_vec.sort_by_key(|t| t.get_typed::<i64>("node").unwrap());

        let t0 = acts_vec[0];
        assert_eq!(t0.get_typed::<i64>("node").unwrap(), 0);
        assert!((t0.get_typed::<f64>("val").unwrap() - 0.6).abs() < 1e-6);

        let t1 = acts_vec[1];
        assert_eq!(t1.get_typed::<i64>("node").unwrap(), 1);
        assert!((t1.get_typed::<f64>("val").unwrap() - 0.6).abs() < 1e-6);
    }
}
