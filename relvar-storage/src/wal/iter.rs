//! WAL Iterator implementation.
//!
//! This module provides an iterator over WAL records parsed from a raw byte buffer.

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
        self.offset = self.offset.checked_add(8).expect("Offset overflow");
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
        self.offset = self.offset.checked_add(8).expect("Offset overflow");
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
    fn test_iter_short_buffer() {
        let buffer = vec![1, 2, 3];
        let mut iter = WalRecordIter::new(&buffer);
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_iter_record_too_large() {
        let mut buffer = Vec::new();
        // LSN
        buffer.extend_from_slice(&1u64.to_le_bytes());
        // Length (much larger than remaining buffer)
        buffer.extend_from_slice(&1000u64.to_le_bytes());
        // Data (short)
        buffer.extend_from_slice(&[1, 2, 3]);

        let mut iter = WalRecordIter::new(&buffer);
        let result = iter.next().unwrap();
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(matches!(e, WalError::Corrupted(_, _)));
        }
    }
}
