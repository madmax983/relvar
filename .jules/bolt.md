## 2025-06-25 - Consuming Relations in Query Execution
**Learning:** In `Query::execute`, operations like `Project` and `Rename` were unnecessarily allocating intermediate relation data by using borrowing methods (`project` and `rename`) on owned relation results instead of their consuming counterparts (`project_into` and `rename_into`).
**Action:** Use consuming `_into` methods when transforming owned types like `Relation` in pipelines to avoid unnecessary cloning of sets/data, keeping the zero-cost abstractions philosophy.
## 2025-06-25 - Consuming Relations in Query Execution
**Learning:** In `Query::execute`, operations like `Project` and `Rename` were unnecessarily allocating intermediate relation data by using borrowing methods (`project` and `rename`) on owned relation results instead of their consuming counterparts (`project_into` and `rename_into`).
**Action:** Use consuming `_into` methods when transforming owned types like `Relation` in pipelines to avoid unnecessary cloning of sets/data, keeping the zero-cost abstractions philosophy.
## 2025-06-25 - Consuming Relations in Query Execution
**Learning:** In `Query::execute`, operations like `Project` and `Rename` were unnecessarily allocating intermediate relation data by using borrowing methods (`project` and `rename`) on owned relation results instead of their consuming counterparts (`project_into` and `rename_into`).
**Action:** Use consuming `_into` methods when transforming owned types like `Relation` in pipelines to avoid unnecessary cloning of sets/data, keeping the zero-cost abstractions philosophy.
