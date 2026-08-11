# 🌟 Nova: [Relational Voting System]

💡 **The Spark:** I noticed we use relations for constraints and logic, but what about collective decision-making? Can we evaluate complex election methods natively within the database?

🚀 **The Feature:** Implemented `VotingEngine` using purely relational operations. It declaratively computes Borda Count scores and resolves Condorcet winners via pairwise self-joins, extensions, and aggregations.

🔮 **The Potential:** Could be used for decentralized autonomous organizations (DAOs), collective decision systems, or preference ranking analysis directly in the DB!

⚠️ **Risk:** Low. Isolated entirely within `src/experimental/voting.rs` and exposed only behind the `nova` feature flag.
