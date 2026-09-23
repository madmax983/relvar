with open("relvar-storage/src/storage/mod.rs", "r") as f:
    text = f.read()

text = text.replace("pub use page::{Page, PageError, PageFile};", "pub use page::{Page, PageError, PageFile, PAGE_SIZE};")

with open("relvar-storage/src/storage/mod.rs", "w") as f:
    f.write(text)
