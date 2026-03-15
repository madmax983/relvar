//! Relational AutoML (Naive Bayes)
//!
//! This module implements a Naive Bayes classifier using purely relational algebra
//! primitives (Summarize, Count) for training.
//!
//! # Concept
//!
//! Naive Bayes is based on counting frequencies:
//! - **P(Class)**: Frequency of each class in the dataset.
//! - **P(Feature | Class)**: Frequency of a feature value given the class.
//!
//! We use the `summarize` operator to compute these counts efficiently in the database engine.
//!
//! # Example
//!
//! ```
//! use relvar::{Database, InMemoryEngine, tuple};
//! use relvar::{RelationType, ScalarType, TupleType};
//! use relvar::{ScalarValue, Relation};
//! use relvar::experimental::automl::NaiveBayesClassifier;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // 1. Setup Database
//! let mut db = Database::new(InMemoryEngine::new());
//!
//! // 2. Create Training Data Schema
//! let heading = TupleType::new()
//!     .with_attribute("outlook", ScalarType::String)
//!     .with_attribute("humidity", ScalarType::String)
//!     .with_attribute("play", ScalarType::String);
//!
//! db.create_relvar("GOLF", RelationType::new(heading))?;
//!
//! // 3. Insert Training Data
//! db.insert("GOLF", tuple! { outlook: "sunny", humidity: "high", play: "no" })?;
//! db.insert("GOLF", tuple! { outlook: "overcast", humidity: "normal", play: "yes" })?;
//! // ... (in real usage, you'd add more data) ...
//!
//! // 4. Train Model
//! let golf = db.query("GOLF")?;
//! let model = NaiveBayesClassifier::train(&golf, "play")?;
//!
//! // 5. Predict
//! let t = tuple! { outlook: "sunny", humidity: "high" };
//! let prediction = model.predict(&t);
//!
//! // With very limited data (sunny=no, overcast=yes), sunny predicts "no"
//! assert_eq!(prediction, ScalarValue::String("no".to_string()));
//! # Ok(())
//! # }
//! ```

use relvar_core::algebra::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::values::{Relation, ScalarValue, Tuple};
use std::collections::HashMap;

/// A simple Naive Bayes classifier.
pub struct NaiveBayesClassifier {
    /// Class Prior Probabilities: P(C)
    /// Key: Class Value (as String) -> Value: Log Probability
    priors: HashMap<String, f64>,

    /// Feature Conditional Probabilities: P(Feature=Value | Class)
    /// Key: (Feature Name, Feature Value, Class Value) -> Value: Log Probability
    conditionals: HashMap<(String, String, String), f64>,

    /// Target attribute name
    #[allow(dead_code)]
    target_attr: String,

    /// Feature attribute names
    feature_attrs: Vec<String>,

    /// Classes (unique values of target attribute)
    classes: Vec<String>,
}

