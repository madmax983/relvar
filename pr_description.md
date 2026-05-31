💡 **The Spark:** I noticed we can perform iterative updates natively with relational algebra but don't have an implementation for Reinforcement Learning/Dynamic Programming algorithms. Can we solve a Markov Decision Process (MDP) using purely relational sets?

🚀 **The Feature:** Implemented `MarkovDecisionProcess` within `relvar/src/experimental/mdp.rs`. This feature uses purely relational algebra (via Joins, Summarizations using Sum/Max Aggregations, and Extensions) to perform Value Iteration to find optimal values, and extract the resulting policy. It is completely additive.

🔭 **The Potential:** Relational engines can natively compute dynamic programming iterations and state updates. This proves that an RDBMS can also act as a reinforcement learning solver without leaving the database layer!

⚠️ **Risk:** Low. The feature is purely additive and safely contained within the `src/experimental/` module structure.
