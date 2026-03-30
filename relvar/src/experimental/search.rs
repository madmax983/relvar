//! Relational Full-Text Search
//!
//! This module implements a full-text search engine using pure relational algebra.
//! It demonstrates that an inverted index is simply a relation, and search queries
//! can be expressed as relational operations (Restrict, Join, Summarize).
//!
//! # Example
//!
//! ```
//! use relvar::experimental::search::FullTextIndex;
//! use relvar_core::Database;
//! use relvar_core::storage_engine::InMemoryEngine;
//! use relvar_core::types::ScalarType;
//! use relvar_core::values::ScalarValue;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut db = Database::new(InMemoryEngine::new());
//!     let index = FullTextIndex::new("idx_docs", ScalarType::Int);
//!
//!     // 1. Initialize the index relation
//!     index.create(&mut db)?;
//!
//!     // 2. Index some documents
//!     index.index_document(&mut db, ScalarValue::Int(1), "The quick brown fox")?;
//!     index.index_document(&mut db, ScalarValue::Int(2), "The quick blue fox")?;
//!
//!     // 3. Search for terms
//!     let results = index.search(&db, "quick fox")?;
//!
//!     // Both documents match "quick" and "fox" (score 2)
//!     assert_eq!(results.cardinality(), 2);
//!
//!     Ok(())
//! }
//! ```
//!
//! # Disclaimer
//!
//! This is an **experimental demonstration** of relational principles. It is not intended
//! for production use as a high-performance search engine. It lacks advanced features
//! like stemming, stop-word removal (mostly), and TF-IDF scoring.
//!
//! # How it works
//!
//! 1. **Indexing**: Text is tokenized into words.
//! 2. **Storage**: An inverted index relation is created: `(term, doc_id, count)`.
//! 3. **Search**:
//!    - Query is tokenized.
//!    - Index is restricted to matching terms.
//!    - Results are grouped by `doc_id` and ranked by `sum(count)`.

use relvar_core::{
    Database,
    algebra::Aggregation,
    error::DatabaseError,
    storage_engine::StorageEngine,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue, Tuple},
};
use std::collections::HashMap;

/// Tokenizes text into a frequency map of terms.
///
/// Normalizes to lowercase and removes punctuation.
pub fn tokenize(text: &str) -> HashMap<String, i64> {
    let mut counts = HashMap::new();

    // Simple tokenization: lowercase, split by non-alphanumeric
    for word in text
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
    {
        let count = counts.entry(word.to_string()).or_insert(0i64);
        *count = (*count).saturating_add(1);
    }

    counts
}

/// A full-text search index backed by a relation.
///
/// # Examples
///
/// ```
/// use relvar::experimental::search::FullTextIndex;
/// use relvar_core::Database;
/// use relvar_core::storage_engine::InMemoryEngine;
/// use relvar_core::types::ScalarType;
/// use relvar_core::values::ScalarValue;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut db = Database::new(InMemoryEngine::new());
///     let index = FullTextIndex::new("idx_docs", ScalarType::Int);
///
///     // 1. Initialize the index relation
///     index.create(&mut db)?;
///
///     // 2. Index some documents
///     index.index_document(&mut db, ScalarValue::Int(1), "The quick brown fox")?;
///     index.index_document(&mut db, ScalarValue::Int(2), "The quick blue fox")?;
///
///     // 3. Search for terms
///     let results = index.search(&db, "quick fox")?;
///
///     // Both documents match "quick" and "fox" (score 2)
///     assert_eq!(results.cardinality(), 2);
///
///     Ok(())
/// }
/// ```
pub struct FullTextIndex {
    /// The name of the relation storing the index.
    index_name: String,
    /// The type of the document ID (usually Int or String).
    doc_id_type: ScalarType,
}

impl FullTextIndex {
    /// Creates a new index definition.
    pub fn new(index_name: &str, doc_id_type: ScalarType) -> Self {
        Self {
            index_name: index_name.to_string(),
            doc_id_type,
        }
    }

    /// Initializes the index relation in the database.
    ///
    /// Schema: `(term: String, doc_id: <doc_id_type>, count: Int)`
    pub fn create<S: StorageEngine>(&self, db: &mut Database<S>) -> Result<(), DatabaseError> {
        let heading = TupleType::new()
            .with_attribute("term".to_string(), ScalarType::String)
            .with_attribute("doc_id".to_string(), self.doc_id_type.clone())
            .with_attribute("count".to_string(), ScalarType::Int);

        let rel_type = RelationType::new(heading);
        db.create_relvar(&self.index_name, rel_type)
    }

