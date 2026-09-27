//! Allocation proof for the bounded MVCC pools.
//!
//! The v0.9 contract promises that steady-state MVCC infrastructure work —
//! claiming and releasing version records, checking out page working
//! buffers, resolving transaction liveness, and evaluating version
//! visibility — performs **zero heap allocations** after the pools are
//! built. Pool construction itself allocates once (the pools are
//! caller-owned, fixed-capacity); that one-time cost is outside this
//! proof's window.
//!
//! The proof arms a counting [`GlobalAlloc`] for the calling thread only
//! (a thread-local gate), so the test binary's other threads cannot pollute
//! the measurement. A negative control ([`test_counter_observes_allocation`])
//! shows the counter is live: a known allocation inside the window is
//! observed.
//!
//! Honest boundary: this proves the MVCC *infrastructure* is
//! allocation-free. It does not claim whole relation loads are: `load_page`,
//! result vectors, and returned [`Tuple`](relvar_core::tuple::Tuple)
//! construction still allocate by design.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::pool::TxnPool;
use super::snapshot::TransactionSnapshot;
use super::version_pool::VersionPool;
use super::visibility::{VersionMetadata, is_visible};
use crate::wal::{Lsn, TransactionId};

thread_local! {
    /// Armed only around the measured operation, on this thread.
    static MEASURING: Cell<bool> = const { Cell::new(false) };
}

/// Total allocator calls observed while any thread had its gate armed.
static ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);

/// Debug aid: whether counted allocations print a backtrace. Cached once
/// outside every measurement window — reading the environment inside
/// [`GlobalAlloc::alloc`] would itself allocate.
static TRACE_ALLOCATIONS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Latched by `measured` before arming the gate; read by the allocator.
/// The environment is never read from inside [`GlobalAlloc::alloc`].
static TRACE_ARMED: AtomicBool = AtomicBool::new(false);

fn trace_allocations() -> bool {
    *TRACE_ALLOCATIONS.get_or_init(|| std::env::var("RELVAR_ALLOC_TRACE").is_ok())
}

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // The gate cell is touched before arming (see `measured`), so this
        // `with` never allocates inside the window. The atomic bump does
        // not allocate either.
        if MEASURING.with(|measuring| measuring.get()) {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            // Backtraces and formatting allocate: disarm around them.
            if TRACE_ARMED.load(Ordering::Relaxed) {
                MEASURING.with(|measuring| measuring.set(false));
                eprintln!("counted alloc:\n{:?}", std::backtrace::Backtrace::capture());
                MEASURING.with(|measuring| measuring.set(true));
            }
        }
        // SAFETY: forwards to the system allocator with the same layout.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr`/`layout` came from `alloc` above.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

/// Runs `f` with allocation counting armed for this thread and returns
/// `(f's return value, allocator calls made inside f)`.
///
/// The thread-local gate is touched before arming so the allocator's own
/// gate check cannot allocate inside the window. A drop guard disarms the
/// gate even if `f` panics.
fn measured<R>(f: impl FnOnce() -> R) -> (R, u64) {
    struct Disarm;
    impl Drop for Disarm {
        fn drop(&mut self) {
            MEASURING.with(|measuring| measuring.set(false));
        }
    }

    MEASURING.with(|measuring| measuring.set(false));
    let _disarm = Disarm;
    // Latch the trace flag BEFORE arming: `trace_allocations` reads the
    // environment (which allocates) and must never run while armed.
    TRACE_ARMED.store(trace_allocations(), Ordering::Relaxed);
    MEASURING.with(|measuring| measuring.set(true));
    let before = ALLOC_COUNT.load(Ordering::Relaxed);
    let result = f();
    let after = ALLOC_COUNT.load(Ordering::Relaxed);
    drop(_disarm);
    TRACE_ARMED.store(false, Ordering::Relaxed);
    (result, after - before)
}

fn test_txn_id(value: u64) -> TransactionId {
    TransactionId::new(value)
}

fn test_lsn(value: u64) -> Lsn {
    Lsn::new(value)
}

#[test]
fn test_counter_observes_allocation() {
    // Negative control: a known allocation inside the window must be
    // counted, or every zero below would be vacuous.
    let (_, allocs) = measured(|| {
        let _v: Vec<u8> = vec![0u8; 64];
    });
    assert!(
        allocs >= 1,
        "the counter must observe a plain Vec allocation, saw {allocs}"
    );
}

