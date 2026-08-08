💡 **The Spark:** "Wordle uses a tricky left-to-right frequency constraint for 'Yellow' vs 'Gray' duplicate letters. Can we model this using pure declarative relational algebra instead of procedural loops?"
🚀 **The Feature:** "Implemented `WordleEngine` that translates letters to relations and computes Wordle state results declaratively. Uses `theta_join` and aggregation to generate positional ranks that satisfy the duplicate constraints!"
🔭 **The Potential:** "Demonstrates that even complex ordered game logic constraints can compile down to unordered pure relational operations."
⚠️ **Risk:** "Low. Isolated in `src/experimental/wordle.rs`."
