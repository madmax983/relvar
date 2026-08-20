# ⚡ Bolt: [Query Plan Rename Into Optimization]

💡 What: Changed the `Query::Rename` operator evaluation to use `relation.rename_into` instead of `relation.rename`.
🎯 Why: `Query::Rename` owns the intermediate result relation, so it can perform an in-place renaming of tuples rather than cloning the attributes.
📊 Impact: Eliminates tuple/string allocations when renaming attributes within a query pipeline.
🔬 Measurement: Run `cargo bench` and observe improvements in query execution for pipelines utilizing renaming.
