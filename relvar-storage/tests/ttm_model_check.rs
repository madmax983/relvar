//! Exhaustive model checking of the persistent engine's transactions.
//!
//! TTM: RM Prescription 17 (transactions are atomic and isolated), RM
//! Proscription 3 (no duplicate tuples, including across versions), RM
//! Proscription 6 (versions never leak into the logical model).
//!
//! Stateright explores every sequence (up to a depth bound) of two
//! concurrent `RepeatableRead` transactions doing begin / insert / delete /
//! commit / rollback, interleaved with crashes (drop + reopen). Each history
//! is replayed on a real `PersistentEngine`, and after every step:
//!
//! - every live transaction reads exactly its snapshot plus its own writes
//!   (no dirty reads, repeatable reads, read-your-writes);
//! - a read outside any transaction returns exactly the committed state
//!   predicted by the model (atomic commit, atomic rollback, durability, and
//!   crash recovery that keeps committed work and drops uncommitted work);
//! - each operation succeeds or fails exactly when the model says it should:
//!   deletes conflict first-updater-wins, as in `store_relation`, and so do
//!   inserts, because every relation's whole heading is a key. Inserting a
//!   tuple the transaction already sees is a no-op; inserting a tuple a
//!   concurrent transaction created (live, or committed after this one
//!   began, and not deleted again by its creator) is a write conflict.
//!
//! The model check found the engine's missing implicit-key enforcement:
//! inserts used to always create a new version, so a visible duplicate
//! became a second physical version that a later delete did not mark (a
//! lost committed delete, depth 6), and concurrent inserts of the same
//! tuple both committed (depth 4).
//!
//! The abstract model is the one proved in `verify/verus/database.rs`
//! (`TxnState`, `begin`/`statement`/`commit`/`rollback`/`crash`) extended
//! with snapshot isolation; the per-version visibility rules it relies on
//! are proved in `verify/verus/mvcc.rs`.
//!
//! The search depth defaults to 5 (about 9 thousand states, seconds in
//! release mode) and can be raised with `RELVAR_MODEL_CHECK_DEPTH`; depth 7
//! explores about 650 thousand states (two minutes on four cores). Run it
//! in release mode:
//! `cargo test --release -p relvar-storage --test ttm_model_check`.

use std::collections::BTreeSet;
use std::path::Path;

use relvar_core::storage_engine::{IsolationLevel, StorageEngine};
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;
use relvar_storage::PersistentEngine;
use relvar_storage::persistent_engine::PersistentSnapshot;
use stateright::{Checker, Model, Property};

/// The relvar under test (heading `{id: Int}`).
const RELVAR: &str = "R";
/// Number of concurrent transactions.
const TXNS: usize = 2;
/// The tuple values transactions insert and delete.
const VALUES: [i64; 2] = [1, 2];
/// Committed before the search starts.
const INITIAL: i64 = 1;

/// A history step: an operation of transaction `t`, or a crash (the
/// engine is dropped without commit or flush, then reopened).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Action {
    Begin(usize),
    Insert(usize, i64),
    Delete(usize, i64),
    Commit(usize),
    Rollback(usize),
    Crash,
}

/// A live transaction in the model.
///
/// Its writes are tracked the way the engine records them: committed
/// tuples whose versions it marked deleted, and tuples it created a
/// version of. Every relation's whole heading is a key (RM Proscription 3),
/// so inserting a tuple the transaction already sees is a no-op, and two
/// concurrent transactions writing the same tuple is a write conflict
/// (first-updater-wins), exactly like two concurrent deletes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Txn {
    /// When it began (number of commits before it).
    began_at: usize,
    /// The committed state when it began.
    snapshot: BTreeSet<i64>,
    /// Snapshot tuples whose versions it marked deleted (sticky until the
    /// transaction ends, even if it re-inserts the tuple).
    marked: BTreeSet<i64>,
    /// Tuples it created a (still undeleted) version of.
    created: BTreeSet<i64>,
}

