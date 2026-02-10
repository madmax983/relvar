## 2024-05-22 - Database God Object and Inconsistent Validation
**Learning:** `relvar-core/src/database.rs` is becoming a "God Object," handling all constraint logic (keys, FKs, checks, types) alongside transaction management and query execution. Additionally, `update` operations appear to bypass Type and Check constraints, unlike `insert`.
**Action:** Future refactors should consider extracting constraint validation into a dedicated `ConstraintValidator` or `IntegrityManager` component.

## 2024-05-23 - Constraint Logic Extraction Success
**Learning:** Extracting constraint logic from `Database` to `ConstraintManager` significantly reduced the size and complexity of the main struct. Passing `&mut Engine` explicitly to the manager's methods resolved potential borrowing conflicts.
**Action:** Use the "Manager" pattern with explicit dependency injection (passing `&mut Dependency` to methods) when extracting logic that requires access to sibling fields.

## 2024-05-24 - Database Method Logic Extraction
**Learning:** `Database` methods `insert`, `update`, and `delete` contained mixed levels of abstraction (validation, logic calculation, storage execution). Extracting "calculation" logic (e.g., `compute_relation_after_delete`) and "validation" logic (e.g., `validate_insert`) into helper methods significantly improved readability and separation of concerns.
**Action:** When a method performs distinct phases (validate -> compute -> execute), extract each phase into a separate helper method.

## 2024-05-25 - Storage Layer Duplication
**Learning:** `HeapFile` contained significant code duplication between standard and versioned (MVCC) operations. Logic for extracting tuples and handling page insertion loops was copy-pasted, increasing maintenance burden.
**Action:** Extract common logic into helper methods (`extract_all_tuples`, `find_page_for_insertion`) using internal traits (`SlotDescriptor`) to abstract over slight structural differences.

## 2024-05-26 - HeapFile Helper Extraction
**Learning:** `HeapFile` still contained significant duplication in tuple extraction (`scan`) and slot allocation (`insert`). Extracting generic helpers (`find_or_allocate_slot`, `extract_tuples_from_slots`, `validate_slot_bounds`) simplified `scan` and `insert` logic, removing redundant loops and bounds checks.
**Action:** Look for "copy-paste" logic in distinct but similar code paths (like versioned vs standard page handling) and try to unify them with generic helpers or traits, even if the data structures are slightly different.

## 2024-05-27 - PersistentEngine Responsibilities
**Learning:** `PersistentEngine` handles storage, recovery, and transaction management, leading to mixed levels of abstraction in methods like `undo_uncommitted_inserts` and `store_relation`. Extracting helper methods for specific tasks (like file replacement or relation rebuilding) clarified the high-level flow.
**Action:** When a method mixes "what to do" (policy) with "how to do it" (file I/O, serialization), extract the "how" into private helper methods.
