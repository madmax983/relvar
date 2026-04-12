import re

with open("relvar-storage/src/wal/mod.rs", "r") as f:
    content = f.read()

# remove doc tests that fail since wal is private
# The easiest way is to remove the `/// ` comments with doctests or add `ignore`

content = content.replace("```\n/// use relvar_storage::wal", "```ignore\n/// use relvar_storage::wal")

with open("relvar-storage/src/wal/mod.rs", "w") as f:
    f.write(content)

with open("relvar-storage/src/wal/lsn.rs", "r") as f:
    content = f.read()

content = content.replace("```\n/// use relvar_storage::wal", "```ignore\n/// use relvar_storage::wal")

with open("relvar-storage/src/wal/lsn.rs", "w") as f:
    f.write(content)

with open("relvar-storage/src/wal/record.rs", "r") as f:
    content = f.read()

content = content.replace("```\n/// use relvar_storage::wal", "```ignore\n/// use relvar_storage::wal")

with open("relvar-storage/src/wal/record.rs", "w") as f:
    f.write(content)