impl Txn {
    /// What it sees: its snapshot plus its own writes.
    fn view(&self) -> BTreeSet<i64> {
        self.snapshot
            .difference(&self.marked)
            .copied()
            .chain(self.created.iter().copied())
            .collect()
    }
}

/// A committed transaction's writes, for first-updater-wins checks.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CommittedWrites {
    seq: usize,
    marked: BTreeSet<i64>,
    created: BTreeSet<i64>,
}

/// The abstract database: committed state, live transactions, and the
/// writes of committed transactions (for first-updater-wins checks).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Abstract {
    committed: BTreeSet<i64>,
    commits: usize,
    txns: [Option<Txn>; TXNS],
    history: Vec<CommittedWrites>,
}

impl Abstract {
    /// The initial database: one committed tuple, so deletes of committed
    /// data are reachable within a few steps.
    fn new() -> Self {
        Self {
            committed: BTreeSet::from([INITIAL]),
            commits: 0,
            txns: [None, None],
            history: Vec::new(),
        }
    }

    /// Has a transaction concurrent with `t` written tuple `v` — a live one,
    /// or one that committed after `t` began? `live` and `committed` select
    /// which writes (deletions or creations) count.
    fn concurrent_write(
        &self,
        t: usize,
        v: i64,
        live: impl Fn(&Txn) -> &BTreeSet<i64>,
        committed: impl Fn(&CommittedWrites) -> &BTreeSet<i64>,
    ) -> bool {
        let Some(txn) = &self.txns[t] else {
            return false;
        };
        let live_conflict = self
            .txns
            .iter()
            .enumerate()
            .any(|(u, other)| u != t && other.as_ref().is_some_and(|o| live(o).contains(&v)));
        let committed_conflict = self
            .history
            .iter()
            .any(|w| w.seq >= txn.began_at && committed(w).contains(&v));
        live_conflict || committed_conflict
    }

    /// The model's verdict on an action: `true` = must succeed,
    /// `false` = must fail with a write conflict (state unchanged).
    fn apply(&mut self, action: Action) -> bool {
        match action {
            Action::Begin(t) => {
                self.txns[t] = Some(Txn {
                    began_at: self.commits,
                    snapshot: self.committed.clone(),
                    marked: BTreeSet::new(),
                    created: BTreeSet::new(),
                });
                true
            }
            Action::Insert(t, v) => {
                let Some(txn) = &self.txns[t] else {
                    return true;
                };
                if txn.view().contains(&v) {
                    // Already visible: set semantics, nothing to write.
                    return true;
                }
                if self.concurrent_write(t, v, |o| &o.created, |w| &w.created) {
                    return false;
                }
                if let Some(txn) = &mut self.txns[t] {
                    txn.created.insert(v);
                }
                true
            }
            Action::Delete(t, v) => {
                let Some(txn) = &self.txns[t] else {
                    return true;
                };
                if txn.created.contains(&v) {
                    // Deleting its own version.
                    if let Some(txn) = &mut self.txns[t] {
                        txn.created.remove(&v);
                    }
                    return true;
                }
                if !txn.snapshot.contains(&v) || txn.marked.contains(&v) {
                    // Not visible: nothing to delete.
                    return true;
                }
                if self.concurrent_write(t, v, |o| &o.marked, |w| &w.marked) {
                    return false;
                }
                if let Some(txn) = &mut self.txns[t] {
                    txn.marked.insert(v);
                }
                true
            }
            Action::Commit(t) => {
                if let Some(txn) = self.txns[t].take() {
                    for v in &txn.marked {
                        self.committed.remove(v);
                    }
                    self.committed.extend(txn.created.iter().copied());
                    self.history.push(CommittedWrites {
                        seq: self.commits,
                        marked: txn.marked,
                        created: txn.created,
                    });
                    self.commits += 1;
                }
                true
            }
            Action::Rollback(t) => {
                self.txns[t] = None;
                true
            }
            Action::Crash => {
                self.txns = [None, None];
                true
            }
        }
    }
}

/// A Stateright state: the history replayed on the engine, and the model
/// state it leads to.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct State {
    history: Vec<Action>,
    model: Abstract,
}

