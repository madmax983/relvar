//! Relational Knowledge Graph (RDF/SPARQL)
//!
//! This module demonstrates how a Knowledge Graph (like RDF) and graph pattern matching (like SPARQL)
//! can be implemented using purely relational algebra.
//!
//! # Concept
//!
//! - **Triples**: The fundamental unit of a knowledge graph. We store them in a single relation
//!   with the schema `(subject: String, predicate: String, object: String)`.
//! - **Graph Pattern Matching**: In SPARQL, queries are formulated as graph patterns containing
//!   variables (e.g., `?person rdf:type :Artist . ?person :bornIn ?city`). We can evaluate these
//!   patterns purely relationally by joining the `triples` relation with itself for each pattern,
//!   renaming columns to match the variable names so that the Natural Join correctly aligns them.

use relvar_core::{
    error::DatabaseError,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue, Tuple},
};
use std::collections::HashMap;

/// A component of a triple pattern, which can be either a concrete value or a variable.
///
/// # Example
/// ```
/// use relvar::experimental::knowledge_graph::Term;
/// let t1 = Term::var("X");
/// let t2 = Term::val("Alice");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    /// A concrete URI or literal value.
    Value(String),
    /// A variable to be bound during query execution (e.g., "?person").
    Variable(String),
}

impl Term {
    /// Helper to create a variable term.
    pub fn var(name: &str) -> Self {
        Term::Variable(name.to_string())
    }

    /// Helper to create a value term.
    pub fn val(value: &str) -> Self {
        Term::Value(value.to_string())
    }
}

/// A triple pattern representing a condition to match against the knowledge graph.
/// Example: `?person type Artist`
#[derive(Debug, Clone)]
pub struct TriplePattern {
    /// The subject of the pattern.
    pub subject: Term,
    /// The predicate of the pattern.
    pub predicate: Term,
    /// The object of the pattern.
    pub object: Term,
}

impl TriplePattern {
    /// Creates a new TriplePattern.
    pub fn new(subject: Term, predicate: Term, object: Term) -> Self {
        Self {
            subject,
            predicate,
            object,
        }
    }
}

/// A Relational Knowledge Graph representing RDF-like triples.
pub struct KnowledgeGraph {
    /// The relation storing the triples. Schema: (subject: String, predicate: String, object: String)
    pub triples: Relation,
}

impl Default for KnowledgeGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeGraph {
    /// Creates a new, empty Knowledge Graph.
    ///
    /// # Example
    /// ```
    /// use relvar::experimental::knowledge_graph::KnowledgeGraph;
    /// let kg = KnowledgeGraph::new();
    /// ```
    pub fn new() -> Self {
        let heading = TupleType::new()
            .with_attribute("subject".to_string(), ScalarType::String)
            .with_attribute("predicate".to_string(), ScalarType::String)
            .with_attribute("object".to_string(), ScalarType::String);
        let triples = Relation::new(RelationType::new(heading));

        Self { triples }
    }

    /// Inserts a new triple into the knowledge graph.
    pub fn insert(
        &mut self,
        subject: &str,
        predicate: &str,
        object: &str,
    ) -> Result<(), DatabaseError> {
        let mut vals = HashMap::new();
        vals.insert(
            "subject".to_string(),
            ScalarValue::String(subject.to_string()),
        );
        vals.insert(
            "predicate".to_string(),
            ScalarValue::String(predicate.to_string()),
        );
        vals.insert(
            "object".to_string(),
            ScalarValue::String(object.to_string()),
        );

        let tuple = Tuple::new(self.triples.relation_type().heading().clone(), vals)
            .map_err(|e| DatabaseError::AlgebraError(format!("Tuple validation failed: {}", e)))?;
        self.triples.insert(tuple)?;
        Ok(())
    }

