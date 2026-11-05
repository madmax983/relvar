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
// Combining assignments to the same target (RM Prescription 21)
// ---------------------------------------------------------------------------

/// One assignment in a multiple assignment: `R := v`, or the shorthands
/// `INSERT R x` (`R := R UNION x`) and `DELETE R x` (`R := R MINUS x`).
/// Sources are values computed before the statement, as in
/// `Database::multiple_assign`.
pub enum AssignOp {
    Replace(Relation),
    Insert(Relation),
    Delete(Relation),
}

/// The relation an operation carries.
pub open spec fn op_relation(op: AssignOp) -> Relation {
    match op {
        AssignOp::Replace(v) => v,
        AssignOp::Insert(x) => x,
        AssignOp::Delete(x) => x,
    }
}

pub open spec fn apply_op(r: Relation, op: AssignOp) -> Relation {
    match op {
        AssignOp::Replace(v) => v,
        AssignOp::Insert(x) => Relation { heading: r.heading, body: r.body.union(x.body) },
        AssignOp::Delete(x) => Relation { heading: r.heading, body: r.body.difference(x.body) },
    }
}

/// The new value of every target after the statement's operations, applied
/// in order, each to the result of the previous operations on the same
/// target (or to the pre-statement value). This is TTM's rule that
/// assignments to the same target are combined.
pub open spec fn combine(db: Db, ops: Seq<(RelvarName, AssignOp)>) -> Map<RelvarName, Relation>
    decreases ops.len(),
{
    if ops.len() == 0 {
        Map::empty()
    } else {
        let prefix = combine(db, ops.drop_last());
        let n = ops.last().0;
        let current = if prefix.dom().contains(n) { prefix[n] } else { db.relvars[n] };
        prefix.insert(n, apply_op(current, ops.last().1))
    }
}

/// Every operation targets an existing relvar with a well-formed relation
/// of that relvar's heading.
pub open spec fn ops_well_typed(db: Db, s: Schema, ops: Seq<(RelvarName, AssignOp)>) -> bool {
    forall|i: int|
        0 <= i < ops.len() ==> db.relvars.dom().contains(#[trigger] ops[i].0) && wf(op_relation(ops[i].1))
            && op_relation(ops[i].1).heading == s.headings[ops[i].0]
}

/// Combining well-typed operations yields a well-typed assignment: the
/// targets are existing relvars, and every combined value is a well-formed
/// relation of the target's heading.
pub proof fn lemma_combine_well_typed(db: Db, s: Schema, ops: Seq<(RelvarName, AssignOp)>)
    requires
        consistent(db, s),
        ops_well_typed(db, s, ops),
    ensures
        assignment_well_typed(db, s, combine(db, ops)),
    decreases ops.len(),
{
    if ops.len() > 0 {
        let rest = ops.drop_last();
        assert forall|i: int| 0 <= i < rest.len() implies db.relvars.dom().contains(#[trigger] rest[i].0)
            && wf(op_relation(rest[i].1)) && op_relation(rest[i].1).heading == s.headings[rest[i].0] by {
            assert(rest[i] == ops[i]);
        }
        lemma_combine_well_typed(db, s, rest);
        let prefix = combine(db, rest);
        let n = ops.last().0;
        let last = ops[ops.len() - 1];
        assert(ops.last() == last);
        assert(db.relvars.dom().contains(last.0));
        let current = if prefix.dom().contains(n) { prefix[n] } else { db.relvars[n] };
        assert(wf(current) && current.heading == s.headings[n]);
        let next = apply_op(current, ops.last().1);
        assert(wf(next) && next.heading == s.headings[n]) by {
            assert(wf(op_relation(last.1)));
        }
        assert(combine(db, ops) == prefix.insert(n, next));
    }
}

/// The combined statement obeys the Golden Rule like any other: it is
/// installed whole if the combined state satisfies every constraint, and
/// otherwise changes nothing. A well-typed combined statement is rejected
/// only for violating a constraint.
pub proof fn theorem_combined_assignment(db: Db, s: Schema, ops: Seq<(RelvarName, AssignOp)>)
    requires
        consistent(db, s),
        ops_well_typed(db, s, ops),
    ensures
        consistent(execute(db, s, combine(db, ops)).0, s),
        execute(db, s, combine(db, ops)).1 <==> consistent(assign(db, combine(db, ops)), s),
{
    lemma_combine_well_typed(db, s, ops);
    theorem_golden_rule(db, s, combine(db, ops));
}

/// TTM's example: `INSERT R r1, INSERT R r2` is `R := (R UNION r1) UNION r2`.
pub proof fn lemma_combine_two_inserts(db: Db, n: RelvarName, r1: Relation, r2: Relation)
    ensures
        combine(db, seq![(n, AssignOp::Insert(r1)), (n, AssignOp::Insert(r2))])[n].body
            == db.relvars[n].body.union(r1.body).union(r2.body),
{
    reveal_with_fuel(combine, 3);
    let ops = seq![(n, AssignOp::Insert(r1)), (n, AssignOp::Insert(r2))];
    assert(ops.drop_last() =~= seq![(n, AssignOp::Insert(r1))]);
    assert(seq![(n, AssignOp::Insert(r1))].drop_last() =~= Seq::<(RelvarName, AssignOp)>::empty());
}

/// An insert followed by a delete of the same tuples on one target removes
/// them, whatever was there before: order matters, as in TTM.
pub proof fn lemma_combine_insert_then_delete(db: Db, n: RelvarName, x: Relation)
    ensures
        combine(db, seq![(n, AssignOp::Insert(x)), (n, AssignOp::Delete(x))])[n].body
            == db.relvars[n].body.union(x.body).difference(x.body),
        forall|t: Tuple| #[trigger] x.body.contains(t) ==> !combine(
            db,
            seq![(n, AssignOp::Insert(x)), (n, AssignOp::Delete(x))],
        )[n].body.contains(t),
{
    reveal_with_fuel(combine, 3);
    let ops = seq![(n, AssignOp::Insert(x)), (n, AssignOp::Delete(x))];
    assert(ops.drop_last() =~= seq![(n, AssignOp::Insert(x))]);
    assert(seq![(n, AssignOp::Insert(x))].drop_last() =~= Seq::<(RelvarName, AssignOp)>::empty());
}

/// The last `Replace` of a target wins (its source is a value, so it does
/// not depend on earlier assignments to the target).
pub proof fn lemma_combine_last_replace_wins(db: Db, ops: Seq<(RelvarName, AssignOp)>, n: RelvarName, v: Relation)
    ensures
        combine(db, ops.push((n, AssignOp::Replace(v))))[n] == v,
{
    assert(ops.push((n, AssignOp::Replace(v))).drop_last() =~= ops);
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
