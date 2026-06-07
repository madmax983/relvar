🪒 Razor: [Simplicity Enforcement]

This PR simplifies overly complicated nested `Result<Option<...>>` return types in accordance with Razor's essentialist philosophy: "This `Result<Option<Result<T>>>` is a crime."

### ✂️ Reduction

**Bloat:** The methods `KeyConstraints::would_violate_on_insert` and `RelationalAutomaton::process_input_string` returned `Result<Option<...>>`, which forced callers into nested match statements and muddled the error channel with the success state.

**Cut:** I refactored these to directly return `Result<(), KeyConstraintError>` and `Result<Relation, DatabaseError>`, using the `Err` channel specifically for constraint violations, rather than returning `Ok(Some(violation))` or an unnecessary wrapper.

**Saved:** Eliminates nested matches and unwraps across testing and application logic. Callers now use standard `?` bubbling and the codebase aligns better with standard Rust error handling semantics.
