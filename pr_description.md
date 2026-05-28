# 🪒 Razor: Eliminate Factory Factory and Zombie Code

## [Reduction]
**Bloat:** `KeyConstraints::would_violate_on_insert` was a zombie method only used in its own tests, while the actual validation logic in `ConstraintManager` used lower-level methods (`would_violate`). The method also returned a convoluted `Result<Option<Vec<String>>, KeyConstraintError>`.
**Cut:** Deleted `KeyConstraints::would_violate_on_insert` and its associated isolated tests.
**Saved:** Unnecessary abstraction layer and roughly 50 lines of zombie code and tests.

## [Reduction]
**Bloat:** `MockRelation` used a "Factory Factory" (Builder Pattern) merely to accept two simple configuration arguments (`count`, `seed`) alongside the mandatory `relation_type`.
**Cut:** Flattened the builder pattern by removing `count()` and `seed()` methods, and modified `MockRelation::new` to accept `(relation_type, count, seed)` directly.
**Saved:** Boilerplate builder pattern methods and simpler instantiation at call sites.
