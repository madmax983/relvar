import sys

filepath = 'relvar/tests/sentry_timeseries_collision.rs'
with open(filepath, 'r') as f:
    content = f.read()

content = content.replace(
    '|t| t.get_typed::<i64>("time") == Some(2)',
    '|t: &Tuple| t.get_typed::<i64>("time") == Some(2)'
)

with open(filepath, 'w') as f:
    f.write(content)

filepath = 'relvar/tests/warden_csv_injection.rs'
with open(filepath, 'r') as f:
    content = f.read()
content = content.replace('use relvar::tools::exporter;', 'use relvar::data::exporter;')
with open(filepath, 'w') as f:
    f.write(content)

filepath = 'relvar/tests/warden_json_import.rs'
with open(filepath, 'r') as f:
    content = f.read()
content = content.replace('use relvar::tools::importer::{self, ImporterError};', 'use relvar::data::importer::{self, ImporterError};')
with open(filepath, 'w') as f:
    f.write(content)

filepath = 'relvar/tests/warden_exploit_csv_global_dos.rs'
with open(filepath, 'r') as f:
    content = f.read()
content = content.replace('use relvar::tools::importer::{self, ImporterError};', 'use relvar::data::importer::{self, ImporterError};')
with open(filepath, 'w') as f:
    f.write(content)
