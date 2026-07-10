//! Relational Decision Tree Evaluator
//!
//! This module demonstrates how to evaluate a Decision Tree classifier
//! over a dataset using purely relational algebra operations.
//!
//! # Concept
//!
//! - **Nodes**: Relation `(node_id: Int, feature: String, threshold: Float, left_child: Int, right_child: Int, is_leaf: Bool, class_label: String)`.
//! - **Instances**: Relation `(instance_id: Int, feature: String, value: Float)`.
//!
//! The evaluation process:
//! 1. All instances start at the root node.
//! 2. State relation tracks `(instance_id, node_id)`.
//! 3. Join State with Nodes to get the split conditions.
//! 4. Natural join with Instances to extract exactly the feature needed for the current node split! (This is an elegant use of Natural Join matching on both `instance_id` and `feature`).
//! 5. Extend to compute the next node (left or right branch).
//! 6. Loop until all instances reach a leaf node.

use relvar_core::{
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational Decision Tree Evaluator.
///
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::decision_tree::DecisionTree;
/// // Note: This is a placeholder example
/// ```
pub struct DecisionTree {
    /// The nodes of the decision tree.
    /// Schema: (node_id: Int, feature: String, threshold: Float, left_child: Int, right_child: Int, is_leaf: Bool, class_label: String)
    pub nodes: Relation,
}

impl DecisionTree {
    /// Creates a new Relational Decision Tree.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::decision_tree::DecisionTree;
    /// // Note: This is a placeholder example
    /// ```
    pub fn new(nodes: Relation) -> Self {
        Self { nodes }
    }

    /// Evaluates the decision tree on a set of instances.
    ///
    /// Returns a relation with schema: `(instance_id: Int, class_label: String)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::decision_tree::DecisionTree;
    /// // Note: This is a placeholder example
    /// ```
    pub fn predict(
        &self,
        instances: &Relation,
        root_node_id: i64,
    ) -> Result<Relation, DatabaseError> {
        // Initial state: all instances start at the root node.
        let mut current_state = instances
            .project(&["instance_id"])
            .extend("node_id", ScalarType::Int, move |_| {
                ScalarValue::Int(root_node_id)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Initialize finished relation (instance_id, class_label)
        let mut finished = instances
            .project(&["instance_id"])
            .extend("class_label", ScalarType::String, |_| {
                ScalarValue::String("".to_string())
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .restrict(|_| false);

        loop {
            if current_state.cardinality() == 0 {
                break;
            }

            // Join state with nodes to get node properties
            let state_nodes = current_state.join(&self.nodes)?;

            // Separate leaves and internal nodes
            let leaves = state_nodes.restrict(|t| t.get_typed::<bool>("is_leaf").unwrap_or(false));
            let internals =
                state_nodes.restrict(|t| !t.get_typed::<bool>("is_leaf").unwrap_or(false));

            // Move leaves to finished
            let newly_finished = leaves.project(&["instance_id", "class_label"]);
            finished = finished
                .union(&newly_finished)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            if internals.cardinality() == 0 {
                break;
            }

            // For internal nodes, we need to join with instances to get feature values.
            // Brilliant trick: Natural Join on (instance_id, feature) automatically fetches the right feature!
            let internals_instances = internals.join(instances)?;

            // Determine next node
            let next_nodes = internals_instances
                .extend("next_node", ScalarType::Int, |t| {
                    let value = t.get_typed::<f64>("value").unwrap();
                    let threshold = t.get_typed::<f64>("threshold").unwrap();
                    let left = t.get_typed::<i64>("left_child").unwrap();
                    let right = t.get_typed::<i64>("right_child").unwrap();

                    if value <= threshold {
                        ScalarValue::Int(left)
                    } else {
                        ScalarValue::Int(right)
                    }
                })
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // Update current state
            current_state = next_nodes
                .project(&["instance_id", "next_node"])
                .rename(&[("next_node", "node_id")]);
        }

        Ok(finished)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::{
        tuple,
        types::{RelationType, TupleType},
    };

    #[test]
    fn test_decision_tree_evaluator() {
        // 1. Setup Nodes
        let nodes_heading = TupleType::new()
            .with_attribute("node_id", ScalarType::Int)
            .with_attribute("feature", ScalarType::String)
            .with_attribute("threshold", ScalarType::Float)
            .with_attribute("left_child", ScalarType::Int)
            .with_attribute("right_child", ScalarType::Int)
            .with_attribute("is_leaf", ScalarType::Bool)
            .with_attribute("class_label", ScalarType::String);

        let mut nodes = Relation::new(RelationType::new(nodes_heading));

        // Tree:
        // Node 0: if "age" <= 30.0 go 1 else go 2
        // Node 1: leaf "young"
        // Node 2: if "income" <= 50000.0 go 3 else go 4
        // Node 3: leaf "mid_low"
        // Node 4: leaf "mid_high"

        nodes.insert(tuple! { node_id: 0i64, feature: "age", threshold: 30.0f64, left_child: 1i64, right_child: 2i64, is_leaf: false, class_label: "" }).unwrap();
        nodes.insert(tuple! { node_id: 1i64, feature: "", threshold: 0.0f64, left_child: -1i64, right_child: -1i64, is_leaf: true, class_label: "young" }).unwrap();
        nodes.insert(tuple! { node_id: 2i64, feature: "income", threshold: 50000.0f64, left_child: 3i64, right_child: 4i64, is_leaf: false, class_label: "" }).unwrap();
        nodes.insert(tuple! { node_id: 3i64, feature: "", threshold: 0.0f64, left_child: -1i64, right_child: -1i64, is_leaf: true, class_label: "mid_low" }).unwrap();
        nodes.insert(tuple! { node_id: 4i64, feature: "", threshold: 0.0f64, left_child: -1i64, right_child: -1i64, is_leaf: true, class_label: "mid_high" }).unwrap();

        let dt = DecisionTree::new(nodes);

        // 2. Setup Instances
        let inst_heading = TupleType::new()
            .with_attribute("instance_id", ScalarType::Int)
            .with_attribute("feature", ScalarType::String)
            .with_attribute("value", ScalarType::Float);

        let mut instances = Relation::new(RelationType::new(inst_heading));

        // Instance 1: age=25, income=100000 -> Should be "young"
        instances
            .insert(tuple! { instance_id: 1i64, feature: "age", value: 25.0f64 })
            .unwrap();
        instances
            .insert(tuple! { instance_id: 1i64, feature: "income", value: 100000.0f64 })
            .unwrap();

        // Instance 2: age=40, income=40000 -> Should be "mid_low"
        instances
            .insert(tuple! { instance_id: 2i64, feature: "age", value: 40.0f64 })
            .unwrap();
        instances
            .insert(tuple! { instance_id: 2i64, feature: "income", value: 40000.0f64 })
            .unwrap();

        // Instance 3: age=35, income=60000 -> Should be "mid_high"
        instances
            .insert(tuple! { instance_id: 3i64, feature: "age", value: 35.0f64 })
            .unwrap();
        instances
            .insert(tuple! { instance_id: 3i64, feature: "income", value: 60000.0f64 })
            .unwrap();

        // 3. Predict
        let results = dt.predict(&instances, 0).unwrap();

        assert_eq!(results.cardinality(), 3);

        let mut predictions = std::collections::HashMap::new();
        for t in results.tuples() {
            predictions.insert(
                t.get_typed::<i64>("instance_id").unwrap(),
                t.get_typed::<String>("class_label").unwrap(),
            );
        }

        assert_eq!(predictions.get(&1).unwrap(), "young");
        assert_eq!(predictions.get(&2).unwrap(), "mid_low");
        assert_eq!(predictions.get(&3).unwrap(), "mid_high");
    }
}
