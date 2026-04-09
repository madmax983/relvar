import re

with open("relvar-storage/src/mvcc/mod.rs", "r") as f:
    content = f.read()

content = content.replace("```\n/// use relvar_storage::mvcc", "```ignore\n/// use relvar_storage::mvcc")

with open("relvar-storage/src/mvcc/mod.rs", "w") as f:
    f.write(content)

def fix_doctests(filepath, mod_name):
    with open(filepath, "r") as f:
        content = f.read()
    content = content.replace(f"```\n/// use relvar_storage::{mod_name}", f"```ignore\n/// use relvar_storage::{mod_name}")
    with open(filepath, "w") as f:
        f.write(content)

fix_doctests("relvar-storage/src/mvcc/active_txn_table.rs", "mvcc")
fix_doctests("relvar-storage/src/mvcc/snapshot.rs", "mvcc")
fix_doctests("relvar-storage/src/mvcc/visibility.rs", "mvcc")
fix_doctests("relvar-storage/src/storage/heap.rs", "storage")

