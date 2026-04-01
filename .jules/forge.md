# Contributing to Relvar

This document outlines the standards and practices for contributing to Relvar. These principles guide both human contributors and AI assistants working on the codebase.

## Core Principles

### 1. Strict Adherence to The Third Manifesto

Relvar implements the relational model as defined by C.J. Date and Hugh Darwen in *The Third Manifesto*. All contributions must respect TTM principles:

**✅ Required:**
- No NULL values (Proscription 1)
- No duplicate tuples (Proscription 2)
- No ordering of tuples (Proscription 3)
- No ordering of attributes (Proscription 4)
- No tuple-level IDs exposed in public APIs (Proscription 6)
- Type safety enforced
- Relational algebra completeness

**❌ Forbidden:**
- Introducing NULL values in any form
- Exposing physical storage details (TupleId, rowid, etc.)
- Ordered collections where sets are required
- SQL-isms that violate the relational model

**See:** [TTM Compliance Tracker](https://github.com/madmax983/relvar/milestones) for current status and open issues.

### 2. Test-Driven Development (Red-Green-Refactor)

All code must follow the TDD cycle:

**🔴 Red Phase:**
- Write a failing test first
- Test should be specific and focused
- One test per behavior/requirement

**🟢 Green Phase:**
- Write minimal code to make the test pass
- No extra features or optimizations yet
- Code can be "ugly" at this stage

**♻️ Refactor Phase:**
- Clean up the code while keeping tests green
- Extract common patterns
- Optimize performance
- Improve readability

**Example workflow:**

```bash
# 1. Write test (Red)
$ cargo test test_division_operator
# Expected: test fails

# 2. Implement minimal code (Green)
$ cargo test test_division_operator
# Expected: test passes

# 3. Refactor while keeping tests green
$ cargo test
# Expected: all tests pass

# 4. Run full quality checks
$ cargo fmt && cargo clippy && cargo test
```

### 3. Performance-First Mindset

While correctness and TTM compliance are paramount, performance matters:

**Benchmarking:**
- All new features require benchmarks
- Benchmarks must use `iter_batched` to separate setup from measurement
- Document performance characteristics
- Compare against baselines

**Optimization guidelines:**
- Profile before optimizing
- Focus on algorithmic improvements over micro-optimizations
- Don't sacrifice TTM compliance for speed
- Document trade-offs in comments

**Performance targets:**
- See [benches/README.md](benches/README.md) for target metrics
- Regressions > 10% require explanation and approval

## Pre-Commit Checklist

Before committing, **all** of the following must pass:

### 1. Code Formatting

```bash
cargo fmt --all
```

**Required:** Zero changes. Code must be formatted before commit.

### 2. Linting

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

**Required:** Zero warnings. Use `#[allow(clippy::...)]` only when justified with a comment explaining why.

### 3. Tests

```bash
cargo test --all-features
```

**Required:** All tests pass. No flaky tests allowed.

### 4. Benchmarks (for performance-critical changes)

```bash
cargo bench --benches
```

**Required:** No regressions > 10% without justification.

### Quick pre-commit command:

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features
```

## Code Standards

### Naming Conventions

**Follow TTM terminology:**
- Use "relation" not "table"
- Use "tuple" not "row"
- Use "attribute" not "column" or "field"
- Use "relvar" not "table variable"
- Use "heading" not "schema"
- Use "body" not "rows" or "data"

**Rust conventions:**
- `snake_case` for functions and variables
- `PascalCase` for types
- `SCREAMING_SNAKE_CASE` for constants
- Descriptive names over short names

### Error Handling

**Use `thiserror` for all errors:**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MyError {
    #[error("Description: {0}")]
    SomeError(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

**Never use:**
- `.unwrap()` in production code (tests are okay)
- `.expect()` in production code (tests are okay)
- Panics for recoverable errors

### Type Safety

**Leverage Rust's type system:**

```rust
// ✅ Good - distinct types
struct WidgetId(i64);
struct SupplierId(i64);

// ❌ Bad - both are i64
type WidgetId = i64;
type SupplierId = i64;
```

**Use newtypes for semantic distinction:**
- Even when backed by the same representation
- Prevents accidental mixing of concepts
- Critical for TTM compliance (see Issue #4)

### Documentation

**All public APIs require documentation:**

```rust
/// Brief one-line description.
///
/// Longer explanation if needed. Can include:
/// - Use cases
/// - Examples
/// - Performance characteristics
/// - TTM principles it implements
///
/// # Examples
///
/// ```
/// let relation = Relation::new(rel_type);
/// relation.insert(tuple)?;
/// ```
///
/// # Errors
///
/// Returns `Err` if...
pub fn my_function() -> Result<(), MyError> {
    // ...
}
```

**TTM compliance notes:**

When implementing TTM features, reference the prescription/proscription:

```rust
/// Project operator (π) from relational algebra.
///
/// TTM: RM Prescription 7 - Relational algebra completeness.
///
/// Returns a relation with only the specified attributes.
/// Duplicates are eliminated (set semantics).
```

### Testing

**Test organization:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Helper functions at top
    fn create_test_relation() -> Relation {
        // ...
    }

    // Tests grouped by feature
    #[test]
    fn test_basic_functionality() {
        // Arrange
        let relation = create_test_relation();

        // Act
        let result = relation.project(&["attr1"]);

        // Assert
        assert_eq!(result.degree(), 1);
    }

    #[test]
    fn test_error_case() {
        let relation = create_test_relation();
        let result = relation.project(&["nonexistent"]);
        assert!(result.is_err());
    }
}
```

**Test quality:**
- One assertion per test when possible
- Clear test names describing the scenario
- Use `#[should_panic]` sparingly (prefer `assert!(result.is_err())`)
- Test both success and failure cases
- Test edge cases (empty relations, single tuple, etc.)

### Benchmarking

**All benchmarks must separate setup from measurement:**

```rust
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};

fn bench_operation(c: &mut Criterion) {
    c.bench_function("operation", |b| {
        b.iter_batched(
            || {
                // Setup (not measured)
                create_test_data()
            },
            |data| {
                // Operation being measured
                perform_operation(black_box(data))
            },
            BatchSize::SmallInput,
        );
    });
}
```

**Why:** Including setup in the measurement loop skews results. We want to measure the operation, not file I/O or allocation.

## Git Workflow

### Commits

**Commit message format:**

```
Brief summary (50 chars or less)

Longer explanation if needed (wrap at 72 chars). Explain:
- Why the change was made
- What problem it solves
- Any TTM principles it addresses

Fixes #123

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
```

**Commit granularity:**
- One logical change per commit
- Tests and implementation in the same commit
- Don't commit broken code (all checks must pass)

### Pull Requests

**PR checklist:**

- [ ] All pre-commit checks pass
- [ ] New tests added for new functionality
- [ ] Benchmarks added for performance-critical code
- [ ] Documentation updated
- [ ] TTM compliance verified
- [ ] No regressions in existing tests or benchmarks

**PR description should include:**
- What changes were made
- Why they were made
- Which TTM principles are relevant
- Performance impact (if any)
- Breaking changes (if any)

### Branches

**Branch naming:**
- `feature/short-description` - New features
- `fix/issue-number-description` - Bug fixes
- `ttm/issue-number-description` - TTM compliance work
- `perf/short-description` - Performance improvements

## TTM Compliance Workflow

When working on TTM compliance issues:

1. **Read the issue carefully**
   - Understand which prescription/proscription is violated
   - Review relevant TTM sections
   - Check for related issues

2. **Write tests that enforce TTM compliance**
   - Tests should fail if TTM is violated
   - Tests should pass when compliant

3. **Implement the fix**
   - Follow TDD (Red-Green-Refactor)
   - May require significant refactoring
   - Don't break existing functionality

4. **Verify no new violations**
   - Run full test suite
   - Check that fix doesn't introduce new TTM violations
   - Update compliance documentation if needed

5. **Benchmark if relevant**
   - Some TTM compliance work impacts performance
   - Document any trade-offs

## Performance Optimization Workflow

When optimizing performance:

1. **Profile first**
   - Use `cargo flamegraph` or similar
   - Identify actual bottlenecks
   - Don't optimize based on guesses

2. **Write benchmark**
   - Establish baseline
   - Use `iter_batched` to isolate measurement
   - Document expected improvement

3. **Optimize**
   - Focus on algorithmic improvements
   - Don't sacrifice correctness for speed
   - Don't violate TTM principles

4. **Verify**
   - Re-run benchmarks
   - Ensure tests still pass
   - Document the optimization

5. **Document trade-offs**
   - If optimization adds complexity, explain why
   - If it makes code less clear, add comments
   - Quantify the performance gain

## Common Patterns

### Creating a new relational operator

```rust
// 1. Define trait in src/algebra/
pub trait MyOperatorOps {
    fn my_operator(&self, args: Args) -> Result<Relation, MyOpError>;
}

// 2. Implement for Relation
impl MyOperatorOps for Relation {
    fn my_operator(&self, args: Args) -> Result<Relation, MyOpError> {
        // Implementation
    }
}

// 3. Write comprehensive tests
#[cfg(test)]
mod tests {
    // Test normal cases
    // Test edge cases
    // Test error cases
    // Test TTM compliance
}

// 4. Add benchmarks
// In benches/algebra.rs
fn bench_my_operator(c: &mut Criterion) {
    // Benchmark implementation
}
```

### Adding a constraint type

```rust
// 1. Define error type
#[derive(Debug, Error)]
pub enum MyConstraintError {
    #[error("Constraint violated: {0}")]
    Violation(String),
}

// 2. Define constraint struct
#[derive(Debug, Clone)]
pub struct MyConstraint {
    // Fields
}

impl MyConstraint {
    pub fn new(params: Params) -> Result<Self, MyConstraintError> {
        // Validation
    }

    pub fn is_satisfied_by(&self, value: &T) -> Result<bool, MyConstraintError> {
        // Check logic
    }
}

// 3. Integrate with Database
// Add to constraint checking in insert/update/delete

// 4. Test thoroughly
#[cfg(test)]
mod tests {
    // Test validation
    // Test enforcement
    // Test error messages
}
```

## Questions?

- Check existing issues and PRs
- Review TTM compliance documentation
- Look at similar code in the codebase
- Ask in discussions

## Resources

- [The Third Manifesto](http://www.thethirdmanifesto.com/)
- [TTM Compliance Tracker](https://github.com/madmax983/relvar/milestones)
- [Benchmark Documentation](benches/README.md)
- [Database in Depth](https://www.oreilly.com/library/view/database-in-depth/0596100124/) by C.J. Date

---

**Remember:** Correctness > Performance > Cleverness

Build it right, make it fast, keep it simple.

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

## 2024-05-27 - Foreign Key Validation Duplication
**Learning:** `ConstraintManager` was manually re-implementing optimized foreign key validation (HashSet) because `ForeignKey`'s native method was O(N*M). This led to duplication and logic leaks.
**Action:** Optimized the core domain method (`ForeignKey::is_satisfied_by`) to use the efficient algorithm, allowing `ConstraintManager` to delegate back to the domain object (Rich Domain Model).

## 2024-05-28 - Join Logic Duplication
**Learning:** `relvar-core/src/algebra/join.rs` contained significant duplication in key extraction (`extract_join_key`) and tuple combination logic (`combine_tuples`) between `join`, `theta_join`, and `probe_and_combine`.
**Action:** Extract core algebraic operations (key extraction, tuple merging) into private helper functions to enforce DRY and ensure consistent behavior across different join implementations.

## 2024-05-29 - Summarize Logic De-Duplication
**Learning:** `relvar-core/src/algebra/summarize.rs` contained complex logic within `Aggregation::compute` (large match block) and `Relation::summarize` (validation, construction, execution). Extracting these into helper functions (`compute_sum`, `compute_avg`, `validate_grouping_attributes`, etc.) significantly improved readability and reduced cognitive load.
**Action:** When a method performs multiple distinct phases (e.g., validate -> prepare -> execute) or contains a large `match` statement with complex arms, extract each phase or arm into a private helper method.

## 2025-06-03 - Algebraic Operator Refactoring
**Learning:** Complex algebraic operators like `group` and `ungroup` often mix validation, schema derivation, and tuple processing. Extracting these into distinct phases (`validate`, `build_heading`, `compute_tuples`) improves readability and testability.
**Action:** Apply this "Three-Phase Operator" pattern when refactoring other complex operators like `join` or `summarize`.

## 2025-06-04 - Transitive Closure Loop Extraction
**Learning:** The `tclose` operator's loop contained complex logic spanning multiple relational operations (rename, join, project, difference), making it difficult to follow the core recursive algorithm.
**Action:** Extract the inner loop "compute path iteration" into a private, named helper function (`compute_next_paths`) to flatten the pyramid and improve readability.
## 2025-06-03 - Summarize operator Three-Phase refactoring
**Learning:** Extracting `compute_summarized_tuples` from `Relation::summarize` improves readability by separating tuple calculation from validation and heading construction logic. The extraction uses `&HashMap<Vec<&ScalarValue>, Vec<&Tuple>>` which matches `group_tuples`'s return type.
**Action:** When extracting functions handling grouped data, match the signature of the grouping function (`group_tuples` here) exactly.

## 2026-03-14 - Extract Complex Type Comparison Logic
**Learning:** `Ord::cmp` implementations in `relvar-core/src/types/scalar.rs` (`ScalarType`) and `relvar-core/src/values/scalar.rs` (`ScalarValue`) had deeply nested `match` branches (up to 12 levels) for complex variant comparisons like `Relation` and `UserDefined`.
**Action:** Replace nested `match` inside `Ord::cmp` with a guard clause on discriminant/type mismatch first, and then extract complex variant match arms into private helper functions like `cmp_relation_types` and `cmp_user_defined_types`.

## 2025-06-05 - Collaborative Filtering God Functions
**Learning:** `relvar/src/experimental/recommend.rs` contained "God Functions" (`compute_user_similarities` and `predict_unseen_items`) that handled multiple complex relational algebra operations in a single block, making them hard to read and test.
**Action:** Applied the "Three-Phase Operator" pattern to extract logic into private helper methods (`compute_similarity_products`, `summarize_and_calculate_similarity`, `compute_prediction_components`, `summarize_and_calculate_predictions`) to improve clarity and maintainability.
## 2026-03-14 - Extract Complex PartialEq and Hash Logic
**Learning:** `PartialEq::eq` and `Hash::hash` implementations in `relvar-core/src/values/scalar.rs` (`ScalarValue`) had deeply nested `match` branches for complex variants like `UserDefined`.
**Action:** Replaced nested `match` inside `PartialEq::eq` with an upfront `std::mem::discriminant` guard clause for type mismatch, and extracted the complex variant match arms into private helper functions like `eq_user_defined_values` and `hash_user_defined_value`. Ensure `&Box<T>` references are explicitly mapped to `&T` using `.as_ref()` in loops.

## 2026-03-14 - Extract Database Test Setup Logic
**Learning:** `relvar-core/src/database/tests.rs` contained significant duplicate test setup logic for creating `PARENT` and `CHILD` relation types and setting up their tables, making the tests noisy and harder to read.
**Action:** Extract duplicate test setup into a helper function `setup_parent_child_db` inside the test module. This removes repetitive boilerplate across testing functions and simplifies test logic.
