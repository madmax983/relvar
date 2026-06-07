//! Relational Linear Regression
//!
//! This module demonstrates how to train a Linear Regression model using
//! Gradient Descent implemented entirely in pure relational algebra.
//!
//! # Concept
//!
//! Machine learning algorithms fundamentally rely on linear algebra and aggregation.
//! By modeling our dataset as an Entity-Attribute-Value (EAV) relation, we can express
//! gradient descent as a series of joins, extensions, and aggregations, completely
//! eliminating the need for loops over data outside the relational engine.
//!
//! - **Features**: Relation `(instance_id: Int, feature: String, value: Float)`
//! - **Targets**: Relation `(instance_id: Int, target: Float)`
//! - **Weights**: Relation `(feature: String, weight: Float)`
//!
//! The training loop consists of:
//! 1. **Forward Pass**: Join `Features` and `Weights`, multiply, and summarize to get predictions.
//! 2. **Loss Computation**: Join predictions with `Targets` and compute errors.
//! 3. **Backward Pass**: Join errors with `Features`, multiply, and summarize to get gradients.
//! 4. **Update**: Update `Weights` using the gradients and a learning rate.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational Linear Regression trainer using Gradient Descent.
///
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType, tuple};
/// use relvar::experimental::linear_regression::LinearRegression;
/// // See tests for complete usage.
/// ```
pub struct LinearRegression {
    /// Features relation. Schema: (instance_id: Int, feature: String, value: Float)
    pub features: Relation,
    /// Targets relation. Schema: (instance_id: Int, target: Float)
    pub targets: Relation,
}

impl LinearRegression {
    /// Creates a new LinearRegression model with the given features and targets.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::linear_regression::LinearRegression;
    /// // Placeholder
    /// ```
    pub fn new(features: Relation, targets: Relation) -> Self {
        Self { features, targets }
    }

