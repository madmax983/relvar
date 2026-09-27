#![allow(unused_imports)]
use crate::mvcc::TransactionSnapshot;
use crate::storage::heap::*;
use crate::wal::Lsn;
use relvar_core::tuple;
use relvar_core::types::{ScalarType, TupleType};
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

/// Test version pool: small fixed capacities keep tests fast while still
/// exercising the pooled write path (no per-version allocation).
pub(crate) fn test_version_pool() -> crate::mvcc::VersionPool {
    crate::mvcc::VersionPool::with_buffer_capacity(1024, 2)
}

/// Builds a [`TxnPool`](crate::mvcc::TxnPool) with the given
/// `(txn_id, commit_lsn)` pairs committed (each begun 10 LSNs earlier, with
/// an open horizon), for tests that need GC to resolve commit LSNs.
pub(crate) fn pool_with_commits(commits: &[(u64, u64)]) -> crate::mvcc::TxnPool {
    let mut pool = crate::mvcc::TxnPool::new(16);
    let horizon = crate::wal::TransactionId::new(u64::MAX);
    for &(id, commit_lsn) in commits {
        pool.begin(test_txn(id), test_lsn(commit_lsn - 10), horizon)
            .unwrap();
        pool.commit(test_txn(id), test_lsn(commit_lsn)).unwrap();
    }
    pool
}

/// Test fixture for `scan_visible`: a bounded transaction pool plus the
/// engine's committed set, mirroring the engine's begin/commit flow so
/// visibility tests exercise the real liveness derivation.
pub(crate) struct MvccFixture {
    pub pool: crate::mvcc::TxnPool,
    pub committed: std::collections::HashSet<crate::wal::TransactionId>,
    /// Caller-owned version pool for the pooled write path. Behind a
    /// `RefCell` so the `&self` scan helper can check out buffers.
    pub version_pool: std::cell::RefCell<crate::mvcc::VersionPool>,
}

impl MvccFixture {
    pub fn new() -> Self {
        Self {
            pool: crate::mvcc::TxnPool::new(16),
            committed: std::collections::HashSet::new(),
            version_pool: std::cell::RefCell::new(test_version_pool()),
        }
    }

    /// Begins `txn_id` at `lsn` with an open horizon; returns its snapshot.
    pub fn begin(
        &mut self,
        txn_id: crate::wal::TransactionId,
        lsn: u64,
    ) -> crate::mvcc::TransactionSnapshot {
        self.pool
            .begin(
                txn_id,
                crate::wal::Lsn::new(lsn),
                crate::wal::TransactionId::new(u64::MAX),
            )
            .unwrap()
    }

    /// Commits `txn_id` at `lsn` and records it in the committed set.
    pub fn commit(&mut self, txn_id: crate::wal::TransactionId, lsn: u64) {
        self.pool.commit(txn_id, crate::wal::Lsn::new(lsn)).unwrap();
        self.committed.insert(txn_id);
    }

    /// Runs `heap.scan_visible` for `snapshot` against this fixture's pool.
    pub fn scan_visible<D: relvar_storage_core::device::BlockDevice>(
        &self,
        heap: &mut HeapFile<D>,
        snapshot: &crate::mvcc::TransactionSnapshot,
    ) -> Result<Vec<relvar_core::values::Tuple>, crate::storage::heap::HeapError> {
        heap.scan_visible(
            snapshot,
            &self.pool,
            &mut self.version_pool.borrow_mut(),
            &self.committed,
        )
    }
}

/// Test block device that fails writes on demand after a configured count.
///
/// Wraps [`MemBlockDevice`] with two independent failure modes:
/// - `fail_after(n)`: the first `write_page` issued once `n` writes have
///   succeeded fails once, then disarms (so a rollback's writes can
///   succeed).
/// - `skip_then_fail_next(skip, n)`: the next `skip` `write_page` calls
///   succeed, then the following `n` fail, then the device disarms itself.
///   This exercises double-failure paths where both the operation and its
///   compensating rollback hit device errors.
///
/// The threshold and counters are shared via `Rc`, so the test can arm the
/// failure after setup writes complete (the device itself is owned by the
/// heap).
pub(crate) struct FailAfterDevice {
    inner: relvar_storage_core::device::MemBlockDevice,
    writes: std::rc::Rc<std::cell::Cell<usize>>,
    fail_after: std::rc::Rc<std::cell::Cell<usize>>,
    fail_remaining: std::rc::Rc<std::cell::Cell<usize>>,
    skip_writes: std::rc::Rc<std::cell::Cell<usize>>,
}

