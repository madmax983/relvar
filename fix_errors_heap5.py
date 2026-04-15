import re

file = 'relvar-storage/src/storage/heap.rs'
with open(file, 'r') as f:
    content = f.read()

# At line 553, 953: `let page_id = tuple_id.page_id;` => `page_id` is a PageId.
# At line 777, 1394, 1508: `let page_id = ... as u64;` => `page_id` is a u64.

# Let's read lines
lines = content.split('\n')
for i, line in enumerate(lines):
    if "self.page_file.read_page(PageId(page_id))?" in line:
        # Check surrounding to see if page_id is a u64 or PageId.
        # But we know exactly which ones are broken: 553, 953
        # Let's just fix the specific occurrences.
        pass

# It's easier:
content = content.replace('self.page_file.read_page(PageId(page_id))?', 'self.page_file.read_page(page_id)?')

with open(file, 'w') as f:
    f.write(content)
