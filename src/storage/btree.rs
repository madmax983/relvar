use crate::values::{ScalarValue, Tuple};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BTreeIndexError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Key not found")]
    KeyNotFound,
}

/// A B-tree index mapping scalar values to tuples.
///
/// **TTM Compliance Note:**
/// This redesigned index stores full tuple values instead of TupleId references
/// to comply with TTM Proscription 6 (no tuple-level identifiers).
///
/// **Design Tradeoffs:**
/// - **Memory overhead:** Tuples are duplicated in both heap and indexes
/// - **Consistency:** Indexes must be updated when heap tuples change (not yet implemented)
/// - **Benefit:** No exposure of physical storage identifiers; pure value-based indexing
///
/// **Future considerations:**
/// When indexes are actively used by Database layer, consider alternative approaches:
/// - Store only indexed attributes + primary key values, join back to heap
/// - Implement automatic index maintenance on heap updates
/// - Use copy-on-write or versioning to reduce duplication overhead
///
/// **Current status:** Database doesn't use indexes yet, so this design is acceptable
/// for TTM compliance demonstration. Will need refinement before production use.
///
/// **Implementation:** Simplified in-memory BTreeMap. Production systems would
/// implement proper on-disk B-tree with more sophisticated storage strategies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BTreeIndex {
    /// Map from key value to list of tuples
    index: BTreeMap<ScalarValue, Vec<Tuple>>,
}

impl BTreeIndex {
    pub fn new() -> Self {
        Self {
            index: BTreeMap::new(),
        }
    }

    /// Insert a key-tuple pair
    pub fn insert(&mut self, key: ScalarValue, tuple: Tuple) {
        self.index.entry(key).or_default().push(tuple);
    }

    /// Search for a key and return all matching tuples
    pub fn search(&self, key: &ScalarValue) -> Option<&[Tuple]> {
        self.index.get(key).map(|v| v.as_slice())
    }

    /// Remove a specific tuple for a key
    pub fn remove(&mut self, key: &ScalarValue, tuple: &Tuple) -> bool {
        if let Some(tuples) = self.index.get_mut(key)
            && let Some(pos) = tuples.iter().position(|t| t == tuple)
        {
            tuples.swap_remove(pos);
            if tuples.is_empty() {
                self.index.remove(key);
            }
            return true;
        }
        false
    }

    /// Delete all entries for a key
    pub fn delete(&mut self, key: &ScalarValue) -> bool {
        self.index.remove(key).is_some()
    }

    /// Range scan: find all keys in [start, end]
    pub fn range_scan(&self, start: &ScalarValue, end: &ScalarValue) -> Vec<(ScalarValue, Tuple)> {
        let mut results = Vec::new();
        for (key, tuples) in self.index.range(start.clone()..=end.clone()) {
            // Optimize: clone key once per outer loop, not for every tuple
            if let Some((last_tuple, first_tuples)) = tuples.split_last() {
                let key_clone = key.clone();
                for tuple in first_tuples {
                    results.push((key_clone.clone(), tuple.clone()));
                }
                results.push((key_clone, last_tuple.clone()));
            }
        }
        results
    }

    pub fn key_count(&self) -> usize {
        self.index.len()
    }

