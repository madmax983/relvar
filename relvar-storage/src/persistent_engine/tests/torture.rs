//! Crash-consistency torture test (TTM RM Prescription 17).
//!
//! Deterministic and seeded: random sequences of begin / insert /
//! store_relation (deletes and updates) / commit / rollback run against a
//! simple model, with WAL log/flush failures injected at random points
//! and simulated crashes (the engine is dropped with a transaction still
//! open, sometimes after forcing its uncommitted heap pages to disk).
//! After every transaction and every reopen the engine's committed state
//! must equal the model: exactly the committed transactions' effects,
//! nothing from uncommitted or aborted ones.
//!
//! A commit whose commit-point flush fails is "outcome uncertain" by
//! contract: the engine ends it as committed, and its Commit record
//! becomes durable with the next successful WAL flush. Until then the
//! model accepts either outcome after a crash.

use super::common::*;
use crate::persistent_engine::*;
use relvar_core::storage_engine::{IsolationLevel, StorageEngine};
use relvar_core::tuple;
use relvar_core::values::{Relation, ScalarValue};
use std::collections::BTreeSet;
use std::path::Path;
use tempfile::TempDir;

/// The relation value as a set of `(id, name)` pairs.
type Model = BTreeSet<(i64, String)>;

/// SplitMix64: tiny, deterministic, good enough for test schedules.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// True with probability `percent`/100.
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

fn to_relation(model: &Model) -> Relation {
    let mut relation = Relation::new(test_rel_type());
    for (id, name) in model {
        relation
            .insert(tuple! { id: *id, name: name.as_str() })
            .unwrap();
    }
    relation
}

fn to_model(relation: &Relation) -> Model {
    relation
        .tuples()
        .map(|t| {
            let id = t.get_typed::<i64>("id").unwrap();
            let name = match t.get("name") {
                Some(ScalarValue::String(name)) => name.clone(),
                other => panic!("unexpected name {other:?}"),
            };
            (id, name)
        })
        .collect()
}

/// The committed state visible outside any transaction.
fn committed_state(engine: &PersistentEngine) -> Model {
    assert!(engine.current_txn.is_none());
    to_model(&engine.load_relation("T").unwrap())
}

/// Arms one random WAL fault (or none). Returns how many WAL records
/// succeed before an armed log fault fires, if one was armed.
fn maybe_inject(engine: &mut PersistentEngine, rng: &mut Rng, percent: u64) -> Option<u32> {
    if !rng.chance(percent) {
        return None;
    }
    if rng.chance(50) {
        // A log failure after `n` records: n > 0 fails a multi-record
        // statement part-way, exercising its compensation.
        let n = rng.below(4) as u32;
        engine.wal.inject_log_failure_in(n);
        Some(n)
    } else {
        engine.wal.inject_flush_failure_in(rng.below(3) as u32);
        None
    }
}

/// Disarms any pending fault so bookkeeping steps run clean.
fn disarm(engine: &mut PersistentEngine) {
    engine.wal.clear_injected_failures();
}

/// How often each path was exercised: the test asserts every path ran,
/// so a schedule change cannot silently stop covering one.
#[derive(Debug, Default)]
struct Stats {
    commits: usize,
    uncertain_commits: usize,
    failed_commits: usize,
    rollbacks: usize,
    retried_rollbacks: usize,
    crashes: usize,
    crashes_after_heap_flush: usize,
    failed_statements: usize,
    partially_applied_failures: usize,
    failed_begins: usize,
    reader_checks: usize,
    checkpoints: usize,
}

/// One random statement against the transaction's working state.
/// Returns whether it succeeded.
fn random_statement(
    engine: &mut PersistentEngine,
    rng: &mut Rng,
    working: &mut Model,
    serial: &mut u64,
) -> bool {
    *serial += 1;
    let mut next = working.clone();
    let result = match rng.below(4) {
        // Insert one tuple (sometimes a value already present).
        0 => {
            let id = rng.below(24) as i64;
            let name = format!("n{}", rng.below(3));
            next.insert((id, name.clone()));
            engine.insert_tuple("T", tuple! { id: id, name: name.as_str() })
        }
        // Delete a random subset.
        1 => {
            next.retain(|_| !rng.chance(30));
            engine.store_relation("T", &to_relation(&next))
        }
        // Update: rename a random subset, add a few fresh tuples.
        2 => {
            let updated: Model = next
                .iter()
                .map(|(id, name)| {
                    if rng.chance(30) {
                        (*id, format!("u{serial}"))
                    } else {
                        (*id, name.clone())
                    }
                })
                .collect();
            next = updated;
            for _ in 0..rng.below(3) {
                next.insert((100 + rng.below(50) as i64, format!("f{serial}")));
            }
            engine.store_relation("T", &to_relation(&next))
        }
        // Unchanged store (empty diff).
        _ => engine.store_relation("T", &to_relation(&next)),
    };
    // On error the statement is atomic: `working` is unchanged (unless
    // the transaction itself was ended, which the caller detects).
    let ok = result.is_ok();
    if ok {
        *working = next;
    }
    ok
}

