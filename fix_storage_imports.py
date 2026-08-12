import sys

filepath = 'relvar-storage/src/storage/mod.rs'
with open(filepath, 'r') as f:
    content = f.read()

# Add #[allow(unused_imports)] to the PAGE_SIZE
content = content.replace(
    'pub use page::{PAGE_SIZE, Page, PageError, PageFile};',
    '#[allow(unused_imports)]\npub use page::{PAGE_SIZE, Page, PageError, PageFile};'
)

with open(filepath, 'w') as f:
    f.write(content)
