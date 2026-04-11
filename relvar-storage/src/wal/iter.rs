use crate::wal::record::WalRecord;
use crate::wal::{Lsn, WalError};
use std::convert::TryFrom;

/// Iterator over WAL records parsed from a raw byte buffer.
pub(crate) struct WalRecordIter<'a> {
    buffer: &'a [u8],
    offset: usize,
}

impl<'a> WalRecordIter<'a> {
    /// Creates a new iterator over the given buffer.
    pub fn new(buffer: &'a [u8]) -> Self {
        Self { buffer, offset: 0 }
    }
}

impl<'a> WalRecordIter<'a> {
    fn read_lsn(&mut self) -> Lsn {
        let lsn_bytes: [u8; 8] = self.buffer[self.offset..self.offset + 8]
            .try_into()
            .unwrap(); // Length is checked above
        let lsn_u64 = u64::from_le_bytes(lsn_bytes);
        self.offset += 8;
        Lsn::new(lsn_u64)
    }

    fn read_length(&mut self, lsn: Lsn) -> Result<(u64, usize), WalError> {
        let len_bytes: [u8; 8] = self.buffer[self.offset..self.offset + 8]
            .try_into()
            .unwrap(); // Length is checked above
        let record_len_u64 = u64::from_le_bytes(len_bytes);
        let record_len = match usize::try_from(record_len_u64) {
            Ok(len) => len,
            Err(_) => {
                return Err(WalError::Corrupted(
                    lsn,
                    format!("Record length {} exceeds memory limits", record_len_u64),
                ));
            }
        };
        self.offset += 8;
        Ok((record_len_u64, record_len))
    }
}

impl<'a> Iterator for WalRecordIter<'a> {
    type Item = Result<(Lsn, u64, &'a [u8]), WalError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Need at least 16 bytes for LSN (8 bytes) + length (8 bytes)
        if self.offset + 16 > self.buffer.len() {
            return None;
        }

        let lsn = self.read_lsn();

        let (record_len_u64, record_len) = match self.read_length(lsn) {
            Ok(res) => res,
            Err(e) => return Some(Err(e)),
        };

        // Validate remaining bytes
        let end_offset = match self.offset.checked_add(record_len) {
            Some(end) => end,
            None => {
                return Some(Err(WalError::Corrupted(
                    lsn,
                    "Record length causes offset overflow".to_string(),
                )));
            }
        };

        if end_offset > self.buffer.len() {
            // Here we return Corrupted, so that scan() can fail on partial records.
            // scan_for_last_lsn() will catch the error and break gracefully,
            // treating it as the end of the valid log.
            return Some(Err(WalError::Corrupted(
                lsn,
                "Record extends beyond file".to_string(),
            )));
        }

        let record_bytes = &self.buffer[self.offset..end_offset];
        self.offset = end_offset;

        Some(Ok((lsn, record_len_u64, record_bytes)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_buffer() {
        let buffer = vec![];
        let mut iter = WalRecordIter::new(&buffer);
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_valid_record() {
        let mut buffer = vec![];
        // LSN
        buffer.extend_from_slice(&1u64.to_le_bytes());
        // Length
        buffer.extend_from_slice(&5u64.to_le_bytes());
        // Data
        buffer.extend_from_slice(&[1, 2, 3, 4, 5]);

        let mut iter = WalRecordIter::new(&buffer);
        let item = iter.next().unwrap().unwrap();
        assert_eq!(item.0.value(), 1);
        assert_eq!(item.1, 5);
        assert_eq!(item.2, &[1, 2, 3, 4, 5]);

        assert!(iter.next().is_none());
    }

    #[test]
    fn test_corrupted_length_exceeds_memory() {
        let mut buffer = vec![];
        // LSN
        buffer.extend_from_slice(&1u64.to_le_bytes());
        // Length - extremely large
        buffer.extend_from_slice(&u64::MAX.to_le_bytes());

        let mut iter = WalRecordIter::new(&buffer);
        let err = iter.next().unwrap().unwrap_err();
        match err {
            WalError::Corrupted(lsn, msg) => {
                assert_eq!(lsn.value(), 1);
                assert!(msg.contains("exceeds memory limits") || msg.contains("offset overflow"));
            }
            _ => panic!("Expected Corrupted error"),
        }
    }

    #[test]
    fn test_corrupted_record_extends_beyond_file() {
        let mut buffer = vec![];
        // LSN
        buffer.extend_from_slice(&1u64.to_le_bytes());
        // Length
        buffer.extend_from_slice(&100u64.to_le_bytes()); // Expects 100 bytes of data
        // Data
        buffer.extend_from_slice(&[1, 2, 3]); // Only 3 bytes available

        let mut iter = WalRecordIter::new(&buffer);
        let err = iter.next().unwrap().unwrap_err();
        match err {
            WalError::Corrupted(lsn, msg) => {
                assert_eq!(lsn.value(), 1);
                assert!(msg.contains("Record extends beyond file"));
            }
            _ => panic!("Expected Corrupted error"),
        }
    }
}
