//! Relational Neural Network (Perceptron)
//!
//! This module demonstrates how a simple neural network layer (Perceptron) can be implemented
//! using purely relational algebra operations. It represents weights, biases, and inputs as
//! relations, and computes the forward pass using Join, Extend, and Summarize.
//!
//! # Concept
//!
//! - **Inputs**: Relation `(input_id: Int, value: Float)`.
//! - **Weights**: Relation `(neuron_id: Int, input_id: Int, weight: Float)`.
//! - **Biases**: Relation `(neuron_id: Int, bias: Float)`.
//!
//! The forward pass process:
//! 1. Join `Inputs` and `Weights` on `input_id`.
//! 2. Extend to compute the product of `value` and `weight` for each connection.
//! 3. Summarize by grouping on `neuron_id` and summing the products to get the weighted sum.
//! 4. Join with `Biases` to add the bias term.
//! 5. Extend to apply an activation function (e.g., Step function) to produce the final output.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational Neural Network Layer (Perceptron).
pub struct NeuralNetLayer {
    /// Weights relation. Schema: `(neuron_id: Int, input_id: Int, weight: Float)`
    pub weights: Relation,
    /// Biases relation. Schema: `(neuron_id: Int, bias: Float)`
    pub biases: Relation,
}

impl NeuralNetLayer {
    /// Creates a new Neural Network Layer.
    pub fn new(weights: Relation, biases: Relation) -> Self {
        Self { weights, biases }
    }

