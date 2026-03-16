import re

with open('relvar-storage/src/storage/heap.rs', 'r') as f:
    content = f.read()

# Fix usage of repack_slots and extract_all_tuples
content = content.replace('self.extract_tuples_from_slots(&page, versioned_page.slots.iter().flatten())?', 'self.extract_tuples_from_versioned_slots(&page, versioned_page.slots.iter().flatten())?')
content = content.replace('self.extract_all_tuples(&page, &vp.slots)?', 'self.extract_all_versioned_tuples(&page, &vp.slots)?')
content = content.replace('Self::repack_slots(\n            &mut versioned_page.slots,\n            &existing_tuples,\n            USABLE_PAGE_SIZE_V2,\n        )?', 'Self::repack_versioned_slots(\n            &mut versioned_page.slots,\n            &existing_tuples,\n            USABLE_PAGE_SIZE_V2,\n        )?')
content = content.replace('self.extract_all_tuples(&page, &versioned_page.slots)?', 'self.extract_all_versioned_tuples(&page, &versioned_page.slots)?')
content = content.replace('Self::repack_slots(\n                    &mut versioned_page.slots,\n                    &existing_tuples,\n                    USABLE_PAGE_SIZE_V2,\n                )?', 'Self::repack_versioned_slots(\n                    &mut versioned_page.slots,\n                    &existing_tuples,\n                    USABLE_PAGE_SIZE_V2,\n                )?')

with open('relvar-storage/src/storage/heap.rs', 'w') as f:
    f.write(content)