/// Runs one seeded schedule of `rounds` engine sessions.
fn run_schedule(seed: u64, rounds: usize, dir: &Path, stats: &mut Stats) {
    let mut rng = Rng(seed);
    let mut committed = Model::new();
    // An uncertain commit not yet resolved by a later durable flush:
    // the states before and after it.
    let mut uncertain: Option<(Model, Model)> = None;
    let mut serial = 0u64;

    {
        let mut engine = PersistentEngine::open(dir).unwrap();
        engine.create_relation("T", test_rel_type()).unwrap();
    }

    for round in 0..rounds {
        let mut engine = PersistentEngine::open(dir)
            .unwrap_or_else(|e| panic!("seed {seed} round {round}: reopen failed: {e}"));
        let recovered = committed_state(&engine);
        if let Some((before, after)) = uncertain.take() {
            assert!(
                recovered == before || recovered == after,
                "seed {seed} round {round}: uncertain commit recovered to neither state"
            );
            committed = recovered.clone();
        }
        assert_eq!(recovered, committed, "seed {seed} round {round}: recovery");

        let txns = 1 + rng.below(6);
        for txn in 0..txns {
            // Occasionally a RepeatableRead reader overlaps the writer.
            let reader = if rng.chance(20) {
                disarm(&mut engine);
                let snapshot = engine
                    .begin_transaction_with_isolation(IsolationLevel::RepeatableRead)
                    .unwrap();
                engine.suspend_transaction().unwrap();
                uncertain = None; // begin flushed the WAL: resolved
                stats.reader_checks += 1;
                Some((snapshot, committed.clone()))
            } else {
                None
            };

            let _ = maybe_inject(&mut engine, &mut rng, 15);
            let Ok(snapshot) = engine.begin_transaction() else {
                stats.failed_begins += 1;
                disarm(&mut engine);
                assert_eq!(committed_state(&engine), committed);
                continue;
            };
            // Begin's flush succeeded: any earlier uncertain commit's
            // record is durable now.
            uncertain = None;
            let txn_id = snapshot.txn_id;
            let mut working = committed.clone();

            for _ in 0..1 + rng.below(5) {
                let log_fault = maybe_inject(&mut engine, &mut rng, 15);
                if !random_statement(&mut engine, &mut rng, &mut working, &mut serial) {
                    stats.failed_statements += 1;
                    if log_fault.is_some_and(|n| n > 0) {
                        stats.partially_applied_failures += 1;
                    }
                }
                disarm(&mut engine);
                if !engine.txn_pool.is_live(txn_id) {
                    break; // a failed compensation ended the transaction
                }
                assert_eq!(
                    to_model(&engine.load_relation("T").unwrap()),
                    working,
                    "seed {seed} round {round} txn {txn}: in-transaction view"
                );
            }

            let last_txn = txn + 1 == txns;
            let crash = last_txn && rng.chance(40);
            if crash {
                stats.crashes += 1;
                if rng.chance(50) {
                    stats.crashes_after_heap_flush += 1;
                    // Force the uncommitted heap pages to disk first.
                    engine
                        .storage_manager
                        .write()
                        .unwrap()
                        .flush_heap_files()
                        .unwrap();
                }
                break; // drop the engine with the transaction open
            }

            if !engine.txn_pool.is_live(txn_id) {
                // Ended by a failed compensation: rolled back.
                assert_eq!(committed_state(&engine), committed);
            } else if rng.chance(70) {
                let _ = maybe_inject(&mut engine, &mut rng, 20);
                match engine.commit_transaction(snapshot) {
                    Ok(()) => {
                        stats.commits += 1;
                        committed = working;
                    }
                    Err(error) if error.to_string().contains("outcome uncertain") => {
                        stats.uncertain_commits += 1;
                        uncertain = Some((committed.clone(), working.clone()));
                        committed = working;
                    }
                    Err(_) => stats.failed_commits += 1,
                }
                disarm(&mut engine);
            } else {
                let _ = maybe_inject(&mut engine, &mut rng, 20);
                stats.rollbacks += 1;
                if engine.rollback_transaction(snapshot).is_err() {
                    stats.retried_rollbacks += 1;
                    // A failed Abort flush leaves the transaction live
                    // and retryable.
                    disarm(&mut engine);
                    engine
                        .rollback_transaction(PersistentSnapshot { txn_id })
                        .unwrap();
                }
                disarm(&mut engine);
            }
            assert!(!engine.txn_pool.is_live(txn_id));
            assert_eq!(
                committed_state(&engine),
                committed,
                "seed {seed} round {round} txn {txn}: after end"
            );

            if let Some((reader, seen)) = reader {
                engine.resume_transaction(&reader).unwrap();
                assert_eq!(
                    to_model(&engine.load_relation("T").unwrap()),
                    seen,
                    "seed {seed} round {round} txn {txn}: reader snapshot"
                );
                engine.commit_transaction(reader).unwrap();
            }

            if rng.chance(15) {
                disarm(&mut engine);
                engine.checkpoint().unwrap();
                stats.checkpoints += 1;
                uncertain = None;
                assert_eq!(committed_state(&engine), committed);
            }
        }
        // Clean shutdown or crash: either way the engine is dropped
        // without any explicit close.
        drop(engine);
    }

    let engine = PersistentEngine::open(dir).unwrap();
    let recovered = committed_state(&engine);
    if let Some((before, after)) = uncertain {
        assert!(recovered == before || recovered == after);
    } else {
        assert_eq!(recovered, committed, "seed {seed}: final recovery");
    }
}

#[test]
fn test_crash_consistency_torture() {
    let mut stats = Stats::default();
    for seed in 0..32u64 {
        let dir = TempDir::new().unwrap();
        run_schedule(0x5EED_0000 + seed, 16, dir.path(), &mut stats);
    }
    // Every path must have been exercised.
    let counts = [
        stats.commits,
        stats.uncertain_commits,
        stats.failed_commits,
        stats.rollbacks,
        stats.retried_rollbacks,
        stats.crashes,
        stats.crashes_after_heap_flush,
        stats.failed_statements,
        stats.partially_applied_failures,
        stats.failed_begins,
        stats.reader_checks,
        stats.checkpoints,
    ];
    assert!(counts.iter().all(|&n| n > 0), "unexercised path: {stats:?}");
}
