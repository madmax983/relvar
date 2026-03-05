# Bard's Journal 🎻

## 2024-05-23 - Rename Operator Collision Behavior
**Confusion:** When renaming multiple attributes to the same target name (e.g., A -> C, B -> C), the behavior was "magic" and undocumented.
**Clarification:** The `rename` operator iterates over source attributes in lexicographical order (due to `BTreeMap`). If multiple attributes map to the same target name, the last one visited overwrites previous ones. This is "Last Write Wins" behavior based on source attribute name order.

## 2024-05-24 - The Missing Workspace

**Confusion:** The root `README.md` described a monolithic `src/` directory structure, but the project is actually a Cargo workspace with `relvar-core`, `relvar-storage`, and `relvar` crates. This made it impossible for a new user to find the source files mentioned in the architecture diagram.

**Clarification:** I've updated the `README.md` to explicitly describe the workspace architecture:
- `relvar-core`: Pure logic (types, algebra)
- `relvar-storage`: Persistence (heap files, pages)
- `relvar`: Facade crate that re-exports the core API

This ensures the map matches the territory.

## 2024-05-24 - Project Operator Silently Ignores Attributes
**Confusion:** Users might expect `relation.project(&["non_existent"])` to panic or return a Result::Err.
**Clarification:** The `project` operator follows set intersection logic. It projects the relation onto the intersection of the relation's heading and the requested attributes. If a requested attribute doesn't exist, it is simply not in the result. Asking for a set of non-existent attributes correctly returns a relation with an empty heading (TABLE_DEE or TABLE_DUM), which is mathematically consistent but potentially surprising to SQL users.

## 2024-05-24 - Update Validation Performance
**Confusion:** Updating a large relation seemed slower than expected, even for small changes.
**Clarification:** To ensure absolute data integrity, `Database::update` currently re-validates ALL constraints (Type, CHECK, Foreign Key) against EVERY tuple in the resulting relation, not just the modified ones. This is an O(N) operation. While safe, it is a known performance bottleneck that will be optimized in future versions.

## 2024-05-24 - Implemented Features Listed as Future
**Confusion:** The README listed MVCC and WAL as "Future Enhancements", but they are fully implemented in `relvar-storage`.
**Clarification:** I updated the README to reflect the current state of the codebase. The `relvar-storage` crate implements full MVCC snapshot isolation and Write-Ahead Logging.

## 2024-05-25 - Implemented Features Hidden in Roadmap
**Confusion:** The README listed "Views (virtual relvars)" and "Derived types (POSSREP)" as "Future Enhancements", but they are fully implemented and functional in `relvar-core`.
**Clarification:** I verified these features with executable examples and moved them to the "Implemented Features" section of the README to accurately reflect the project's capabilities.

## 2024-05-27 - The Ghost Variants
**Confusion:** The `ScalarValueError` and `RelationError` enums had variants that were technically public but lacked documentation explaining *when* they occur or *what* they mean. This forced users to guess based on the variant name.
**Clarification:** I've added detailed documentation to these enum variants, explaining the specific conditions that trigger them (e.g., `NotUserDefined` only happens when calling `observer()` on a built-in type).

## 2024-05-27 - Time Series Self-Join Collision
**Confusion:** The `moving_average` function silently failed or produced incorrect results when the input relation had attributes ending in `_prev`.
**Clarification:** The function implements a self-join by renaming the "previous" relation's attributes with a `_prev` suffix. This naive implementation causes a name collision if the input relation already has such attributes. I've documented this as a `# Known Issue` and advised users to avoid the `_prev` suffix in input data.

## 2024-05-27 - Broken Intra-Doc Links for Errors
**Confusion:** Rustdoc was emitting warnings because public documentation (like `Relation::from_tuples`) contained intra-doc links to error types (`RelationError`, `TupleError`) that were private at the module boundary, making them unresolvable to external readers.
**Clarification:** Exported the missing error enums (`RelationError` and `TupleError`) in `relvar-core/src/values/mod.rs` using `pub use relation::RelationError;` and `pub use tuple::TupleError;` so that they are visible in public documentation and Rustdoc can correctly generate hyperlinks.

## 2024-05-28 - Undocumented `Delta` Operator

**Confusion:** The `Delta` module in `relvar-core/src/algebra/delta.rs` was largely undocumented. It had basic struct descriptions, but no module-level documentation (`//!`) to explain *why* it exists or how it works. Additionally, the public methods (`new`, `between`, `apply`, `invert`, `compose`) lacked executable doc-tests (`## Examples`), leaving users guessing about their usage and the difference between them.
**Clarification:** I rewrote the entire documentation for the `delta.rs` module. I added a module-level `//!` comment explaining its purpose (Materialized View Maintenance, Triggers, Replication). For every public method, I added detailed `///` comments explaining the parameters, `# Errors` sections for panics/failures, and executable `## Example` blocks demonstrating exact usage with code.

## 2024-05-29 - Missing Module Docs and Redundant Boilerplate Comments
**Confusion:** Several modules in `relvar-core` lacked module-level (`//!`) documentation explaining their purpose, causing the "Black Box" problem. Additionally, getter functions had repetitive, unhelpful documentation like "Returns the x".
**Clarification:** I added module-level documentation to `dml.rs`, `virtual_relvar.rs`, `traits.rs`, `recursion.rs`, `mod.rs`, `divide.rs`, and `prepared.rs`. I also replaced "Gets the x" and "Returns the x" comments with descriptive verbs and context across `types`, `values`, and `constraints` modules.
