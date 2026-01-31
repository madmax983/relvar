//! B-tree index for efficient key-based tuple lookup.
//!
//! This module provides a B-tree index that maps scalar key values to tuples.
//! It supports point lookups, range scans, and persistence to disk.
//!
//! # TTM Compliance
//!
//! This index stores full tuples rather than TupleId references, complying
//! with TTM Proscription 6: "The database must not generate or expose
//! tuple-level identifiers."
//!
//! From the relational perspective, tuples are identified by their attribute
//! values (keys), not by physical storage locations.
//!
//! # Implementation Note
//!
//! This is a simplified implementation using Rust's `BTreeMap`. Production
//! systems would implement a proper on-disk B-tree with:
//! - Page-based storage
//! - Split and merge operations
//! - Write-ahead logging
//!
//! # Example
//!
//! ```
//! use relvar_storage::storage::BTreeIndex;
//! use relvar_core::values::ScalarValue;
//! use relvar_core::tuple;
//!
//! let mut index = BTreeIndex::new();
//!
//! // Insert key-tuple pairs
//! index.insert(ScalarValue::Int(1), tuple! { id: 1i64, name: "Alice" });
//! index.insert(ScalarValue::Int(2), tuple! { id: 2i64, name: "Bob" });
//!
//! // Point lookup
//! let results = index.search(&ScalarValue::Int(1)).unwrap();
//! assert_eq!(results.len(), 1);
//!
//! // Range scan
//! let range = index.range_scan(&ScalarValue::Int(1), &ScalarValue::Int(2));
//! assert_eq!(range.len(), 2);
//! ```

use relvar_core::values::{ScalarValue, Tuple};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use thiserror::Error;

/// Errors that can occur during B-tree index operations.
#[derive(Debug, Error)]
pub enum BTreeIndexError {
    /// An I/O error occurred while reading or writing the index.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization or deserialization of the index failed.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// The requested key was not found in the index.
    #[error("Key not found")]
    KeyNotFound,
}

const MAX_INDEX_SIZE: u64 = 1024 * 1024 * 1024; // 1 GB

/// A B-tree index mapping scalar key values to tuples.
///
/// This index provides O(log n) lookup, insertion, and deletion. Multiple
/// tuples can share the same key value (non-unique index). The index
/// maintains keys in sorted order, enabling efficient range scans.
///
/// # TTM Compliance
///
/// This index stores full tuple values instead of TupleId references,
/// complying with TTM Proscription 6 (no tuple-level identifiers).
/// From the relational perspective, tuples are identified by their
/// attribute values, not by physical storage locations.
///
/// # Design Tradeoffs
///
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
///
/// # Example
///
/// ```
/// use relvar_storage::storage::BTreeIndex;
/// use relvar_core::values::ScalarValue;
/// use relvar_core::tuple;
///
/// let mut index = BTreeIndex::new();
///
/// // Index supports duplicate keys
/// index.insert(ScalarValue::String("Sales".to_string()),
///              tuple! { id: 1i64, dept: "Sales" });
/// index.insert(ScalarValue::String("Sales".to_string()),
///              tuple! { id: 2i64, dept: "Sales" });
///
/// // Search returns all matching tuples
/// let results = index.search(&ScalarValue::String("Sales".to_string())).unwrap();
/// assert_eq!(results.len(), 2);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BTreeIndex {
    /// Map from key value to list of tuples with that key.
    index: BTreeMap<ScalarValue, Vec<Tuple>>,
}

