Title: 🌟 Nova: Relational Turing Machine

💡 **The Spark:** "I wonder if we can prove the relational model's computational universality by building a Turing Machine purely out of relations?"

🚀 **The Feature:** "Implemented `TuringMachine` in `src/experimental/turing.rs`. It models the Tape, State, and Rules as standard relations. A single step is computed via `join` (to find applicable rule), `extend` (to compute new head position), and `difference`/`union` (to update the tape). Fully passes tests evaluating a simple algorithm."

🔮 **The Potential:** "This demonstrates that relational algebra is effectively Turing Complete. This conceptual proof allows us to build arbitrarily complex logic evaluation engines directly within the relational kernel."

⚠️ **Risk:** "Low. The feature is purely additive and isolated within the `experimental` module behind the `nova` feature flag."
