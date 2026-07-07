import re

with open("relvar-storage/src/storage/mod.rs", "r") as f:
    content = f.read()

content = content.replace("pub use page::{Page, PageError, PageFile};", "pub use page::{PAGE_SIZE, Page, PageError, PageFile, PageId};")
content = content.replace("pub use catalog::{Catalog, CatalogError};", "pub use catalog::{Catalog, CatalogError, RelationMetadata};")

with open("relvar-storage/src/storage/mod.rs", "w") as f:
    f.write(content)

print("Done")
