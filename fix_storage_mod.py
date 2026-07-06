with open("relvar-storage/src/storage/mod.rs", "r") as f:
    text = f.read()

# Remove unused imports from mod.rs
text = text.replace("pub use catalog::{Catalog, CatalogError, RelationMetadata};", "pub use catalog::{Catalog, CatalogError};")
text = text.replace("pub use page::{PAGE_SIZE, Page, PageError, PageFile, PageId};", "pub use page::{Page, PageError, PageFile};")

with open("relvar-storage/src/storage/mod.rs", "w") as f:
    f.write(text)
