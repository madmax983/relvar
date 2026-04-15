import re

files_to_fix = [
    'relvar-storage/src/storage/heap.rs',
]

for file in files_to_fix:
    with open(file, 'r') as f:
        content = f.read()

    # 535: match insert_fn(self, page_id)
    content = content.replace('match insert_fn(self, page_id) {', 'match insert_fn(self, PageId(page_id)) {')
    # 777, 1394, 1508: self.page_file.read_page(page_id)
    content = content.replace('self.page_file.read_page(page_id)?', 'self.page_file.read_page(PageId(page_id))?')
    # 1405: self.gc_process_page(page_id, &page
    content = content.replace('self.gc_process_page(page_id, &page', 'self.gc_process_page(PageId(page_id), &page')

    with open(file, 'w') as f:
        f.write(content)
