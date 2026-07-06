💡 **The Spark:** I noticed we can simulate Turing machines, game of life, and physics, but we lack a demonstration of emergent multi-agent behavior purely using relational algebra.
🚀 **The Feature:** Implemented `BoidsEngine` in `src/experimental/boids.rs`, a Relational Boids Flocking Simulation (Reynolds, 1987) that implements cohesion, alignment, and separation purely with Relation joins, restricts, and aggregations.
🔮 **The Potential:** Showcases that complex, emergent agent-based modeling can be fully simulated in a declarative relational paradigm without imperative loops. Could be extended into a relational game engine AI module!
⚠️ **Risk:** Low. The feature is completely isolated in `src/experimental/boids.rs`.
