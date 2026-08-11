with open("relvar-storage/src/storage/page.rs", "r") as f:
    content = f.read()

content = content.replace(
    "assert_eq!(page.available_space(), PAGE_SIZE);",
    "assert_eq!(page.available_space(), relvar_storage::storage::page::PAGE_SIZE);"
)

with open("relvar-storage/src/storage/page.rs", "w") as f:
    f.write(content)
