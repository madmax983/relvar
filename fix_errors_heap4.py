import re

file = 'relvar-storage/src/storage/heap.rs'
with open(file, 'r') as f:
    content = f.read()

content = content.replace('self.page_file.read_page(PageId(page_id))?', 'self.page_file.read_page(page_id)?')
# Oh wait, we just introduced an issue where page_id was ALREADY a PageId in those lines.
# So I need to use regex to handle only when page_id is a u64.
# Let's revert the naive replace and only fix lines 777, 1394, 1508 where page_id is a u64, and NOT 553, 953 where it's a PageId.