    /// Evaluates a single triple pattern and returns the resulting relation.
    /// The resulting relation has attributes corresponding to the variables in the pattern.
    fn match_pattern(&self, pattern: &TriplePattern) -> Result<Relation, DatabaseError> {
        let mut result = self.triples.clone();
        let mut rename_map = Vec::new();
        let mut to_project = Vec::new();

        // Evaluate Subject
        match &pattern.subject {
            Term::Value(v) => {
                let v_clone = v.clone();
                result =
                    result.restrict(move |t| t.get_typed::<String>("subject").unwrap() == v_clone);
            }
            Term::Variable(var) => {
                rename_map.push(("subject", var.as_str()));
                to_project.push(var.as_str());
            }
        }

        // Evaluate Predicate
        match &pattern.predicate {
            Term::Value(v) => {
                let v_clone = v.clone();
                result = result
                    .restrict(move |t| t.get_typed::<String>("predicate").unwrap() == v_clone);
            }
            Term::Variable(var) => {
                rename_map.push(("predicate", var.as_str()));
                to_project.push(var.as_str());
            }
        }

        // Evaluate Object
        match &pattern.object {
            Term::Value(v) => {
                let v_clone = v.clone();
                result =
                    result.restrict(move |t| t.get_typed::<String>("object").unwrap() == v_clone);
            }
            Term::Variable(var) => {
                rename_map.push(("object", var.as_str()));
                to_project.push(var.as_str());
            }
        }

        // Apply renames
        if !rename_map.is_empty() {
            result = result.rename(&rename_map);
        }

        // Project to only keep variables (removes columns with concrete values)
        if !to_project.is_empty() {
            result = result.project(&to_project);
        } else {
            // Edge case: no variables in pattern, we just return an empty relation if there are no matches,
            // or a relation with no attributes (DUM/DEE) if it matches.
            let empty_heading = TupleType::new();
            let mut dees_dums = Relation::new(RelationType::new(empty_heading.clone()));
            if result.cardinality() > 0 {
                dees_dums.insert(Tuple::new(empty_heading, HashMap::new()).unwrap())?;
            }
            result = dees_dums;
        }

        Ok(result)
    }

    /// Evaluates a Basic Graph Pattern (a list of triple patterns ANDed together).
    /// This is equivalent to joining the results of all individual triple patterns.
    pub fn query(&self, patterns: &[TriplePattern]) -> Result<Relation, DatabaseError> {
        if patterns.is_empty() {
            return Err(DatabaseError::AlgebraError("Empty query".to_string()));
        }

        let mut current_result = self.match_pattern(&patterns[0])?;

        for pattern in patterns.iter().skip(1) {
            let next_result = self.match_pattern(pattern)?;
            current_result = current_result.join(&next_result)?;
        }

        Ok(current_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knowledge_graph_basic_query() -> Result<(), DatabaseError> {
        let mut kg = KnowledgeGraph::new();

        // Alice is an Artist
        kg.insert("Alice", "type", "Artist")?;
        // Bob is an Artist
        kg.insert("Bob", "type", "Artist")?;
        // Charlie is a Programmer
        kg.insert("Charlie", "type", "Programmer")?;

        // Alice knows Bob
        kg.insert("Alice", "knows", "Bob")?;
        // Bob knows Charlie
        kg.insert("Bob", "knows", "Charlie")?;

        // Find all artists: ?person type Artist
        let q1 = vec![TriplePattern::new(
            Term::var("person"),
            Term::val("type"),
            Term::val("Artist"),
        )];
        let res1 = kg.query(&q1)?;
        assert_eq!(res1.cardinality(), 2);

        let mut artists = Vec::new();
        for t in res1.tuples() {
            artists.push(t.get_typed::<String>("person").unwrap().clone());
        }
        artists.sort();
        assert_eq!(artists, vec!["Alice", "Bob"]);

        // Find artists who know another artist:
        // ?p1 type Artist
        // ?p2 type Artist
        // ?p1 knows ?p2
        let q2 = vec![
            TriplePattern::new(Term::var("p1"), Term::val("type"), Term::val("Artist")),
            TriplePattern::new(Term::var("p2"), Term::val("type"), Term::val("Artist")),
            TriplePattern::new(Term::var("p1"), Term::val("knows"), Term::var("p2")),
        ];

        let res2 = kg.query(&q2)?;
        assert_eq!(res2.cardinality(), 1);

        let mut tuples = res2.tuples();
        let t = tuples.next().unwrap();
        assert_eq!(t.get_typed::<String>("p1").unwrap(), "Alice");
        assert_eq!(t.get_typed::<String>("p2").unwrap(), "Bob");

        Ok(())
    }

    #[test]
    fn test_knowledge_graph_empty_query() {
        let kg = KnowledgeGraph::new();
        assert!(kg.query(&[]).is_err());
    }
}
