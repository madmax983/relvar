with open("relvar-storage/benches/mvcc.rs", "r") as f:
    lines = f.readlines()

for i, line in enumerate(lines):
    if "let engine = PersistentEngine::open" in line:
        lines[i] = line.replace("let engine", "let mut engine")
    if "|(mut engine, _temp_dir): (PersistentEngine, _)|" in line:
        if "load_relation_for_txn" in lines[i+1] or "load_relation_for_txn" in lines[i+2] or "load_relation_for_txn" in lines[i+3] or "load_relation" in lines[i+2] or "load_relation" in lines[i+3]:
            # This is a read operation
            lines[i] = line.replace("mut engine", "engine")

with open("relvar-storage/benches/mvcc.rs", "w") as f:
    f.writelines(lines)
