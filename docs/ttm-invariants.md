# TTM invariant catalog

This catalog covers every prescription and proscription of *The Third
Manifesto* (RM = Relational Model). For each one it lists how relvar
enforces the rule at runtime, the machine-checked proof, and the tests that
tie the two together. [ADR 0001](adr/0001-mechanized-ttm-enforcement.md)
explains the approach.

Where each kind of evidence lives:

- **Proof:** a Verus lemma or theorem in `verify/verus/`. CI runs it on
  every push (`verify.yml`, "Verus proofs"). No `assume`, `admit`,
  `external_body` or axioms are allowed.
- **Refinement:** a proptest property in `relvar-core/tests/ttm_refinement.rs`.
  It compares every algebra operator with a reference model written from
  the same definitions as the Verus spec.
- **Model check:** Stateright exploring every operation sequence up to a
  depth bound, against the real code. There are two:
  - `relvar/tests/ttm_golden_rule_model_check.rs` (database level);
  - `relvar-storage/tests/ttm_model_check.rs` (engine level).
- **Gate:** `relvar/tests/ttm_proscription_6.rs`, a scan of the public API.
- **Regression:** a focused test for a specific violation that has been
  fixed.

Project labels: the code comments use their own numbering. "Proscription 1"
there means no nulls, "2" no duplicates, "3" no tuple ordering, "4" no
attribute ordering and "6" no tuple ids. This catalog uses the book's RM
numbering and gives the project label in brackets.

## RM Proscriptions

| # | Rule | Runtime enforcement | Proof | Tests |
|---|------|---------------------|-------|-------|
| 1 | No attribute ordering [project 4] | Headings and tuples are maps keyed by attribute name. Arena `TupleView`s are stored by position, but they compare and hash by attribute set, and `TupleView::new` validates the layout. | `model.rs`: `Tuple`/`Heading` are `Map`s; `lemma_tuple_attribute_order_irrelevant`, `lemma_heading_attribute_order_irrelevant`, `lemma_tuple_equality_is_extensional` | Refinement (every operator); arena tests `tuple_view_new_*` |
| 2 | No tuple ordering [project 3] | The body is a hash set. `Relation` hashing is order-independent. `Ord` on relation *values* exists only so MIN/MAX and ordered collections work, and it is consistent with `==`. | `model.rs`: the body is a `Set`; `lemma_insertion_order_irrelevant`, `lemma_relation_equality_is_extensional` | `ord_tests.rs::prop_cmp_equal_iff_eq`; refinement |
| 3 | No duplicate tuples [project 2] | (a) `Relation` set semantics. (b) `SlotRelation::insert` is a set insert. (c) `project`/`summarize` deduplicate. (d) The engine enforces the whole-heading key under concurrency: inserting an already-visible tuple does nothing, and concurrent writes of the same tuple conflict (first-updater-wins). An in-memory index makes the check cheap, and a test-only consistency check compares it with a full heap scan. (e) MVCC shows each snapshot at most one version of each tuple. | `lemma_no_duplicates`, `lemma_cardinality_counts_distinct_tuples`, `lemma_heading_is_superkey`; `mvcc.rs::theorem_at_most_one_visible_version`; float `lemma_hash_coherent` | Arena `slot_relation_insert_duplicate_is_noop` and related; `ord_tests.rs::prop_eq_implies_equal_hash`; `persistent_engine/tests/implicit_key.rs`; engine model check |
| 4 | No nulls [project 1] | `ScalarValue` has no null. `Tuple::new` rejects a missing attribute (`MissingValue`). Deserialization validates tuples and relations. `SUMMARIZE` uses PER semantics, so it never needs a "no rows" value. | `model.rs`: `Val` has no null variant; `lemma_no_nulls`, `lemma_missing_attribute_rejected`; `lemma_summarize_groups_nonempty` | Value-layer deserialization tests; refinement well-formedness checks |
| 5 | No nullological mistakes | `TABLE_DEE`/`TABLE_DUM` are ordinary relations. `project` onto no attributes gives DEE or DUM. `GROUP {}` is allowed. `DIVIDEBY` with an empty remainder gives DEE or DUM. The empty key is allowed. | `lemma_dee_dum`, `lemma_degree_zero_is_dee_or_dum`, `lemma_project_onto_nothing`, `lemma_join_dee_dum`, `lemma_empty_key_means_at_most_one_tuple` | Refinement laws (join with DEE is identity, with DUM is empty; project onto none); `ttm_keys.rs` (empty key); divide and group regressions |
| 6 | No internal-level constructs [project 6] | Tuple, page and slot ids, LSNs, transaction ids and xmin/xmax stay internal to the crate. The transaction handle is opaque and prints no id. | `mvcc.rs`: versions exist only below the logical model | Gate (scans every public signature, field, re-export and enum body reachable from the facade); `transaction_handle_is_opaque`; `persistent_engine/tests/proscription6.rs` |
| 7 | No tuple-level operations | `insert`, `delete` and `update` are shorthands for relational assignment. Each one computes the candidate relvar value and passes or fails as a whole statement, through the same Golden Rule path as `multiple_assign`. | `database.rs::execute` (statements are assignments of whole relation values) | Golden Rule model check (each shorthand is checked against `execute`) |
| 8 | No composite attributes | Each attribute holds one value of one declared type. A relation-valued attribute is a single value of a relation type. | `model.rs`: one `Val` per `Name` | By construction |
| 9 | No domain check override | Type checks cannot be bypassed from outside the crate. `Tuple::new_unchecked` is crate-private. Deserialization validates. SUMMARIZE result types are checked. `SlotRelation`/`TupleView` inserts are typed. Natural join rejects common attributes of different types. | `conforms`/`wf` preserved by every operator (`algebra.rs`), plus the closure theorem | Join, summarize and slot-relation type-mismatch regressions; refinement error cases |
| 10 | Not SQL | Design principle: no nulls, no bags, no three-valued logic. The proscriptions above enforce what is mechanically checkable. | — | — |

