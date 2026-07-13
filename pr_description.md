💡 **The Spark:** I noticed we can easily define mathematical updates like the Bellman equation using purely relational operations. By implementing a Q-Learning agent using relational algebra, we can use the engine to solve tabular Reinforcement Learning problems!

🚀 **The Feature:** Implemented `QLearningAgent` in `src/experimental/q_learning.rs` which uses pure relational algebra (`Join`, `Summarize`, `Extend`, `Union`, `Difference`) to evaluate the Bellman equation in parallel for entire batches of experiences.

🔮 **The Potential:** Turns the relational database into a native environment for training tabular Reinforcement Learning agents! This proves that database engines can directly execute complex ML algorithms.

⚠️ **Risk:** Low. Isolated in `src/experimental/q_learning.rs` and properly tested with Bellman equation validations.