/// The Stateright model: histories up to `depth` actions.
struct EngineModel {
    depth: usize,
}

/// The heading of [`RELVAR`].
fn rel_type() -> RelationType {
    RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int))
}

/// The `id` values of a relation.
fn values(relation: &Relation) -> BTreeSet<i64> {
    relation
        .tuples()
        .map(|t| match t.get("id") {
            Some(relvar_core::values::ScalarValue::Int(v)) => *v,
            other => panic!("unexpected attribute value {other:?}"),
        })
        .collect()
}

/// A fresh database directory, on tmpfs when available: it keeps fsync
/// cheap, so the exhaustive search stays fast.
fn scratch_dir() -> tempfile::TempDir {
    let shm = Path::new("/dev/shm");
    if shm.is_dir()
        && let Ok(dir) = tempfile::tempdir_in(shm)
    {
        return dir;
    }
    tempfile::tempdir().expect("temp dir")
}

/// Replays a history on a real engine and checks every step against the
/// model. Returns a description of the first divergence.
fn replay(history: &[Action]) -> Result<(), String> {
    let dir = scratch_dir();
    let mut engine = PersistentEngine::open(dir.path()).map_err(|e| format!("open: {e}"))?;
    engine
        .create_relation(RELVAR, rel_type())
        .map_err(|e| format!("create: {e}"))?;
    engine
        .insert_tuple(RELVAR, tuple! { id: INITIAL })
        .map_err(|e| format!("initial insert: {e}"))?;
    let mut handles: [Option<PersistentSnapshot>; TXNS] = [None, None];
    let mut model = Abstract::new();

    for (step, &action) in history.iter().enumerate() {
        let expect_ok = model.clone().apply(action);
        let outcome = if action == Action::Crash {
            // Drop without commit or flush: the crash. Then recover.
            handles = [None, None];
            drop(engine);
            engine = PersistentEngine::open(dir.path()).map_err(|e| format!("reopen: {e}"))?;
            Ok(())
        } else {
            run(&mut engine, &mut handles, action)
        };
        match (expect_ok, &outcome) {
            (true, Ok(())) | (false, Err(_)) => {}
            (true, Err(e)) => return Err(format!("step {step} {action:?}: engine failed: {e}")),
            (false, Ok(())) => {
                return Err(format!(
                    "step {step} {action:?}: engine accepted a write conflict"
                ));
            }
        }
        model.apply(action);
        check(&mut engine, &handles, &model)
            .map_err(|e| format!("after step {step} {action:?}: {e}"))?;
    }
    Ok(())
}

/// Runs `f` with `handle`'s transaction ambient, then suspends it.
fn with_txn<T>(
    engine: &mut PersistentEngine,
    handle: &PersistentSnapshot,
    f: impl FnOnce(&mut PersistentEngine) -> Result<T, String>,
) -> Result<T, String> {
    let _ = engine.suspend_transaction();
    engine
        .resume_transaction(handle)
        .map_err(|e| format!("resume: {e}"))?;
    let result = f(engine);
    let _ = engine.suspend_transaction();
    result
}

/// Performs one (non-crash) action on the engine.
fn run(
    engine: &mut PersistentEngine,
    handles: &mut [Option<PersistentSnapshot>; TXNS],
    action: Action,
) -> Result<(), String> {
    match action {
        Action::Begin(t) => {
            let _ = engine.suspend_transaction();
            let handle = engine
                .begin_transaction_with_isolation(IsolationLevel::RepeatableRead)
                .map_err(|e| e.to_string())?;
            let _ = engine.suspend_transaction();
            handles[t] = Some(handle);
            Ok(())
        }
        Action::Insert(t, v) => {
            let handle = handles[t].clone().ok_or("no handle")?;
            with_txn(engine, &handle, |e| {
                e.insert_tuple(RELVAR, tuple! { id: v })
                    .map_err(|e| e.to_string())
            })
        }
        Action::Delete(t, v) => {
            let handle = handles[t].clone().ok_or("no handle")?;
            with_txn(engine, &handle, |e| {
                let current = e.load_relation(RELVAR).map_err(|e| e.to_string())?;
                let kept = current.restrict(|tuple| {
                    tuple.get("id") != Some(&relvar_core::values::ScalarValue::Int(v))
                });
                e.store_relation(RELVAR, &kept).map_err(|e| e.to_string())
            })
        }
        Action::Commit(t) => {
            let handle = handles[t].take().ok_or("no handle")?;
            let _ = engine.suspend_transaction();
            engine.commit_transaction(handle).map_err(|e| e.to_string())
        }
        Action::Rollback(t) => {
            let handle = handles[t].take().ok_or("no handle")?;
            let _ = engine.suspend_transaction();
            engine
                .rollback_transaction(handle)
                .map_err(|e| e.to_string())
        }
        Action::Crash => unreachable!("crashes are handled by replay"),
    }
}

