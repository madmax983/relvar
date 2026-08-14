sed -i 's/use relvar::tools::importer;/use relvar::data::importer;/' relvar/tests/warden_exploit_csv_dos.rs
sed -i 's/use relvar::tools::importer::{self, ImporterError};/use relvar::data::importer::{self, ImporterError};/' relvar/tests/warden_exploit_csv_global_dos.rs
sed -i 's/use relvar::tools::importer::{self, ImporterError};/use relvar::data::importer::{self, ImporterError};/' relvar/tests/warden_json_import.rs
sed -i 's/use relvar::tools::exporter;/use relvar::data::exporter;/' relvar/tests/warden_csv_injection.rs
sed -i 's/let cardinality = relation.cardinality();/let cardinality = relation.unwrap().cardinality();/' relvar/tests/warden_exploit_csv_global_dos.rs
