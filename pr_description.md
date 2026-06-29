🌟 Nova: Relational Material Requirements Planning (MRP)

💡 **The Spark:** I noticed we have complex set operations but haven't demonstrated how they apply to classic manufacturing problems like Bill of Materials explosion and supply chain shortages.
🚀 **The Feature:** Implemented `explode_bom` and `compute_shortages` using purely relational algebra (semijoins, semidifferences, joins, extends, and summarizations) in `src/experimental/mrp.rs`.
🔮 **The Potential:** Demonstrates how declarative relational algebra natively solves graph-like tree explosions and constraint resolutions dynamically, without needing procedural node-walking.
⚠️ **Risk:** Low. Completely isolated within the `src/experimental` module.