    /// Computes the forward pass of the neural network layer.
    ///
    /// # Arguments
    ///
    /// * `inputs` - The input relation. Schema: `(input_id: Int, value: Float)`
    ///
    /// # Returns
    ///
    /// A relation with schema `(neuron_id: Int, output: Float)`.
    pub fn forward_pass(&self, inputs: &Relation) -> Result<Relation, DatabaseError> {
        // 1. Join inputs and weights
        let joined = inputs.join(&self.weights)?;

        // 2. Compute product: value * weight
        let products = joined
            .extend("product", ScalarType::Float, |t| {
                let val = t.get_typed::<f64>("value").unwrap_or(0.0);
                let w = t.get_typed::<f64>("weight").unwrap_or(0.0);
                // Memory constraint: use saturating/checked math conceptually, but for floats we just use standard operators
                // as floats don't have checked_mul in the same way integers do, and overflow results in infinity which is standard IEEE 754.
                ScalarValue::Float(val * w)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Summarize by neuron_id to get weighted sum
        let weighted_sums = products
            .summarize(
                &["neuron_id"],
                &[Aggregation::sum_float("weighted_sum", "product")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Join with biases
        let with_biases = weighted_sums.join(&self.biases)?;

        // 5. Apply activation function (Step function: if (sum + bias) >= 0.0 then 1.0 else 0.0)
        let activated = with_biases
            .extend("output", ScalarType::Float, |t| {
                let sum = t.get_typed::<f64>("weighted_sum").unwrap_or(0.0);
                let bias = t.get_typed::<f64>("bias").unwrap_or(0.0);
                let net_input = sum + bias;

                let output = if net_input >= 0.0 { 1.0 } else { 0.0 };
                ScalarValue::Float(output)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Project out intermediate columns
        let result = activated.project(&["neuron_id", "output"]);

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_and_gate_perceptron() {
        // We want to train a perceptron to act as an AND gate.
        // x1, x2 => out
        // 0, 0 => 0
        // 0, 1 => 0
        // 1, 0 => 0
        // 1, 1 => 1

        // Weights: w1 = 1.0, w2 = 1.0
        // Bias: -1.5
        // Net Input: x1*1.0 + x2*1.0 - 1.5
        // If x1=1, x2=1 => 2.0 - 1.5 = 0.5 >= 0 => 1
        // If x1=1, x2=0 => 1.0 - 1.5 = -0.5 < 0 => 0

        let weight_heading = TupleType::new()
            .with_attribute("neuron_id", ScalarType::Int)
            .with_attribute("input_id", ScalarType::Int)
            .with_attribute("weight", ScalarType::Float);
        let mut weights = Relation::new(RelationType::new(weight_heading));
        weights
            .insert(tuple! { neuron_id: 1i64, input_id: 1i64, weight: 1.0f64 })
            .unwrap();
        weights
            .insert(tuple! { neuron_id: 1i64, input_id: 2i64, weight: 1.0f64 })
            .unwrap();

        let bias_heading = TupleType::new()
            .with_attribute("neuron_id", ScalarType::Int)
            .with_attribute("bias", ScalarType::Float);
        let mut biases = Relation::new(RelationType::new(bias_heading));
        biases
            .insert(tuple! { neuron_id: 1i64, bias: -1.5f64 })
            .unwrap();

        let layer = NeuralNetLayer::new(weights, biases);

        let input_heading = TupleType::new()
            .with_attribute("input_id", ScalarType::Int)
            .with_attribute("value", ScalarType::Float);

        // Test cases
        let test_cases = vec![
            (0.0, 0.0, 0.0),
            (0.0, 1.0, 0.0),
            (1.0, 0.0, 0.0),
            (1.0, 1.0, 1.0),
        ];

        for (x1, x2, expected) in test_cases {
            let mut inputs = Relation::new(RelationType::new(input_heading.clone()));
            inputs.insert(tuple! { input_id: 1i64, value: x1 }).unwrap();
            inputs.insert(tuple! { input_id: 2i64, value: x2 }).unwrap();

            let output_rel = layer.forward_pass(&inputs).unwrap();

            assert_eq!(output_rel.cardinality(), 1);
            let out_tuple = output_rel.tuples().next().unwrap();

            assert_eq!(out_tuple.get_typed::<i64>("neuron_id").unwrap(), 1);
            assert_eq!(out_tuple.get_typed::<f64>("output").unwrap(), expected);
        }
    }

    #[test]
    fn test_or_gate_perceptron() {
        // OR gate:
        // Weights: w1 = 1.0, w2 = 1.0
        // Bias: -0.5
        // Net Input: x1*1.0 + x2*1.0 - 0.5

        let weight_heading = TupleType::new()
            .with_attribute("neuron_id", ScalarType::Int)
            .with_attribute("input_id", ScalarType::Int)
            .with_attribute("weight", ScalarType::Float);
        let mut weights = Relation::new(RelationType::new(weight_heading));
        weights
            .insert(tuple! { neuron_id: 1i64, input_id: 1i64, weight: 1.0f64 })
            .unwrap();
        weights
            .insert(tuple! { neuron_id: 1i64, input_id: 2i64, weight: 1.0f64 })
            .unwrap();

        let bias_heading = TupleType::new()
            .with_attribute("neuron_id", ScalarType::Int)
            .with_attribute("bias", ScalarType::Float);
        let mut biases = Relation::new(RelationType::new(bias_heading));
        biases
            .insert(tuple! { neuron_id: 1i64, bias: -0.5f64 })
            .unwrap();

        let layer = NeuralNetLayer::new(weights, biases);

        let input_heading = TupleType::new()
            .with_attribute("input_id", ScalarType::Int)
            .with_attribute("value", ScalarType::Float);

        // Test cases
        let test_cases = vec![
            (0.0, 0.0, 0.0),
            (0.0, 1.0, 1.0),
            (1.0, 0.0, 1.0),
            (1.0, 1.0, 1.0),
        ];

        for (x1, x2, expected) in test_cases {
            let mut inputs = Relation::new(RelationType::new(input_heading.clone()));
            inputs.insert(tuple! { input_id: 1i64, value: x1 }).unwrap();
            inputs.insert(tuple! { input_id: 2i64, value: x2 }).unwrap();

            let output_rel = layer.forward_pass(&inputs).unwrap();

            assert_eq!(output_rel.cardinality(), 1);
            let out_tuple = output_rel.tuples().next().unwrap();

            assert_eq!(out_tuple.get_typed::<f64>("output").unwrap(), expected);
        }
    }
}
