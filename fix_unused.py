import re

with open("relvar-storage/src/storage/mod.rs", "r") as f:
    content = f.read()

content = content.replace("pub use catalog::{Catalog, CatalogError, RelationMetadata};", "pub use catalog::{Catalog, CatalogError};")
content = content.replace("pub use page::{PAGE_SIZE, Page, PageError, PageFile, PageId};", "pub use page::{Page, PageError};")

with open("relvar-storage/src/storage/mod.rs", "w") as f:
    f.write(content)
