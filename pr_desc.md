📖 Chapter: The Core Constraints module and utilities.
🔦 Insight: Several core constraint structures and errors (`CheckConstraint`, `TypeConstraint`, `KeyConstraint`, `ForeignKeyConstraint` error enumerations and configurations) were lacking complete, compileable doc-tests showing correct instantiation and evaluation contexts.
🧪 Example: Added multiple executable doctests across core constraints covering both structs and error enums. Cleaned up multiple files masking examples behind broken ignore headers.
🖼️ Preview: Evaluated by `cargo doc --open`.
