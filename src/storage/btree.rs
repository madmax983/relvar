use crate::storage::heap::TupleId;
use crate::values::ScalarValue;
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

/// A simple B-tree index mapping scalar values to tuple IDs.
/// This is a simplified implementation using Rust's BTreeMap for now.
/// A production system would implement a proper on-disk B-tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BTreeIndex {
    /// Map from key value to list of tuple IDs
    index: BTreeMap<ScalarValue, Vec<TupleId>>,
}

impl BTreeIndex {
    /// Create a new empty index
    pub fn new() -> Self {
        Self {
            index: BTreeMap::new(),
        }
    }

    /// Insert a key-value pair
    pub fn insert(&mut self, key: ScalarValue, tuple_id: TupleId) {
        self.index.entry(key).or_default().push(tuple_id);
    }

    /// Search for a key and return all matching tuple IDs
    pub fn search(&self, key: &ScalarValue) -> Option<&[TupleId]> {
        self.index.get(key).map(|v| v.as_slice())
    }

    /// Remove a key-value pair
    pub fn remove(&mut self, key: &ScalarValue, tuple_id: TupleId) -> bool {
        if let Some(tuple_ids) = self.index.get_mut(key)
            && let Some(pos) = tuple_ids.iter().position(|&tid| tid == tuple_id) {
                tuple_ids.swap_remove(pos);
                if tuple_ids.is_empty() {
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
    pub fn range_scan(
        &self,
        start: &ScalarValue,
        end: &ScalarValue,
    ) -> Vec<(ScalarValue, TupleId)> {
        let mut results = Vec::new();

        for (key, tuple_ids) in self.index.range(start.clone()..=end.clone()) {
            for &tuple_id in tuple_ids {
                results.push((key.clone(), tuple_id));
            }
        }

        results
    }

    /// Get the number of unique keys
    pub fn key_count(&self) -> usize {
        self.index.len()
    }

    /// Get the total number of entries (including duplicates)
    pub fn entry_count(&self) -> usize {
        self.index.values().map(|v| v.len()).sum()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Save index to a file
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

    /// Load index from a file
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
    use crate::values::ScalarValue;
    use tempfile::NamedTempFile;

    #[test]
    fn test_btree_insert_and_search() {
        let mut index = BTreeIndex::new();

        let key = ScalarValue::Int(42);
        let tuple_id = TupleId {
            page_id: 0,
            slot: 0,
        };

        index.insert(key.clone(), tuple_id);

        let results = index.search(&key).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], tuple_id);
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
        let tid1 = TupleId {
            page_id: 0,
            slot: 0,
        };
        let tid2 = TupleId {
            page_id: 0,
            slot: 1,
        };
        let tid3 = TupleId {
            page_id: 1,
            slot: 0,
        };

        index.insert(key.clone(), tid1);
        index.insert(key.clone(), tid2);
        index.insert(key.clone(), tid3);

        let results = index.search(&key).unwrap();
        assert_eq!(results.len(), 3);
        assert!(results.contains(&tid1));
        assert!(results.contains(&tid2));
        assert!(results.contains(&tid3));
    }

    #[test]
    fn test_btree_remove() {
        let mut index = BTreeIndex::new();

        let key = ScalarValue::Int(42);
        let tid1 = TupleId {
            page_id: 0,
            slot: 0,
        };
        let tid2 = TupleId {
            page_id: 0,
            slot: 1,
        };

        index.insert(key.clone(), tid1);
        index.insert(key.clone(), tid2);

        assert!(index.remove(&key, tid1));

        let results = index.search(&key).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], tid2);
    }

    #[test]
    fn test_btree_delete() {
        let mut index = BTreeIndex::new();

        let key = ScalarValue::Int(42);
        let tid1 = TupleId {
            page_id: 0,
            slot: 0,
        };
        let tid2 = TupleId {
            page_id: 0,
            slot: 1,
        };

        index.insert(key.clone(), tid1);
        index.insert(key.clone(), tid2);

        assert!(index.delete(&key));
        assert!(index.search(&key).is_none());
    }

    #[test]
    fn test_btree_range_scan() {
        let mut index = BTreeIndex::new();

        index.insert(
            ScalarValue::Int(10),
            TupleId {
                page_id: 0,
                slot: 0,
            },
        );
        index.insert(
            ScalarValue::Int(20),
            TupleId {
                page_id: 0,
                slot: 1,
            },
        );
        index.insert(
            ScalarValue::Int(30),
            TupleId {
                page_id: 0,
                slot: 2,
            },
        );
        index.insert(
            ScalarValue::Int(40),
            TupleId {
                page_id: 0,
                slot: 3,
            },
        );
        index.insert(
            ScalarValue::Int(50),
            TupleId {
                page_id: 0,
                slot: 4,
            },
        );

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

        index.insert(
            ScalarValue::Int(10),
            TupleId {
                page_id: 0,
                slot: 0,
            },
        );
        index.insert(
            ScalarValue::Int(10),
            TupleId {
                page_id: 0,
                slot: 1,
            },
        );
        index.insert(
            ScalarValue::Int(20),
            TupleId {
                page_id: 0,
                slot: 2,
            },
        );

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
            index.insert(
                ScalarValue::Int(10),
                TupleId {
                    page_id: 0,
                    slot: 0,
                },
            );
            index.insert(
                ScalarValue::Int(20),
                TupleId {
                    page_id: 0,
                    slot: 1,
                },
            );
            index.insert(
                ScalarValue::Int(30),
                TupleId {
                    page_id: 0,
                    slot: 2,
                },
            );

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
            TupleId {
                page_id: 0,
                slot: 0,
            },
        );
        index.insert(
            ScalarValue::String("Bob".to_string()),
            TupleId {
                page_id: 0,
                slot: 1,
            },
        );
        index.insert(
            ScalarValue::String("Charlie".to_string()),
            TupleId {
                page_id: 0,
                slot: 2,
            },
        );

        let results = index.range_scan(
            &ScalarValue::String("Alice".to_string()),
            &ScalarValue::String("Charlie".to_string()),
        );

        assert_eq!(results.len(), 3);
    }
}
