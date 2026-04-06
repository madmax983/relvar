## 2025-04-05 - Extracted Query Module to Facade Crate
**Tangle:** The `query` module (which parses and acts as a generic AST over `relvar-core`) was tightly coupled inside `relvar-core`, expanding the core's surface area.
**Blueprint:** Moved `relvar-core/src/query/` to `relvar/src/query/` to keep the core crate purely focused on Relational Algebra (Types, Values, Constraints, DB storage) and to make the Query layer part of the external UI facade.
## 2025-04-05 - Extracted Query Module to Facade Crate
**Tangle:** The `query` module (which parses and acts as a generic AST over `relvar-core`) was tightly coupled inside `relvar-core`, expanding the core's surface area.
**Blueprint:** Moved `relvar-core/src/query/` to `relvar/src/query/` to keep the core crate purely focused on Relational Algebra (Types, Values, Constraints, DB storage) and to make the Query layer part of the external UI facade.
