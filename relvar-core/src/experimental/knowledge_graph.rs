//! The Knowledge Graph module.
//!
//! Why build a Knowledge Graph out of relational algebra? Because graphs are fundamentally
//! just interconnected relations! This module exists to demonstrate how the foundational
//! operations of relational algebra (like [`Relation::restrict`], [`Relation::project`],
//! and [`Relation::join`]) can be composed to build powerful abstractions.
//!
//! Rather than building a separate graph database engine with complex traversal algorithms,
//! we represent the entire graph as a single relation of triples: `(subject, predicate, object)`.
//! Graph queries (like finding all friends of a person) are simply evaluated as a sequence of
//! relational restrictions (to find a starting node) and joins (to traverse edges).
//!
//! This module showcases the expressive power of the relational model.

use crate::error::DatabaseError;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// A Knowledge Graph modeled purely using relational algebra.
///
/// Under the hood, this struct maintains a single relation storing all data as
/// `(subject, predicate, object)` triples. This approach, known as the Resource
/// Description Framework (RDF) model, allows us to flexibly store any kind of
/// interconnected data without rigid, predefined schemas.
///
/// # Examples
///
/// ```
/// use relvar_core::experimental::knowledge_graph::{KnowledgeGraph, TriplePattern};
///
/// let mut kg = KnowledgeGraph::new();
///
/// // We build our graph by inserting facts
/// kg.insert("Alice", "knows", "Bob").unwrap();
/// kg.insert("Bob", "knows", "Charlie").unwrap();
/// kg.insert("Bob", "age", "30").unwrap();
///
/// // We can query the graph using variables (prefixed with '?')
/// let patterns = vec![
///     TriplePattern::new("Alice", "knows", "?friend"),
///     TriplePattern::new("?friend", "age", "?age"),
/// ];
///
/// // The result is a standard Relation containing our bindings!
/// let result = kg.query(&patterns).unwrap();
/// assert_eq!(result.cardinality(), 1);
/// ```
pub struct KnowledgeGraph {
    /// A single relation storing triples (subject, predicate, object)
    pub triples: Relation,
}

impl KnowledgeGraph {
    /// Creates a new, empty Knowledge Graph.
    ///
    /// This establishes the foundational schema: a single relation with a heading of
    /// `(subject: String, predicate: String, object: String)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::experimental::knowledge_graph::KnowledgeGraph;
    ///
    /// let kg = KnowledgeGraph::new();
    /// assert_eq!(kg.triples.cardinality(), 0);
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

    /// Inserts a new triple into the Knowledge Graph.
    ///
    /// The triple `(subject, predicate, object)` represents a single fact. For example,
    /// `("Alice", "knows", "Bob")` establishes a directed edge from Alice to Bob.
    ///
    /// # Errors
    ///
    /// Returns a [`DatabaseError::AlgebraError`] if the underlying relation insertion fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::experimental::knowledge_graph::KnowledgeGraph;
    ///
    /// let mut kg = KnowledgeGraph::new();
    /// kg.insert("Earth", "orbits", "Sun").unwrap();
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

    /// Queries the graph using a Basic Graph Pattern (BGP).
    ///
    /// A pattern is a list of [`TriplePattern`]s. The engine evaluates each pattern by
    /// mapping known values to restrictions and unknown values (starting with `?`) to
    /// projections and renames. The results of individual patterns are then combined
    /// using natural joins to enforce that shared variables bind to the same values.
    ///
    /// # Errors
    ///
    /// Returns a [`DatabaseError::AlgebraError`] if the pattern list is empty or if
    /// any underlying relational operations (like joins) fail.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::experimental::knowledge_graph::{KnowledgeGraph, TriplePattern};
    ///
    /// let mut kg = KnowledgeGraph::new();
    /// kg.insert("Alice", "likes", "Rust").unwrap();
    ///
    /// let patterns = vec![TriplePattern::new("?person", "likes", "Rust")];
    /// let result = kg.query(&patterns).unwrap();
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

/// Represents a single pattern to match in the graph.
/// A node can be either a variable (starts with '?') or a concrete value.
#[derive(Debug, Clone)]
pub struct TriplePattern {
    /// Subject
    pub subject: String,
    /// Predicate
    pub predicate: String,
    /// Object
    pub object: String,
}

impl TriplePattern {
    /// Creates a new TriplePattern.
    ///
    /// A pattern node that begins with a `?` is treated as a variable. All other nodes
    /// are treated as concrete string values that must match exactly.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::experimental::knowledge_graph::TriplePattern;
    ///
    /// // Match exact relationships
    /// let p1 = TriplePattern::new("Alice", "knows", "Bob");
    ///
    /// // Match any object for a specific subject and predicate
    /// let p2 = TriplePattern::new("Alice", "knows", "?friend");
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
