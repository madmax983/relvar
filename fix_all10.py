import re

with open('relvar-storage/src/storage/manager.rs', 'r') as f:
    content = f.read()

content = content.replace("snapshot: &TransactionSnapshot,", "_snapshot: &TransactionSnapshot,")
content = content.replace("committed_txns: &HashSet<TransactionId>,", "_committed_txns: &HashSet<TransactionId>,")

with open('relvar-storage/src/storage/manager.rs', 'w') as f:
    f.write(content)

with open('relvar-storage/src/storage/heap.rs', 'r') as f:
    content = f.read()

content = content.replace("use relvar_core::tuple;\n", "#[allow(unused_imports)]\n        use relvar_core::tuple;\n")

with open('relvar-storage/src/storage/heap.rs', 'w') as f:
    f.write(content)
