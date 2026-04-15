import re

file = 'relvar-storage/src/storage/heap.rs'
with open(file, 'r') as f:
    content = f.read()

content = content.replace('self.page_file.read_page(page_id)?', 'self.page_file.read_page(PageId(page_id))?')
content = content.replace('Page::from_data(i, page_data)', 'Page::from_data(PageId(i), page_data)')

with open(file, 'w') as f:
    f.write(content)
