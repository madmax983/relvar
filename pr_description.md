🪒 Razor: [Result-Option Simplification]

**Bloat:** `Result<Option<Relation>, DatabaseError>` in `relvar/src/experimental/automata.rs`. The `Option` wrapping was an unnecessary layer of complexity; an empty relation natively represents the "no active states" condition.

**Cut:** Simplified the return types to `Result<Relation, DatabaseError>`. Replaced `return Ok(None)` with `break`ing and returning the empty `Relation`, using `.cardinality() == 0` for checks.

**Saved:** Multiple layers of pattern matching, boilerplate unwrapping, and unnecessary abstraction, adhering tightly to the KISS principle.
