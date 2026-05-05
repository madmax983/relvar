use crate::error::DatabaseError;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// A Knowledge Graph modeled purely using relational algebra.
/// # Examples
///
/// ```
/// use relvar_core::experimental::knowledge_graph::{KnowledgeGraph, TriplePattern};
///
/// let mut kg = KnowledgeGraph::new();
/// kg.insert("Alice", "knows", "Bob").unwrap();
///
/// let patterns = vec![TriplePattern::new("Alice", "knows", "?person")];
/// let result = kg.query(&patterns).unwrap();
///
/// assert_eq!(result.cardinality(), 1);
/// ```
pub struct KnowledgeGraph {
    /// A single relation storing triples (subject, predicate, object)
    pub triples: Relation,
}

impl KnowledgeGraph {
    /// Create a new empty Knowledge Graph.
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

    /// Insert a new triple into the Knowledge Graph.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::experimental::knowledge_graph::KnowledgeGraph;
    ///
    /// let mut kg = KnowledgeGraph::new();
    /// kg.insert("Alice", "likes", "Rust").unwrap();
    /// assert_eq!(kg.triples.cardinality(), 1);
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

    /// Query the graph using a basic graph pattern (BGP).
    /// A pattern is represented as a list of `TriplePattern` structs.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::experimental::knowledge_graph::{KnowledgeGraph, TriplePattern};
    ///
    /// let mut kg = KnowledgeGraph::new();
    /// kg.insert("Alice", "knows", "Bob").unwrap();
    /// kg.insert("Bob", "likes", "Rust").unwrap();
    ///
    /// let patterns = vec![
    ///     TriplePattern::new("Alice", "knows", "?friend"),
    ///     TriplePattern::new("?friend", "likes", "?lang"),
    /// ];
    ///
    /// let result = kg.query(&patterns).unwrap();
    /// assert_eq!(result.cardinality(), 1);
    ///
    /// let expected_tuple = relvar_core::tuple! {
    ///     friend: "Bob",
    ///     lang: "Rust"
    /// };
    /// assert!(result.contains(&expected_tuple));
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
    /// Create a new TriplePattern
    /// # Examples
    ///
    /// ```
    /// use relvar_core::experimental::knowledge_graph::TriplePattern;
    ///
    /// let pattern = TriplePattern::new("?person", "knows", "Bob");
    /// assert_eq!(pattern.subject, "?person");
    /// assert_eq!(pattern.predicate, "knows");
    /// assert_eq!(pattern.object, "Bob");
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
