import re

files_to_fix = [
    'relvar-storage/src/storage/page.rs',
]

for file in files_to_fix:
    with open(file, 'r') as f:
        content = f.read()

    # implement Display for PageId
    if 'impl std::fmt::Display for PageId' not in content:
        content = content.replace(
            'pub struct PageId(pub u64);',
            'pub struct PageId(pub u64);\n\nimpl std::fmt::Display for PageId {\n    fn fmt(&self, f: &mut std::fmt::Formatter<\\\'_>) -> std::fmt::Result {\n        write!(f, "{}", self.0)\n    }\n}\n'.replace('\\\'', "'")
        )

    with open(file, 'w') as f:
        f.write(content)
