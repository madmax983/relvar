# 🌟 Nova: Relational Voting System

💡 **The Spark:** I noticed we have ranking and aggregation capabilities but haven't applied them to social choice theory. Can we model complex voting methods like Condorcet using purely relational operations?

🚀 **The Feature:** Implemented `CondorcetElection` in a new `voting` experimental module. It takes a preferences relation `(voter, candidate, rank)`, evaluates all pairwise matchups using Joins and Extend, scores them with Summarize, and identifies the overall undefeated Condorcet winner through relational difference.

🔮 **The Potential:** Could be used for group decision-making tools, recommendation system consensus, or implementing other complex ranked-choice election systems (like STV/IRV) purely within a database query.

⚠️ **Risk:** Low. Isolated in `src/experimental/voting.rs` and added to `src/experimental/mod.rs`.
