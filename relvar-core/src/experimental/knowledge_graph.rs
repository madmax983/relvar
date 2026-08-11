//! A relational approach to Semantic Web and Knowledge Graph querying.
//!
//! This module provides a minimal, experimental implementation of a Knowledge Graph
//! built entirely on top of relational algebra. Instead of using complex graph traversal
//! algorithms or dedicated triple-stores, it stores `(subject, predicate, object)` triples
//! in a single [`crate::values::Relation`] and evaluates Basic Graph Patterns (BGPs) by translating
//! them into standard relational operations like `Restrict`, `Project`, and `Join`.
//!
//! This serves as a demonstration of the power and universality of the relational model,
//! proving that even "graph workloads" can be gracefully handled by standard algebraic primitives.

use crate::error::DatabaseError;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// A Knowledge Graph modeled purely using relational algebra.
///
/// Rather than storing pointers or adjacency lists, this structure maintains
/// a single, universal relation of triples. This allows querying the graph
/// using standard set operations, which are often highly optimized.
///
/// # Examples
///
/// ```
/// use relvar_core::experimental::knowledge_graph::{KnowledgeGraph, TriplePattern};
///
/// let mut graph = KnowledgeGraph::new();
/// graph.insert("Alice", "knows", "Bob").unwrap();
/// graph.insert("Bob", "likes", "Rust").unwrap();
///
/// // Find out what Alice's friends like
/// let query = vec![
///     TriplePattern::new("Alice", "knows", "?friend"),
///     TriplePattern::new("?friend", "likes", "?language"),
/// ];
/// let results = graph.query(&query).unwrap();
///
/// // The result is a relation containing { friend: "Bob", language: "Rust" }
/// assert_eq!(results.cardinality(), 1);
/// ```
pub struct KnowledgeGraph {
    /// A single relation storing triples (subject, predicate, object)
    pub triples: Relation,
}

impl KnowledgeGraph {
    /// Bootstraps an empty graph environment.
    ///
    /// This establishes the foundational [`Relation`] with the standard `(subject, predicate, object)`
    /// schema. It acts as the blank canvas upon which your web of knowledge will be woven.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::experimental::knowledge_graph::KnowledgeGraph;
    ///
    /// let graph = KnowledgeGraph::new();
    /// assert_eq!(graph.triples.cardinality(), 0);
    /// ```
    pub fn new() -> Self {
        let heading = TupleType::new()
            .with_attribute("subject", ScalarType::String)
            .with_attribute("predicate", ScalarType::String)
            .with_attribute("object", ScalarType::String);
        let rel_type = RelationType::new(heading);
        Self {
            triples: Relation::new(rel_type),
        }
    }

    /// Records a new fact in the knowledge base.
    ///
    /// By inserting a `(subject, predicate, object)` triple, you expand the graph's understanding
    /// of the world. Internally, this translates the strings into a `Tuple` and inserts it into
    /// the underlying [`Relation`].
    ///
    /// # Errors
    /// Returns a [`DatabaseError::AlgebraError`] if the underlying relation rejects the tuple
    /// (e.g., due to schema mismatch, though structurally impossible here unless corrupted).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::experimental::knowledge_graph::KnowledgeGraph;
    ///
    /// let mut graph = KnowledgeGraph::new();
    /// graph.insert("Earth", "orbits", "Sun").unwrap();
    /// ```
    pub fn insert(
        &mut self,
        subject: &str,
        predicate: &str,
        object: &str,
    ) -> Result<(), DatabaseError> {
        let t = crate::tuple! {
            subject: subject,
            predicate: predicate,
            object: object
        };
        self.triples
            .insert(t)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(())
    }

    /// Explores the graph to find subgraphs matching a specific structure.
    ///
    /// Evaluates a Basic Graph Pattern (BGP), which is a list of [`TriplePattern`] constraints.
    /// Variables (indicated by a `?` prefix) link the patterns together. Under the hood, this
    /// performs a series of restrictions, projections, and finally, relational joins to construct
    /// the matched bindings.
    ///
    /// # Errors
    /// - Returns a [`DatabaseError::AlgebraError`] if the provided pattern list is empty,
    ///   as there is no query to execute.
    /// - Propagates any internal relational algebra errors that might occur during joins.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::experimental::knowledge_graph::{KnowledgeGraph, TriplePattern};
    ///
    /// let mut kg = KnowledgeGraph::new();
    /// kg.insert("Device1", "type", "Sensor").unwrap();
    /// kg.insert("Device1", "status", "Active").unwrap();
    ///
    /// // Query to find all active sensors
    /// let patterns = vec![
    ///     TriplePattern::new("?device", "type", "Sensor"),
    ///     TriplePattern::new("?device", "status", "Active"),
    /// ];
    ///
    /// let results = kg.query(&patterns).unwrap();
    /// assert_eq!(results.cardinality(), 1);
    /// ```
    pub fn query(&self, patterns: &[TriplePattern]) -> Result<Relation, DatabaseError> {
        if patterns.is_empty() {
            return Err(DatabaseError::AlgebraError("Empty BGP".to_string()));
        }

        let mut result = self.evaluate_pattern(&patterns[0])?;

        for pattern in &patterns[1..] {
            let next_rel = self.evaluate_pattern(pattern)?;
            result = result.join(&next_rel)?;
        }

        Ok(result)
    }