impl BTreeIndex {
    /// Creates a new empty B-tree index.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_storage::storage::BTreeIndex;
    ///
    /// let index = BTreeIndex::new();
    /// assert!(index.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            index: BTreeMap::new(),
        }
    }

    /// Inserts a key-tuple pair into the index.
    ///
    /// Multiple tuples can be associated with the same key. This is useful
    /// for non-unique indexes (e.g., indexing by department name).
    ///
    /// # Arguments
    ///
    /// * `key` - The key value to index by
    /// * `tuple` - The tuple to associate with this key
    pub fn insert(&mut self, key: ScalarValue, tuple: Tuple) {
        self.index.entry(key).or_default().push(tuple);
    }

    /// Searches for all tuples with the given key.
    ///
    /// Returns `None` if no tuples match the key, or `Some(&[Tuple])`
    /// containing all matching tuples.
    ///
    /// # Arguments
    ///
    /// * `key` - The key value to search for
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_storage::storage::BTreeIndex;
    /// use relvar_core::values::ScalarValue;
    /// use relvar_core::tuple;
    ///
    /// let mut index = BTreeIndex::new();
    /// index.insert(ScalarValue::Int(42), tuple! { id: 42i64, name: "Answer" });
    ///
    /// assert!(index.search(&ScalarValue::Int(42)).is_some());
    /// assert!(index.search(&ScalarValue::Int(0)).is_none());
    /// ```
    pub fn search(&self, key: &ScalarValue) -> Option<&[Tuple]> {
        self.index.get(key).map(|v| v.as_slice())
    }

    /// Removes a specific tuple for a key.
    ///
    /// If the key has multiple tuples, only the matching tuple is removed.
    /// If this was the last tuple for the key, the key is also removed.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to look up
    /// * `tuple` - The specific tuple to remove
    ///
    /// # Returns
    ///
    /// `true` if the tuple was found and removed, `false` otherwise.
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

    /// Deletes all entries for a key.
    ///
    /// Removes the key and all associated tuples from the index.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to delete
    ///
    /// # Returns
    ///
    /// `true` if the key existed and was deleted, `false` otherwise.
    pub fn delete(&mut self, key: &ScalarValue) -> bool {
        self.index.remove(key).is_some()
    }

    /// Performs a range scan for keys in the inclusive range `[start, end]`.
    ///
    /// Returns all (key, tuple) pairs where `start <= key <= end`, in
    /// sorted order by key.
    ///
    /// # Arguments
    ///
    /// * `start` - The lower bound of the range (inclusive)
    /// * `end` - The upper bound of the range (inclusive)
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_storage::storage::BTreeIndex;
    /// use relvar_core::values::ScalarValue;
    /// use relvar_core::tuple;
    ///
    /// let mut index = BTreeIndex::new();
    /// index.insert(ScalarValue::Int(10), tuple! { id: 10i64 });
    /// index.insert(ScalarValue::Int(20), tuple! { id: 20i64 });
    /// index.insert(ScalarValue::Int(30), tuple! { id: 30i64 });
    ///
    /// let results = index.range_scan(&ScalarValue::Int(15), &ScalarValue::Int(25));
    /// assert_eq!(results.len(), 1); // Only key 20 is in range
    /// ```
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

    /// Returns the number of unique keys in the index.
    pub fn key_count(&self) -> usize {
        self.index.len()
    }

    /// Returns the total number of tuple entries in the index.
    ///
    /// This may be larger than `key_count()` if keys have multiple tuples.
    pub fn entry_count(&self) -> usize {
        self.index.values().map(|v| v.len()).sum()
    }

    /// Returns `true` if the index contains no entries.
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Saves the index to a file.
    ///
    /// The index is serialized using bincode and written to the specified path.
    ///
    /// # Errors
    ///
    /// Returns [`BTreeIndexError::Io`] if the file cannot be written.
    /// Returns [`BTreeIndexError::Serialization`] if serialization fails.
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

    /// Loads an index from a file.
    ///
    /// If the file is empty, returns an empty index.
    ///
    /// # Errors
    ///
    /// Returns [`BTreeIndexError::Io`] if the file cannot be read.
    /// Returns [`BTreeIndexError::Serialization`] if deserialization fails.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, BTreeIndexError> {
        Self::load_with_limit(path, MAX_INDEX_SIZE)
    }

    fn load_with_limit<P: AsRef<Path>>(path: P, limit: u64) -> Result<Self, BTreeIndexError> {
        let file = File::open(path)?;
        let mut reader = file.take(limit + 1);
        let mut contents = Vec::new();
        reader.read_to_end(&mut contents)?;

        if contents.len() as u64 > limit {
            return Err(BTreeIndexError::Serialization(
                "Index file too large".to_string(),
            ));
        }

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
    use relvar_core::tuple;
    use relvar_core::values::ScalarValue;
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

    #[test]
    fn test_btree_load_limit() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Write 20 bytes
        {
            let mut file = File::create(path).unwrap();
            file.write_all(&[b'a'; 20]).unwrap();
        }

        // Try to load with limit 10
        let result = BTreeIndex::load_with_limit(path, 10);
        assert!(result.is_err());
        match result.unwrap_err() {
            BTreeIndexError::Serialization(msg) => {
                assert!(msg.contains("Index file too large"));
            }
            err => panic!("Expected Serialization error, got {:?}", err),
        }
    }
}
