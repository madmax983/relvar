import os
for root, _, files in os.walk("relvar-storage/src/mvcc/"):
    for file in files:
        if file.endswith('.rs'):
            filepath = os.path.join(root, file)
            with open(filepath, 'r') as f:
                content = f.read()
            content = content.replace("```rust,ignore\n", "```ignore\n")
            with open(filepath, 'w') as f:
                f.write(content)