    pub fn entry_count(&self) -> usize {
        self.index.values().map(|v| v.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), BTreeIndexError> {
        let contents =
            bincode::serialize(self).map_err(|e| BTreeIndexError::Serialization(e.to_string()))?;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        file.write_all(&contents)?;
        file.sync_all()?;

        Ok(())
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, BTreeIndexError> {
        let mut file = File::open(path)?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;

        if contents.is_empty() {
            return Ok(Self::new());
        }

        bincode::deserialize(&contents).map_err(|e| BTreeIndexError::Serialization(e.to_string()))
    }
}

impl Default for BTreeIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::values::ScalarValue;
    use tempfile::NamedTempFile;

    fn create_test_tuple(id: i64, name: &str) -> Tuple {
        tuple! { id: id, name: name }
    }

    #[test]
    fn test_btree_insert_and_search() {
        let mut index = BTreeIndex::new();

        let key = ScalarValue::Int(42);
        let tuple = create_test_tuple(42, "Answer");

        index.insert(key.clone(), tuple.clone());

        let results = index.search(&key).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], tuple);
    }

    #[test]
    fn test_btree_search_not_found() {
        let index = BTreeIndex::new();

        let key = ScalarValue::Int(42);
        let results = index.search(&key);

        assert!(results.is_none());
    }

    #[test]
    fn test_btree_duplicate_keys() {
        let mut index = BTreeIndex::new();

        let key = ScalarValue::String("Alice".to_string());
        let tuple1 = create_test_tuple(1, "Alice");
        let tuple2 = create_test_tuple(2, "Alice");
        let tuple3 = create_test_tuple(3, "Alice");

        index.insert(key.clone(), tuple1.clone());
        index.insert(key.clone(), tuple2.clone());
        index.insert(key.clone(), tuple3.clone());

        let results = index.search(&key).unwrap();
        assert_eq!(results.len(), 3);
        assert!(results.contains(&tuple1));
        assert!(results.contains(&tuple2));
        assert!(results.contains(&tuple3));
    }

    #[test]
    fn test_btree_remove() {
        let mut index = BTreeIndex::new();

        let key = ScalarValue::Int(42);
        let tuple1 = create_test_tuple(1, "First");
        let tuple2 = create_test_tuple(2, "Second");

        index.insert(key.clone(), tuple1.clone());
        index.insert(key.clone(), tuple2.clone());

        assert!(index.remove(&key, &tuple1));

        let results = index.search(&key).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], tuple2);
    }

    #[test]
    fn test_btree_delete() {
        let mut index = BTreeIndex::new();

        let key = ScalarValue::Int(42);
        let tuple1 = create_test_tuple(1, "First");
        let tuple2 = create_test_tuple(2, "Second");

        index.insert(key.clone(), tuple1);
        index.insert(key.clone(), tuple2);

        assert!(index.delete(&key));
        assert!(index.search(&key).is_none());
    }

    #[test]
    fn test_btree_range_scan() {
        let mut index = BTreeIndex::new();

        index.insert(ScalarValue::Int(10), create_test_tuple(10, "Ten"));
        index.insert(ScalarValue::Int(20), create_test_tuple(20, "Twenty"));
        index.insert(ScalarValue::Int(30), create_test_tuple(30, "Thirty"));
        index.insert(ScalarValue::Int(40), create_test_tuple(40, "Forty"));
        index.insert(ScalarValue::Int(50), create_test_tuple(50, "Fifty"));

        let results = index.range_scan(&ScalarValue::Int(20), &ScalarValue::Int(40));

        assert_eq!(results.len(), 3);
        assert!(results.iter().any(|(k, _)| k == &ScalarValue::Int(20)));
        assert!(results.iter().any(|(k, _)| k == &ScalarValue::Int(30)));
        assert!(results.iter().any(|(k, _)| k == &ScalarValue::Int(40)));
    }

    #[test]
    fn test_btree_counts() {
        let mut index = BTreeIndex::new();

        assert_eq!(index.key_count(), 0);
        assert_eq!(index.entry_count(), 0);
        assert!(index.is_empty());

        index.insert(ScalarValue::Int(10), create_test_tuple(1, "First"));
        index.insert(ScalarValue::Int(10), create_test_tuple(2, "Second"));
        index.insert(ScalarValue::Int(20), create_test_tuple(3, "Third"));

        assert_eq!(index.key_count(), 2); // Two unique keys: 10 and 20
        assert_eq!(index.entry_count(), 3); // Three total entries
        assert!(!index.is_empty());
    }

    #[test]
    fn test_btree_save_and_load() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Create and save index
        {
            let mut index = BTreeIndex::new();
            index.insert(ScalarValue::Int(10), create_test_tuple(10, "Ten"));
            index.insert(ScalarValue::Int(20), create_test_tuple(20, "Twenty"));
            index.insert(ScalarValue::Int(30), create_test_tuple(30, "Thirty"));

            index.save(path).unwrap();
        }

        // Load index
        {
            let index = BTreeIndex::load(path).unwrap();
            assert_eq!(index.key_count(), 3);
            assert_eq!(index.entry_count(), 3);

            assert!(index.search(&ScalarValue::Int(10)).is_some());
            assert!(index.search(&ScalarValue::Int(20)).is_some());
            assert!(index.search(&ScalarValue::Int(30)).is_some());
        }
    }

    #[test]
    fn test_btree_string_keys() {
        let mut index = BTreeIndex::new();

        index.insert(
            ScalarValue::String("Alice".to_string()),
            create_test_tuple(1, "Alice"),
        );
        index.insert(
            ScalarValue::String("Bob".to_string()),
            create_test_tuple(2, "Bob"),
        );
        index.insert(
            ScalarValue::String("Charlie".to_string()),
            create_test_tuple(3, "Charlie"),
        );

        let results = index.range_scan(
            &ScalarValue::String("Alice".to_string()),
            &ScalarValue::String("Charlie".to_string()),
        );

        assert_eq!(results.len(), 3);
    }

    /// Test verifying BTreeIndex stores tuples, not TupleIds (TTM Proscription 6)
    #[test]
    fn test_btree_stores_tuples_not_tuple_ids() {
        let mut index = BTreeIndex::new();

        let key = ScalarValue::Int(42);
        let tuple = tuple! { id: 42i64, name: "Answer" };

        // Type check: insert takes Tuple not TupleId
        index.insert(key.clone(), tuple.clone());

        // Type check: search returns &[Tuple] not &[TupleId]
        let results = index.search(&key).unwrap();
        assert_eq!(results[0], tuple);
    }
}
