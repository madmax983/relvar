⚡ Bolt: Optimize query evaluation using consuming operations

💡 **What:** Modified `Query::execute` to use consuming `_into` methods (`project_into`, `rename_into`) instead of their borrowing counterparts (`project`, `rename`) when transforming `Relation` results.
🎯 **Why:** Since the intermediate relation created by the child query is owned by the current query node and will not be used again, using borrowing methods caused unnecessary data cloning and heap allocations for tuples. Consuming methods allow reusing the existing data allocation.
📊 **Impact:** Reduces heap allocations in complex query evaluation pipelines and makes operations like `project` and `rename` significantly faster on large datasets. Benchmarking on `rename` shows measurable performance improvements.
🔬 **Measurement:** Execute `cargo bench -p relvar-core --bench algebra -- bench_rename` to verify.
