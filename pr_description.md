💡 **The Spark:** We have `particles` in `physics.rs`, but what about complex emergent behavior? Can we model Craig Reynolds' Boids (flocking) using purely relational algebra?
🚀 **The Feature:** Implemented `BoidSimulation` which models separation, alignment, and cohesion purely through relational cross-joins, distances, restrictions (neighbors), extensions, and summarizations.
🔮 **The Potential:** Proves that complex multi-agent simulations and emergent behaviors can be evaluated entirely declaratively within the database engine without imperative looping.
⚠️ **Risk:** Low. Isolated in `src/experimental/boids.rs`.
