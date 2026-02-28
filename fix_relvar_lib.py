with open("relvar/src/lib.rs", "r") as f:
    content = f.read()
content = content.replace("pub use relvar_core::algebra::{", "pub use relvar_core::{Aggregation, AggregationFn};\npub use relvar_core::algebra::{")
with open("relvar/src/lib.rs", "w") as f:
    f.write(content)
