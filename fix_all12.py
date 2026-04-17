import re

with open('relvar-storage/src/storage/heap.rs', 'r') as f:
    content = f.read()

# Fix duplicated attributes
content = re.sub(r'#\[allow\(unused_imports\)\]\n        #\[allow\(unused_imports\)\]', '#[allow(unused_imports)]', content)
content = re.sub(r'    #\[allow\(unused_imports\)\]\n        #\[allow\(unused_imports\)\]', '    #[allow(unused_imports)]', content)

with open('relvar-storage/src/storage/heap.rs', 'w') as f:
    f.write(content)
