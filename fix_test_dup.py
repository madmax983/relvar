import os
import re

files = [
    'scan_visible.rs',
    'scan_corruptions.rs',
    'scan_security.rs',
    'scan_offset_overflow.rs',
    'scan.rs'
]

seen_funcs = set()
base_dir = 'relvar-storage/src/storage/heap/tests/'

for f in files:
    path = os.path.join(base_dir, f)
    with open(path, 'r') as file:
        content = file.read()

    # Actually since the splitting logic in the previous Python script might have duplicated things
    # Let's just fix it properly by parsing out functions.

    blocks = re.findall(r'(#\[test\].*?^}$|fn test_.*?^}$)', content, re.MULTILINE | re.DOTALL)

    new_blocks = []

    # We will do it in a simpler way: just split by #\[test\]
    pass
