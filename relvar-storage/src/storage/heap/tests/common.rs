#![allow(unused_imports)]
use crate::mvcc::TransactionSnapshot;
use crate::storage::heap::*;
use crate::wal::Lsn;
use relvar_core::tuple;
use relvar_core::types::{ScalarType, TupleType};
use relvar_core::values::Relation;
use relvar_core::values::Tuple;
use tempfile::NamedTempFile;

pub(crate) fn create_test_relation_type() -> RelationType {
    let heading = TupleType::new()
        .with_attribute("id".to_string(), ScalarType::Int)
        .with_attribute("name".to_string(), ScalarType::String);
    RelationType::new(heading)
}

pub(crate) fn deserialize_versioned_page_for_test(
    page_data: &[u8],
) -> Result<VersionedSlottedPage, postcard::Error> {
    if page_data.len() >= 5 && page_data[0] == PAGE_FORMAT_VERSION {
        let slot_dir_len = u32::from_le_bytes(
            page_data[1..5]
                .try_into()
                .map_err(|_| postcard::Error::DeserializeUnexpectedEnd)?,
        ) as usize;
        postcard::from_bytes(&page_data[5..5 + slot_dir_len])
    } else {
        postcard::from_bytes(page_data)
    }
}
pub(crate) fn test_lsn(value: u64) -> crate::wal::Lsn {
    crate::wal::Lsn::new(value)
}

pub(crate) fn test_txn(value: u64) -> crate::wal::TransactionId {
    crate::wal::TransactionId::new(value)
}
