//! IEEE CRC-32 (ISO 3309) checksum.
//!
//! Pure `core` implementation with a `const`-computed lookup table — no
//! dependencies, no heap allocation, usable on every target including
//! `thumbv8m`. Used by the WAL v2 frame format to detect torn writes:
//! each frame's CRC covers its length headers plus payload, so a partial
//! write can never decode silently.
//!
//! Polynomial: `0xEDB88320` (reflected form of the IEEE polynomial).

/// CRC-32 lookup table, computed at compile time.
const fn build_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 == 1 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

static CRC_TABLE: [u32; 256] = build_table();

/// Computes the IEEE CRC-32 checksum of `data`.
///
/// # Examples
///
/// ```
/// use relvar_storage_core::crc::crc32;
///
/// // Standard check vector.
/// assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
/// ```
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        let index = ((crc ^ u32::from(byte)) & 0xFF) as usize;
        crc = CRC_TABLE[index] ^ (crc >> 8);
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32_standard_vector() {
        // The canonical CRC-32 check value.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn test_crc32_empty() {
        assert_eq!(crc32(b""), 0x0000_0000);
    }

    #[test]
    fn test_crc32_known_vectors() {
        assert_eq!(crc32(b"a"), 0xE8B7_BE43);
        assert_eq!(crc32(b"abc"), 0x3524_41C2);
        assert_eq!(crc32(b"message digest"), 0x2015_9D7F);
    }

    #[test]
    fn test_crc32_single_bit_flip_changes_checksum() {
        let a = crc32(b"hello world");
        let mut tampered = *b"hello world";
        tampered[0] ^= 0x01;
        assert_ne!(a, crc32(&tampered));
    }

    #[test]
    fn test_crc32_is_deterministic() {
        let data = b"The quick brown fox jumps over the lazy dog";
        assert_eq!(crc32(data), crc32(data));
    }

    #[test]
    fn test_crc32_table_first_entries() {
        // Spot-check the const-computed table against known values.
        assert_eq!(CRC_TABLE[0], 0x0000_0000);
        assert_eq!(CRC_TABLE[1], 0x7707_3096);
        assert_eq!(CRC_TABLE[255], 0x2D02_EF8D);
    }
}