/// Checks every live transaction's view and the committed state against
/// the model.
fn check(
    engine: &mut PersistentEngine,
    handles: &[Option<PersistentSnapshot>; TXNS],
    model: &Abstract,
) -> Result<(), String> {
    for (t, handle) in handles.iter().enumerate() {
        match (handle, &model.txns[t]) {
            (Some(handle), Some(txn)) => {
                let seen = with_txn(engine, handle, |e| {
                    e.load_relation(RELVAR).map_err(|e| e.to_string())
                })?;
                if values(&seen) != txn.view() {
                    return Err(format!(
                        "T{t} reads {:?}, model view is {:?} (snapshot {:?})",
                        values(&seen),
                        txn.view(),
                        txn.snapshot
                    ));
                }
            }
            (None, None) => {}
            _ => return Err(format!("T{t}: handle/model liveness disagree")),
        }
    }
    let _ = engine.suspend_transaction();
    let committed = engine.load_relation(RELVAR).map_err(|e| e.to_string())?;
    if values(&committed) != model.committed {
        return Err(format!(
            "committed state is {:?}, model says {:?}",
            values(&committed),
            model.committed
        ));
    }
    Ok(())
}

impl Model for EngineModel {
    type State = State;
    type Action = Action;

    fn init_states(&self) -> Vec<Self::State> {
        vec![State {
            history: Vec::new(),
            model: Abstract::new(),
        }]
    }

    fn actions(&self, state: &Self::State, actions: &mut Vec<Self::Action>) {
        for t in 0..TXNS {
            if state.model.txns[t].is_some() {
                for v in VALUES {
                    actions.push(Action::Insert(t, v));
                    actions.push(Action::Delete(t, v));
                }
                actions.push(Action::Commit(t));
                actions.push(Action::Rollback(t));
            } else {
                actions.push(Action::Begin(t));
            }
        }
        actions.push(Action::Crash);
    }

    fn next_state(&self, state: &Self::State, action: Self::Action) -> Option<Self::State> {
        let mut next = state.clone();
        next.model.apply(action);
        next.history.push(action);
        Some(next)
    }

    fn within_boundary(&self, state: &Self::State) -> bool {
        state.history.len() <= self.depth
    }

    fn properties(&self) -> Vec<Property<Self>> {
        vec![Property::always(
            "engine refines the transaction model",
            |_, state: &State| match replay(&state.history) {
                Ok(()) => true,
                Err(e) => {
                    eprintln!("divergence for {:?}: {e}", state.history);
                    false
                }
            },
        )]
    }
}

#[test]
fn persistent_engine_refines_snapshot_isolation_model() {
    let depth = std::env::var("RELVAR_MODEL_CHECK_DEPTH")
        .ok()
        .and_then(|d| d.parse().ok())
        .unwrap_or(5);
    let checker = EngineModel { depth }
        .checker()
        .threads(std::thread::available_parallelism().map_or(1, |n| n.get()))
        .spawn_bfs()
        .join();
    checker.assert_properties();
    eprintln!(
        "model check: depth {depth}, {} states",
        checker.unique_state_count()
    );
    assert!(checker.unique_state_count() > 1);
}