    fn evaluate_pattern(&self, pattern: &TriplePattern) -> Result<Relation, DatabaseError> {
        let mut rel = self.triples.clone();

        // Restrict based on constant values
        if !pattern.subject.starts_with('?') {
            let s = pattern.subject.clone();
            rel = rel.restrict(move |t| t.get_typed::<String>("subject") == Some(s.clone()));
        }
        if !pattern.predicate.starts_with('?') {
            let p = pattern.predicate.clone();
            rel = rel.restrict(move |t| t.get_typed::<String>("predicate") == Some(p.clone()));
        }
        if !pattern.object.starts_with('?') {
            let o = pattern.object.clone();
            rel = rel.restrict(move |t| t.get_typed::<String>("object") == Some(o.clone()));
        }

        // Project and Rename for variables
        let mut vars = Vec::new();
        let mut rename_map = Vec::new();

        if pattern.subject.starts_with('?') {
            let var_name = &pattern.subject[1..];
            vars.push("subject");
            rename_map.push(("subject", var_name));
        }
        if pattern.predicate.starts_with('?') {
            let var_name = &pattern.predicate[1..];
            vars.push("predicate");
            rename_map.push(("predicate", var_name));
        }
        if pattern.object.starts_with('?') {
            let var_name = &pattern.object[1..];
            vars.push("object");
            rename_map.push(("object", var_name));
        }

        let projected: Relation = rel.project(&vars);

        let mut final_rename_map = Vec::new();
        for (old, new) in &rename_map {
            // Need to convert to &str for the rename API
            final_rename_map.push((*old, *new));
        }

        Ok(projected.rename(&final_rename_map))
    }
}

/// A blueprint for discovering facts within the [`KnowledgeGraph`].
///
/// A pattern consists of three components: subject, predicate, and object.
/// Each component can be either a concrete string literal (e.g., `"Alice"`) or
/// a variable (prefixed with `?`, e.g., `"?friend"`). When queried, variables
/// capture the matching data and output them as attributes in the resulting [`Relation`].
///
/// # Examples
///
/// ```
/// use relvar_core::experimental::knowledge_graph::TriplePattern;
///
/// // A pattern looking for anyone who "knows" someone named "Bob"
/// let pattern = TriplePattern::new("?person", "knows", "Bob");
/// ```
#[derive(Debug, Clone)]
pub struct TriplePattern {
    /// The source node of the edge
    pub subject: String,
    /// The label of the edge
    pub predicate: String,
    /// The target node of the edge
    pub object: String,
}

impl TriplePattern {
    /// Constructs a pattern to match against the graph.
    ///
    /// Use strings prefixed with `?` to indicate variables that you want to bind
    /// and retrieve in your query results.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::experimental::knowledge_graph::TriplePattern;
    ///
    /// // Match anything with the status "Active"
    /// let p = TriplePattern::new("?entity", "status", "Active");
    /// ```
    pub fn new(subject: &str, predicate: &str, object: &str) -> Self {
        Self {
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            object: object.to_string(),
        }
    }
}

impl Default for KnowledgeGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knowledge_graph_basic_query() {
        let mut kg = KnowledgeGraph::new();

        kg.insert("Alice", "knows", "Bob").unwrap();
        kg.insert("Alice", "knows", "Charlie").unwrap();
        kg.insert("Bob", "knows", "David").unwrap();
        kg.insert("Alice", "age", "30").unwrap();
        kg.insert("Bob", "age", "25").unwrap();

        let patterns = vec![
            TriplePattern::new("Alice", "knows", "?friend"),
            TriplePattern::new("?friend", "age", "?age"),
        ];

        let result = kg.query(&patterns).unwrap();

        assert_eq!(result.cardinality(), 1);

        let expected_tuple = crate::tuple! {
            friend: "Bob",
            age: "25"
        };
        assert!(result.contains(&expected_tuple));
    }

    #[test]
    fn test_knowledge_graph_empty_query() {
        let kg = KnowledgeGraph::new();
        let result = kg.query(&[]);
        assert!(result.is_err());
    }
}
