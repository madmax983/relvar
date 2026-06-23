import re

with open('relvar/src/lib.rs', 'r') as f:
    content = f.read()

# Let's just fix the whole file up to that point
# Instead of regex, let's locate "pub mod tools;" and just replace the rest of the file
index = content.find("pub mod tools;")
if index != -1:
    new_content = content[:index] + """pub mod tools;

/// Deprecated visualizer module
#[deprecated(note = "Use `relvar::tools` instead")]
pub mod visualizer {
    pub use crate::tools::SchemaVisualizer;
}

/// Data import and export functionality.
#[deprecated(note = "Use `relvar::tools` instead")]
pub mod data {
    /// Deprecated exporter module
    pub mod exporter {
        pub use crate::tools::{to_csv, to_json, to_ascii_table, ExporterError};
    }
    /// Deprecated importer module
    pub mod importer {
        pub use crate::tools::{from_csv, from_json, ImporterError};
    }
}
"""
    with open('relvar/src/lib.rs', 'w') as f:
        f.write(new_content)