/// Handle for arming and inspecting a [`FailAfterDevice`].
pub(crate) struct FailAfterHandle {
    writes: std::rc::Rc<std::cell::Cell<usize>>,
    fail_after: std::rc::Rc<std::cell::Cell<usize>>,
    fail_remaining: std::rc::Rc<std::cell::Cell<usize>>,
    skip_writes: std::rc::Rc<std::cell::Cell<usize>>,
}

impl FailAfterDevice {
    /// Creates the device with failures disabled, plus a handle to arm them.
    pub(crate) fn new() -> (Self, FailAfterHandle) {
        let writes = std::rc::Rc::new(std::cell::Cell::new(0usize));
        let fail_after = std::rc::Rc::new(std::cell::Cell::new(usize::MAX));
        let fail_remaining = std::rc::Rc::new(std::cell::Cell::new(0usize));
        let skip_writes = std::rc::Rc::new(std::cell::Cell::new(0usize));
        (
            Self {
                inner: relvar_storage_core::device::MemBlockDevice::new(),
                writes: writes.clone(),
                fail_after: fail_after.clone(),
                fail_remaining: fail_remaining.clone(),
                skip_writes: skip_writes.clone(),
            },
            FailAfterHandle {
                writes,
                fail_after,
                fail_remaining,
                skip_writes,
            },
        )
    }
}

impl FailAfterHandle {
    /// Fail the first `write_page` issued once `n` writes have succeeded;
    /// the device disarms itself after injecting the failure.
    pub(crate) fn fail_after(&self, n: usize) {
        self.fail_after.set(n);
    }

    /// Let `skip` writes succeed, then fail the next `n` writes, then disarm.
    pub(crate) fn skip_then_fail_next(&self, skip: usize, n: usize) {
        self.skip_writes.set(skip);
        self.fail_remaining.set(n);
    }

    /// Number of writes that have succeeded so far.
    pub(crate) fn writes(&self) -> usize {
        self.writes.get()
    }
}

impl relvar_storage_core::device::BlockDevice for FailAfterDevice {
    type Error = std::io::Error;

    fn read_page(
        &mut self,
        page_id: relvar_storage_core::page::PageId,
        buf: &mut [u8; relvar_storage_core::page::PAGE_SIZE],
    ) -> Result<(), Self::Error> {
        self.inner
            .read_page(page_id, buf)
            .map_err(|e| std::io::Error::other(format!("fail-after device read failed: {e:?}")))
    }

    fn write_page(
        &mut self,
        page_id: relvar_storage_core::page::PageId,
        buf: &[u8; relvar_storage_core::page::PAGE_SIZE],
    ) -> Result<(), Self::Error> {
        let skip = self.skip_writes.get();
        if skip > 0 {
            // Let this write through; failures start after `skip` writes.
            self.skip_writes.set(skip - 1);
        } else {
            let remaining = self.fail_remaining.get();
            if remaining > 0 {
                // Fail the next `fail_remaining` writes, then disarm so later
                // writes (such as test verification reads-after-writes) work.
                self.fail_remaining.set(remaining - 1);
                return Err(std::io::Error::other(
                    "fail-after device: injected write failure",
                ));
            }
            let n = self.writes.get();
            if n >= self.fail_after.get() {
                // Inject one failure, then disarm so a rollback's writes can
                // succeed.
                self.fail_after.set(usize::MAX);
                return Err(std::io::Error::other(
                    "fail-after device: injected write failure",
                ));
            }
        }
        let n = self.writes.get();
        self.writes.set(n + 1);
        self.inner
            .write_page(page_id, buf)
            .map_err(|e| std::io::Error::other(format!("fail-after device write failed: {e:?}")))
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn page_count(&self) -> relvar_storage_core::page::PageId {
        self.inner.page_count()
    }
}
