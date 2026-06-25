🌟 Nova: Relational Turing Machine

💡 **The Spark:** "Can we prove Relational Algebra is Turing Complete by literally building a Turing Machine engine in it?"
🚀 **The Feature:** Implemented `turing_machine::step` which evaluates a single machine cycle. `tape`, `head`, and `transitions` are modeled natively as Relations. The transition logic is resolved entirely through `Join`, `Extend`, `Union`, and `Difference`. Included a 2-state Busy Beaver test that successfully halts after 6 steps!
🔮 **The Potential:** Demonstrates the profound expressiveness of purely declarative relational operators. We don't need loops to perform arbitrary computation if we can iterate set operations!
⚠️ **Risk:** Low. Fully isolated in `src/experimental/turing_machine.rs` with no changes to core database logic.
