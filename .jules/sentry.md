## 2025-02-18 - Arbitrary Limits in Loops
**Learning:** Found a hardcoded `if page_id > 1000` break in `HeapFile::scan`, intended as a safety guard but acting as a severe data truncation bug for datasets > 4MB.
**Action:** Always scrutinize "magic numbers" in loop termination conditions. Test boundaries explicitly (e.g., if limit is 1000, test 1001).

## 2025-02-18 - Silent Corruption Masking in Page Read
**Learning:** `PageFile::read_page` was returning an empty valid page when encountering corrupted length prefixes or partial reads, potentially masking data corruption as "no data".
**Action:** When reading length-prefixed data, always validate that the declared length matches the available buffer size. Explicitly handle EOF vs Partial/Corrupted reads.

## 2026-01-31 - Silent Error Swallowing in Collection Iterators
**Learning:** `HeapFile::scan` was silently ignoring deserialization errors during iteration, effectively hiding data corruption from the caller.
**Action:** Avoid `filter_map` or swallowing `Result::Err` in core storage loops. Always propagate serialization errors up to the caller to fail fast on corruption.

## 2026-02-18 - Silent Corruption Healing in Storage Operations
**Learning:** `HeapFile` modification methods (`insert`, `update`, `delete`) were silently swallowing corrupted slots (pointing outside page bounds) by replacing them with empty tuples during page rewrite. This effectively "healed" the page by deleting the inaccessible data without warning.
**Action:** In storage modification paths, always treat structural corruption (e.g., out-of-bounds pointers) as a hard error. Never "skip" or "default" corrupted data during a rewrite, as this makes data loss permanent.

## 2026-05-21 - Silent Data Loss in Rename Collision
**Learning:** `Relation::rename` handles attribute collisions (multiple attributes renamed to same target) by iterating map entries. Due to `BTreeMap` order, the lexicographically last attribute overwrites earlier ones without warning.
**Action:** When testing renaming or mapping operations, always verify "collision" scenarios. Document the behavior explicitly in tests to prevent accidental regression if iteration order changes.

## 2026-06-15 - Unnecessary Integer Overflow in Aggregation
**Learning:** `Avg` aggregation was using `i64` accumulator, causing it to fail on large datasets where the sum exceeds `i64::MAX` even if the average is small.
**Action:** Use `i128` (or larger type) accumulators for integer aggregations to prevent intermediate overflows.

## 2026-10-24 - Inconsistent NaN Handling in Scalar Values
**Learning:** Found that  relied on  for equality and hashing, causing different NaN payloads to be treated as distinct values (and distinct groups in ).
**Action:** When implementing database types, always normalize NaNs in , , and  to ensure set semantics (all NaNs are equal), regardless of the underlying bit pattern.

## 2026-10-24 - Inconsistent NaN Handling in Scalar Values
**Learning:** Found that `ScalarValue::Float` relied on `f64::to_bits()` for equality and hashing, causing different NaN payloads to be treated as distinct values (and distinct groups in `summarize`).
**Action:** When implementing database types, always normalize NaNs in `Eq`, `Hash`, and `Ord` to ensure set semantics (all NaNs are equal), regardless of the underlying bit pattern.

## 2027-02-27 - Slot Reuse Data Corruption
**Learning:** `HeapFile::try_insert_into_page` (and versioned variants) caused data corruption when reusing a freed slot (e.g. from deletion). It was calling `vec.insert()` which shifts subsequent elements, but the `slots` vector was not shifted (since we reused an index). This misaligned slots and tuple data, causing subsequent tuples to be lost or corrupted during page repack.
**Action:** When managing parallel vectors (slots and data), ensure modifications are symmetric. If reusing a slot index, use index assignment (`vec[i] = val`) instead of insertion (`vec.insert(i, val)`).

## 2027-05-22 - Virtual Relvar Side-Effect Vulnerability
**Learning:** `VirtualRelvarDefinition` passes `&mut Database` to the evaluator closure, allowing "read-only" views to perform writes (insert/update/delete) as side effects.
**Action:** When designing extension points or callback APIs, strictly enforce immutability (`&self`) if the operation is conceptually read-only. Avoid passing mutable context unless mutation is the explicit goal.

## 2026-02-17 - Strict Float Equality in Foreign Keys
**Learning:** Foreign Keys on floating-point columns enforce strict bit-pattern equality (e.g., `-0.0` != `0.0`), rejecting references even if numerically equal.
**Action:** Avoid using floating-point columns as foreign keys unless strict bitwise identity is guaranteed, or implement fuzzy matching logic explicitly.

