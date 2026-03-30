import re

def process_file(filepath, replacements):
    with open(filepath, 'r') as f:
        content = f.read()

    for old, new in replacements:
        if old in content:
            content = content.replace(old, new)
        else:
            print(f"Warning: Could not find '{old}' in {filepath}")

    with open(filepath, 'w') as f:
        f.write(content)

process_file("relvar-storage/src/wal/error.rs", [
    (
        "/// use relvar_storage::wal::WalError;",
        "/// use relvar_storage::wal::error::WalError;"
    )
])

process_file("relvar-storage/src/wal/record.rs", [
    (
        "/// use relvar_storage::wal::{WalRecord, TransactionId};",
        "/// use relvar_storage::wal::record::WalRecord;\n/// use relvar_storage::wal::lsn::TransactionId;"
    ),
    (
        "/// use relvar_storage::wal::WalRecordError;",
        "/// use relvar_storage::wal::record::WalRecordError;"
    )
])
