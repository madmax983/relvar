import re

with open('relvar-storage/src/storage/manager.rs', 'r') as f:
    content = f.read()

# Only replace the specific unused ones
content = re.sub(
    r'    pub fn scan_relation\(\n        &mut self,\n        name: &str,\n        _snapshot: &TransactionSnapshot,\n        _committed_txns: &HashSet<TransactionId>,\n    \) -> Result<Relation, StorageError> \{',
    r'    pub fn scan_relation(\n        &mut self,\n        name: &str,\n        _snapshot: &TransactionSnapshot,\n        _committed_txns: &HashSet<TransactionId>,\n    ) -> Result<Relation, StorageError> {',
    content
)

# Fix the garbage collect one that we incorrectly renamed
content = content.replace("gc_remove_dead_versions(gc_lsn, _committed_txns)", "gc_remove_dead_versions(gc_lsn, committed_txns)")
content = content.replace("    pub fn garbage_collect_versions(\n        &mut self,\n        gc_lsn: crate::wal::Lsn,\n        _committed_txns: &HashSet<TransactionId>,\n    )", "    pub fn garbage_collect_versions(\n        &mut self,\n        gc_lsn: crate::wal::Lsn,\n        committed_txns: &HashSet<TransactionId>,\n    )")

with open('relvar-storage/src/storage/manager.rs', 'w') as f:
    f.write(content)
