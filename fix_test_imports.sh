for file in relvar/tests/warden_json_import.rs relvar/tests/warden_exploit_csv_global_dos.rs relvar/tests/visualizer_security.rs; do
    sed -i 's|use relvar::tools::importer::{self, ImporterError};|use relvar::tools::importer::{self, ImporterError};|g' "$file"
done
