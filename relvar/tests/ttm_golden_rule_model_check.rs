//! Exhaustive model checking of the Golden Rule on the real `Database`.
//!
//! TTM: RM Prescription 23 (every constraint holds at every statement
//! boundary), RM Prescription 17 (transactions are atomic), RM Prescription
//! 21 (multiple assignment), RM Prescription 16 (a database keeps its
//! constraints, across reopen too).
//!
//! The schema is a parent/child pair: `P {pid}` with key `{pid}` and
//! `C {cid, pid}` with key `{cid}` and a foreign key `C.pid -> P.pid`, plus a
//! persisted declarative assertion `IS_EMPTY((P WHERE pid = 2) NOT MATCHING
//! C{pid})` — parent 2 may exist only with a child, so only a multiple
//! assignment can add it. The schema is defined once: after a crash the
//! database is reopened WITHOUT redefining anything, so every check after a
//! reopen also verifies that keys, foreign keys and the assertion persisted.
//! Stateright explores every sequence (up to a depth bound) of inserts,
//! deletes, updates, a multiple assignment, begin/commit/rollback and — on
//! the persistent engine — crashes followed by reopening. Each history is
//! replayed on a real `Database`, and after every step:
//!
//! - `Database::verify_integrity` passes (the Golden Rule);
//! - the statement was accepted or rejected exactly as the model of
//!   `verify/verus/database.rs` says: `execute` installs the candidate state
//!   if and only if it satisfies every constraint, otherwise nothing changes;
//! - the visible relvar values equal the model's (working state inside a
//!   transaction, committed state outside one; a crash is a rollback).
//!
//! Search depth: `RELVAR_MODEL_CHECK_DEPTH` (default 4 in memory, one less
//! on the persistent engine).

use std::collections::BTreeSet;
#[cfg(feature = "storage")]
use std::path::Path;

use relvar::constraints::{
    AssertionCondition, CmpOp, ConstraintExpression, DeclarativeAssertion, ForeignKey,
    ForeignKeyConstraints, KeyConstraints, PrimaryKey, ValueOrRef,
};
use relvar::{
    Assignment, Database, InMemoryEngine, Query, Relation, RelationType, ScalarType, ScalarValue,
    StorageEngine, Tuple, TupleType, tuple,
};
use stateright::{Checker, Model, Property};

const PIDS: [i64; 2] = [1, 2];
const CID: i64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Action {
    InsertP(i64),
    DeleteP(i64),
    InsertC(i64),
    DeleteC,
    UpdateC(i64),
    /// `P := P ∪ {p}, C := C ∪ {(CID, p)}` as one multiple assignment.
    AssignPair(i64),
    /// `INSERT P {p}, DELETE P {p}` in one statement: assignments to the
    /// same target combine in order, so this is `P := (P ∪ {p}) − {p}`.
    InsertThenDeleteP(i64),
    /// `C := {}, INSERT C {(CID, p)}`: replace, then insert into the result.
    ReplaceThenInsertC(i64),
    /// `INSERT P {1}, INSERT P {2}`: two inserts into the same target.
    InsertBothP,
    Begin,
    Commit,
    Rollback,
    Crash,
}

/// A database value: `P` and `C` (as `(cid, pid)` pairs).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
struct DbValue {
    p: BTreeSet<i64>,
    c: BTreeSet<(i64, i64)>,
}

