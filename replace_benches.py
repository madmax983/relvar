with open("relvar-storage/benches/storage.rs", "r") as f:
    content = f.read()

content = content.replace(
    "|(mut page_file, page, _temp_file)| {",
    "|(mut page_file, page, _temp_file): (PageFile, Page, tempfile::NamedTempFile)| {"
)

content = content.replace(
    "|(mut heap, _temp_file)| {",
    "|(mut heap, _temp_file): (HeapFile, tempfile::NamedTempFile)| {"
)

with open("relvar-storage/benches/storage.rs", "w") as f:
    f.write(content)
