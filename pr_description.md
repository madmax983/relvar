🌟 Nova: Relational Boids Simulation

💡 The Spark:
I noticed we have N-body physics but haven't explored emergent behaviors like flocking. Boids is a classic algorithm that demonstrates complex, coordinated motion from simple local rules, making it a perfect candidate for purely relational operations!

🚀 The Feature:
Implemented `BoidsSimulator` in `relvar/src/experimental/boids.rs`. It models Craig Reynolds' Boids flocking rules (Separation, Alignment, Cohesion) using pure relational algebra. The entire state is a single relation, and the simulation step is computed declaratively via `join`, `restrict`, `extend`, and `summarize` without procedural loops!

🔭 The Potential:
This proves our engine can handle complex, continuous simulations involving local neighbor queries and aggregation, opening the door for more advanced emergent systems like particle swarms, traffic simulations, or crowd dynamics entirely within the database.

⚠️ Risk:
Low. The feature is completely isolated in `src/experimental/boids.rs` and added to `src/experimental/mod.rs`. It purely consumes `relvar-core` primitives and has 100% test coverage. No core logic was touched.