## RM Prescriptions

| # | Rule | Runtime enforcement | Proof | Tests |
|---|------|---------------------|-------|-------|
| 1 | Scalar types | Built-in types plus user-defined types with a possible representation (`ScalarType::UserDefined`). | `Ty::Scalar` | Scalar type tests |
| 2 | Scalar values are typed | Every `ScalarValue` has exactly one type (`scalar_type()`). `select` checks the representation. | `type_of` is total | `ScalarValue::select` tests; deserialization validation |
| 3 | Read-only vs update operators | The algebra takes `&self` and returns new values. Updates exist only on `Database` (`&mut self`). The Rust type system enforces the split. | — | Compiler |
| 4–5 | Possible representations, selectors, observers | `ScalarValue::select` / `observer`. User-defined types with the same representation but different names are distinct. | — | `ord_tests.rs` (WidgetId vs SupplierId) |
| 6 | TUPLE type generator | `TupleType` (headings) and validated `Tuple::new`. | `conforms` | Value-layer tests |
| 7 | RELATION type generator | `RelationType`, and relation-valued attributes through `ScalarType::Relation`. | `Ty::Rel`; `lemma_group_closed`, `lemma_group_rva_wf`, `lemma_ungroup_closed` | Refinement: `ungroup(group(r)) == r` |
| 8 | Equality | Tuple and relation equality is by value. Float equality is an equivalence (NaN = NaN, -0.0 ≠ 0.0). `Ord` agrees with `==`, and the hash respects it. | `lemma_relation_equality_is_extensional`; `scalar.rs`: `lemma_float_eq_is_equivalence`, `lemma_hash_coherent`, `lemma_order_consistent_with_eq`, `lemma_order_total`, verified transcriptions `float_bits_eq` / `float_hash_bits` / `cmp_float_bits` | `ord_tests.rs` properties |
| 9 | Tuples | Exactly the heading's attributes, each correctly typed, enforced at construction and at deserialization. | `conforms`, `lemma_insert_requires_conformance` | Value-layer tests |
| 10 | Relations | Every tuple conforms to the heading. `from_tuples` no longer trusts heading addresses. | `wf`, `lemma_insert_wf`, `lemma_delete_wf` | `from_tuples` regression; refinement well-formedness |
| 11–12 | Scalar and tuple variables | Host-language (Rust) variables, type-checked by the compiler. | — | Compiler |
| 13 | Relation variables | `Database` relvars. A relvar's heading never changes through assignment. | `theorem_headings_fixed`, `theorem_assignment_principle` | Golden Rule model check |
| 14 | Real and virtual relvars | Virtual relvars are defined by expressions. | `expr.rs::theorem_virtual_relvar_well_formed` | `database/tests/virtual_relvar.rs` |
| 15 | Candidate keys | Declared keys are validated: names exist, no duplicates, irreducible, and the empty key is allowed. The key index probe is exact. Without a declared key, the whole heading is the key, and the engine enforces it under concurrency. | `keys.rs`: `lemma_candidate_key_exists`, `lemma_superkey_contains_candidate_key`, `lemma_checked_insert_preserves_key`, `lemma_failed_check_would_violate_key`, `lemma_index_probe_is_exact`, `lemma_index_insert`, `lemma_index_delete` | `ttm_keys.rs`; Golden Rule model check; engine model check |
| 15′ | Foreign keys (referential constraints) | An FK must reference a declared candidate key with matching types. It is checked on the post-statement state, so self-references and cycles work. Dropping a referenced relvar is refused. | `lemma_fk_insert_referencing`, `lemma_fk_delete_referenced`, `lemma_fk_target_unique` | `ttm_foreign_keys.rs`, `ttm_self_reference.rs`, `ttm_ddl.rs`; Golden Rule model check |
| 16 | Databases | A database contains its relvars *and* its constraints. Declarative constraints persist in the catalog, written atomically (temp file, fsync, rename): keys, foreign keys, CHECK and type constraints, and declarative assertions (`DeclarativeAssertion`, an `IS_EMPTY`/subset/equality/cardinality condition over `Query` expressions), which `Database::new` restores with nothing to re-register. A relvar an assertion refers to cannot be dropped. A closure-based assertion cannot be stored, but its name is: a reopened database that records an unregistered closure assertion refuses every change (`DatabaseError::UnregisteredAssertion`) and fails `verify_integrity` until it is registered again (and validated) or explicitly dropped with `forget_assertion`, so no assertion is silently lost. | `database.rs::Schema` | `relvar/tests/ttm_persistent_constraints.rs`, `relvar/tests/ttm_persistent_assertions.rs`; `database/tests/declarative_assertions.rs`, `database/tests/unregistered_assertions.rs`; Golden Rule model check (crash + reopen restores constraints) |
| 17 | Transactions | Atomic commit and rollback. Inside a transaction, statements never touch committed state. A crash is a rollback. Deletes and updates are MVCC versions that abort and recovery undo. A stale handle cannot commit. A torn WAL tail is the end of the log. The directory is locked against a second open. | `theorem_transactions_preserve_consistency`, `theorem_rollback_is_atomic`, `theorem_commit_publishes_working_state`; `mvcc.rs`: `lemma_no_dirty_reads`, `lemma_snapshot_is_stable`, `lemma_read_your_writes` | `persistent_engine/tests/{atomicity,lifecycle,lock,wal_tail,torture}.rs`; both model checks (rollback, crash, isolation) |
| 18 | Relational algebra | Every operator is closed and rejects ill-typed calls: join type mismatch, theta-join shared names, project/rename of unknown names, rename collisions, ungroup collisions, SUMMARIZE result types, `TCLOSE` with from = to. | `algebra.rs` closure lemma for each operator, plus laws (`lemma_set_op_laws`, `lemma_join_commutative`, `lemma_semijoin_closed_and_partition`, `lemma_semijoin_is_projected_join`, `lemma_divide_meaning`, `lemma_tclose_transitive`, `lemma_rename_roundtrip`, `lemma_project_identity`); `expr.rs::theorem_closure` for any nested well-typed expression | Refinement (every operator against the model, plus laws and error cases) |
| 19 | Relvar names, relation selectors, recursion | Expressions over relvar names. `TCLOSE` is the recursive operator. | `expr.rs` (`Expr::Var`), `lemma_tclose_*` | Refinement (`tclose`) |
| 20 | User-defined operators | Host-language closures passed to `restrict`/`extend`. Their results are type-checked (`extend` rejects a value of the wrong type). | `extend_ok` precondition | `extend` regressions |
| 21 | Assignment, multiple assignment | `Database::multiple_assign`: several relvars assigned as one statement and checked on the combined final state. The Assignment Principle holds. | `theorem_assignment_principle`, `theorem_multiple_assignment_is_one_statement`, `theorem_statement_atomic` | `multiple_assignment.rs`, `relvar/tests/ttm_multiple_assignment.rs`; Golden Rule model check (`AssignPair`) |
| 22 | Comparisons | Relation equality (see 8). | `lemma_relation_equality_is_extensional` | Refinement |
| 23 | Integrity constraints, the Golden Rule | Every statement is checked against every constraint on the candidate state, and installed or rejected whole. Database assertions, declarative or closure, are evaluated after the statement's writes, which are undone on violation or on an assertion that cannot be evaluated. `Database::verify_integrity` checks the entire database. | `database.rs::theorem_golden_rule` | `database/tests/golden_rule.rs`, `database/tests/declarative_assertions.rs`; Golden Rule model check (`verify_integrity` after every step) |
| 24 | Total database constraint | The conjunction of every constraint, enforced as in 23. | `database.rs::consistent` | As 23 |
| 25 | Catalog | The catalog holds relvar types and constraints. It is saved atomically, and a catalog whose constraints name an unknown relvar is rejected on load. | — | Catalog unit tests; `ttm_persistent_constraints.rs` (old-catalog compatibility) |
| 26 | Language design | Design principle; not a mechanically checkable invariant. | — | — |

## Known limits

- **Closure assertions.** `DatabaseAssertion` constraints are Rust
  closures, so the catalog stores only their names. They must be
  registered again after every open; until then the database refuses
  every change, so the gap is loud, not silent. Every other kind of
  constraint persists in full, including `DeclarativeAssertion`s, which
  cover any rule expressible as a condition over `Query` expressions.
- **Multiple assignment to the same target.** `multiple_assign` rejects a
  statement that names the same relvar twice. TTM would combine the two
  assignments instead.
- **Model-check depth.** The model checks are exhaustive only up to their
  depth bounds. The test suite runs depth 4 in memory and 3 on the
  persistent engine for the Golden Rule check, and depth 5 for the engine
  check. CI's model-check job goes deeper: depth 7 for the engine (647k
  states), and depth 5 in memory and 4 persistent for the Golden Rule. Beyond those depths, coverage
  comes from the proofs, the seeded crash-consistency torture test and the
  proptest properties.
