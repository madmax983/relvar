with open("relvar-storage/src/storage/catalog.rs", 'r') as f:
    content = f.read()

content = content.replace("/// - `heap_file_path` - Path to the heap file containing the relation's tuples\n#[derive(Debug, Clone, Serialize, Deserialize)]\n/// Metadata for a stored relation (relvar).", "/// - `heap_file_path` - Path to the heap file containing the relation's tuples\n#[derive(Debug, Clone, Serialize, Deserialize)]")

with open("relvar-storage/src/storage/catalog.rs", 'w') as f:
    f.write(content)
