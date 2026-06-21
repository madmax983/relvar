## $(date +%Y-%m-%d) - Optimize intermediate collection in divide

**Learning:** `collect()` into intermediate collections before passing to a constructor or function that takes an iterator adds heap allocation overhead. Using `.values().clone()` and `.insert()` avoids chaining and collecting into a temporary `BTreeMap`.
**Action:** Replaced `.chain().map().collect()` with `.clone()` and `.insert()` inside `divide.rs`, resulting in improved performance.


## 2026-06-21 - Optimize intermediate collection in divide

**Learning:** `collect()` into intermediate collections before passing to a constructor or function that takes an iterator adds heap allocation overhead. Using `.values().clone()` and `.insert()` avoids chaining and collecting into a temporary `BTreeMap`.
**Action:** Replaced `.chain().map().collect()` with `.clone()` and `.insert()` inside `divide.rs`, resulting in improved performance.
