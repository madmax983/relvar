Title: 🌟 Nova: Relational Turing Machine

💡 **The Spark:** "Can we prove our relational algebra engine is Turing complete by actually building a Turing Machine inside it?"
🚀 **The Feature:** Implemented `RelationalTuringMachine`, a Turing machine simulator where the tape and transition rules are modeled purely as relations, and state progression is resolved using relational queries and operations.
🔮 **The Potential:** Proves the expressive power of the underlying relational framework and opens up possibilities for executing complex algorithmic workflows entirely within the database engine!
⚠️ **Risk:** Low. The feature is completely isolated in `src/experimental/turing.rs` and gated behind the `nova` feature flag.
