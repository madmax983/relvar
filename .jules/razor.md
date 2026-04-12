## [Reduction]
**Bloat:** Empty `ImageProcessor` and `Tokenizer` structs acting purely as namespaces for static methods (Enterprise FizzBuzz "God Struct" style).
**Cut:** Removed the empty structs and `impl` blocks, changing the static methods into module-level free functions. Updated usage sites.
**Saved:** Reduced boilerplate, improved clarity, explicit module-level function calls instead of Object-Oriented faux namespaces.

## [Reduction]
**Bloat:** `OperationType` enum with only `Insert` and `Update` variants in `relvar-storage/src/storage/heap.rs`.
**Cut:** Replaced `OperationType` enum with a simple `bool` flag (`is_update: bool`) since it only had two states.
**Saved:** Removed enum definition and simplified pattern matching.

## [Reduction]
**Bloat:** `DepthGuarded<T>` struct wrapper used to prevent recursive serialization/deserialization panics. Added nesting and `.0` boilerplate when unpacking scalars.
**Cut:** Removed the wrapper struct, replacing it with a custom `deserialize_guarded` function applied via `#[serde(deserialize_with = "...")]` to directly flatten recursive structures.
**Saved:** Reduced boilerplate wrapper types, streamlined Serde parsing logic, and eliminated intermediate `.0` tuple accesses.

## [Reduction]
**Bloat:** Redundant per-tuple type validation using `self.insert()` in a loop inside `union_into` when relation headings are already explicitly verified to match.
**Cut:** Exposed `body` as `pub(crate)` in `Relation` and replaced the loop with `.reserve()` and `.extend()` to directly insert tuples into the underlying `HashSet`.
**Saved:** Eliminated O(N) redundant validations and reduced overhead in relational algebra merge operations.
