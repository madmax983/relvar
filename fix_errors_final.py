import re

file = 'relvar-storage/src/storage/heap.rs'
with open(file, 'r') as f:
    content = f.read()

# fix the regex mess ups
content = content.replace('PageId(1)34', '134')
content = content.replace('PageId(1)00', '100')
content = content.replace('self.page_file.read_page(PageId(page_id))?', 'self.page_file.read_page(page_id)?')
# actually wait, earlier we wanted page_id to be a PageId. If it is already a PageId, read_page(page_id) works.
# in heap.rs 553, 953 page_id is a PageId because it came from tuple_id.page_id.

with open(file, 'w') as f:
    f.write(content)
