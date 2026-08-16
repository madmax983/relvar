# ⚒️ Forge: [Refactor KnowledgeGraph match_pattern]

🚮 Smell: The `match_pattern` function in `relvar/src/experimental/knowledge_graph.rs` was a long function with 3 exact repetitions of a large `match` block to evaluate the subject, predicate, and object terms.
✨ Solution: Extracted the evaluation logic into a helper function `evaluate_term` and replaced the repetitive blocks with concise calls.
🧼 Benefit: Significantly reduces boilerplate and cognitive load. The function is much easier to read and maintain.
🛡️ Verification: Tests passed. No logic changed.
