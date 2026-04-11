## [Reduction]
**Bloat:** Empty `ImageProcessor` and `Tokenizer` structs acting purely as namespaces for static methods (Enterprise FizzBuzz "God Struct" style).
**Cut:** Removed the empty structs and `impl` blocks, changing the static methods into module-level free functions. Updated usage sites.
**Saved:** Reduced boilerplate, improved clarity, explicit module-level function calls instead of Object-Oriented faux namespaces.

## [Reduction]
**Bloat:** `OperationType` enum with only `Insert` and `Update` variants in `relvar-storage/src/storage/heap.rs`.
**Cut:** Replaced `OperationType` enum with a simple `bool` flag (`is_update: bool`) since it only had two states.
**Saved:** Removed enum definition and simplified pattern matching.

## [Reduction]
**Bloat:** `PrimaryKey` struct that was simply a single-field wrapper around `CandidateKey` with all its methods explicitly passed through to the wrapped type. This was a classic "Layer Lasagna" / redundant type abstraction.
**Cut:** Removed the `PrimaryKey` struct entirely. Replaced `primary_key: Option<PrimaryKey>` with `primary_key: Option<CandidateKey>` inside `KeyConstraints`. Refactored all usage sites (tests, `visualizer.rs`, etc.) to use `CandidateKey` directly as the primary key.
**Saved:** ~30 lines of boilerplate code and cognitive load involved in jumping between identical types.