impl NaiveBayesClassifier {
    /// Trains a Naive Bayes classifier on a relation.
    ///
    /// # Arguments
    ///
    /// * `relation` - The relation containing training data.
    /// * `target_attr` - The name of the target attribute (class label).
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::AttributeNotFound` if `target_attr` is not in the relation.
    /// Returns `DatabaseError::AlgebraError` if internal algebra operations fail.
    pub fn train(relation: &Relation, target_attr: &str) -> Result<Self, DatabaseError> {
        let heading = relation.relation_type().heading();

        if !heading.has_attribute(target_attr) {
            return Err(DatabaseError::AttributeNotFound(
                target_attr.to_string(),
                "<input_relation>".to_string(),
            ));
        }

        let feature_attrs: Vec<String> = heading
            .attribute_names()
            .filter(|&name| name != target_attr)
            .cloned()
            .collect();

        // 1. Calculate Class Priors P(C)
        // Group by target, Count(*)
        let class_counts_rel = relation
            .summarize(&[target_attr], &[Aggregation::count("count")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let total_count = relation.cardinality() as f64;
        let mut priors = HashMap::new();
        let mut classes = Vec::new();
        let mut class_counts_map = HashMap::new(); // Store raw counts for conditional calc

        for tuple in class_counts_rel.tuples() {
            let class_val = tuple.get(target_attr).unwrap();
            let count = tuple.get_typed::<i64>("count").unwrap() as f64;
            let class_str = scalar_to_string(class_val);

            // P(C) = count(C) / total
            priors.insert(class_str.clone(), (count / total_count).ln());
            classes.push(class_str.clone());
            class_counts_map.insert(class_str, count);
        }

        // 2. Calculate Conditional Probabilities P(F=f | C)
        // For each feature F: Group by (target, F), Count(*)
        let mut conditionals = HashMap::new();
        let epsilon = 1.0; // Laplace smoothing (add-one)

        for feature in &feature_attrs {
            // Count(F=f, C)
            let feat_counts_rel = relation
                .summarize(&[target_attr, feature], &[Aggregation::count("count")])
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // We need to handle zero frequency (smoothing).
            // But first, let's collect observed counts.
            // Map: (Class, FeatVal) -> Count
            let mut observed_counts: HashMap<(String, String), f64> = HashMap::new();
            let mut vocab: HashMap<String, std::collections::HashSet<String>> = HashMap::new(); // Class -> Set of Feature Values

            for tuple in feat_counts_rel.tuples() {
                let class_val = tuple.get(target_attr).unwrap();
                let feat_val = tuple.get(feature).unwrap();
                let count = tuple.get_typed::<i64>("count").unwrap() as f64;

                let class_str = scalar_to_string(class_val);
                let feat_str = scalar_to_string(feat_val);

                observed_counts.insert((class_str.clone(), feat_str.clone()), count);
                vocab.entry(class_str).or_default().insert(feat_str);
            }

            // Calculate probabilities with smoothing
            // P(F=f | C) = (count(f, C) + 1) / (count(C) + |V|)
            // where |V| is number of distinct values for feature F (vocabulary size)

            // First, find global vocabulary size for this feature
            let mut global_vocab = std::collections::HashSet::new();
            for tuple in relation.tuples() {
                let val = tuple.get(feature).unwrap();
                global_vocab.insert(scalar_to_string(val));
            }
            let vocab_size = global_vocab.len() as f64;

            for class in &classes {
                let class_count = *class_counts_map.get(class).unwrap_or(&0.0);

                // For every value in global vocabulary (even if count is 0 for this class)
                for val in &global_vocab {
                    let count = *observed_counts
                        .get(&(class.clone(), val.clone()))
                        .unwrap_or(&0.0);

                    let prob = (count + epsilon) / (class_count + vocab_size * epsilon);
                    conditionals.insert((feature.clone(), val.clone(), class.clone()), prob.ln());
                }
            }
        }

        Ok(NaiveBayesClassifier {
            priors,
            conditionals,
            target_attr: target_attr.to_string(),
            feature_attrs,
            classes,
        })
    }

    /// Predicts the class for a given tuple.
    ///
    /// # Arguments
    ///
    /// * `tuple` - The tuple to classify. Must contain all feature attributes used in training.
    pub fn predict(&self, tuple: &Tuple) -> ScalarValue {
        let mut best_class = None;
        let mut max_score = f64::NEG_INFINITY;

        for class in &self.classes {
            // Start with Prior P(C)
            let mut score = *self.priors.get(class).unwrap_or(&f64::NEG_INFINITY);

            // Add Log Likelihoods sum(log(P(xi | C)))
            for feature in &self.feature_attrs {
                if let Some(val) = tuple.get(feature) {
                    let val_str = scalar_to_string(val);
                    let key = (feature.clone(), val_str, class.clone());

                    // If value seen during training, use probability.
                    // If unseen, ignore (or use smoothing if we had a global vocab fallback).
                    // For MVP, if unseen, we add a very small log prob (penalty).
                    let log_prob = self.conditionals.get(&key).copied().unwrap_or(-10.0); // -10.0 ~= log(4.5e-5)
                    score += log_prob;
                }
            }

            if score > max_score {
                max_score = score;
                best_class = Some(class.clone());
            }
        }

        // Return class as string (assuming target was string/int converted to string)
        // In a real implementation, we would store the original type of the target
        // and convert back. For now, return String.
        ScalarValue::String(best_class.unwrap_or_else(|| "Unknown".to_string()))
    }
}

/// Helper to convert scalar values to string keys.
fn scalar_to_string(val: &ScalarValue) -> String {
    match val {
        ScalarValue::Int(v) => v.to_string(),
        ScalarValue::Float(v) => v.to_string(),
        ScalarValue::String(v) => v.clone(),
        ScalarValue::Bool(v) => v.to_string(),
        _ => format!("{:?}", val),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::database::Database;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_play_golf_dataset() -> Result<(), DatabaseError> {
        let mut db = Database::new(InMemoryEngine::new());

        // 1. Setup Data
        let heading = TupleType::new()
            .with_attribute("outlook", ScalarType::String)
            .with_attribute("temp", ScalarType::String)
            .with_attribute("humidity", ScalarType::String)
            .with_attribute("windy", ScalarType::String)
            .with_attribute("play", ScalarType::String);

        db.create_relvar("GOLF", RelationType::new(heading))?;

        // Training Data (Subset of typical dataset)
        // Sunny, Hot, High, False -> No
        db.insert(
            "GOLF",
            tuple! { outlook: "sunny", temp: "hot", humidity: "high", windy: "false", play: "no" },
        )?;
        // Overcast, Hot, High, False -> Yes
        db.insert("GOLF", tuple! { outlook: "overcast", temp: "hot", humidity: "high", windy: "false", play: "yes" })?;
        // Rain, Mild, High, False -> Yes
        db.insert(
            "GOLF",
            tuple! { outlook: "rain", temp: "mild", humidity: "high", windy: "false", play: "yes" },
        )?;
        // Sunny, Mild, High, True -> No
        db.insert(
            "GOLF",
            tuple! { outlook: "sunny", temp: "mild", humidity: "high", windy: "true", play: "no" },
        )?;

        // 2. Train
        let golf = db.query("GOLF")?;
        let model = NaiveBayesClassifier::train(&golf, "play")?;

        // 3. Predict
        // Test 1: Sunny (No), Hot (No), High (No), False (No/Yes) -> Expect "no"
        // (Since Sunny is strong indicator for No in this tiny dataset)
        let t1 = tuple! { outlook: "sunny", temp: "hot", humidity: "high", windy: "false" };
        let p1 = model.predict(&t1);
        assert_eq!(p1, ScalarValue::String("no".to_string()));

        // Test 2: Overcast -> Yes (Overcast is 100% Yes here)
        let t2 = tuple! { outlook: "overcast", temp: "hot", humidity: "high", windy: "false" };
        let p2 = model.predict(&t2);
        assert_eq!(p2, ScalarValue::String("yes".to_string()));

        Ok(())
    }

    #[test]
    fn test_train_error_handling() -> Result<(), DatabaseError> {
        let mut db = Database::new(InMemoryEngine::new());
        let heading = TupleType::new()
            .with_attribute("outlook", ScalarType::String)
            .with_attribute("play", ScalarType::String);
        db.create_relvar("GOLF", RelationType::new(heading))?;
        db.insert("GOLF", tuple! { outlook: "sunny", play: "no" })?;

        let golf = db.query("GOLF")?;

        let err = NaiveBayesClassifier::train(&golf, "missing_target");
        assert!(matches!(err, Err(DatabaseError::AttributeNotFound(..))));

        Ok(())
    }

    #[test]
    fn test_predict_unseen_values() -> Result<(), DatabaseError> {
        let mut db = Database::new(InMemoryEngine::new());
        let heading = TupleType::new()
            .with_attribute("f1", ScalarType::Int)
            .with_attribute("f2", ScalarType::Float)
            .with_attribute("f3", ScalarType::Bool)
            .with_attribute("target", ScalarType::String);
        db.create_relvar("DATA", RelationType::new(heading))?;
        db.insert("DATA", tuple! { f1: 1, f2: 1.0, f3: true, target: "A" })?;
        db.insert("DATA", tuple! { f1: 2, f2: 2.0, f3: false, target: "B" })?;

        let data = db.query("DATA")?;
        let model = NaiveBayesClassifier::train(&data, "target")?;

        // Prediction with completely unseen values
        let t = tuple! { f1: 99, f2: 99.0, f3: true };
        let p = model.predict(&t);
        // Should still predict something without panicking
        assert!(
            p == ScalarValue::String("A".to_string()) || p == ScalarValue::String("B".to_string())
        );

        Ok(())
    }
}
