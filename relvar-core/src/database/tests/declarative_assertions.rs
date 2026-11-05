//! Declarative database assertions (TTM RM Prescriptions 16 and 23).
//!
//! A [`DeclarativeAssertion`] is a relational boolean over [`Query`]
//! expressions, so — unlike a closure-backed
//! [`DatabaseAssertion`](crate::constraints::DatabaseAssertion) — it is part
//! of the database's stored constraint catalog. It is validated when it is
//! registered and enforced at every statement boundary, exactly like every
//! other constraint, and a reopened database enforces it without any
//! re-registration.

use super::constraint_persistence::{CatalogEngine, reopen};
use super::golden_rule::{fk, pk};
use crate::constraints::{
    AssertionCondition, AssertionError, CmpOp, ConstraintCatalog, ConstraintExpression,
    ConstraintManager, ConstraintManagerError, DatabaseAssertion, DeclarativeAssertion, ValueOrRef,
};
use crate::database::Database;
use crate::error::DatabaseError;
use crate::query::Query;
use crate::storage_engine::{InMemoryEngine, StorageEngine};
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, ScalarValue, Tuple};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

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

fn p_rel(pids: &[i64]) -> Relation {
    Relation::from_tuples(p_type(), pids.iter().map(|p| tuple! { pid: *p })).unwrap()
}

fn c_rel(pairs: &[(i64, i64)]) -> Relation {
    Relation::from_tuples(
        c_type(),
        pairs.iter().map(|(c, p)| tuple! { cid: *c, pid: *p }),
    )
    .unwrap()
}

/// P{pid} keyed on pid, C{cid, pid} keyed on cid, and no foreign key: the
/// assertions under test do the cross-relvar work. Starts with P = {1}
/// and C = {(10, 1)}.
fn pc_db<E: StorageEngine>(engine: E) -> Database<E> {
    let mut db = Database::new(engine);
    db.create_relvar("P", p_type()).unwrap();
    db.create_relvar("C", c_type()).unwrap();
    db.set_key_constraints("P", pk(&["pid"])).unwrap();
    db.set_key_constraints("C", pk(&["cid"])).unwrap();
    db.insert("P", tuple! { pid: 1i64 }).unwrap();
    db.insert("C", tuple! { cid: 10i64, pid: 1i64 }).unwrap();
    db
}

fn memory_db() -> Database<InMemoryEngine> {
    pc_db(InMemoryEngine::new())
}

/// IS_EMPTY(C NOT MATCHING P): every child references an existing parent.
fn child_has_parent() -> DeclarativeAssertion {
    DeclarativeAssertion::new(
        "child_has_parent",
        "Every child references an existing parent",
        AssertionCondition::IsEmpty(Query::scan("C").not_matching(Query::scan("P"))),
    )
}

fn pid_eq(value: i64) -> ConstraintExpression {
    ConstraintExpression::Cmp {
        left: "pid".to_string(),
        op: CmpOp::Eq,
        right: ValueOrRef::Value(ScalarValue::Int(value)),
    }
}

fn state<E: StorageEngine>(db: &Database<E>) -> (Vec<i64>, Vec<(i64, i64)>) {
    let mut p: Vec<i64> = db
        .query("P")
        .unwrap()
        .tuples()
        .filter_map(|t| t.get_typed::<i64>("pid"))
        .collect();
    p.sort_unstable();
    let mut c: Vec<(i64, i64)> = db
        .query("C")
        .unwrap()
        .tuples()
        .filter_map(|t| Some((t.get_typed::<i64>("cid")?, t.get_typed::<i64>("pid")?)))
        .collect();
    c.sort_unstable();
    (p, c)
}

fn is_violation(result: &Result<impl core::fmt::Debug, DatabaseError>, name: &str) -> bool {
    matches!(result, Err(DatabaseError::AssertionViolation(message)) if message.contains(name))
}

fn orphan() -> Tuple {
    tuple! { cid: 11i64, pid: 9i64 }
}

// --- Enforcement on every statement kind ---------------------------------