## 2027-08-15 - Integer Truncation and Overflow in Storage Layers
**Learning:** `WalManager` and `PageFile` were casting `u64` length prefixes to `usize` without checking for truncation (on 32-bit systems) or overflow during offset calculation. This allowed malicious payloads to cause panics or potentially bypass size checks.
**Action:** Always validate `u64` values from external sources (disk/network) using `checked_add` and `usize::try_from` before using them for memory indexing or allocation.
## 2025-03-03 - Delta Error Paths
**Learning:** The `DatabaseError::AlgebraError` return paths for type mismatch in the `Delta` struct (`new`, `between`, `apply`, `compose`) were entirely untested, despite being standard error branches.
**Action:** When testing algebra structs, explicitly check that all operations fail correctly when passed relations with mismatched types.

## 2025-03-04 - ScalarType Coverage Gaps
**Learning:** Certain `ScalarType` recursive boundaries and unreachable trait implementations (`Ord` match) were entirely missing coverage, masking potential issues if the type structure were to change. Additionally, type mismatch errors in the `selector` logic lacked tests verifying correct error string output.
**Action:** Always ensure full trait coverage, including unreachable branches when logic relies on matched enum variants, and systematically test recursion depth guards with deep structures.
## 2025-03-05 - PreparedConstraint Coverage Gaps
**Learning:** Found multiple untested branches in `PreparedConstraintExpression` evaluation, specifically around uncommonly tested `CmpOp` variants (`Ne`, `Lt`, `Le`, `Ge`), nested logical expressions (`Or`, `Not`), and type-mismatch error handling in `Like`.
**Action:** When testing constraint or expression engines, ensure a full suite of table-driven comparison operator tests and explicitly write test cases for both logical nesting structures and intentionally bad types (e.g., passing an integer to a string `LIKE` operation).

## 2025-03-14 - ConstraintManager Coverage Gaps
**Learning:** Multiple functions in `ConstraintManager` relating to evaluating constraints natively against individual tuples, checking constraint preconditions prior to setup, and handling dependent foreign key updates via `validate_referencing_foreign_keys` were completely unexercised. This leaves open logic flaws where an update sequence may evaluate validations improperly.
**Action:** When implementing database or storage engine layers, constraint logic generally needs comprehensive functional integration tests since unit tests on the constraint primitives rarely stress the engine orchestration itself, leading to gaps in `manager.rs`.
## 2025-03-16 - Virtual Relvars Evaluation and Database Error Coverage
**Learning:** Found several untested code paths regarding the mutability limitations of `VirtualRelvarDefinition` (preventing mutation because `evaluator` takes an immutable reference but error mapping missing tests), and transaction nesting issues inside `relvar-core/src/database/mod.rs` mapping to `DatabaseError::TransactionError`.
**Action:** When working on APIs containing view-like concepts (`virtual_relvars`), ensure the evaluation error paths are fully tested. When implementing Database transactions, ensure explicit testing for double-begin and missing-begin cases for commits and rollbacks.

