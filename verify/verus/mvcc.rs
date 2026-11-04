//! MVCC visibility (relvar-storage/src/mvcc/visibility.rs) and why it keeps
//! the logical model intact: RM Proscription 6 (physical versions never
//! reach the logical layer) and RM Proscription 3 (a snapshot sees at most
//! one version of each logical tuple, so versioning never produces
//! duplicates).
//!
//! `created_visible` and `deletion_invisible` transcribe the runtime's
//! `is_created_visible` and `is_deletion_invisible` rule for rule, over
//! abstract transaction ids.
use vstd::prelude::*;

verus! {

pub type TxnId = int;

/// The snapshot a transaction reads from (`TransactionSnapshot` + the pool's
/// "active at this snapshot" answers).
pub struct Snapshot {
    /// The reading transaction.
    pub me: TxnId,
    /// Transactions with id >= horizon began after the snapshot.
    pub horizon: TxnId,
    /// Transactions that were active (begun, not finished) at the snapshot.
    pub active: Set<TxnId>,
}

/// `is_created_visible`: rules 1-4 in order.
pub open spec fn created_visible(s: Snapshot, committed: Set<TxnId>, xmin: TxnId) -> bool {
    if xmin == s.me {
        true
    } else if xmin >= s.horizon {
        false
    } else if s.active.contains(xmin) && xmin != s.me {
        false
    } else {
        committed.contains(xmin)
    }
}

/// `is_deletion_invisible`: rules 1-6 in order.
pub open spec fn deletion_invisible(s: Snapshot, committed: Set<TxnId>, xmax: Option<TxnId>) -> bool {
    match xmax {
        None => true,
        Some(x) => if x == s.me {
            false
        } else if x >= s.horizon {
            true
        } else if !committed.contains(x) {
            true
        } else if s.active.contains(x) && x != s.me {
            true
        } else {
            false
        },
    }
}

/// `is_visible`.
pub open spec fn visible(s: Snapshot, committed: Set<TxnId>, xmin: TxnId, xmax: Option<TxnId>) -> bool {
    created_visible(s, committed, xmin) && deletion_invisible(s, committed, xmax)
}

/// The two rule lists are exact complements: a deletion by `x` is invisible
/// to a snapshot exactly when a creation by `x` would be invisible to it.
/// This is the property everything else rests on.
pub proof fn lemma_deletion_is_complement_of_creation(s: Snapshot, committed: Set<TxnId>, x: TxnId)
    ensures
        deletion_invisible(s, committed, Some(x)) <==> !created_visible(s, committed, x),
{
}

/// A transaction sees its own writes and not its own deletes.
pub proof fn lemma_read_your_writes(s: Snapshot, committed: Set<TxnId>, xmax: Option<TxnId>)
    ensures
        visible(s, committed, s.me, None),
        !visible(s, committed, s.me, Some(s.me)),
{
}

/// No dirty reads: a version created by another, uncommitted transaction is
/// invisible; a deletion by another, uncommitted transaction is ignored.
pub proof fn lemma_no_dirty_reads(s: Snapshot, committed: Set<TxnId>, x: TxnId)
    requires
        x != s.me,
        !committed.contains(x),
    ensures
        !created_visible(s, committed, x),
        deletion_invisible(s, committed, Some(x)),
{
}

/// Repeatable read: commits that happen after the snapshot was taken (by
/// transactions active at the snapshot or begun after it) change nothing
/// the snapshot sees.
pub proof fn lemma_snapshot_is_stable(
    s: Snapshot,
    c1: Set<TxnId>,
    c2: Set<TxnId>,
    xmin: TxnId,
    xmax: Option<TxnId>,
)
    requires
        c1.subset_of(c2),
        forall|x: TxnId| #[trigger] c2.contains(x) && !c1.contains(x) ==> x == s.me || x >= s.horizon
            || s.active.contains(x),
    ensures
        visible(s, c1, xmin, xmax) == visible(s, c2, xmin, xmax),
{
}

// ---------------------------------------------------------------------------
// At most one visible version per logical tuple.
// ---------------------------------------------------------------------------

/// A version chain of one logical tuple: version `k` was created by
/// `creators[k]`, and superseded (xmax) by the creator of version `k+1`;
/// the last version is not superseded.
pub open spec fn version_visible(s: Snapshot, committed: Set<TxnId>, creators: Seq<TxnId>, k: int) -> bool {
    visible(
        s,
        committed,
        creators[k],
        if k + 1 < creators.len() { Some(creators[k + 1]) } else { None },
    )
}

/// The engine's update discipline, stated per snapshot: a transaction can
/// only supersede a version it could see, so if a snapshot sees the
/// creation of version `k+1`, it also sees the creation of version `k`
/// (the superseding transaction began after the superseded one committed,
/// or they are the same transaction).
pub open spec fn chain_discipline(s: Snapshot, committed: Set<TxnId>, creators: Seq<TxnId>) -> bool {
    forall|k: int|
        1 <= k < creators.len() && #[trigger] created_visible(s, committed, creators[k])
            ==> created_visible(s, committed, creators[k - 1])
}

proof fn lemma_creation_visible_backwards(
    s: Snapshot,
    committed: Set<TxnId>,
    creators: Seq<TxnId>,
    i: int,
    j: int,
)
    requires
        chain_discipline(s, committed, creators),
        0 <= i <= j < creators.len(),
        created_visible(s, committed, creators[j]),
    ensures
        created_visible(s, committed, creators[i]),
    decreases j - i,
{
    if i < j {
        assert(created_visible(s, committed, creators[j - 1]));
        lemma_creation_visible_backwards(s, committed, creators, i, j - 1);
    }
}

/// RM Proscription 3 at the physical layer: a snapshot sees at most one
/// version of each logical tuple.
pub proof fn theorem_at_most_one_visible_version(
    s: Snapshot,
    committed: Set<TxnId>,
    creators: Seq<TxnId>,
    i: int,
    j: int,
)
    requires
        chain_discipline(s, committed, creators),
        0 <= i < creators.len(),
        0 <= j < creators.len(),
        version_visible(s, committed, creators, i),
        version_visible(s, committed, creators, j),
    ensures
        i == j,
{
    if i < j {
        lemma_deletion_is_complement_of_creation(s, committed, creators[i + 1]);
        lemma_creation_visible_backwards(s, committed, creators, i + 1, j);
    } else if j < i {
        lemma_deletion_is_complement_of_creation(s, committed, creators[j + 1]);
        lemma_creation_visible_backwards(s, committed, creators, j + 1, i);
    }
}

// ---------------------------------------------------------------------------
// The discipline follows from timestamps.
// ---------------------------------------------------------------------------

/// A timestamp view of the engine: each committed transaction has a begin
/// and a commit time with begin < commit; the snapshot was taken at time
/// `at`. A transaction is "seen" if it committed before `at`.
pub struct Clock {
    pub begin: Map<TxnId, int>,
    pub commit: Map<TxnId, int>,
    pub at: int,
}

pub open spec fn clock_ok(c: Clock) -> bool {
    forall|x: TxnId| #[trigger] c.commit.dom().contains(x) ==> c.begin.dom().contains(x) && c.begin[x] < c.commit[x]
}

/// The snapshot's id/active/horizon encoding agrees with the clock: it sees
/// exactly the transactions committed before `at`, plus itself.
pub open spec fn snapshot_matches_clock(s: Snapshot, committed: Set<TxnId>, c: Clock) -> bool {
    forall|x: TxnId| #[trigger] created_visible(s, committed, x) <==> (x == s.me || (c.commit.dom().contains(x)
        && c.commit[x] < c.at))
}

