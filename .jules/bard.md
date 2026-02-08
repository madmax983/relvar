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
