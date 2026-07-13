## 2026-07-13 - [Tarpaulin Implicit Error Paths]
**Learning:** Tarpaulin misses implicit error bubbles (like `?`) on tests when not explicitly targeted.
**Action:** Always write an explicit matching test with `assert!(matches!(...))` for any bubbling error from an operation like `update` or `insert`.
