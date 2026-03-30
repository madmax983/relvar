use relvar::tools::importer;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use std::io::Read;

struct DuplicateReader {
    line: &'static [u8],
    count: usize,
    limit: usize,
    pos: usize,
}

impl Read for DuplicateReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.count >= self.limit {
            return Ok(0);
        }
        let mut written = 0;
        while written < buf.len() && self.count < self.limit {
            let n = std::cmp::min(buf.len() - written, self.line.len() - self.pos);
            buf[written..written + n].copy_from_slice(&self.line[self.pos..self.pos + n]);
            written += n;
            self.pos += n;
            if self.pos >= self.line.len() {
                self.pos = 0;
                self.count += 1;
            }
        }
        Ok(written)
    }
}

#[test]
fn test_csv_importer_duplicate_row_limit() {
    let heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);
    let rel_type = RelationType::new(heading);

    // Header + 100_001 duplicate rows.
    // MAX_IMPORT_ROWS is 100_000.
    // If the importer checks cardinality instead of processed rows, it will process all 200,000 without returning LimitExceeded, because cardinality is 1.

    let mut header = std::io::Cursor::new("id,name\n".as_bytes());
    let duplicates = DuplicateReader {
        line: b"1,\"Alice\"\n",
        count: 0,
        limit: 200_000,
        pos: 0,
    };

    let reader = std::io::Read::chain(&mut header, duplicates);

    let result = importer::from_csv(reader, rel_type, ',');

    // It must fail with LimitExceeded
    match result {
        Err(importer::ImporterError::LimitExceeded(_)) => {
            // Expected
        }
        res => panic!("Expected LimitExceeded, got: {:?}", res),
    }
}
