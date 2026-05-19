🗺️ Atlas: [architectural change]

🕸️ Tangle: The `RelationMetadata` struct name was duplicated across `relvar-core::storage_engine` and `relvar-storage::storage::catalog`, representing different concepts (public logical metadata vs internal physical location tracking), violating clear domain boundaries and causing a naming collision.
📐 Blueprint: Renamed the catalog-specific struct to `CatalogEntry` to distinguish it from the core logical `RelationMetadata`, removing the name collision and clarifying the physical storage responsibilities.
🧱 Stability: Reduced coupling by clarifying domain boundaries; internal physical metadata is now distinctly named from logical core metadata.
🔬 Verification: Builds successfully, all tests pass, and strict separation is enforced without leaky abstractions.