#[test]
fn is_empty_assertion_rejects_violating_insert() {
    let mut db = memory_db();
    db.add_declarative_assertion(child_has_parent()).unwrap();

    assert!(is_violation(&db.insert("C", orphan()), "child_has_parent"));
    assert_eq!(state(&db), (vec![1], vec![(10, 1)]));
    // The key index was not touched by the undone insert.
    db.insert("P", tuple! { pid: 9i64 }).unwrap();
    db.insert("C", orphan()).unwrap();
}

#[test]
fn is_empty_assertion_rejects_violating_bulk_insert() {
    let mut db = memory_db();
    db.add_declarative_assertion(child_has_parent()).unwrap();

    let result = db.bulk_insert("C", vec![tuple! { cid: 12i64, pid: 1i64 }, orphan()]);
    assert!(is_violation(&result, "child_has_parent"));
    assert_eq!(state(&db), (vec![1], vec![(10, 1)]));
}

#[test]
fn is_empty_assertion_rejects_violating_delete() {
    let mut db = memory_db();
    db.add_declarative_assertion(child_has_parent()).unwrap();

    assert!(is_violation(&db.delete("P", |_| true), "child_has_parent"));
    assert_eq!(state(&db), (vec![1], vec![(10, 1)]));
}

#[test]
fn is_empty_assertion_rejects_violating_update() {
    let mut db = memory_db();
    db.add_declarative_assertion(child_has_parent()).unwrap();

    let result = db.update("C", |_| true, |_| tuple! { cid: 10i64, pid: 2i64 });
    assert!(is_violation(&result, "child_has_parent"));
    assert_eq!(state(&db), (vec![1], vec![(10, 1)]));
}

#[test]
fn is_empty_assertion_rejects_violating_multiple_assignment() {
    let mut db = memory_db();
    db.add_declarative_assertion(child_has_parent()).unwrap();

    let result = db.multiple_assign(vec![("P", p_rel(&[2])), ("C", c_rel(&[(10, 1)]))]);
    assert!(is_violation(&result, "child_has_parent"));
    assert_eq!(state(&db), (vec![1], vec![(10, 1)]));
}