#[test]
fn test_txn_pool_liveness_iteration_allocates_nothing() {
    let mut pool = TxnPool::new(16);
    let mut snapshots = Vec::new();
    for id in 1..=8 {
        snapshots.push(
            pool.begin(test_txn_id(id), test_lsn(id * 10), test_txn_id(100))
                .unwrap(),
        );
    }
    // One commit, to also exercise the history-ring path.
    pool.commit(test_txn_id(1), test_lsn(95)).unwrap();

    let (_, allocs) = measured(|| {
        let mut hits = 0;
        for id in 1..=8 {
            let snapshot = pool.get_snapshot(test_txn_id(id));
            if pool.is_active_at(test_txn_id(id), test_lsn(50)) {
                hits += 1;
            }
            // Snapshot-level liveness goes through the same pool arrays.
            let view = TransactionSnapshot::new(test_txn_id(100), test_lsn(55));
            if view.is_active(test_txn_id(id), &pool) {
                hits += 1;
            }
            std::hint::black_box(snapshot);
        }
        let _ = pool.oldest_active_lsn();
        let _ = pool.active_count();
        let _ = pool.history_len();
        std::hint::black_box(hits);
    });
    assert_eq!(
        allocs, 0,
        "TxnPool liveness iteration must not allocate, made {allocs}"
    );
}

#[test]
fn test_version_pool_claim_release_allocates_nothing() {
    let mut pool = VersionPool::with_capacities(64, 4, 16);
    let txn = test_txn_id(7);
    // The handle Vec lives outside the window (pre-reserved): only the
    // pool's own behavior is measured.
    let mut handles: Vec<super::VersionHandle> = Vec::with_capacity(32);

    let (_, allocs) = measured(|| {
        for _ in 0..32 {
            handles.push(pool.claim(txn).unwrap());
        }
        // Release half by handle, half by transaction.
        for handle in handles.drain(..16) {
            assert!(pool.release(handle));
        }
        let released = pool.release_for_txn(txn);
        assert_eq!(released, 16);
        std::hint::black_box(pool.used_versions());
    });
    assert_eq!(
        allocs, 0,
        "VersionPool claim/release must not allocate, made {allocs}"
    );
}

#[test]
fn test_page_buffer_checkout_reset_allocates_nothing() {
    let mut pool = VersionPool::with_capacities(64, 4, 16);

    let (_, allocs) = measured(|| {
        for _ in 0..4 {
            let mut guard = pool.acquire_buffer().unwrap();
            // Write into the pre-reserved image: capacity was reserved at
            // construction, so no growth allocation is possible.
            guard.page_image.extend_from_slice(&[0xABu8; 1024]);
            guard.reset();
            std::hint::black_box(guard.page.slots.capacity());
            drop(guard);
        }
    });
    assert_eq!(
        allocs, 0,
        "page buffer checkout/reset must not allocate, made {allocs}"
    );
}

#[test]
fn test_visibility_evaluation_allocates_nothing() {
    let mut pool = TxnPool::new(16);
    let writer = pool
        .begin(test_txn_id(1), test_lsn(10), test_txn_id(100))
        .unwrap();
    pool.commit(writer.txn_id, test_lsn(20)).unwrap();
    let reader = pool
        .begin(test_txn_id(2), test_lsn(30), test_txn_id(100))
        .unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn_id(1));

    // A spread of versions: own-write, committed, in-flight, future,
    // deleted.
    let versions = [
        VersionMetadata {
            xmin: test_txn_id(1),
            xmax: None,
        },
        VersionMetadata {
            xmin: test_txn_id(1),
            xmax: Some(test_txn_id(1)),
        },
        VersionMetadata {
            xmin: test_txn_id(2),
            xmax: None,
        },
        VersionMetadata {
            xmin: test_txn_id(3),
            xmax: None,
        },
    ];

    let (_, allocs) = measured(|| {
        let mut visible = 0;
        for _ in 0..100 {
            for version in &versions {
                if is_visible(version, &reader, &pool, &committed) {
                    visible += 1;
                }
            }
        }
        std::hint::black_box(visible);
    });
    assert_eq!(
        allocs, 0,
        "visibility evaluation must not allocate, made {allocs}"
    );
}
