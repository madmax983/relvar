with open("relvar-storage/src/storage/mod.rs", "r") as f:
    text = f.read()

text = text.replace("pub use page::{Page, PageError, PageFile, PAGE_SIZE};", "pub use page::{Page, PageError, PageFile};")

with open("relvar-storage/src/storage/mod.rs", "w") as f:
    f.write(text)