impl DbValue {
    /// Every declared constraint: key `{cid}` on C, the foreign key, and the
    /// declarative assertion. (Key `{pid}` on P holds by construction: P is a
    /// set of pids.)
    fn consistent(&self) -> bool {
        let cids: BTreeSet<i64> = self.c.iter().map(|(cid, _)| *cid).collect();
        let parent_two_has_a_child =
            !self.p.contains(&2) || self.c.iter().any(|(_, pid)| *pid == 2);
        cids.len() == self.c.len()
            && self.c.iter().all(|(_, pid)| self.p.contains(pid))
            && parent_two_has_a_child
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
struct Abstract {
    committed: DbValue,
    working: Option<DbValue>,
}

impl Abstract {
    fn current(&self) -> &DbValue {
        self.working.as_ref().unwrap_or(&self.committed)
    }

    /// The candidate state of a statement, or `None` when the runtime
    /// rejects it before evaluating constraints (inserting a tuple whose
    /// key value is already present — the runtime reports this as a key
    /// violation even for an identical tuple; see
    /// `keys.rs::lemma_index_probe_for_present_tuple`).
    fn candidate(&self, action: Action) -> Option<DbValue> {
        let mut next = self.current().clone();
        match action {
            Action::InsertP(p) => {
                if !next.p.insert(p) {
                    return None;
                }
            }
            Action::DeleteP(p) => {
                next.p.remove(&p);
            }
            Action::InsertC(p) => {
                if next.c.iter().any(|(cid, _)| *cid == CID) {
                    return None;
                }
                next.c.insert((CID, p));
            }
            Action::DeleteC => next.c.retain(|(cid, _)| *cid != CID),
            Action::UpdateC(p) => {
                next.c = next
                    .c
                    .iter()
                    .map(|&(cid, pid)| if cid == CID { (cid, p) } else { (cid, pid) })
                    .collect();
            }
            Action::AssignPair(p) => {
                next.p.insert(p);
                next.c.insert((CID, p));
            }
            // Combined per target, in statement order (verify/verus/database.rs
            // `combine`).
            Action::InsertThenDeleteP(p) => {
                next.p.insert(p);
                next.p.remove(&p);
            }
            Action::ReplaceThenInsertC(p) => {
                next.c = BTreeSet::from([(CID, p)]);
            }
            Action::InsertBothP => {
                next.p.insert(1);
                next.p.insert(2);
            }
            Action::Begin | Action::Commit | Action::Rollback | Action::Crash => {}
        }
        Some(next)
    }

    /// Apply an action; returns whether the runtime must accept it.
    fn apply(&mut self, action: Action) -> bool {
        match action {
            Action::Begin => {
                self.working = Some(self.committed.clone());
                true
            }
            Action::Commit => {
                if let Some(working) = self.working.take() {
                    self.committed = working;
                }
                true
            }
            Action::Rollback | Action::Crash => {
                self.working = None;
                true
            }
            statement => match self.candidate(statement) {
                // The Golden Rule: install the candidate only if consistent.
                Some(next) if next.consistent() => {
                    match &mut self.working {
                        Some(working) => *working = next,
                        None => self.committed = next,
                    }
                    true
                }
                _ => false,
            },
        }
    }
}

fn p_type() -> RelationType {
    RelationType::new(TupleType::new().with_attribute("pid", ScalarType::Int))
}

fn c_type() -> RelationType {
    RelationType::new(
        TupleType::new()
            .with_attribute("cid", ScalarType::Int)
            .with_attribute("pid", ScalarType::Int),
    )
}

fn define_schema<E: StorageEngine>(db: &mut Database<E>) -> Result<(), String> {
    db.create_relvar("P", p_type()).map_err(|e| e.to_string())?;
    db.create_relvar("C", c_type()).map_err(|e| e.to_string())?;
    let p_key = PrimaryKey::new(vec!["pid".to_string()]).map_err(|e| e.to_string())?;
    db.set_key_constraints("P", KeyConstraints::new().with_primary_key(p_key))
        .map_err(|e| e.to_string())?;
    let c_key = PrimaryKey::new(vec!["cid".to_string()]).map_err(|e| e.to_string())?;
    db.set_key_constraints("C", KeyConstraints::new().with_primary_key(c_key))
        .map_err(|e| e.to_string())?;
    let fk = ForeignKey::new(
        vec!["pid".to_string()],
        "P".to_string(),
        vec!["pid".to_string()],
    )
    .map_err(|e| e.to_string())?;
    db.set_foreign_key_constraints("C", ForeignKeyConstraints::new().with_foreign_key(fk))
        .map_err(|e| e.to_string())?;
    db.add_declarative_assertion(DeclarativeAssertion::new(
        "parent_two_has_a_child",
        "Parent 2 exists only together with a child",
        AssertionCondition::IsEmpty(
            Query::scan("P")
                .restrict(ConstraintExpression::Cmp {
                    left: "pid".to_string(),
                    op: CmpOp::Eq,
                    right: ValueOrRef::Value(ScalarValue::Int(2)),
                })
                .not_matching(Query::scan("C").project(vec!["pid"])),
        ),
    ))
    .map_err(|e| e.to_string())
}

fn p_relation(pids: &[i64]) -> Result<Relation, String> {
    let mut relation = Relation::new(p_type());
    for &pid in pids {
        relation
            .insert(tuple! { pid: pid })
            .map_err(|e| e.to_string())?;
    }
    Ok(relation)
}

fn c_relation(children: &[(i64, i64)]) -> Result<Relation, String> {
    let mut relation = Relation::new(c_type());
    for &(cid, pid) in children {
        relation
            .insert(tuple! { cid: cid, pid: pid })
            .map_err(|e| e.to_string())?;
    }
    Ok(relation)
}

fn int(tuple: &Tuple, name: &str) -> i64 {
    match tuple.get(name) {
        Some(ScalarValue::Int(v)) => *v,
        other => panic!("{name} is not an Int: {other:?}"),
    }
}

fn observe<E: StorageEngine>(db: &Database<E>) -> Result<DbValue, String> {
    let p = db.query("P").map_err(|e| e.to_string())?;
    let c = db.query("C").map_err(|e| e.to_string())?;
    Ok(DbValue {
        p: p.tuples().map(|t| int(t, "pid")).collect(),
        c: c.tuples().map(|t| (int(t, "cid"), int(t, "pid"))).collect(),
    })
}

fn with_tuple(relation: Relation, tuple: Tuple) -> Result<Relation, String> {
    let mut relation = relation;
    relation.insert(tuple).map_err(|e| e.to_string())?;
    Ok(relation)
}

/// Runs one statement on the real database; `Ok(true)` = accepted.
fn run<E: StorageEngine>(db: &mut Database<E>, action: Action) -> Result<bool, String> {
    let accepted = match action {
        Action::InsertP(p) => db.insert("P", tuple! { pid: p }).is_ok(),
        Action::DeleteP(p) => db.delete("P", |t| int(t, "pid") == p).is_ok(),
        Action::InsertC(p) => db.insert("C", tuple! { cid: CID, pid: p }).is_ok(),
        Action::DeleteC => db.delete("C", |t| int(t, "cid") == CID).is_ok(),
        Action::UpdateC(p) => db
            .update(
                "C",
                |t| int(t, "cid") == CID,
                |_| tuple! { cid: CID, pid: p },
            )
            .is_ok(),
        Action::AssignPair(p) => {
            let new_p = with_tuple(db.query("P").map_err(|e| e.to_string())?, tuple! { pid: p })?;
            let new_c = with_tuple(
                db.query("C").map_err(|e| e.to_string())?,
                tuple! { cid: CID, pid: p },
            )?;
            db.multiple_assign([("P", new_p), ("C", new_c)]).is_ok()
        }
        Action::InsertThenDeleteP(p) => db
            .multiple_assign([
                ("P", Assignment::Insert(p_relation(&[p])?)),
                ("P", Assignment::Delete(p_relation(&[p])?)),
            ])
            .is_ok(),
        Action::ReplaceThenInsertC(p) => db
            .multiple_assign([
                ("C", Assignment::Replace(c_relation(&[])?)),
                ("C", Assignment::Insert(c_relation(&[(CID, p)])?)),
            ])
            .is_ok(),
        Action::InsertBothP => db
            .multiple_assign([
                ("P", Assignment::Insert(p_relation(&[1])?)),
                ("P", Assignment::Insert(p_relation(&[2])?)),
            ])
            .is_ok(),
        Action::Begin => db.begin().is_ok(),
        Action::Commit => db.commit().is_ok(),
        Action::Rollback => db.rollback().is_ok(),
        Action::Crash => unreachable!("crashes are handled by the engine harness"),
    };
    Ok(accepted)
}

/// How to build and crash a database for one engine.
trait Harness {
    type Engine: StorageEngine;
    type Scratch;
    fn fresh() -> Result<(Self::Scratch, Database<Self::Engine>), String>;
    /// Drop the database without committing and open it again, or `None` if
    /// the engine cannot crash (in memory).
    fn crash(
        scratch: &Self::Scratch,
        db: Database<Self::Engine>,
    ) -> Option<Result<Database<Self::Engine>, String>>;
}

struct InMemory;

impl Harness for InMemory {
    type Engine = InMemoryEngine;
    type Scratch = ();
    fn fresh() -> Result<((), Database<InMemoryEngine>), String> {
        let mut db = Database::new(InMemoryEngine::new());
        define_schema(&mut db)?;
        Ok(((), db))
    }
    fn crash(
        _: &(),
        _: Database<InMemoryEngine>,
    ) -> Option<Result<Database<InMemoryEngine>, String>> {
        None
    }
}

#[cfg(feature = "storage")]
struct Persistent;

#[cfg(feature = "storage")]
fn scratch_dir() -> tempfile::TempDir {
    // tmpfs keeps fsync cheap, so the exhaustive search stays fast.
    let shm = Path::new("/dev/shm");
    shm.is_dir()
        .then(|| tempfile::tempdir_in(shm).ok())
        .flatten()
        .unwrap_or_else(|| tempfile::tempdir().expect("temp dir"))
}

#[cfg(feature = "storage")]
impl Harness for Persistent {
    type Engine = relvar::PersistentEngine;
    type Scratch = tempfile::TempDir;
    fn fresh() -> Result<(tempfile::TempDir, Database<relvar::PersistentEngine>), String> {
        let dir = scratch_dir();
        let mut db = relvar::open(dir.path()).map_err(|e| e.to_string())?;
        define_schema(&mut db)?;
        Ok((dir, db))
    }
    fn crash(
        dir: &tempfile::TempDir,
        db: Database<relvar::PersistentEngine>,
    ) -> Option<Result<Database<relvar::PersistentEngine>, String>> {
        drop(db);
        // Reopen WITHOUT redefining the schema: relvars and constraints must
        // come back from the catalog.
        Some(relvar::open(dir.path()).map_err(|e| e.to_string()))
    }
}

fn replay<H: Harness>(history: &[Action]) -> Result<(), String> {
    let (scratch, mut db) = H::fresh()?;
    let mut model = Abstract::default();
    for (step, &action) in history.iter().enumerate() {
        let expected = model.apply(action);
        let accepted = if action == Action::Crash {
            match H::crash(&scratch, db) {
                Some(reopened) => {
                    db = reopened?;
                    true
                }
                None => return Err("crash on an engine that cannot crash".to_string()),
            }
        } else {
            run(&mut db, action)?
        };
        if accepted != expected {
            return Err(format!(
                "step {step} {action:?}: runtime {} it, model {} it",
                if accepted { "accepted" } else { "rejected" },
                if expected { "accepts" } else { "rejects" }
            ));
        }
        db.verify_integrity()
            .map_err(|e| format!("after step {step} {action:?}: Golden Rule violated: {e}"))?;
        let seen = observe(&db)?;
        if &seen != model.current() {
            return Err(format!(
                "after step {step} {action:?}: database is {seen:?}, model says {:?}",
                model.current()
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct State {
    history: Vec<Action>,
    model: Abstract,
}

struct GoldenRule<H> {
    depth: usize,
    crashes: bool,
    harness: std::marker::PhantomData<H>,
}

impl<H: Harness + Send + Sync + 'static> Model for GoldenRule<H> {
    type State = State;
    type Action = Action;

    fn init_states(&self) -> Vec<State> {
        vec![State {
            history: Vec::new(),
            model: Abstract::default(),
        }]
    }

    fn actions(&self, state: &State, actions: &mut Vec<Action>) {
        for p in PIDS {
            actions.push(Action::InsertP(p));
            actions.push(Action::DeleteP(p));
            actions.push(Action::InsertC(p));
            actions.push(Action::UpdateC(p));
            actions.push(Action::AssignPair(p));
            actions.push(Action::InsertThenDeleteP(p));
            actions.push(Action::ReplaceThenInsertC(p));
        }
        actions.push(Action::InsertBothP);
        actions.push(Action::DeleteC);
        if state.model.working.is_some() {
            actions.push(Action::Commit);
            actions.push(Action::Rollback);
        } else {
            actions.push(Action::Begin);
        }
        if self.crashes {
            actions.push(Action::Crash);
        }
    }

    fn next_state(&self, state: &State, action: Action) -> Option<State> {
        let mut next = state.clone();
        next.model.apply(action);
        next.history.push(action);
        Some(next)
    }

    fn within_boundary(&self, state: &State) -> bool {
        state.history.len() <= self.depth
    }

    fn properties(&self) -> Vec<Property<Self>> {
        vec![Property::always(
            "the Golden Rule holds and the runtime refines the model",
            |_, state: &State| match replay::<H>(&state.history) {
                Ok(()) => true,
                Err(e) => {
                    eprintln!("divergence for {:?}: {e}", state.history);
                    false
                }
            },
        )]
    }
}

fn depth(default: usize) -> usize {
    std::env::var("RELVAR_MODEL_CHECK_DEPTH")
        .ok()
        .and_then(|d| d.parse::<usize>().ok())
        .map_or(default, |d| d.saturating_sub(4 - default))
}

fn check<H: Harness + Send + Sync + 'static>(depth: usize, crashes: bool) {
    let checker = GoldenRule::<H> {
        depth,
        crashes,
        harness: std::marker::PhantomData,
    }
    .checker()
    .threads(std::thread::available_parallelism().map_or(1, |n| n.get()))
    .spawn_bfs()
    .join();
    checker.assert_properties();
    eprintln!(
        "golden rule model check: depth {depth}, {} states",
        checker.unique_state_count()
    );
    assert!(checker.unique_state_count() > 1);
}

#[test]
fn golden_rule_in_memory() {
    check::<InMemory>(depth(4), false);
}

#[cfg(feature = "storage")]
#[test]
fn golden_rule_persistent_with_crashes() {
    check::<Persistent>(depth(3), true);
}
