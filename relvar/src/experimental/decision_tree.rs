//! Relational Decision Tree Inference
//!
//! This module demonstrates how to perform Decision Tree inference (prediction)
//! using pure relational algebra.
//!
//! # Concept
//! - **Features**: `(row_id: Int, feature: String, value: Float)`
//! - **Tree Nodes**: `(node_id: Int, is_leaf: Bool, feature: String, threshold: Float, left_id: Int, right_id: Int, leaf_value: String)`
//! - **State**: `(row_id: Int, node_id: Int)` tracking each row's progression down the tree.

use relvar_core::{
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational Decision Tree.
pub struct DecisionTree {
    /// The nodes of the decision tree.
    pub tree_nodes: Relation,
}

impl DecisionTree {
    /// Creates a new DecisionTree.
    pub fn new(tree_nodes: Relation) -> Self {
        Self { tree_nodes }
    }

    /// Predicts the class for a set of instances.
    /// `features` should have schema: (row_id: Int, feature: String, value: Float)
    /// Returns a relation with schema: (row_id: Int, prediction: String)
    pub fn predict(&self, features: &Relation) -> Result<Relation, DatabaseError> {
        // 1. Initialize state: every row_id starts at node 0.
        // First, get unique row_ids from features.
        let row_ids = features.project(&["row_id"]);
        let mut state = row_ids
            .extend("node_id", ScalarType::Int, |_| ScalarValue::Int(0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Accumulator for finished predictions
        // Schema: (row_id, prediction)
        let heading = relvar_core::types::TupleType::new()
            .with_attribute("row_id".to_string(), ScalarType::Int)
            .with_attribute("prediction".to_string(), ScalarType::String);
        let mut predictions = Relation::new(relvar_core::types::RelationType::new(heading));

        // Iterate until all rows reach a leaf
        while state.cardinality() > 0 {
            // Join state with tree_nodes to get node info
            let state_nodes = state.join(&self.tree_nodes)?;

            // Process leaves: is_leaf == true
            let leaves = state_nodes.restrict(|t| t.get_typed::<bool>("is_leaf").unwrap_or(false));
            if leaves.cardinality() > 0 {
                let finished = leaves
                    .project(&["row_id", "leaf_value"])
                    .rename(&[("leaf_value", "prediction")]);
                predictions = predictions
                    .union(&finished)
                    .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
            }

            // Process internal nodes: is_leaf == false
            let internal =
                state_nodes.restrict(|t| !t.get_typed::<bool>("is_leaf").unwrap_or(false));

            if internal.cardinality() == 0 {
                break;
            }

            // Join with features to get the value for the splitting feature
            // internal schema: (row_id, node_id, is_leaf, feature, threshold, left_id, right_id, leaf_value)
            // features schema: (row_id, feature, value)
            let joined_features = internal.join(features)?;

            // Evaluate split condition
            state = joined_features
                .extend("next_node_id", ScalarType::Int, |t| {
                    let val = t.get_typed::<f64>("value").unwrap_or(0.0);
                    let threshold = t.get_typed::<f64>("threshold").unwrap_or(0.0);
                    if val <= threshold {
                        ScalarValue::Int(t.get_typed::<i64>("left_id").unwrap())
                    } else {
                        ScalarValue::Int(t.get_typed::<i64>("right_id").unwrap())
                    }
                })
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
                .project(&["row_id", "next_node_id"])
                .rename(&[("next_node_id", "node_id")]);
        }

        Ok(predictions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::{
        tuple,
        types::{RelationType, ScalarType, TupleType},
    };

    #[test]
    fn test_decision_tree_predict() {
        let node_heading = TupleType::new()
            .with_attribute("node_id".to_string(), ScalarType::Int)
            .with_attribute("is_leaf".to_string(), ScalarType::Bool)
            .with_attribute("feature".to_string(), ScalarType::String)
            .with_attribute("threshold".to_string(), ScalarType::Float)
            .with_attribute("left_id".to_string(), ScalarType::Int)
            .with_attribute("right_id".to_string(), ScalarType::Int)
            .with_attribute("leaf_value".to_string(), ScalarType::String);
        let mut tree_nodes = Relation::new(RelationType::new(node_heading));

        // Node 0: if age <= 30 go left(1) else right(2)
        tree_nodes.insert(tuple! { node_id: 0i64, is_leaf: false, feature: "age", threshold: 30.0f64, left_id: 1i64, right_id: 2i64, leaf_value: "" }).unwrap();

        // Node 1: if income <= 50000 go left(3) else right(4)
        tree_nodes.insert(tuple! { node_id: 1i64, is_leaf: false, feature: "income", threshold: 50000.0f64, left_id: 3i64, right_id: 4i64, leaf_value: "" }).unwrap();

        // Node 2: Leaf "Yes"
        tree_nodes.insert(tuple! { node_id: 2i64, is_leaf: true, feature: "", threshold: 0.0f64, left_id: -1i64, right_id: -1i64, leaf_value: "Yes" }).unwrap();

        // Node 3: Leaf "No"
        tree_nodes.insert(tuple! { node_id: 3i64, is_leaf: true, feature: "", threshold: 0.0f64, left_id: -1i64, right_id: -1i64, leaf_value: "No" }).unwrap();

        // Node 4: Leaf "Yes"
        tree_nodes.insert(tuple! { node_id: 4i64, is_leaf: true, feature: "", threshold: 0.0f64, left_id: -1i64, right_id: -1i64, leaf_value: "Yes" }).unwrap();

        let dt = DecisionTree::new(tree_nodes);

        let feat_heading = TupleType::new()
            .with_attribute("row_id".to_string(), ScalarType::Int)
            .with_attribute("feature".to_string(), ScalarType::String)
            .with_attribute("value".to_string(), ScalarType::Float);
        let mut features = Relation::new(RelationType::new(feat_heading));

        // Row 1: age=25, income=40000 -> Node 0 -> Node 1 -> Node 3 ("No")
        features
            .insert(tuple! { row_id: 1i64, feature: "age", value: 25.0f64 })
            .unwrap();
        features
            .insert(tuple! { row_id: 1i64, feature: "income", value: 40000.0f64 })
            .unwrap();

        // Row 2: age=25, income=60000 -> Node 0 -> Node 1 -> Node 4 ("Yes")
        features
            .insert(tuple! { row_id: 2i64, feature: "age", value: 25.0f64 })
            .unwrap();
        features
            .insert(tuple! { row_id: 2i64, feature: "income", value: 60000.0f64 })
            .unwrap();

        // Row 3: age=45, income=20000 -> Node 0 -> Node 2 ("Yes")
        features
            .insert(tuple! { row_id: 3i64, feature: "age", value: 45.0f64 })
            .unwrap();
        features
            .insert(tuple! { row_id: 3i64, feature: "income", value: 20000.0f64 })
            .unwrap();

        let predictions = dt.predict(&features).unwrap();

        assert_eq!(predictions.cardinality(), 3);
        let p1 = predictions
            .tuples()
            .find(|t| t.get_typed::<i64>("row_id") == Some(1))
            .unwrap();
        assert_eq!(p1.get_typed::<String>("prediction").unwrap(), "No");

        let p2 = predictions
            .tuples()
            .find(|t| t.get_typed::<i64>("row_id") == Some(2))
            .unwrap();
        assert_eq!(p2.get_typed::<String>("prediction").unwrap(), "Yes");

        let p3 = predictions
            .tuples()
            .find(|t| t.get_typed::<i64>("row_id") == Some(3))
            .unwrap();
        assert_eq!(p3.get_typed::<String>("prediction").unwrap(), "Yes");
    }
}