#[test]
fn satisfying_statements_still_succeed() {
    let mut db = memory_db();
    db.add_declarative_assertion(child_has_parent()).unwrap();

    db.insert("P", tuple! { pid: 2i64 }).unwrap();
    db.bulk_insert("C", vec![tuple! { cid: 11i64, pid: 2i64 }])
        .unwrap();
    db.update(
        "C",
        |t| t.get_typed::<i64>("cid") == Some(11),
        |_| {
            tuple! { cid: 11i64, pid: 1i64 }
        },
    )
    .unwrap();
    db.delete("P", |t| t.get_typed::<i64>("pid") == Some(2))
        .unwrap();
    db.multiple_assign(vec![("P", p_rel(&[3])), ("C", c_rel(&[(12, 3)]))])
        .unwrap();
    assert_eq!(state(&db), (vec![3], vec![(12, 3)]));
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn violation_inside_transaction_undoes_only_that_statement() {
    let mut db = memory_db();
    db.add_declarative_assertion(child_has_parent()).unwrap();

    db.begin().unwrap();
    db.insert("P", tuple! { pid: 2i64 }).unwrap();
    assert!(is_violation(&db.insert("C", orphan()), "child_has_parent"));
    assert!(is_violation(&db.delete("P", |_| true), "child_has_parent"));
    db.insert("C", tuple! { cid: 11i64, pid: 2i64 }).unwrap();
    db.commit().unwrap();

    assert_eq!(state(&db), (vec![1, 2], vec![(10, 1), (11, 2)]));
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn rollback_after_enforced_statements_restores_the_start() {
    let mut db = memory_db();
    db.add_declarative_assertion(child_has_parent()).unwrap();

    db.begin().unwrap();
    db.insert("P", tuple! { pid: 2i64 }).unwrap();
    assert!(is_violation(&db.insert("C", orphan()), "child_has_parent"));
    db.rollback().unwrap();

    assert_eq!(state(&db), (vec![1], vec![(10, 1)]));
}

// --- Every condition kind -------------------------------------------------

#[test]
fn not_empty_assertion_keeps_a_relvar_populated() {
    let mut db = memory_db();
    db.add_declarative_assertion(DeclarativeAssertion::new(
        "has_parent",
        "P is never empty",
        AssertionCondition::NotEmpty(Query::scan("P")),
    ))
    .unwrap();

    db.insert("P", tuple! { pid: 2i64 }).unwrap();
    assert!(is_violation(&db.delete("P", |_| true), "has_parent"));
    assert!(is_violation(
        &db.multiple_assign(vec![("P", p_rel(&[]))]),
        "has_parent"
    ));
    db.delete("P", |t| t.get_typed::<i64>("pid") == Some(2))
        .unwrap();
    assert_eq!(state(&db).0, vec![1]);
}

#[test]
fn subset_assertion_is_an_inclusion_dependency() {
    let mut db = memory_db();
    db.add_declarative_assertion(DeclarativeAssertion::new(
        "child_pids_are_parents",
        "C{pid} is a subset of P{pid}",
        AssertionCondition::Subset(
            Query::scan("C").project(vec!["pid"]),
            Query::scan("P").project(vec!["pid"]),
        ),
    ))
    .unwrap();

    assert!(is_violation(
        &db.insert("C", orphan()),
        "child_pids_are_parents"
    ));
    assert!(is_violation(
        &db.delete("P", |_| true),
        "child_pids_are_parents"
    ));
    db.insert("P", tuple! { pid: 9i64 }).unwrap();
    db.insert("C", orphan()).unwrap();
}

#[test]
fn equal_assertion_pairs_every_parent_with_a_child() {
    // P{pid} = C{pid}: every parent has a child and every child a parent,
    // so a new parent and its first child can only arrive together.
    let mut db = memory_db();
    db.add_declarative_assertion(DeclarativeAssertion::new(
        "one_to_one_or_more",
        "P{pid} equals C{pid}",
        AssertionCondition::Equal(Query::scan("P"), Query::scan("C").project(vec!["pid"])),
    ))
    .unwrap();

    assert!(is_violation(
        &db.insert("P", tuple! { pid: 2i64 }),
        "one_to_one_or_more"
    ));
    assert!(is_violation(
        &db.insert("C", tuple! { cid: 11i64, pid: 2i64 }),
        "one_to_one_or_more"
    ));
    db.multiple_assign(vec![
        ("P", p_rel(&[1, 2])),
        ("C", c_rel(&[(10, 1), (11, 2)])),
    ])
    .unwrap();
    // A second child of an existing parent keeps the projection equal.
    db.insert("C", tuple! { cid: 12i64, pid: 2i64 }).unwrap();
    assert_eq!(state(&db).0, vec![1, 2]);
}

#[test]
fn cardinality_assertion_bounds_a_relvar() {
    let mut db = memory_db();
    db.add_declarative_assertion(DeclarativeAssertion::new(
        "at_most_two_parents",
        "P holds at most two tuples",
        AssertionCondition::Cardinality {
            query: Query::scan("P"),
            comparison: CmpOp::Le,
            value: 2,
        },
    ))
    .unwrap();

    db.insert("P", tuple! { pid: 2i64 }).unwrap();
    assert!(is_violation(
        &db.insert("P", tuple! { pid: 3i64 }),
        "at_most_two_parents"
    ));
    assert!(is_violation(
        &db.bulk_insert("P", vec![tuple! { pid: 3i64 }, tuple! { pid: 4i64 }]),
        "at_most_two_parents"
    ));
    assert_eq!(state(&db).0, vec![1, 2]);
}

#[test]
fn cardinality_comparisons_follow_their_operator() {
    // P holds exactly one tuple.
    let db = memory_db();
    let holds = |comparison: CmpOp, value: u64| {
        AssertionCondition::Cardinality {
            query: Query::scan("P"),
            comparison,
            value,
        }
        .is_satisfied_by(&db)
        .unwrap()
    };
    assert!(holds(CmpOp::Eq, 1) && !holds(CmpOp::Eq, 2));
    assert!(holds(CmpOp::Ne, 0) && !holds(CmpOp::Ne, 1));
    assert!(holds(CmpOp::Lt, 2) && !holds(CmpOp::Lt, 1));
    assert!(holds(CmpOp::Le, 1) && !holds(CmpOp::Le, 0));
    assert!(holds(CmpOp::Gt, 0) && !holds(CmpOp::Gt, 1));
    assert!(holds(CmpOp::Ge, 1) && !holds(CmpOp::Ge, 2));
}

#[test]
fn restricted_parent_needs_a_child_only_multiple_assignment_can_satisfy() {
    // C.pid -> P.pid, and IS_EMPTY((P WHERE pid = 2) NOT MATCHING C{pid}):
    // parent 2 may exist only with a child. Adding parent 2 alone breaks
    // the assertion; adding its child alone breaks the foreign key; only
    // one statement assigning both relvars gets from one consistent state
    // to the next.
    let mut db = memory_db();
    db.set_foreign_key_constraints("C", fk(&["pid"], "P", &["pid"]))
        .unwrap();
    db.add_declarative_assertion(DeclarativeAssertion::new(
        "parent_two_has_a_child",
        "Parent 2 exists only together with a child",
        AssertionCondition::IsEmpty(
            Query::scan("P")
                .restrict(pid_eq(2))
                .not_matching(Query::scan("C").project(vec!["pid"])),
        ),
    ))
    .unwrap();

    assert!(is_violation(
        &db.insert("P", tuple! { pid: 2i64 }),
        "parent_two_has_a_child"
    ));
    assert!(matches!(
        db.insert("C", tuple! { cid: 11i64, pid: 2i64 }),
        Err(DatabaseError::Constraint(
            ConstraintManagerError::ForeignKeyViolation(_)
        ))
    ));
    db.multiple_assign(vec![
        ("P", p_rel(&[1, 2])),
        ("C", c_rel(&[(10, 1), (11, 2)])),
    ])
    .unwrap();
    assert_eq!(state(&db), (vec![1, 2], vec![(10, 1), (11, 2)]));
    assert!(db.verify_integrity().is_ok());
}

// --- Registration ----------------------------------------------------------

#[test]
fn registration_is_rejected_when_the_state_violates_it() {
    let mut db = memory_db();
    db.insert("C", orphan()).unwrap();

    let result = db.add_declarative_assertion(child_has_parent());
    assert!(is_violation(&result, "child_has_parent"));
    assert!(db.declarative_assertions().is_empty());
}

#[test]
fn registration_is_rejected_when_the_name_is_taken() {
    let mut db = memory_db();
    db.add_declarative_assertion(child_has_parent()).unwrap();
    db.add_assertion(DatabaseAssertion::new(
        "closure_rule",
        "Always holds",
        |_: &mut Database<InMemoryEngine>| true,
    ))
    .unwrap();

    let declarative_clash = db.add_declarative_assertion(child_has_parent());
    assert!(matches!(
        declarative_clash,
        Err(DatabaseError::Assertion(AssertionError::DuplicateName(name))) if name == "child_has_parent"
    ));
    let closure_clash = db.add_declarative_assertion(DeclarativeAssertion::new(
        "closure_rule",
        "Clashes with the closure assertion",
        AssertionCondition::NotEmpty(Query::scan("P")),
    ));
    assert!(matches!(
        closure_clash,
        Err(DatabaseError::Assertion(AssertionError::DuplicateName(name))) if name == "closure_rule"
    ));
    let reverse_clash = db.add_assertion(DatabaseAssertion::new(
        "child_has_parent",
        "Clashes with the declarative assertion",
        |_: &mut Database<InMemoryEngine>| true,
    ));
    assert!(matches!(
        reverse_clash,
        Err(DatabaseError::Assertion(AssertionError::DuplicateName(name))) if name == "child_has_parent"
    ));
    assert_eq!(db.declarative_assertions().len(), 1);
    assert_eq!(db.assertions().len(), 1);
}

#[test]
fn registration_is_rejected_for_an_unknown_relvar() {
    let mut db = memory_db();
    let result = db.add_declarative_assertion(DeclarativeAssertion::new(
        "ghost",
        "Refers to a relvar that does not exist",
        AssertionCondition::IsEmpty(Query::scan("P").minus(Query::scan("GHOST"))),
    ));
    assert!(matches!(
        result,
        Err(DatabaseError::Assertion(AssertionError::UnknownRelvar { relvar, .. })) if relvar == "GHOST"
    ));
    assert!(db.declarative_assertions().is_empty());
}

#[test]
fn registration_is_rejected_for_a_virtual_relvar() {
    // Virtual relvars are closures and are not persisted, so a stored
    // assertion cannot depend on one.
    let mut db = memory_db();
    db.define_virtual_relvar("V", p_type(), |_| Ok(Relation::new(p_type())))
        .unwrap();
    let result = db.add_declarative_assertion(DeclarativeAssertion::new(
        "virtual",
        "Refers to a virtual relvar",
        AssertionCondition::IsEmpty(Query::scan("V")),
    ));
    assert!(matches!(
        result,
        Err(DatabaseError::Assertion(AssertionError::UnknownRelvar { relvar, .. })) if relvar == "V"
    ));
}

#[test]
fn registration_is_rejected_when_the_condition_is_ill_typed() {
    let mut db = memory_db();
    for condition in [
        AssertionCondition::Subset(Query::scan("P"), Query::scan("C")),
        AssertionCondition::Equal(Query::scan("C"), Query::scan("P")),
        AssertionCondition::IsEmpty(Query::scan("P").union(Query::scan("C"))),
    ] {
        let result =
            db.add_declarative_assertion(DeclarativeAssertion::new("ill_typed", "", condition));
        assert!(
            matches!(
                result,
                Err(DatabaseError::Assertion(AssertionError::Evaluation { .. }))
            ),
            "{result:?}"
        );
    }
    assert!(db.declarative_assertions().is_empty());
}

#[test]
fn registration_is_rejected_inside_a_transaction() {
    let mut db = memory_db();
    db.begin().unwrap();
    assert!(matches!(
        db.add_declarative_assertion(child_has_parent()),
        Err(DatabaseError::TransactionError(_))
    ));
    db.rollback().unwrap();
    assert!(db.declarative_assertions().is_empty());
}

#[test]
fn evaluation_failure_during_a_statement_rejects_it() {
    // `pid = "x"` compares an Int with a String: harmless while P is empty
    // (no tuple is ever compared), a type error once P has a tuple.
    let mut db = Database::new(InMemoryEngine::new());
    db.create_relvar("P", p_type()).unwrap();
    db.add_declarative_assertion(DeclarativeAssertion::new(
        "ill_typed_predicate",
        "Compares pid with a string",
        AssertionCondition::IsEmpty(Query::scan("P").restrict(ConstraintExpression::Cmp {
            left: "pid".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::String("x".to_string())),
        })),
    ))
    .unwrap();

    let result = db.insert("P", tuple! { pid: 1i64 });
    assert!(matches!(
        result,
        Err(DatabaseError::Assertion(AssertionError::Evaluation { assertion_name, .. }))
            if assertion_name == "ill_typed_predicate"
    ));
    assert!(db.query("P").unwrap().is_empty());
}

// --- Removal, verification, DDL --------------------------------------------

#[test]
fn removing_an_assertion_stops_enforcement() {
    let mut db = memory_db();
    db.add_declarative_assertion(child_has_parent()).unwrap();

    assert!(db.remove_assertion("child_has_parent").unwrap());
    assert!(!db.remove_assertion("child_has_parent").unwrap());
    assert!(db.declarative_assertions().is_empty());
    db.insert("C", orphan()).unwrap();
}

#[test]
fn removing_an_assertion_inside_a_transaction_is_rejected() {
    let mut db = memory_db();
    db.add_declarative_assertion(child_has_parent()).unwrap();
    db.begin().unwrap();
    assert!(matches!(
        db.remove_assertion("child_has_parent"),
        Err(DatabaseError::TransactionError(_))
    ));
    db.rollback().unwrap();
    assert_eq!(db.declarative_assertions().len(), 1);
}

#[test]
fn verify_integrity_detects_a_declarative_violation() {
    let mut db = memory_db();
    db.add_declarative_assertion(child_has_parent()).unwrap();
    assert!(db.verify_integrity().is_ok());

    // Written behind the database's back, as another tool might.
    db.engine.insert_tuple("C", orphan()).unwrap();
    assert!(is_violation(&db.verify_integrity(), "child_has_parent"));
}

#[test]
fn dropping_a_relvar_an_assertion_refers_to_is_refused() {
    let mut db = memory_db();
    db.add_declarative_assertion(child_has_parent()).unwrap();

    let result = db.drop_relvar("P");
    assert!(matches!(
        result,
        Err(DatabaseError::Assertion(AssertionError::RelvarReferenced { relvar, assertion_name }))
            if relvar == "P" && assertion_name == "child_has_parent"
    ));
    assert!(db.relvar_exists("P"));

    db.remove_assertion("child_has_parent").unwrap();
    db.drop_relvar("P").unwrap();
}

// --- Persistence -----------------------------------------------------------

#[test]
fn registering_stores_the_assertion_in_the_catalog() {
    let mut db = pc_db(CatalogEngine::default());
    db.add_declarative_assertion(child_has_parent()).unwrap();

    let stored = db.engine.stored.clone().unwrap();
    let names: Vec<&str> = stored
        .database()
        .assertions
        .iter()
        .map(DeclarativeAssertion::name)
        .collect();
    assert_eq!(names, vec!["child_has_parent"]);

    db.remove_assertion("child_has_parent").unwrap();
    assert!(
        db.engine
            .stored
            .as_ref()
            .unwrap()
            .database()
            .assertions
            .is_empty()
    );
}

#[test]
fn reopened_database_enforces_the_assertion_without_re_registration() {
    let mut db = pc_db(CatalogEngine::default());
    db.add_declarative_assertion(child_has_parent()).unwrap();

    let mut db = reopen(db);
    assert_eq!(db.declarative_assertions().len(), 1);
    assert!(is_violation(&db.insert("C", orphan()), "child_has_parent"));
    assert!(is_violation(&db.delete("P", |_| true), "child_has_parent"));
    assert!(db.verify_integrity().is_ok());
}

#[test]
fn failed_store_leaves_the_assertion_unregistered() {
    let mut db = pc_db(CatalogEngine::default());
    db.engine.fail_store = true;

    assert!(matches!(
        db.add_declarative_assertion(child_has_parent()),
        Err(DatabaseError::Storage(_))
    ));
    assert!(db.declarative_assertions().is_empty());
    db.insert("C", orphan()).unwrap();
}

#[test]
fn failed_store_keeps_a_removed_assertion() {
    let mut db = pc_db(CatalogEngine::default());
    db.add_declarative_assertion(child_has_parent()).unwrap();
    db.engine.fail_store = true;

    assert!(matches!(
        db.remove_assertion("child_has_parent"),
        Err(DatabaseError::Storage(_))
    ));
    assert_eq!(db.declarative_assertions().len(), 1);
}

#[test]
fn catalog_round_trips_assertions_through_json() {
    let mut db = memory_db();
    db.add_declarative_assertion(child_has_parent()).unwrap();

    let text = serde_json::to_string(&db.constraints.to_catalog()).unwrap();
    let back: ConstraintCatalog = serde_json::from_str(&text).unwrap();
    let manager = ConstraintManager::from_catalog(back);
    let restored = &manager.database_constraints().assertions;
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].name(), "child_has_parent");
    assert_eq!(
        restored[0].description(),
        "Every child references an existing parent"
    );
    assert_eq!(restored[0].referenced_relvars(), vec!["C", "P"]);
}
