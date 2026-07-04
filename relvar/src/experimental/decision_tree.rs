//! Relational Decision Tree Inference
//!
//! Evaluates a decision tree model against a batch of inputs using pure relational algebra.
//!
//! # Concept
//! - **Nodes**: Schema `(node_id: Int, feature: String, threshold: Float, left_child: Int, right_child: Int, is_leaf: Bool, leaf_value: String)`
//! - **Inputs**: Schema `(input_id: Int, feature: String, value: Float)`
//! - **State**: We track `Active` inputs: `(input_id: Int, node_id: Int)` and `Finished`: `(input_id: Int, prediction: String)`.

use relvar_core::{
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational Decision Tree Evaluator.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::decision_tree::DecisionTree;
/// // Note: This is a placeholder example
/// ```
pub struct DecisionTree {
    /// The tree structure. Schema: (node_id: Int, feature: String, threshold: Float, left_child: Int, right_child: Int, is_leaf: Bool, leaf_value: String)
    pub nodes: Relation,
    /// The input batch. Schema: (input_id: Int, feature: String, value: Float)
    pub inputs: Relation,
}

impl DecisionTree {
    /// Creates a new DecisionTree evaluator.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::decision_tree::DecisionTree;
    /// // Note: This is a placeholder example
    /// ```
    pub fn new(nodes: Relation, inputs: Relation) -> Self {
        Self { nodes, inputs }
    }

    /// Evaluates the tree until all inputs have reached a leaf node.
    /// Returns a relation of predictions: (input_id: Int, prediction: String).
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::decision_tree::DecisionTree;
    /// // Note: This is a placeholder example
    /// ```
    pub fn predict(&self, initial_node_id: i64) -> Result<Relation, DatabaseError> {
        // 1. Initialize Active State: All distinct input_ids starting at root
        let active = self
            .inputs
            .project(&["input_id"])
            .extend("node_id", ScalarType::Int, move |_| {
                ScalarValue::Int(initial_node_id)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 2. Initialize Finished State: Empty relation with (input_id, prediction)
        let finished_heading = relvar_core::types::TupleType::new()
            .with_attribute("input_id".to_string(), ScalarType::Int)
            .with_attribute("prediction".to_string(), ScalarType::String);
        let mut finished = Relation::new(relvar_core::types::RelationType::new(finished_heading));

        let mut current_active = active;

        while current_active.cardinality() > 0 {
            // Join Active with Nodes to get node properties
            let joined = current_active.join(&self.nodes)?;

            // newly_finished: nodes where is_leaf == true
            let newly_finished = joined
                .restrict(|t| t.get_typed::<bool>("is_leaf").unwrap_or(false))
                .project(&["input_id", "leaf_value"])
                .rename(&[("leaf_value", "prediction")]);

            // Add to Finished
            finished = finished
                .union(&newly_finished)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // evaluating: nodes where is_leaf == false
            let evaluating = joined.restrict(|t| !t.get_typed::<bool>("is_leaf").unwrap_or(false));

            if evaluating.cardinality() == 0 {
                break;
            }

            // For evaluating, join with Inputs to get the feature value
            let eval_with_inputs = evaluating.join(&self.inputs)?;

            // Determine next node
            current_active = eval_with_inputs
                .extend("next_node", ScalarType::Int, |t| {
                    let val = t.get_typed::<f64>("value").unwrap();
                    let thresh = t.get_typed::<f64>("threshold").unwrap();
                    let left = t.get_typed::<i64>("left_child").unwrap();
                    let right = t.get_typed::<i64>("right_child").unwrap();
                    if val <= thresh {
                        ScalarValue::Int(left)
                    } else {
                        ScalarValue::Int(right)
                    }
                })
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
                .project(&["input_id", "next_node"])
                .rename(&[("next_node", "node_id")]);
        }

        Ok(finished)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_decision_tree_predict() {
        let node_heading = TupleType::new()
            .with_attribute("node_id", ScalarType::Int)
            .with_attribute("feature", ScalarType::String)
            .with_attribute("threshold", ScalarType::Float)
            .with_attribute("left_child", ScalarType::Int)
            .with_attribute("right_child", ScalarType::Int)
            .with_attribute("is_leaf", ScalarType::Bool)
            .with_attribute("leaf_value", ScalarType::String);
        let mut nodes = Relation::new(RelationType::new(node_heading));

        // Node 1: root. feature="age", thresh=30.0. left=2, right=3.
        nodes.insert(tuple! { node_id: 1i64, feature: "age", threshold: 30.0f64, left_child: 2i64, right_child: 3i64, is_leaf: false, leaf_value: "" }).unwrap();
        // Node 2: leaf "Youth"
        nodes.insert(tuple! { node_id: 2i64, feature: "", threshold: 0.0f64, left_child: 0i64, right_child: 0i64, is_leaf: true, leaf_value: "Youth" }).unwrap();
        // Node 3: feature="income", thresh=50000.0. left=4, right=5
        nodes.insert(tuple! { node_id: 3i64, feature: "income", threshold: 50000.0f64, left_child: 4i64, right_child: 5i64, is_leaf: false, leaf_value: "" }).unwrap();
        // Node 4: leaf "Low Income Adult"
        nodes.insert(tuple! { node_id: 4i64, feature: "", threshold: 0.0f64, left_child: 0i64, right_child: 0i64, is_leaf: true, leaf_value: "Low Income Adult" }).unwrap();
        // Node 5: leaf "High Income Adult"
        nodes.insert(tuple! { node_id: 5i64, feature: "", threshold: 0.0f64, left_child: 0i64, right_child: 0i64, is_leaf: true, leaf_value: "High Income Adult" }).unwrap();

        let input_heading = TupleType::new()
            .with_attribute("input_id", ScalarType::Int)
            .with_attribute("feature", ScalarType::String)
            .with_attribute("value", ScalarType::Float);
        let mut inputs = Relation::new(RelationType::new(input_heading));

        // Input 1: Age 25
        inputs
            .insert(tuple! { input_id: 1i64, feature: "age", value: 25.0f64 })
            .unwrap();
        // Input 2: Age 40, Income 40000.0
        inputs
            .insert(tuple! { input_id: 2i64, feature: "age", value: 40.0f64 })
            .unwrap();
        inputs
            .insert(tuple! { input_id: 2i64, feature: "income", value: 40000.0f64 })
            .unwrap();
        // Input 3: Age 35, Income 80000.0
        inputs
            .insert(tuple! { input_id: 3i64, feature: "age", value: 35.0f64 })
            .unwrap();
        inputs
            .insert(tuple! { input_id: 3i64, feature: "income", value: 80000.0f64 })
            .unwrap();

        let tree = DecisionTree::new(nodes, inputs);
        let predictions = tree.predict(1).unwrap();

        assert_eq!(predictions.cardinality(), 3);

        let mut results = std::collections::HashMap::new();
        for t in predictions.tuples() {
            results.insert(
                t.get_typed::<i64>("input_id").unwrap(),
                t.get_typed::<String>("prediction").unwrap(),
            );
        }

        assert_eq!(results.get(&1).unwrap(), "Youth");
        assert_eq!(results.get(&2).unwrap(), "Low Income Adult");
        assert_eq!(results.get(&3).unwrap(), "High Income Adult");
    }
}
