import re

file = 'relvar-storage/src/storage/heap.rs'
with open(file, 'r') as f:
    content = f.read()

content = content.replace('PageId(0)); // Nothing removed', '0); // Nothing removed')
content = content.replace('assert_eq!(removed, PageId(0));', 'assert_eq!(removed, 0);')
content = content.replace('assert_eq!(visible.len(), PageId(0));', 'assert_eq!(visible.len(), 0);')
content = content.replace('assert_eq!(visible.len(), PageId(1));', 'assert_eq!(visible.len(), 1);')
content = content.replace('assert_eq!(removed1, PageId(1));', 'assert_eq!(removed1, 1);')
content = content.replace('assert_eq!(removed2, PageId(0));', 'assert_eq!(removed2, 0);')
content = content.replace('assert_eq!(removed, PageId(1));', 'assert_eq!(removed, 1);')
content = content.replace('PageId(1)00', '100')
content = content.replace('[195, PageId(1)34, 217, 234, 4]', '[195, 134, 217, 234, 4]')
content = content.replace('assert_eq!(slots[0].as_ref().unwrap().offset, PageId(0));', 'assert_eq!(slots[0].as_ref().unwrap().offset, 0);')
content = content.replace('vec![PAGE_FORMAT_VERSION, PageId(0), PageId(0)]', 'vec![PAGE_FORMAT_VERSION, 0, 0]')

with open(file, 'w') as f:
    f.write(content)
