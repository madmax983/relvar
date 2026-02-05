## 2024-05-23 - Rename Operator Collision Behavior
**Confusion:** When renaming multiple attributes to the same target name (e.g., A -> C, B -> C), the behavior was "magic" and undocumented.
**Clarification:** The `rename` operator iterates over source attributes in lexicographical order (due to `BTreeMap`). If multiple attributes map to the same target name, the last one visited overwrites previous ones. This is "Last Write Wins" behavior based on source attribute name order.
