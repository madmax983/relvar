with open("relvar-storage/benches/mvcc.rs", "r") as f:
    content = f.read()

content = content.replace(
    "|(engine, _temp_dir)| {",
    "|(engine, _temp_dir): (PersistentEngine, tempfile::TempDir)| {"
)

content = content.replace(
    "|(mut engine, relation, _temp_dir)| {",
    "|(mut engine, relation, _temp_dir): (PersistentEngine, relvar_core::values::Relation, tempfile::TempDir)| {"
)

content = content.replace(
    "|(mut engine, _temp_dir)| {",
    "|(mut engine, _temp_dir): (PersistentEngine, tempfile::TempDir)| {"
)

with open("relvar-storage/benches/mvcc.rs", "w") as f:
    f.write(content)
