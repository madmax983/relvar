//! Databases, statements and transactions: RM Prescriptions 16 (databases),
//! 17 (transactions), 21 (assignment and multiple assignment), 23 (the
//! Golden Rule) and 24 (total database constraints).
//!
//! The model of a statement is the one `Database` implements: compute the
//! candidate post-statement state, check every constraint against it, and
//! either install the whole state or leave the database unchanged. The
//! incremental checks the runtime actually runs (key-index probes, FK
//! lookups) are shown equivalent to "check the candidate state" in
//! `keys.rs`.
use vstd::prelude::*;
use crate::model::*;

verus! {

broadcast use vstd::map_lib::group_map_union;

pub type RelvarName = int;

/// A database value: the current value of each relvar.
pub struct Db {
    pub relvars: Map<RelvarName, Relation>,
}

/// A database constraint: any truth-valued function of the database value
/// (RM Prescription 23). Keys, foreign keys, CHECK constraints, type
/// constraints and assertions are all instances.
pub type Constraint = spec_fn(Db) -> bool;

/// The database definition: each relvar's declared heading and the
/// constraints (RM Prescription 16: a database contains both).
pub struct Schema {
    pub headings: Map<RelvarName, Heading>,
    pub constraints: Seq<Constraint>,
}

/// The total database constraint (RM Prescription 24) together with type
/// correctness: every relvar holds a well-formed relation of its declared
/// heading, and every declared constraint holds.
pub open spec fn consistent(db: Db, s: Schema) -> bool {
    &&& db.relvars.dom() == s.headings.dom()
    &&& forall|n: RelvarName| #[trigger] db.relvars.dom().contains(n) ==> wf(db.relvars[n])
        && db.relvars[n].heading == s.headings[n]
    &&& forall|i: int| 0 <= i < s.constraints.len() ==> (#[trigger] s.constraints[i])(db)
}

/// Multiple assignment (RM Prescription 21): all the target relvars get
/// their new values "at the same time".
pub open spec fn assign(db: Db, updates: Map<RelvarName, Relation>) -> Db {
    Db { relvars: db.relvars.union_prefer_right(updates) }
}

/// A statement's targets must be existing relvars and its new values must
/// be relations of the right heading (otherwise it is a type error).
pub open spec fn assignment_well_typed(db: Db, s: Schema, updates: Map<RelvarName, Relation>) -> bool {
    &&& updates.dom().subset_of(db.relvars.dom())
    &&& forall|n: RelvarName| #[trigger] updates.dom().contains(n) ==> wf(updates[n])
        && updates[n].heading == s.headings[n]
}

/// Execute one statement: the result state and whether it was accepted.
pub open spec fn execute(db: Db, s: Schema, updates: Map<RelvarName, Relation>) -> (Db, bool) {
    let next = assign(db, updates);
    if assignment_well_typed(db, s, updates) && consistent(next, s) {
        (next, true)
    } else {
        (db, false)
    }
}

/// THE GOLDEN RULE (RM Prescription 23): starting from a consistent
/// database, every statement leaves it consistent — accepted or not.
pub proof fn theorem_golden_rule(db: Db, s: Schema, updates: Map<RelvarName, Relation>)
    requires
        consistent(db, s),
    ensures
        consistent(execute(db, s, updates).0, s),
{
}

/// Statements are atomic: the result is the complete new state or the
/// unchanged old state, never a mixture.
pub proof fn theorem_statement_atomic(db: Db, s: Schema, updates: Map<RelvarName, Relation>)
    ensures
        execute(db, s, updates).0 == assign(db, updates) || execute(db, s, updates).0 == db,
        !execute(db, s, updates).1 ==> execute(db, s, updates).0 == db,
{
}

/// The Assignment Principle (RM Prescription 21): after a successful
/// assignment of `v` to relvar `n`, `n` has value `v`; relvars not assigned
/// to are unchanged.
pub proof fn theorem_assignment_principle(
    db: Db,
    s: Schema,
    updates: Map<RelvarName, Relation>,
    n: RelvarName,
)
    requires
        execute(db, s, updates).1,
        db.relvars.dom().contains(n),
    ensures
        updates.dom().contains(n) ==> execute(db, s, updates).0.relvars[n] == updates[n],
        !updates.dom().contains(n) ==> execute(db, s, updates).0.relvars[n] == db.relvars[n],
{
}

/// Relvar headings never change through assignment (a relvar's type is
/// fixed when it is defined).
pub proof fn theorem_headings_fixed(db: Db, s: Schema, updates: Map<RelvarName, Relation>, n: RelvarName)
    requires
        consistent(db, s),
        db.relvars.dom().contains(n),
    ensures
        execute(db, s, updates).0.relvars.dom() == db.relvars.dom(),
        execute(db, s, updates).0.relvars[n].heading == db.relvars[n].heading,
{
    if execute(db, s, updates).1 {
        assert(assign(db, updates).relvars.dom() =~= db.relvars.dom());
    }
}

/// Why multiple assignment matters: checking constraints only on the final
/// state of a multiple assignment accepts exactly when the combined state
/// is consistent, even if every intermediate single assignment would be
/// rejected (e.g. inserting a parent and a child under mutual foreign keys).
/// Two single-target assignments run as one statement agree with their
/// combination.
pub proof fn theorem_multiple_assignment_is_one_statement(
    db: Db,
    s: Schema,
    u1: Map<RelvarName, Relation>,
    u2: Map<RelvarName, Relation>,
)
    requires
        u1.dom().disjoint(u2.dom()),
    ensures
        assign(assign(db, u1), u2) == assign(db, u1.union_prefer_right(u2)),
{
    assert(assign(assign(db, u1), u2).relvars =~= assign(db, u1.union_prefer_right(u2)).relvars);
}

// ---------------------------------------------------------------------------
// Transactions (RM Prescription 17)
// ---------------------------------------------------------------------------

/// The committed state and, inside a transaction, the working state.
pub struct TxnState {
    pub committed: Db,
    pub working: Option<Db>,
}

pub open spec fn txn_inv(st: TxnState, s: Schema) -> bool {
    &&& consistent(st.committed, s)
    &&& (st.working matches Some(w) ==> consistent(w, s))
}

pub open spec fn begin(st: TxnState) -> TxnState {
    TxnState { committed: st.committed, working: Some(st.committed) }
}

/// A statement runs against the working state inside a transaction, and
/// against the committed state (autocommit) outside one.
pub open spec fn statement(st: TxnState, s: Schema, updates: Map<RelvarName, Relation>) -> TxnState {
    match st.working {
        Some(w) => TxnState { committed: st.committed, working: Some(execute(w, s, updates).0) },
        None => TxnState { committed: execute(st.committed, s, updates).0, working: None },
    }
}

pub open spec fn commit(st: TxnState) -> TxnState {
    match st.working {
        Some(w) => TxnState { committed: w, working: None },
        None => st,
    }
}

pub open spec fn rollback(st: TxnState) -> TxnState {
    TxnState { committed: st.committed, working: None }
}

/// A crash loses exactly the uncommitted work: it is a rollback.
pub open spec fn crash(st: TxnState) -> TxnState {
    rollback(st)
}

/// Every transition preserves consistency of both committed and working
/// state (so commit can never publish an inconsistent database).
pub proof fn theorem_transactions_preserve_consistency(
    st: TxnState,
    s: Schema,
    updates: Map<RelvarName, Relation>,
)
    requires
        txn_inv(st, s),
    ensures
        txn_inv(begin(st), s),
        txn_inv(statement(st, s, updates), s),
        txn_inv(commit(st), s),
        txn_inv(rollback(st), s),
        txn_inv(crash(st), s),
{
    match st.working {
        Some(w) => theorem_golden_rule(w, s, updates),
        None => theorem_golden_rule(st.committed, s, updates),
    }
}

/// A sequence of statements.
pub open spec fn run(st: TxnState, s: Schema, stmts: Seq<Map<RelvarName, Relation>>) -> TxnState
    decreases stmts.len(),
{
    if stmts.len() == 0 {
        st
    } else {
        run(statement(st, s, stmts[0]), s, stmts.subrange(1, stmts.len() as int))
    }
}

/// Inside a transaction, statements never touch the committed state.
pub proof fn lemma_run_in_txn_keeps_committed(st: TxnState, s: Schema, stmts: Seq<Map<RelvarName, Relation>>)
    requires
        st.working is Some,
    ensures
        run(st, s, stmts).committed == st.committed,
        run(st, s, stmts).working is Some,
    decreases stmts.len(),
{
    if stmts.len() > 0 {
        lemma_run_in_txn_keeps_committed(statement(st, s, stmts[0]), s, stmts.subrange(1, stmts.len() as int));
    }
}

/// Atomicity: begin, any statements, rollback (or crash) is a no-op on the
/// database.
pub proof fn theorem_rollback_is_atomic(st: TxnState, s: Schema, stmts: Seq<Map<RelvarName, Relation>>)
    requires
        st.working is None,
    ensures
        rollback(run(begin(st), s, stmts)) == st,
        crash(run(begin(st), s, stmts)) == st,
{
    lemma_run_in_txn_keeps_committed(begin(st), s, stmts);
}

/// Commit publishes exactly the working state, all at once.
pub proof fn theorem_commit_publishes_working_state(st: TxnState, s: Schema, stmts: Seq<Map<RelvarName, Relation>>)
    requires
        st.working is None,
    ensures
        commit(run(begin(st), s, stmts)).committed == run(begin(st), s, stmts).working->Some_0,
        commit(run(begin(st), s, stmts)).working is None,
{
    lemma_run_in_txn_keeps_committed(begin(st), s, stmts);
}

} // verus!
