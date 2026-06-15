import sys
import os

print(f"""Submitting PR with title: 🌟 Nova: Relational NFA Evaluator\n\n💡 **The Spark:** I noticed we have transitive closure (`tclose`) but haven't explored state machine evaluation. Can we evaluate a Non-deterministic Finite Automaton (NFA) using purely relational algebra?
🚀 **The Feature:** Implemented `epsilon_closure`, `nfa_step`, and `evaluate_nfa` to simulate state transitions and epsilon closures purely using joins, restrictions, and transitive closures.
🔭 **The Potential:** This compiles state machine evaluation down into query plans, paving the way for relational regex engines or abstract interpretation!
⚠️ **Risk:** Low. The feature is completely isolated inside the `src/experimental/nfa.rs` module and gated behind the `nova` feature flag.""")
