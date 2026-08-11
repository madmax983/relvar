with open("relvar-storage/src/storage/page.rs", "r") as f:
    content = f.read()

content = content.replace("use relvar_storage::storage::{Page, PageFile, PAGE_SIZE};", "use relvar_storage::storage::{Page, PageFile};")
content = content.replace("use relvar_storage::storage::{Page, PAGE_SIZE};", "use relvar_storage::storage::Page;")

with open("relvar-storage/src/storage/page.rs", "w") as f:
    f.write(content)
