import re

def delete_blocks(filepath, blocks):
    with open(filepath, 'r') as f:
        content = f.read()

    patterns = [
        r'    /// Read a tuple by its TupleId \(internal use only per TTM Proscription 6\)\n    ///\n    /// NOTE: Currently unused - reserved for future MVCC transaction implementation\.\n    #\[allow\(dead_code\)\]\n    pub\(crate\) fn read_tuple\(.*?Ok\(tuple\)\n    \}\n',
        r'    /// Reads a tuple from a versioned page\.\n    ///\n    /// Used internally by MVCC operations\.\n    ///\n    /// NOTE: Currently unused - reserved for future MVCC transaction implementation\.\n    #\[allow\(dead_code\)\]\n    pub\(crate\) fn read_tuple_versioned\(.*?Ok\(tuple\)\n    \}\n',
        r'    /// Updates a tuple in a versioned page\.\n    ///\n    /// This performs an out-of-place update, marking the old version as deleted\n    /// by the current transaction and inserting a new version with the new data\.\n    /// Used internally by MVCC operations\.\n    ///\n    /// NOTE: Currently unused - reserved for future MVCC transaction implementation\.\n    #\[allow\(dead_code\)\]\n    pub\(crate\) fn update_tuple_versioned\(.*?\}\n    \}\n',
        r'    /// Deletes a tuple from a versioned page\.\n    ///\n    /// Used internally by MVCC operations\.\n    ///\n    /// NOTE: Currently unused - reserved for future MVCC transaction implementation\.\n    #\[allow\(dead_code\)\]\n    pub\(crate\) fn delete_tuple_versioned\(.*?Ok\(\(\)\)\n    \}\n',
        r'    /// Scans all visible tuples for a given transaction snapshot\.\n    ///\n    /// # Arguments\n    /// \* `snapshot` - Transaction snapshot determining visibility\n    /// \* `committed` - Set of all committed transaction IDs\n    ///\n    /// # Returns\n    /// Vector of visible tuples\n    ///\n    /// # Errors\n    /// Returns \[`HeapError::Serialization`\] if tuples cannot be deserialized\.\n    /// Returns \[`HeapError::Page`\] if page I/O error occurs\.\n    ///\n    /// NOTE: Currently unused - reserved for future MVCC transaction implementation\.\n    #\[allow\(dead_code\)\]\n    pub\(crate\) fn scan_visible\(.*?Ok\(results\)\n    \}\n',

        r'    #\[allow\(dead_code\)\]\n    fn test_txn\(value: u64\) -> crate::wal::TransactionId \{\n        crate::wal::TransactionId::new\(value\)\n    \}\n',
        r'        #\[allow\(dead_code\)\]\n        fn test_txn\(value: u64\) -> crate::wal::TransactionId \{\n            crate::wal::TransactionId::new\(value\)\n        \}\n',

        r'    /// Loads a relation using a specific transaction snapshot\.\n    ///\n    /// NOTE: Currently unused - reserved for future MVCC transaction implementation\.\n    #\[allow\(dead_code\)\]\n    pub\(crate\) fn load_relation_for_txn\(.*?\n    \}\n',
        r'    /// Inserts a tuple as part of a specific transaction\.\n    ///\n    /// NOTE: Currently unused - reserved for future MVCC transaction implementation\.\n    #\[allow\(dead_code\)\]\n    pub\(crate\) fn insert_tuple_in_txn\(.*?\n    \}\n'
    ]

    for p in patterns:
        content = re.sub(p, '', content, flags=re.DOTALL)

    with open(filepath, 'w') as f:
        f.write(content)

delete_blocks('relvar-storage/src/storage/heap.rs', [])
delete_blocks('relvar-storage/src/persistent_engine.rs', [])
