import re

file = 'relvar-storage/src/storage/heap.rs'
with open(file, 'r') as f:
    content = f.read()

content = content.replace('assert_eq!(tuples.len(), PageId(1));', 'assert_eq!(tuples.len(), 1);')
content = content.replace('assert_eq!(page.slots.len(), PageId(0));', 'assert_eq!(page.slots.len(), 0);')
content = content.replace('assert_eq!(tuple_id.slot, PageId(0));', 'assert_eq!(tuple_id.slot, 0);')
content = content.replace('assert_eq!(tid1.slot, PageId(0));', 'assert_eq!(tid1.slot, 0);')
content = content.replace('assert_eq!(tid2.slot, PageId(1));', 'assert_eq!(tid2.slot, 1);')
content = content.replace('assert!(tuple_id.slot < PageId(1000));', 'assert!(tuple_id.slot < 1000);')
content = content.replace('assert_eq!(tid3.slot, PageId(2));', 'assert_eq!(tid3.slot, 2);')
content = content.replace('assert_eq!(tid1.slot, PageId(0));', 'assert_eq!(tid1.slot, 0);')
content = content.replace('assert_eq!(tid2.slot, PageId(0));', 'assert_eq!(tid2.slot, 0);')

with open(file, 'w') as f:
    f.write(content)
