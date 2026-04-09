import re

with open("relvar-storage/src/lib.rs", "r") as f:
    content = f.read()

content = content.replace("use relvar_storage::wal::Lsn;", "")
content = content.replace("use relvar_storage::wal::TransactionId;", "")
content = content.replace("use relvar_storage::wal::{TransactionId, TransactionIdGenerator};", "")
content = content.replace("use relvar_storage::wal::TransactionIdGenerator;", "")
content = content.replace("use relvar_storage::wal::{WalRecord, TransactionId};", "")
content = content.replace("use relvar_storage::wal::{WalRecord, TransactionId}", "")
content = content.replace("pub mod wal;", "pub(crate) mod wal;")

with open("relvar-storage/src/lib.rs", "w") as f:
    f.write(content)
