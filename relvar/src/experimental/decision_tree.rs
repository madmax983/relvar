use relvar_core::DatabaseError;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue, Tuple};

/// A relational decision tree inference engine.
pub struct DecisionTree {
    nodes: Relation,
    leaves: Relation,
}

impl DecisionTree {
    /// Creates a new DecisionTree inference engine.
    ///
    /// `nodes` must have: node_id (Int), feature_id (String), threshold (Float), left_child (Int), right_child (Int)
    /// `leaves` must have: node_id (Int), class_label (String)
    pub fn new(nodes: Relation, leaves: Relation) -> Self {
        Self { nodes, leaves }
    }

    /// Evaluates the decision tree on a set of instances.
    ///
    /// `instances` must have: instance_id (Int), feature_id (String), value (Float)
    ///
    /// Returns a relation of: instance_id (Int), class_label (String)
    pub fn predict(&self, instances: &Relation) -> Result<Relation, DatabaseError> {
        // Initialize state: all instances start at root node (node_id = 0)
        // First, get unique instances
        let unique_instances = instances.project(&["instance_id"]);
        let state = unique_instances
            .extend("current_node_id", ScalarType::Int, |_| ScalarValue::Int(0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let mut active_state = state;
        let mut completed_predictions = Relation::new(RelationType::new(
            TupleType::new()
                .with_attribute("instance_id", ScalarType::Int)
                .with_attribute("class_label", ScalarType::String),
        ));

        while active_state.cardinality() > 0 {
            // Find which instances have reached a leaf
            // leaves: node_id, class_label
            // active_state: instance_id, current_node_id
            let leaves_renamed = self.leaves.rename(&[("node_id", "current_node_id")]);

            let reached_leaves = active_state.join(&leaves_renamed)?;
            if reached_leaves.cardinality() > 0 {
                let predictions = reached_leaves.project(&["instance_id", "class_label"]);
                completed_predictions = completed_predictions
                    .union(&predictions)
                    .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
            }

            // Keep only instances that are at internal nodes
            let internal_state = active_state
                .difference(&reached_leaves.project(&["instance_id", "current_node_id"]))
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            if internal_state.cardinality() == 0 {
                break;
            }

            // Join internal state with nodes to get the routing conditions
            // internal_state: instance_id, current_node_id
            // nodes: node_id, feature_id, threshold, left_child, right_child
            let nodes_renamed = self.nodes.rename(&[("node_id", "current_node_id")]);
            let state_with_nodes = internal_state.join(&nodes_renamed)?;

            // Join with instances to get the feature value for the condition
            // state_with_nodes: instance_id, current_node_id, feature_id, threshold, left_child, right_child
            // instances: instance_id, feature_id, value
            let state_with_values = state_with_nodes.join(instances)?;

            // Evaluate condition: goes_left = value <= threshold
            let evaluated = state_with_values
                .extend("goes_left", ScalarType::Bool, |t: &Tuple| {
                    let value = t.get_typed::<f64>("value").unwrap();
                    let threshold = t.get_typed::<f64>("threshold").unwrap();
                    ScalarValue::Bool(value <= threshold)
                })
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // Route to left child
            let left_moves: Relation = evaluated
                .restrict(|t: &Tuple| t.get_typed::<bool>("goes_left").unwrap())
                .project(&["instance_id", "left_child"])
                .rename(&[("left_child", "current_node_id")]);

            // Route to right child
            let right_moves: Relation = evaluated
                .restrict(|t: &Tuple| !t.get_typed::<bool>("goes_left").unwrap())
                .project(&["instance_id", "right_child"])
                .rename(&[("right_child", "current_node_id")]);

            // Update active state for next iteration
            active_state = left_moves
                .union(&right_moves)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        }

        Ok(completed_predictions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;

    #[test]
    fn test_decision_tree_inference() {
        let nodes_type = RelationType::new(
            TupleType::new()
                .with_attribute("node_id", ScalarType::Int)
                .with_attribute("feature_id", ScalarType::String)
                .with_attribute("threshold", ScalarType::Float)
                .with_attribute("left_child", ScalarType::Int)
                .with_attribute("right_child", ScalarType::Int),
        );
        let mut nodes = Relation::new(nodes_type);
        // Root node 0: if "age" <= 30.0 go to 1 else go to 2
        nodes.insert(tuple! { node_id: 0i64, feature_id: "age", threshold: 30.0f64, left_child: 1i64, right_child: 2i64 }).unwrap();
        // Node 2: if "income" <= 50000.0 go to 3 else go to 4
        nodes.insert(tuple! { node_id: 2i64, feature_id: "income", threshold: 50000.0f64, left_child: 3i64, right_child: 4i64 }).unwrap();

        let leaves_type = RelationType::new(
            TupleType::new()
                .with_attribute("node_id", ScalarType::Int)
                .with_attribute("class_label", ScalarType::String),
        );
        let mut leaves = Relation::new(leaves_type);
        leaves
            .insert(tuple! { node_id: 1i64, class_label: "Youth" })
            .unwrap();
        leaves
            .insert(tuple! { node_id: 3i64, class_label: "MiddleAged_LowIncome" })
            .unwrap();
        leaves
            .insert(tuple! { node_id: 4i64, class_label: "MiddleAged_HighIncome" })
            .unwrap();

        let instances_type = RelationType::new(
            TupleType::new()
                .with_attribute("instance_id", ScalarType::Int)
                .with_attribute("feature_id", ScalarType::String)
                .with_attribute("value", ScalarType::Float),
        );
        let mut instances = Relation::new(instances_type);
        // Instance 100: age 25, income 40000 -> Should be "Youth" (Left at root)
        instances
            .insert(tuple! { instance_id: 100i64, feature_id: "age", value: 25.0f64 })
            .unwrap();
        instances
            .insert(tuple! { instance_id: 100i64, feature_id: "income", value: 40000.0f64 })
            .unwrap();

        // Instance 101: age 35, income 45000 -> Should be "MiddleAged_LowIncome" (Right at root, Left at node 2)
        instances
            .insert(tuple! { instance_id: 101i64, feature_id: "age", value: 35.0f64 })
            .unwrap();
        instances
            .insert(tuple! { instance_id: 101i64, feature_id: "income", value: 45000.0f64 })
            .unwrap();

        // Instance 102: age 40, income 60000 -> Should be "MiddleAged_HighIncome" (Right at root, Right at node 2)
        instances
            .insert(tuple! { instance_id: 102i64, feature_id: "age", value: 40.0f64 })
            .unwrap();
        instances
            .insert(tuple! { instance_id: 102i64, feature_id: "income", value: 60000.0f64 })
            .unwrap();

        let tree = DecisionTree::new(nodes, leaves);
        let predictions = tree.predict(&instances).unwrap();

        assert_eq!(predictions.cardinality(), 3);

        // Check results
        let youth =
            predictions.restrict(|t: &Tuple| t.get_typed::<i64>("instance_id").unwrap() == 100);
        assert_eq!(
            youth
                .tuples()
                .next()
                .unwrap()
                .get_typed::<String>("class_label")
                .unwrap(),
            "Youth"
        );

        let low =
            predictions.restrict(|t: &Tuple| t.get_typed::<i64>("instance_id").unwrap() == 101);
        assert_eq!(
            low.tuples()
                .next()
                .unwrap()
                .get_typed::<String>("class_label")
                .unwrap(),
            "MiddleAged_LowIncome"
        );

        let high =
            predictions.restrict(|t: &Tuple| t.get_typed::<i64>("instance_id").unwrap() == 102);
        assert_eq!(
            high.tuples()
                .next()
                .unwrap()
                .get_typed::<String>("class_label")
                .unwrap(),
            "MiddleAged_HighIncome"
        );
    }
}
