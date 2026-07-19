💡 **The Spark:** I noticed we have complex mathematics and aggregations, but no real-time multi-agent simulations! What if flocking logic was just a database query?
🚀 **The Feature:** Implemented `BoidsSimulation` which calculates Craig Reynolds' Boids flocking behaviors (separation, alignment, cohesion) purely through relational algebra (`theta_join`, `extend`, `summarize`). Included fix for clippy warning in `timeseries.rs`.
🔮 **The Potential:** Proves that complex multi-agent physics and spatial interactions can be modeled entirely declaratively in our relational engine. This paves the way for particle systems, swarm robotics, or spatial games running directly inside the DB!
⚠️ **Risk:** Low. Isolated in `src/experimental/boids.rs` and added tests.