    /// Indexes a document.
    ///
    /// Tokenizes the text and inserts (term, doc_id, count) tuples.
    /// Note: This is an additive operation. To update a document, you should
    /// delete its entries first (not implemented here for brevity).
    pub fn index_document<S: StorageEngine>(
        &self,
        db: &mut Database<S>,
        doc_id: ScalarValue,
        text: &str,
    ) -> Result<(), DatabaseError> {
        let tokens = tokenize(text);

        // In a real implementation, we would batch insert.
        // For now, we insert one by one.
        for (term, count) in tokens {
            let mut tuple_values = HashMap::new();
            tuple_values.insert("term".to_string(), ScalarValue::String(term));
            tuple_values.insert("doc_id".to_string(), doc_id.clone());
            tuple_values.insert("count".to_string(), ScalarValue::Int(count));

            // We need to construct the tuple manually because we don't have the TupleType handy
            // without querying the database or reconstructing it.
            // Reconstructing it is safe because we defined it in `create`.
            let heading = TupleType::new()
                .with_attribute("term".to_string(), ScalarType::String)
                .with_attribute("doc_id".to_string(), self.doc_id_type.clone())
                .with_attribute("count".to_string(), ScalarType::Int);

            let tuple = Tuple::new(heading, tuple_values).map_err(|e| {
                DatabaseError::AlgebraError(format!("Tuple validation failed: {}", e))
            })?;

            db.insert(&self.index_name, tuple)?;
        }

        Ok(())
    }

    /// Searches the index for the given query string.
    ///
    /// Returns a relation `(doc_id, score)` where score is the sum of term frequencies.
    pub fn search<E: StorageEngine>(
        &self,
        db: &Database<E>,
        query: &str,
    ) -> Result<Relation, DatabaseError> {
        // 1. Tokenize query
        let tokens = tokenize(query);
        let query_terms: Vec<String> = tokens.keys().cloned().collect();

        if query_terms.is_empty() {
            // Return empty relation with correct heading
            let heading = TupleType::new()
                .with_attribute("doc_id".to_string(), self.doc_id_type.clone())
                .with_attribute("score".to_string(), ScalarType::Int);
            return Ok(Relation::new(RelationType::new(heading)));
        }

        // 2. Get the index relation
        let index_rel = db.query(&self.index_name)?;

        // 3. Restrict to matching terms: term IN (query_terms)
        // We need to clone query_terms for the closure
        let terms_set = query_terms.clone();

        let matches = index_rel.restrict(move |tuple| {
            if let Some(ScalarValue::String(term)) = tuple.get("term") {
                terms_set.contains(term)
            } else {
                false
            }
        });

        // 4. Summarize: Group by doc_id, sum(count) as score
        // We use the `summarize` operator from relvar-core
        let result = matches
            .summarize(&["doc_id"], &[Aggregation::sum("score", "count")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // The result is now (doc_id, score).
        // Sorting is not part of relational algebra, but the client can sort the resulting tuples.

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::storage_engine::InMemoryEngine;

    #[test]
    fn test_full_text_search() -> Result<(), DatabaseError> {
        let mut db = Database::new(InMemoryEngine::new());
        let index = FullTextIndex::new("idx_documents", ScalarType::Int);

        // 1. Create Index
        index.create(&mut db)?;

        // 2. Index Documents
        index.index_document(&mut db, ScalarValue::Int(1), "The quick brown fox")?;
        index.index_document(&mut db, ScalarValue::Int(2), "The quick blue fox")?;
        index.index_document(&mut db, ScalarValue::Int(3), "The lazy dog")?;

        // 3. Search for "quick fox"
        // Doc 1: "quick" (1) + "fox" (1) = score 2
        // Doc 2: "quick" (1) + "fox" (1) = score 2
        // Doc 3: matches nothing
        let results = index.search(&db, "quick fox")?;

        assert_eq!(results.cardinality(), 2);

        // Check scores
        for tuple in results.tuples() {
            let doc_id = tuple.get_typed::<i64>("doc_id").unwrap();
            let score = tuple.get_typed::<i64>("score").unwrap();

            assert!(doc_id == 1 || doc_id == 2);
            assert_eq!(score, 2);
        }

        // 4. Search for "brown"
        // Doc 1: "brown" (1) = score 1
        let results_brown = index.search(&db, "brown")?;
        assert_eq!(results_brown.cardinality(), 1);
        let tuple = results_brown.tuples().next().unwrap();
        assert_eq!(tuple.get_typed::<i64>("doc_id").unwrap(), 1);
        assert_eq!(tuple.get_typed::<i64>("score").unwrap(), 1);

        Ok(())
    }

    #[test]
    fn test_tokenizer() {
        let text = "Hello, World! This is a test.";
        let tokens = tokenize(text);

        assert_eq!(tokens.get("hello"), Some(&1));
        assert_eq!(tokens.get("world"), Some(&1));
        assert_eq!(tokens.get("test"), Some(&1));
        assert_eq!(tokens.len(), 6);
        // "This" -> "this"
        // "is" -> "is"
        // "a" -> "a"
    }
}
