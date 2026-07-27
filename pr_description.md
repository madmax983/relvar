Title: ⚒️ Forge: [Refactor compute_ungrouped_tuples]

🚮 Smell: The compute_ungrouped_tuples function in relvar-core/src/algebra/group.rs was a God function that deeply nested iteration, manual relation-valued attribute extraction, and tuple merging into a single long block.
✨ Solution: Extracted the get_rva_relation helper to safely retrieve and cast the relation-valued attribute. Used iterator patterns (.filter(), .map(), .extend()) to replace manual loops for populating the invariant non-RVA attributes and combining tuples.
🧼 Benefit: Significantly flattens the nesting structure, separates concerns (attribute extraction vs. tuple composition), and leverages idiomatic Rust iterators to improve readability without altering logical behavior.
🛡️ Verification: Tests passed. No logic changed.