## 2025-03-20 - Unvalidated Referencing Foreign Keys on Update
**Learning:** `Database::update` in `relvar-core/src/database/data.rs` applies updates and validates the updated relation against its own constraints, but crucially forgets to validate referencing foreign keys (i.e. if this relation is a parent to another relation, and a primary key was updated, the child relation's foreign keys might be violated). The `delete` method correctly calls `self.constraints.validate_referencing_foreign_keys`, but `update` does not.
**Action:** Always validate `validate_referencing_foreign_keys` in both `delete` and `update` logic paths for relational systems.
## 2025-03-28 - Database Schema Error Paths and Virtual Relvars
**Learning:** Found several untested code paths regarding schema operations on `virtual_relvars`, specifically defining a virtual relvar when a base or virtual relvar already exists, and requesting types/dropping nonexistent relvars, mapping to `DatabaseError::RelationAlreadyExists` and `DatabaseError::Storage(StorageEngineError::RelationNotFound)`.
**Action:** When implementing database schemas, explicitly test the boundary conditions between base relvars and virtual relvars, as virtual relvars share the namespace. Ensure `get_relvar_type` and existence tests adequately cover both.
## 2026-04-10 - Extend and Group Error Paths
**Learning:** Found several untested code paths regarding the failure branches of `extend` and `group` algebra operations, specifically `ExtendError::TupleCreation` for computation type mismatches, and `GroupError::ResultAttributeExists`, `UngroupError::AttributeNotFound`, `UngroupError::NotRelationValued`, and `UngroupError::TupleCreation`.
**Action:** When implementing database algebra operators that construct new tuples or relation types from existing data, always write specific test cases targeting type validations, attribute name conflicts, and missing attributes.

## 2026-04-06 - Tarpaulin Reporting on Multiline Closures
**Learning:** Line-based coverage tools like `cargo tarpaulin` often report missing coverage for multi-line format arguments or complex closures defined as macro/function parameters, even when the underlying logic path is hit.
**Action:** Before trying to restructure methods strictly to satisfy the coverage tool on these specific lines, verify that it's a tooling artifact. If it is, accept it or look for real logical branches (like actual `if let Err(...)` statements) that are missing tests.
## 2025-05-18 - [ForeignKey Constraint Panic]
**Learning:** `ForeignKey` constraint validation methods (`is_satisfied_by`, `would_violate_on_insert`, `would_violate_on_delete`) panicked with `.unwrap()` when they were passed a tuple that was missing a required attribute, as dynamic relations might accept partially valid or mismatched inputs.
**Action:** When validating constraints against tuples, use `.ok_or_else()` to explicitly handle missing attributes by returning an error instead of panicking via `.unwrap()`.
## 2024-04-10 - Database Facade Constraint Tests
**Learning:** Many of the Database facade constraint manipulation methods (`get_key_constraints`, `set_key_constraints`, etc) were uncovered because constraints were typically interacted with directly, but the facade handles translating names properly to the underlying engine. Also `drop_relvar` and `drop_virtual_relvar` error conditions were not covered.
**Action:** Wrote targeted coverage tests for Database methods verifying they behave correctly and emit the right error enumerations (like `TupleMismatch` and constraint errors) directly on `Database<E>`. Always ensure integration-style tests cover the outermost boundary layer.
## 2024-05-01 - [Coverage Gap in Storage Page Buffer Reading]
**Learning:** `PageFile::read_page` had uncovered error paths when handling short, malformed, or corrupt file buffers from disk.
**Action:** Wrote 5 new unit tests mapping specifically to those missing branch coverage blocks (PageError::Serialization mappings).
## 2026-04-13 - Data Constraint Coverage Gaps
**Learning:** Found several untested code paths regarding DML operations in `relvar-core/src/database/data.rs`. Specifically, the failure paths for constraint bulk validation during `update` and `validate_referencing_foreign_keys` during `update` and `delete` were entirely unexercised. This masked whether changes invalidating parent constraints actually correctly errored out in the database API facade.
**Action:** When implementing DML APIs, verify constraint orchestration layers thoroughly, specifically focusing on how operations on parent datasets affect linked child datasets, and ensure operations triggering bulk evaluations handle constraints exactly as expected.
## 2024-05-18 - Database Integrity Operations Coverage
**Learning:** We needed full coverage for public APIs `get_key_constraints` and `set_type_constraints` in `database::integrity`.
**Action:** Added `test_database_integrity_getters` and `test_database_integrity_type_and_check` to `sentry_database_integrity_coverage.rs` to comprehensively test getters/setters for constraints without destroying existing tests.

## 2025-05-20 - Storage Heap Serialization Error Paths
**Learning:** Certain `HeapError::Serialization` conditions (like tuple offset and length overflow limits or parsing errors via corrupted boundaries) inside `relvar-storage/src/storage/heap.rs` were completely missing test coverage, leading to untested internal safety guard limits on maximum allowed sizes.
**Action:** Targeted internal methods `find_page_for_insertion` error paths, bounds limits like `check_versioned_tuple_size_limit`, and deserialization helper routines `extract_tuples_from_versioned_slots` with targeted safety constraint tests ensuring exact `HeapError` mappings.
## 2026-04-16 - Transaction ID Generation Panics
**Learning:** `TransactionIdGenerator::generate` panics when `u64::MAX` is reached to prevent wrapping around and causing LSN collisions, but this panic path had no tests.
**Action:** Wrote an explicit `#[should_panic]` test ensuring `u64::MAX` correctly triggers "TransactionId overflow" instead of wrapping around in `wal/lsn.rs`.

## 2026-04-16 - Extend and Project Missing Branches
**Learning:** `extend_into` error conditions (computation type mismatch) and empty iterator scenarios, as well as `project` greater-than ordering matches for lexicographical attributes were uncovered.
**Action:** Wrote tests targeting `extend_into` specifically mirroring existing `extend` tests, and crafted carefully named attribute headings (`"z"`, `"a"`) for `project` to trigger the required merge-sort iteration states.
## 2024-04-18 - Missing Delta tests
**Learning:** `relvar-core/src/algebra/delta.rs` and `relvar-core/src/algebra/divide.rs` have several edge cases untested. `divide.rs` coverage is low.
**Action:** Write thorough tests for both to ensure robust edge case handling in `relvar-core`.
## 2024-04-18 - Missing Delta & Divide tests (Continued)
**Learning:** `relvar-core/src/algebra/delta.rs` error paths and `relvar-core/src/algebra/divide.rs` were lacking test coverage for type mismatch and structural integrity edge cases.
**Action:** Implemented thorough inner-module tests for both `delta.rs` and `divide.rs` error paths to ensure proper validation checks hit `DatabaseError::AlgebraError` and `DivideError` correctly.
## 2026-04-16 - Extend and Project Missing Branches
**Learning:** `extend_into` error conditions (computation type mismatch) and empty iterator scenarios, as well as `project` greater-than ordering matches for lexicographical attributes were uncovered. Group, Union, Intersect `_into` functions were also untested.
**Action:** Wrote tests targeting `extend_into` specifically mirroring existing `extend` tests, and crafted carefully named attribute headings (`"z"`, `"a"`) for `project` to trigger the required merge-sort iteration states, as well as general testing for `_into` consuming variants and error paths for summaries on empty tables.
## 2026-04-18 - Missing DML Validation and Algebra Into Branch Tests
**Learning:** `Database::update` error paths during `validate_referencing_foreign_keys` validation and various `_into` algebra operations (`extend_into`, `union_into`, `intersect_into`) error paths and basic branch execution handling were entirely missing test coverage. Also `Database::list_relvars` was entirely unexercised.
**Action:** When working on APIs containing DML operations like `update` and `delete`, ensure the error paths for constraints propagation to children referential relations are thoroughly tested. Always ensure the `_into` fast-path consuming operations mirror standard coverage.
## 2026-04-28 - CSV and JSON Importer Error Coverage
**Learning:** Found several untested code paths regarding the failure branches of `from_csv` and `from_json` functions in `relvar/src/tools/importer.rs`. Specifically, the `JsonError`, `LimitExceeded`, and `TypeError`/`FormatError` mappings for invalid imports were entirely unexercised.
**Action:** When implementing importer utilities that handle unvalidated external data, ensure comprehensive tests are written that trigger serialization limits and strict type checking failures to guarantee safe conversion to `Relation` objects.
## 2024-05-01 - Test Coverage Improvements for Core Scalar/Tuple Logic

**Learning:** It is crucial to append tests to exactly correct lines to avoid collision with multiple `mod tests {` or similar. Directly applying `sed` replacements or `patch` is safer than raw appending (`cat >>`). `TryFrom` conversions must carefully type-check expected fields, such as `unwrap_err().expected` being correctly matched.
**Action:** Use `replace_with_git_merge_diff` inside existing modules when working with `cfg(test)` items instead of using raw `cat >>` appended blocks, to prevent compilation errors and redundant definitions.
## 2026-05-04 - Storage Heap Extraction Error Paths
**Learning:** Certain `HeapError::Serialization` conditions triggered by bounds limit checks inside `extract_tuples_from_versioned_slots` and `validate_slot_bounds` in `relvar-storage/src/storage/heap/mod.rs` were entirely missing coverage. These represent the critical error paths protecting against buffer overflows and memory corruption during MVCC page reconstruction.
**Action:** Always create targeted error boundary tests for memory slicing and decoding functions, specifically simulating corrupted page states (like manipulating slot lengths past page boundaries or inserting offset overflows) to guarantee the serialization errors are actually thrown instead of panicking.
## 2026-05-01 - Tuple Grouping and Summarization Attribute Not Found Error Maps
**Learning:** Functions like `group`, `summarize`, and `ungroup` internally iterated and used `.unwrap()` on `tuple.get(attr)` under the assumption that the validated relation type strictly guaranteed attribute presence. While this is mathematically true in the pure relational model, any internal inconsistency, malformed tuples bypassing validation, or memory corruption could trigger an unhandled panic in production.
**Action:** Replaced `.unwrap()` calls in `group.rs` and `summarize.rs` with `.ok_or_else(|| ...)` explicitly mapping to `GroupError::AttributeNotFound` and `SummarizeError::AttributeNotFound`, respectively. Always prefer robust Result propagation over panics, even for seemingly "guaranteed" schema invariants.
## 2026-05-05 - DML Constraints Logic Coverage

**Learning:** Missing coverage branches for specific operations involving database bulk constraints and cascading failure. `compute_relation_after_update` early exit logic via `?` unrolls some lines in `Database::update` if the logic before reaches an error state.

**Action:** When adding tests for database constraints involving `update` and `delete`, ensure varying degrees of constraint violations such as duplicated primary keys after updates and violations on foreign keys mapped against parent relations to fully hit bulk checks.
## 2026-08-15 - DML Primitive Testing
**Learning:** Pure functions inside `relvar-core/src/database/dml.rs` representing the DML data manipulation decoupled from the storage layer were missing tests entirely, specifically the `TupleMismatch` error branch of `compute_relation_after_update`.
**Action:** Always create tests around newly introduced pure algorithmic variants inside the `mod` itself, as these often hold logic that might not be fully exercised by the broader facade APIs.
