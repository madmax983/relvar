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