    /// Initializes a weights relation with 0.0 for all distinct features present in the features relation.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::linear_regression::LinearRegression;
    /// // Placeholder
    /// ```
    pub fn initialize_weights(&self) -> Result<Relation, DatabaseError> {
        self.features
            .project(&["feature"])
            .extend("weight", ScalarType::Float, |_| ScalarValue::Float(0.0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    /// Trains the model using gradient descent for a given number of epochs.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::linear_regression::LinearRegression;
    /// // Placeholder
    /// ```
    pub fn train(
        &self,
        initial_weights: Relation,
        learning_rate: f64,
        epochs: usize,
    ) -> Result<Relation, DatabaseError> {
        let mut weights = initial_weights;
        let num_instances = self.targets.cardinality() as f64;

        if num_instances == 0.0 {
            return Ok(weights);
        }

        for _ in 0..epochs {
            // 1. Forward Pass: Compute predictions
            let joined = self.features.join(&weights)?;

            let contributions = joined
                .extend("contribution", ScalarType::Float, |t| {
                    let v = t.get_typed::<f64>("value").unwrap();
                    let w = t.get_typed::<f64>("weight").unwrap();
                    ScalarValue::Float(v * w)
                })
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            let predictions = contributions
                .summarize(
                    &["instance_id"],
                    &[Aggregation::sum_float("prediction", "contribution")],
                )
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // 2. Compute Errors
            let with_targets = predictions.join(&self.targets)?;

            let errors = with_targets
                .extend("error", ScalarType::Float, |t| {
                    let p = t.get_typed::<f64>("prediction").unwrap();
                    let tg = t.get_typed::<f64>("target").unwrap();
                    ScalarValue::Float(p - tg) // Error = (y_hat - y)
                })
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
                .project(&["instance_id", "error"]);

            // 3. Backward Pass: Compute Gradients
            let error_features = errors.join(&self.features)?;

            let grad_contributions = error_features
                .extend("grad_contrib", ScalarType::Float, |t| {
                    let err = t.get_typed::<f64>("error").unwrap();
                    let val = t.get_typed::<f64>("value").unwrap();
                    ScalarValue::Float(err * val)
                })
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            let gradients = grad_contributions
                .summarize(
                    &["feature"],
                    &[Aggregation::sum_float("gradient", "grad_contrib")],
                )
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // 4. Update Weights
            // Since we initialized weights over all features, inner join works perfectly.
            let weights_with_grads = weights.join(&gradients)?;

            weights = weights_with_grads
                .extend("new_weight", ScalarType::Float, move |t| {
                    let w = t.get_typed::<f64>("weight").unwrap();
                    let g = t.get_typed::<f64>("gradient").unwrap();
                    ScalarValue::Float(w - learning_rate * (g / num_instances))
                })
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
                .project(&["feature", "new_weight"])
                .rename(&[("new_weight", "weight")]);
        }

        Ok(weights)
    }

    /// Predicts target values for a set of features given a set of weights.
    /// Returns a relation with schema: (instance_id: Int, prediction: Float).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::linear_regression::LinearRegression;
    /// // Placeholder
    /// ```
    pub fn predict(
        &self,
        weights: &Relation,
        features: &Relation,
    ) -> Result<Relation, DatabaseError> {
        let joined = features.join(weights)?;

        let contributions = joined
            .extend("contribution", ScalarType::Float, |t| {
                let v = t.get_typed::<f64>("value").unwrap();
                let w = t.get_typed::<f64>("weight").unwrap();
                ScalarValue::Float(v * w)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        contributions
            .summarize(
                &["instance_id"],
                &[Aggregation::sum_float("prediction", "contribution")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_linear_regression() {
        // Setup Feature Data: (instance_id, feature, value)
        let features_type = TupleType::new()
            .with_attribute("instance_id", ScalarType::Int)
            .with_attribute("feature", ScalarType::String)
            .with_attribute("value", ScalarType::Float);

        let mut features = Relation::new(RelationType::new(features_type));

        // Equation to learn: y = 2.0 * x1 + 3.0 * x2

        // Instance 1: x1=1.0, x2=2.0 => y = 8.0
        features
            .insert(tuple! { instance_id: 1i64, feature: "x1", value: 1.0f64 })
            .unwrap();
        features
            .insert(tuple! { instance_id: 1i64, feature: "x2", value: 2.0f64 })
            .unwrap();

        // Instance 2: x1=2.0, x2=1.0 => y = 7.0
        features
            .insert(tuple! { instance_id: 2i64, feature: "x1", value: 2.0f64 })
            .unwrap();
        features
            .insert(tuple! { instance_id: 2i64, feature: "x2", value: 1.0f64 })
            .unwrap();

        // Setup Target Data: (instance_id, target)
        let targets_type = TupleType::new()
            .with_attribute("instance_id", ScalarType::Int)
            .with_attribute("target", ScalarType::Float);

        let mut targets = Relation::new(RelationType::new(targets_type));
        targets
            .insert(tuple! { instance_id: 1i64, target: 8.0f64 })
            .unwrap();
        targets
            .insert(tuple! { instance_id: 2i64, target: 7.0f64 })
            .unwrap();

        let model = LinearRegression::new(features.clone(), targets);
        let initial_weights = model.initialize_weights().unwrap();

        // Train
        let final_weights = model.train(initial_weights, 0.1, 100).unwrap();

        // Check if weights are close to expected [2.0, 3.0]
        for t in final_weights.tuples() {
            let feature = t.get_typed::<String>("feature").unwrap();
            let weight = t.get_typed::<f64>("weight").unwrap();
            if feature == "x1" {
                assert!((weight - 2.0).abs() < 0.1, "x1 weight is {}", weight);
            } else if feature == "x2" {
                assert!((weight - 3.0).abs() < 0.1, "x2 weight is {}", weight);
            }
        }

        // Predict
        let predictions = model.predict(&final_weights, &features).unwrap();
        assert_eq!(predictions.cardinality(), 2);

        for t in predictions.tuples() {
            let id = t.get_typed::<i64>("instance_id").unwrap();
            let p = t.get_typed::<f64>("prediction").unwrap();
            if id == 1 {
                assert!((p - 8.0).abs() < 0.5, "Pred 1 is {}", p);
            } else if id == 2 {
                assert!((p - 7.0).abs() < 0.5, "Pred 2 is {}", p);
            }
        }
    }
}