/// The update discipline in clock form: a superseding transaction either is
/// the creator or began after the creator committed; and the snapshot owner
/// began before `at`.
pub open spec fn supersedes_after_commit(c: Clock, me: TxnId, creators: Seq<TxnId>) -> bool {
    &&& c.begin.dom().contains(me) && c.begin[me] <= c.at
    &&& forall|k: int|
        1 <= k < creators.len() ==> creators[k - 1] == #[trigger] creators[k] || (c.commit.dom().contains(
            creators[k - 1],
        ) && c.begin.dom().contains(creators[k]) && c.commit[creators[k - 1]] < c.begin[creators[k]])
}

pub proof fn lemma_clock_implies_discipline(s: Snapshot, committed: Set<TxnId>, c: Clock, creators: Seq<TxnId>)
    requires
        clock_ok(c),
        snapshot_matches_clock(s, committed, c),
        supersedes_after_commit(c, s.me, creators),
    ensures
        chain_discipline(s, committed, creators),
{
    assert forall|k: int|
        1 <= k < creators.len() && #[trigger] created_visible(s, committed, creators[k])
            implies created_visible(s, committed, creators[k - 1]) by {
        let x = creators[k - 1];
        let y = creators[k];
        if x != y {
            assert(c.commit.dom().contains(x) && c.begin.dom().contains(y) && c.commit[x] < c.begin[y]);
            assert(y == s.me || (c.commit.dom().contains(y) && c.commit[y] < c.at));
            if y != s.me {
                assert(c.begin[y] < c.commit[y]);
            }
            assert(c.commit[x] < c.at);
            assert(created_visible(s, committed, x));
        }
    }
}

} // verus!
