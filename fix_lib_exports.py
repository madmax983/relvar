import os
import re

with open('relvar/src/lib.rs', 'r') as f:
    content = f.read()

# We want to restore backwards compatibility while keeping internal modules private
# The original code was:
# #[deprecated(note = "Use `relvar::tools::visualizer` instead")]
# pub use tools::visualizer;
#
# /// Data import and export functionality.
# #[deprecated(note = "Use `relvar::tools` instead")]
# pub mod data {
#     pub use crate::tools::exporter;
#     pub use crate::tools::importer;
# }

# We changed it to:
# #[deprecated(note = "Use `relvar::tools::visualizer` instead")]
# pub use tools::*;
#
# /// Data import and export functionality.
# #[deprecated(note = "Use `relvar::tools` instead")]
# pub mod data {
#     pub use crate::tools::*;
#
# }

# We need to change it to properly expose visualizer, exporter, and importer.
# To do this without exposing the modules themselves, we can re-export the *contents*
# of the tools module explicitly or create facade modules.
# Wait, if `tools::visualizer` was exposed before, we can just create a facade module `visualizer` in `lib.rs`:

replacement = """pub mod tools;

#[deprecated(note = "Use `relvar::tools` instead")]
pub mod visualizer {
    pub use crate::tools::SchemaVisualizer;
}

/// Data import and export functionality.
#[deprecated(note = "Use `relvar::tools` instead")]
pub mod data {
    pub mod exporter {
        pub use crate::tools::{to_csv, to_json, to_ascii_table, ExporterError};
    }
    pub mod importer {
        pub use crate::tools::{from_csv, from_json, ImporterError};
    }
}
"""

content = re.sub(r'pub mod tools;[\s\S]*pub mod data \{[\s\S]*?\}', replacement, content)

with open('relvar/src/lib.rs', 'w') as f:
    f.write(content)
