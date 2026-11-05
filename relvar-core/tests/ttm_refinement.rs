//! Refinement tests: every relational operator against an independent
//! reference model written straight from the TTM definitions.
//!
//! The model mirrors the Verus specification in `verify/verus/algebra.rs`:
//! a relation is a heading (attribute name to type) and a *set* of tuples
//! (attribute name to value). The model body is a `Vec` kept free of
//! duplicates with `==` alone, so the model does not depend on any ordering
//! of `ScalarValue`. Value equality is `ScalarValue`'s own: `NaN` equals
//! `NaN` (whatever its payload) and `0.0` and `-0.0` are distinct values, so
//! equality is a true identity on values, as TTM requires.
//!
//! Each property checks three things: the operator's result equals the
//! model's (an `Err` exactly when the model says the operation is
//! undefined), the result is well-formed (every tuple has the result
//! heading as its type and conforms to it, recursively through
//! relation-valued attributes), and the consuming (`_into`) variants agree.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::sync::Arc;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use relvar_core::algebra::Aggregation;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue, Tuple};

type Heading = BTreeMap<String, ScalarType>;
type MTuple = BTreeMap<String, ScalarValue>;

/// Attribute names headings are drawn from.
const NAMES: [&str; 4] = ["a", "b", "c", "d"];
/// Names that never occur in a generated heading (EXTEND, RENAME targets,
/// SUMMARIZE results, GROUP RVAs). Together with `NAMES` they also produce
/// collisions.
const ANY_NAMES: [&str; 7] = ["a", "b", "c", "d", "x", "y", "z"];

// ---------------------------------------------------------------------------
// The reference model
// ---------------------------------------------------------------------------

/// A relation in the model: a heading and a duplicate-free list of tuples
/// whose order is irrelevant.
#[derive(Clone, Debug)]
struct Model {
    heading: Heading,
    body: Vec<MTuple>,
}

impl Model {
    fn empty(heading: Heading) -> Self {
        Self {
            heading,
            body: Vec::new(),
        }
    }

    fn insert(&mut self, t: MTuple) {
        if !self.body.contains(&t) {
            self.body.push(t);
        }
    }

    fn contains(&self, t: &MTuple) -> bool {
        self.body.contains(t)
    }

    /// Reads a relation into the model (its heading and its set of tuples).
    fn of(r: &Relation) -> Self {
        let mut m = Self::empty(r.relation_type().heading().attributes().clone());
        for t in r.tuples() {
            m.insert(t.values().clone());
        }
        m
    }

    /// Builds the relation with this heading and body, using only the
    /// validating constructors.
    fn to_relation(&self) -> Relation {
        let tt = Arc::new(tuple_type_of(&self.heading));
        let mut r = Relation::new(RelationType::new((*tt).clone()));
        for t in &self.body {
            r.insert(Tuple::new(tt.clone(), t.clone()).unwrap())
                .unwrap();
        }
        r
    }

    /// Set equality of bodies, equality of headings.
    fn same(&self, other: &Self) -> bool {
        self.heading == other.heading
            && self.body.len() == other.body.len()
            && self.body.iter().all(|t| other.contains(t))
    }
}

fn tuple_type_of(h: &Heading) -> TupleType {
    h.iter().fold(TupleType::new(), |tt, (n, ty)| {
        tt.with_attribute(n.clone(), ty.clone())
    })
}

fn restrict_heading(h: &Heading, names: &BTreeSet<String>) -> Heading {
    h.iter()
        .filter(|(n, _)| names.contains(*n))
        .map(|(n, ty)| (n.clone(), ty.clone()))
        .collect()
}

fn restrict_tuple(t: &MTuple, names: &BTreeSet<String>) -> MTuple {
    t.iter()
        .filter(|(n, _)| names.contains(*n))
        .map(|(n, v)| (n.clone(), v.clone()))
        .collect()
}

fn remove_keys(t: &MTuple, names: &BTreeSet<String>) -> MTuple {
    t.iter()
        .filter(|(n, _)| !names.contains(*n))
        .map(|(n, v)| (n.clone(), v.clone()))
        .collect()
}

fn name_set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| (*n).to_string()).collect()
}

/// `t1` and `t2` agree: equal on every attribute they share.
fn agrees(t1: &MTuple, t2: &MTuple) -> bool {
    t1.iter().all(|(n, v)| t2.get(n).is_none_or(|w| w == v))
}

fn union_tuples(t1: &MTuple, t2: &MTuple) -> MTuple {
    let mut u = t1.clone();
    u.extend(t2.iter().map(|(n, v)| (n.clone(), v.clone())));
    u
}

/// `r WHERE p`.
fn m_restrict(r: &Model, p: impl Fn(&MTuple) -> bool) -> Model {
    let mut out = Model::empty(r.heading.clone());
    for t in r.body.iter().filter(|t| p(t)) {
        out.insert(t.clone());
    }
    out
}

/// `r { names }`; undefined if a name is not in the heading.
fn m_project(r: &Model, names: &[&str]) -> Option<Model> {
    if !names.iter().all(|n| r.heading.contains_key(*n)) {
        return None;
    }
    let names = name_set(names);
    let mut out = Model::empty(restrict_heading(&r.heading, &names));
    for t in &r.body {
        out.insert(restrict_tuple(t, &names));
    }
    Some(out)
}

/// `r RENAME { from AS to, ... }`, all renamings simultaneous; undefined if
/// a source is unknown or repeated, or two result attributes share a name.
fn m_rename(r: &Model, pairs: &[(&str, &str)]) -> Option<Model> {
    let sources: BTreeSet<&str> = pairs.iter().map(|(f, _)| *f).collect();
    if sources.len() != pairs.len() || !sources.iter().all(|f| r.heading.contains_key(*f)) {
        return None;
    }
    let new_name = |n: &str| -> String {
        pairs
            .iter()
            .find(|(f, _)| *f == n)
            .map_or(n, |(_, t)| *t)
            .to_string()
    };
    let heading: Heading = r
        .heading
        .iter()
        .map(|(n, ty)| (new_name(n), ty.clone()))
        .collect();
    if heading.len() != r.heading.len() {
        return None;
    }
    let mut out = Model::empty(heading);
    for t in &r.body {
        out.insert(t.iter().map(|(n, v)| (new_name(n), v.clone())).collect());
    }
    Some(out)
}

/// UNION, INTERSECT and MINUS; undefined for different headings.
fn m_set_op(r1: &Model, r2: &Model, keep: impl Fn(bool, bool) -> bool) -> Option<Model> {
    if r1.heading != r2.heading {
        return None;
    }
    let mut out = Model::empty(r1.heading.clone());
    for t in r1.body.iter().chain(&r2.body) {
        if keep(r1.contains(t), r2.contains(t)) {
            out.insert(t.clone());
        }
    }
    Some(out)
}

fn m_union(r1: &Model, r2: &Model) -> Option<Model> {
    m_set_op(r1, r2, |a, b| a || b)
}

fn m_intersect(r1: &Model, r2: &Model) -> Option<Model> {
    m_set_op(r1, r2, |a, b| a && b)
}

fn m_minus(r1: &Model, r2: &Model) -> Option<Model> {
    m_set_op(r1, r2, |a, b| a && !b)
}

/// `r1 JOIN r2`: the unions of the pairs of tuples that agree on the common
/// attributes; undefined if a common attribute has two types.
fn m_join(r1: &Model, r2: &Model) -> Option<Model> {
    for (n, ty) in &r1.heading {
        if r2.heading.get(n).is_some_and(|ty2| ty2 != ty) {
            return None;
        }
    }
    let mut heading = r1.heading.clone();
    heading.extend(r2.heading.iter().map(|(n, ty)| (n.clone(), ty.clone())));
    let mut out = Model::empty(heading);
    for t1 in &r1.body {
        for t2 in r2.body.iter().filter(|t2| agrees(t1, t2)) {
            out.insert(union_tuples(t1, t2));
        }
    }
    Some(out)
}

/// `r1 MATCHING r2` (`matching == true`) or `r1 NOT MATCHING r2`.
fn m_semi(r1: &Model, r2: &Model, matching: bool) -> Model {
    m_restrict(r1, |t1| r2.body.iter().any(|t2| agrees(t1, t2)) == matching)
}

/// `EXTEND r : { name := f(t) }`; undefined if `name` is in the heading or
/// some `f(t)` is not of type `ty`.
fn m_extend(
    r: &Model,
    name: &str,
    ty: &ScalarType,
    f: impl Fn(&MTuple) -> ScalarValue,
) -> Option<Model> {
    if r.heading.contains_key(name) {
        return None;
    }
    let mut heading = r.heading.clone();
    heading.insert(name.to_string(), ty.clone());
    let mut out = Model::empty(heading);
    for t in &r.body {
        let v = f(t);
        if &v.scalar_type() != ty {
            return None;
        }
        let mut u = t.clone();
        u.insert(name.to_string(), v);
        out.insert(u);
    }
    Some(out)
}

/// An aggregate of a SUMMARIZE, named by `result`.
#[derive(Clone, Debug)]
struct Agg {
    result: String,
    kind: AggKind,
}

#[derive(Clone, Debug)]
enum AggKind {
    Count,
    /// `SUM` declared `Int`.
    Sum(String),
    /// `SUM` declared `Float`.
    SumFloat(String),
    Avg(String),
    Min(String),
    Max(String),
}

impl Agg {
    fn to_aggregation(&self, heading: &Heading) -> Aggregation {
        let ty = |a: &str| heading[a].clone();
        match &self.kind {
            AggKind::Count => Aggregation::count(&self.result),
            AggKind::Sum(a) => Aggregation::sum(&self.result, a),
            AggKind::SumFloat(a) => Aggregation::sum_float(&self.result, a),
            AggKind::Avg(a) => Aggregation::avg(&self.result, a),
            AggKind::Min(a) => Aggregation::min(&self.result, a, ty(a)),
            AggKind::Max(a) => Aggregation::max(&self.result, a, ty(a)),
        }
    }

    /// The result type, or `None` if the aggregate does not apply to the
    /// attribute's type.
    fn result_type(&self, heading: &Heading) -> Option<ScalarType> {
        match &self.kind {
            AggKind::Count => Some(ScalarType::Int),
            AggKind::Sum(a) => (heading[a] == ScalarType::Int).then_some(ScalarType::Int),
            AggKind::SumFloat(a) => (heading[a] == ScalarType::Float).then_some(ScalarType::Float),
            AggKind::Avg(a) => matches!(heading[a], ScalarType::Int | ScalarType::Float)
                .then_some(ScalarType::Float),
            AggKind::Min(a) | AggKind::Max(a) => Some(heading[a].clone()),
        }
    }

    /// The aggregate over a non-empty group, or `None` on overflow.
    fn compute(&self, group: &[&MTuple]) -> Option<ScalarValue> {
        let ints = |a: &str| -> Vec<i64> {
            group
                .iter()
                .map(|t| match &t[a] {
                    ScalarValue::Int(i) => *i,
                    other => panic!("not an Int: {other:?}"),
                })
                .collect()
        };
        let floats = |a: &str| -> Vec<f64> {
            group
                .iter()
                .map(|t| match &t[a] {
                    ScalarValue::Float(f) => *f,
                    other => panic!("not a Float: {other:?}"),
                })
                .collect()
        };
        let finite = |x: f64| x.is_finite().then_some(x);
        let len = group.len() as f64;
        match &self.kind {
            AggKind::Count => Some(ScalarValue::Int(group.len() as i64)),
            AggKind::Sum(a) => ints(a)
                .into_iter()
                .try_fold(0i64, i64::checked_add)
                .map(ScalarValue::Int),
            AggKind::SumFloat(a) => finite(floats(a).iter().sum()).map(ScalarValue::Float),
            AggKind::Avg(a) => match &group[0][a] {
                ScalarValue::Int(_) => {
                    let sum: i128 = ints(a).into_iter().map(i128::from).sum();
                    Some(ScalarValue::Float(sum as f64 / len))
                }
                _ => finite(floats(a).iter().sum()).map(|s| ScalarValue::Float(s / len)),
            },
            AggKind::Min(a) => group.iter().map(|t| t[a].clone()).min(),
            AggKind::Max(a) => group.iter().map(|t| t[a].clone()).max(),
        }
    }
}

/// `SUMMARIZE r PER (r { by }) : { aggs }`: one tuple per tuple of
/// `r { by }`, each aggregate taken over the tuples of `r` in that group.
fn m_summarize(r: &Model, by: &[&str], aggs: &[Agg]) -> Option<Model> {
    let keys = m_project(r, by)?;
    let mut heading = keys.heading.clone();
    for agg in aggs {
        if heading.contains_key(&agg.result) {
            return None;
        }
        heading.insert(agg.result.clone(), agg.result_type(&r.heading)?);
    }
    let by = name_set(by);
    let mut out = Model::empty(heading);
    for key in &keys.body {
        let group: Vec<&MTuple> = r
            .body
            .iter()
            .filter(|t| &restrict_tuple(t, &by) == key)
            .collect();
        let mut u = key.clone();
        for agg in aggs {
            u.insert(agg.result.clone(), agg.compute(&group)?);
        }
        out.insert(u);
    }
    Some(out)
}

/// `r GROUP { names } AS rva`; undefined if a name is unknown or `rva` is
/// one of the remaining attributes.
fn m_group(r: &Model, names: &[&str], rva: &str) -> Option<Model> {
    if !names.iter().all(|n| r.heading.contains_key(*n)) {
        return None;
    }
    let names = name_set(names);
    let mut heading: Heading = r
        .heading
        .iter()
        .filter(|(n, _)| !names.contains(*n))
        .map(|(n, ty)| (n.clone(), ty.clone()))
        .collect();
    if heading.contains_key(rva) {
        return None;
    }
    let inner = restrict_heading(&r.heading, &names);
    heading.insert(
        rva.to_string(),
        ScalarType::Relation(Box::new(RelationType::new(tuple_type_of(&inner)))),
    );
    let mut out = Model::empty(heading);
    for t in &r.body {
        let key = remove_keys(t, &names);
        let mut nest = Model::empty(inner.clone());
        for u in r.body.iter().filter(|u| remove_keys(u, &names) == key) {
            nest.insert(restrict_tuple(u, &names));
        }
        let mut g = key;
        g.insert(rva.to_string(), ScalarValue::Relation(nest.to_relation()));
        out.insert(g);
    }
    Some(out)
}

/// `r UNGROUP rva`; undefined if `rva` is not relation-valued or an inner
/// attribute name is also an outer one.
fn m_ungroup(r: &Model, rva: &str) -> Option<Model> {
    let ScalarType::Relation(inner) = r.heading.get(rva)? else {
        return None;
    };
    let mut heading = r.heading.clone();
    heading.remove(rva);
    for (n, ty) in inner.heading().attributes() {
        if heading.insert(n.clone(), ty.clone()).is_some() {
            return None;
        }
    }
    let mut out = Model::empty(heading);
    for t in &r.body {
        let ScalarValue::Relation(nested) = &t[rva] else {
            return None;
        };
        let mut outer = t.clone();
        outer.remove(rva);
        for v in nested.tuples() {
            out.insert(union_tuples(&outer, v.values()));
        }
    }
    Some(out)
}

/// `r1 DIVIDEBY r2`, Date's Small Divide:
/// `r1{A} MINUS ((r1{A} JOIN r2) MINUS r1){A}` with `A` the attributes of
/// `r1` not in `r2`; undefined unless `r2`'s heading is part of `r1`'s.
fn m_divide(r1: &Model, r2: &Model) -> Option<Model> {
    if !r2
        .heading
        .iter()
        .all(|(n, ty)| r1.heading.get(n) == Some(ty))
    {
        return None;
    }
    let a: Vec<&str> = r1
        .heading
        .keys()
        .filter(|n| !r2.heading.contains_key(*n))
        .map(String::as_str)
        .collect();
    let r1_a = m_project(r1, &a)?;
    let missing = m_project(&m_minus(&m_join(&r1_a, r2)?, r1)?, &a)?;
    m_minus(&r1_a, &missing)
}

/// `TCLOSE r` over `{from, to}`: every pair joined by a path of edges;
/// undefined unless `r` is binary over two distinct attributes of one type.
fn m_tclose(r: &Model, from: &str, to: &str) -> Option<Model> {
    if from == to
        || r.heading.len() != 2
        || !r.heading.contains_key(from)
        || r.heading.get(from) != r.heading.get(to)
    {
        return None;
    }
    let mut edges: Vec<(ScalarValue, ScalarValue)> = r
        .body
        .iter()
        .map(|t| (t[from].clone(), t[to].clone()))
        .collect();
    loop {
        let mut grown = false;
        for (x, y) in edges.clone() {
            for (y2, z) in edges.clone() {
                if y == y2 && !edges.contains(&(x.clone(), z.clone())) {
                    edges.push((x.clone(), z));
                    grown = true;
                }
            }
        }
        if !grown {
            break;
        }
    }
    let mut out = Model::empty(r.heading.clone());
    for (x, y) in edges {
        out.insert([(from.to_string(), x), (to.to_string(), y)].into());
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------

/// Every tuple's type is the relation's heading and every tuple conforms to
/// it, recursively through relation-valued attributes.
fn check_wf(r: &Relation) -> Result<(), TestCaseError> {
    let heading = r.relation_type().heading();
    for t in r.tuples() {
        prop_assert_eq!(t.tuple_type(), heading);
        prop_assert!(t.conforms_to(heading), "{:?} does not conform", t);
        for (n, v) in t.values() {
            if let ScalarValue::Relation(inner) = v {
                let declared = heading.get_attribute_type(n);
                prop_assert_eq!(
                    declared,
                    Some(&ScalarType::Relation(Box::new(
                        inner.relation_type().clone()
                    )))
                );
                check_wf(inner)?;
            }
        }
    }
    Ok(())
}

fn check_refines(r: &Relation, want: &Model) -> Result<(), TestCaseError> {
    check_wf(r)?;
    let got = Model::of(r);
    prop_assert_eq!(got.body.len(), r.cardinality());
    prop_assert!(
        got.same(want),
        "implementation: {:#?}\nmodel: {:#?}",
        got,
        want
    );
    Ok(())
}

/// The result is `Ok` exactly when the model defines it, and then equals it.
fn check<E: Debug>(got: Result<Relation, E>, want: Option<Model>) -> Result<(), TestCaseError> {
    match (got, want) {
        (Ok(r), Some(m)) => check_refines(&r, &m),
        (Err(_), None) => Ok(()),
        (Ok(r), None) => Err(TestCaseError::fail(format!(
            "implementation accepted an undefined operation: {r:?}"
        ))),
        (Err(e), Some(m)) => Err(TestCaseError::fail(format!(
            "implementation rejected ({e:?}) a defined operation: {m:?}"
        ))),
    }
}

fn dee() -> Relation {
    let mut r = Relation::new(RelationType::new(TupleType::new()));
    r.insert(Tuple::new(TupleType::new(), Vec::new()).unwrap())
        .unwrap();
    r
}

fn dum() -> Relation {
    Relation::new(RelationType::new(TupleType::new()))
}

fn empty_like(r: &Relation) -> Relation {
    Relation::new(r.relation_type().clone())
}

/// A RESTRICT predicate: `attr = value`, possibly negated.
#[derive(Clone, Debug)]
struct Pred {
    attr: String,
    value: ScalarValue,
    negate: bool,
}

impl Pred {
    fn holds(&self, t: &MTuple) -> bool {
        (t.get(&self.attr) == Some(&self.value)) != self.negate
    }
}

/// The EXTEND functions: `(declared type, f)`. Function 2 is declared
/// `Float` but produces a `String`, so it is ill-typed on a non-empty
/// relation.
fn ext_type(k: u8) -> ScalarType {
    match k {
        0 => ScalarType::Int,
        1 => ScalarType::Bool,
        _ => ScalarType::Float,
    }
}

fn ext_fn(k: u8, t: &MTuple) -> ScalarValue {
    match k {
        0 => ScalarValue::Int(
            t.values()
                .filter(|v| **v == ScalarValue::Bool(true))
                .count() as i64,
        ),
        1 => ScalarValue::Bool(t.get("a") == t.get("b")),
        _ => ScalarValue::String(format!("{}", t.len())),
    }
}

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

fn scalar_type() -> impl Strategy<Value = ScalarType> {
    prop_oneof![
        Just(ScalarType::Int),
        Just(ScalarType::Bool),
        Just(ScalarType::String),
        Just(ScalarType::Float),
    ]
}

/// Values of a type, from small domains so that operands share values.
/// Floats include `0.0` and `-0.0` (distinct values) and NaNs with three
/// different bit patterns (all one value).
fn value_of(ty: &ScalarType) -> BoxedStrategy<ScalarValue> {
    match ty {
        ScalarType::Int => prop_oneof![
            8 => (-2i64..3).prop_map(ScalarValue::Int),
            1 => Just(ScalarValue::Int(i64::MAX)),
            1 => Just(ScalarValue::Int(i64::MIN)),
        ]
        .boxed(),
        ScalarType::Bool => any::<bool>().prop_map(ScalarValue::Bool).boxed(),
        ScalarType::String => prop::sample::select(vec!["", "p", "q", "é"])
            .prop_map(|s| ScalarValue::String(s.to_string()))
            .boxed(),
        ScalarType::Float => prop::sample::select(vec![
            0.0,
            -0.0,
            1.5,
            -1.5,
            f64::INFINITY,
            f64::NAN,
            f64::from_bits(0x7ff8_0000_0000_0001),
            f64::from_bits(0xfff8_0000_0000_0000),
        ])
        .prop_map(ScalarValue::Float)
        .boxed(),
        other => panic!("no generator for {other:?}"),
    }
}

fn any_value() -> BoxedStrategy<ScalarValue> {
    scalar_type().prop_flat_map(|ty| value_of(&ty)).boxed()
}

/// A type for every name in `NAMES`; headings drawn from one universe are
/// join-compatible.
fn universe() -> impl Strategy<Value = Heading> {
    prop::collection::vec(scalar_type(), NAMES.len())
        .prop_map(|tys| NAMES.iter().map(|n| (*n).to_string()).zip(tys).collect())
}

fn sub_heading(u: &Heading) -> impl Strategy<Value = Heading> + use<> {
    let u = u.clone();
    prop::collection::vec(any::<bool>(), u.len()).prop_map(move |mask| {
        u.iter()
            .zip(mask)
            .filter(|(_, keep)| *keep)
            .map(|((n, ty), _)| (n.clone(), ty.clone()))
            .collect()
    })
}

fn heading() -> impl Strategy<Value = Heading> {
    universe().prop_flat_map(|u| sub_heading(&u))
}

/// 0..8 tuples of the heading, all sharing one `Arc<TupleType>`.
fn relation_of(h: &Heading) -> BoxedStrategy<Relation> {
    let tt = Arc::new(tuple_type_of(h));
    let names: Vec<String> = h.keys().cloned().collect();
    let tuple_values: Vec<BoxedStrategy<ScalarValue>> = h.values().map(value_of).collect();
    prop::collection::vec(tuple_values, 0..8)
        .prop_map(move |body| {
            let mut r = Relation::new(RelationType::new((*tt).clone()));
            for values in body {
                let t = Tuple::new(tt.clone(), names.iter().cloned().zip(values)).unwrap();
                r.insert(t).unwrap();
            }
            r
        })
        .boxed()
}

fn relation() -> impl Strategy<Value = Relation> {
    heading().prop_flat_map(|h| relation_of(&h))
}

/// Two relations of one heading.
fn same_pair() -> impl Strategy<Value = (Relation, Relation)> {
    heading().prop_flat_map(|h| (relation_of(&h), relation_of(&h)))
}

/// Three relations of one heading.
fn same_triple() -> impl Strategy<Value = (Relation, Relation, Relation)> {
    heading().prop_flat_map(|h| (relation_of(&h), relation_of(&h), relation_of(&h)))
}

/// Two relations whose common attributes have the same types.
fn compatible_pair() -> impl Strategy<Value = (Relation, Relation)> {
    universe()
        .prop_flat_map(|u| (sub_heading(&u), sub_heading(&u)))
        .prop_flat_map(|(h1, h2)| (relation_of(&h1), relation_of(&h2)))
}

/// Two relations with independent headings (common attributes may differ
/// in type, and the headings may differ).
fn any_pair() -> impl Strategy<Value = (Relation, Relation)> {
    (relation(), relation())
}

/// A relation and a sub-list of its attribute names (in random order).
fn relation_and_names() -> impl Strategy<Value = (Relation, Vec<String>)> {
    heading().prop_flat_map(|h| {
        let keys: Vec<String> = h.keys().cloned().collect();
        let n = keys.len();
        (
            relation_of(&h),
            prop::sample::subsequence(keys, 0..=n).prop_shuffle(),
        )
    })
}

fn name() -> impl Strategy<Value = String> {
    prop::sample::select(ANY_NAMES.to_vec()).prop_map(str::to_string)
}

fn aggs(h: &Heading) -> impl Strategy<Value = Vec<Agg>> + use<> {
    let keys: Vec<String> = h.keys().cloned().collect();
    prop::collection::vec((0u8..6, any::<prop::sample::Index>(), name()), 0..3).prop_map(
        move |specs| {
            specs
                .into_iter()
                .map(|(k, idx, result)| {
                    let kind = if keys.is_empty() {
                        AggKind::Count
                    } else {
                        let a = idx.get(&keys).clone();
                        match k {
                            0 => AggKind::Count,
                            1 => AggKind::Sum(a),
                            2 => AggKind::SumFloat(a),
                            3 => AggKind::Avg(a),
                            4 => AggKind::Min(a),
                            _ => AggKind::Max(a),
                        }
                    };
                    Agg { result, kind }
                })
                .collect()
        },
    )
}

fn strs(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}

fn heading_of(r: &Relation) -> Heading {
    r.relation_type().heading().attributes().clone()
}

// ---------------------------------------------------------------------------
// Refinement: every operator equals the model
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn restrict_refines_model(
        r in relation(),
        attr in prop::sample::select(NAMES.to_vec()),
        value in any_value(),
        negate in any::<bool>(),
    ) {
        let p = Pred { attr: attr.to_string(), value, negate };
        let want = m_restrict(&Model::of(&r), |t| p.holds(t));
        check_refines(&r.restrict(|t| p.holds(t.values())), &want)?;
        check_refines(&r.clone().restrict_into(|t| p.holds(t.values())), &want)?;
    }

    #[test]
    fn project_refines_model((r, names) in relation_and_names(), dup in any::<bool>()) {
        let mut names = strs(&names);
        if dup && !names.is_empty() {
            names.push(names[0]);
        }
        let want = m_project(&Model::of(&r), &names);
        check(r.project(&names), want.clone())?;
        check(r.clone().project_into(&names), want)?;
    }

    #[test]
    fn project_on_arbitrary_names_refines_model(
        r in relation(),
        names in prop::collection::vec(prop::sample::select(ANY_NAMES.to_vec()), 0..4),
    ) {
        check(r.project(&names), m_project(&Model::of(&r), &names))?;
    }

    #[test]
    fn rename_refines_model(
        r in relation(),
        pairs in prop::collection::vec(
            (prop::sample::select(NAMES.to_vec()), prop::sample::select(ANY_NAMES.to_vec())),
            0..4,
        ),
    ) {
        let want = m_rename(&Model::of(&r), &pairs);
        check(r.rename(&pairs), want.clone())?;
        check(r.clone().rename_into(&pairs), want)?;
    }

    #[test]
    fn union_refines_model((r1, r2) in any_pair()) {
        let (m1, m2) = (Model::of(&r1), Model::of(&r2));
        check(r1.union(&r2), m_union(&m1, &m2))?;
        check(r1.clone().union_into(&r2), m_union(&m1, &m2))?;
    }

    #[test]
    fn union_of_same_heading_refines_model((r1, r2) in same_pair()) {
        let (m1, m2) = (Model::of(&r1), Model::of(&r2));
        check(r1.union(&r2), m_union(&m1, &m2))?;
        check(r1.clone().union_into(&r2), m_union(&m1, &m2))?;
    }

    #[test]
    fn intersect_refines_model((r1, r2) in same_pair(), (s1, s2) in any_pair()) {
        let (m1, m2) = (Model::of(&r1), Model::of(&r2));
        check(r1.intersect(&r2), m_intersect(&m1, &m2))?;
        check(r1.clone().intersect_into(&r2), m_intersect(&m1, &m2))?;
        check(s1.intersect(&s2), m_intersect(&Model::of(&s1), &Model::of(&s2)))?;
    }

    #[test]
    fn difference_refines_model((r1, r2) in same_pair(), (s1, s2) in any_pair()) {
        let (m1, m2) = (Model::of(&r1), Model::of(&r2));
        check(r1.difference(&r2), m_minus(&m1, &m2))?;
        check(r1.minus(&r2), m_minus(&m1, &m2))?;
        check(r1.clone().difference_into(&r2), m_minus(&m1, &m2))?;
        check(r1.clone().minus_into(&r2), m_minus(&m1, &m2))?;
        check(s1.difference(&s2), m_minus(&Model::of(&s1), &Model::of(&s2)))?;
    }

    #[test]
    fn join_refines_model((r1, r2) in compatible_pair()) {
        check(r1.join(&r2), m_join(&Model::of(&r1), &Model::of(&r2)))?;
    }

    #[test]
    fn join_of_independent_headings_refines_model((r1, r2) in any_pair()) {
        check(r1.join(&r2), m_join(&Model::of(&r1), &Model::of(&r2)))?;
    }

    #[test]
    fn semijoin_and_semidifference_refine_model(
        (r1, r2) in compatible_pair(),
        (s1, s2) in any_pair(),
    ) {
        for (a, b) in [(&r1, &r2), (&s1, &s2)] {
            let (m1, m2) = (Model::of(a), Model::of(b));
            let matching = m_semi(&m1, &m2, true);
            let not_matching = m_semi(&m1, &m2, false);
            check_refines(&a.semijoin(b), &matching)?;
            check_refines(&a.matching(b), &matching)?;
            check_refines(&a.clone().semijoin_into(b), &matching)?;
            check_refines(&a.semidifference(b), &not_matching)?;
            check_refines(&a.not_matching(b), &not_matching)?;
            check_refines(&a.clone().semidifference_into(b), &not_matching)?;
        }
    }

    #[test]
    fn extend_refines_model(r in relation(), attr in name(), k in 0u8..3) {
        let ty = ext_type(k);
        let want = m_extend(&Model::of(&r), &attr, &ty, |t| ext_fn(k, t));
        check(r.extend(&attr, ty.clone(), |t| ext_fn(k, t.values())), want.clone())?;
        check(r.clone().extend_into(&attr, ty, |t| ext_fn(k, t.values())), want)?;
    }

    #[test]
    fn summarize_refines_model(
        (r, by, aggs) in relation_and_names().prop_flat_map(|(r, by)| {
            let h = heading_of(&r);
            (Just(r), Just(by), aggs(&h))
        }),
    ) {
        let by = strs(&by);
        let h = heading_of(&r);
        let real: Vec<Aggregation> = aggs.iter().map(|a| a.to_aggregation(&h)).collect();
        check(r.summarize(&by, &real), m_summarize(&Model::of(&r), &by, &aggs))?;
    }

    #[test]
    fn group_refines_model((r, names) in relation_and_names(), rva in name()) {
        let names = strs(&names);
        check(r.group(&names, &rva), m_group(&Model::of(&r), &names, &rva))?;
    }

    #[test]
    fn ungroup_refines_model((r, names) in relation_and_names(), rva in name()) {
        let names = strs(&names);
        if let Ok(g) = r.group(&names, &rva) {
            check(g.ungroup(&rva), m_ungroup(&Model::of(&g), &rva))?;
        }
        // Ungrouping a non-relation-valued or missing attribute is undefined.
        check(r.ungroup(&rva), m_ungroup(&Model::of(&r), &rva))?;
    }

    #[test]
    fn divide_refines_model(
        (r1, r2) in heading().prop_flat_map(|h| {
            let h2 = sub_heading(&h);
            (relation_of(&h), h2.prop_flat_map(|h2| relation_of(&h2)))
        }),
    ) {
        check(r1.divide(&r2), m_divide(&Model::of(&r1), &Model::of(&r2)))?;
    }

    #[test]
    fn divide_of_independent_headings_refines_model((r1, r2) in any_pair()) {
        check(r1.divide(&r2), m_divide(&Model::of(&r1), &Model::of(&r2)))?;
    }

    #[test]
    fn tclose_refines_model(
        r in scalar_type().prop_flat_map(|ty| {
            relation_of(&[("a".to_string(), ty.clone()), ("b".to_string(), ty)].into())
        }),
        reversed in any::<bool>(),
    ) {
        let (from, to) = if reversed { ("b", "a") } else { ("a", "b") };
        check(r.tclose(from, to), m_tclose(&Model::of(&r), from, to))?;
    }

    #[test]
    fn tclose_of_arbitrary_relations_refines_model(
        r in relation(),
        from in prop::sample::select(ANY_NAMES.to_vec()),
        to in prop::sample::select(ANY_NAMES.to_vec()),
    ) {
        check(r.tclose(from, to), m_tclose(&Model::of(&r), from, to))?;
    }
}

// ---------------------------------------------------------------------------
// Algebraic laws
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn union_and_intersect_are_commutative_associative_idempotent(
        (r1, r2, r3) in same_triple(),
    ) {
        prop_assert_eq!(r1.union(&r2).unwrap(), r2.union(&r1).unwrap());
        prop_assert_eq!(
            r1.union(&r2).unwrap().union(&r3).unwrap(),
            r1.union(&r2.union(&r3).unwrap()).unwrap()
        );
        prop_assert_eq!(&r1.union(&r1).unwrap(), &r1);
        prop_assert_eq!(r1.intersect(&r2).unwrap(), r2.intersect(&r1).unwrap());
        prop_assert_eq!(
            r1.intersect(&r2).unwrap().intersect(&r3).unwrap(),
            r1.intersect(&r2.intersect(&r3).unwrap()).unwrap()
        );
        prop_assert_eq!(&r1.intersect(&r1).unwrap(), &r1);
    }

    #[test]
    fn intersect_is_minus_of_minus((r1, r2) in same_pair()) {
        prop_assert_eq!(
            r1.intersect(&r2).unwrap(),
            r1.minus(&r1.minus(&r2).unwrap()).unwrap()
        );
        prop_assert_eq!(
            r1.minus(&r2).unwrap().union(&r1.intersect(&r2).unwrap()).unwrap(),
            r1
        );
    }

    #[test]
    fn join_is_commutative((r1, r2) in compatible_pair()) {
        prop_assert_eq!(r1.join(&r2).unwrap(), r2.join(&r1).unwrap());
    }

    #[test]
    fn join_with_dee_is_identity_and_with_dum_is_empty(r in relation()) {
        prop_assert_eq!(&r.join(&dee()).unwrap(), &r);
        prop_assert_eq!(&dee().join(&r).unwrap(), &r);
        prop_assert_eq!(r.join(&dum()).unwrap(), empty_like(&r));
        prop_assert_eq!(dum().join(&r).unwrap(), empty_like(&r));
    }

    #[test]
    fn project_onto_all_is_identity_and_onto_none_is_dee_or_dum(r in relation()) {
        let h = heading_of(&r);
        let all: Vec<&str> = h.keys().map(String::as_str).collect();
        prop_assert_eq!(&r.project(&all).unwrap(), &r);
        let none = r.project(&[]).unwrap();
        prop_assert_eq!(none, if r.is_empty() { dum() } else { dee() });
    }

    #[test]
    fn matching_and_not_matching_partition_r((r1, r2) in any_pair()) {
        let m = r1.semijoin(&r2);
        let n = r1.semidifference(&r2);
        prop_assert_eq!(&m.union(&n).unwrap(), &r1);
        prop_assert!(m.intersect(&n).unwrap().is_empty());
    }

    #[test]
    fn ungroup_undoes_group((r, names) in relation_and_names(), rva in name()) {
        let names = strs(&names);
        if let Ok(g) = r.group(&names, &rva) {
            prop_assert_eq!(&g.ungroup(&rva).unwrap(), &r);
        }
    }

    /// `ScalarValue`'s ordering agrees with its equality, including on
    /// relation values, so ordered collections of values are sets.
    #[test]
    fn value_ordering_is_consistent_with_equality(
        (r, names) in relation_and_names(),
        (s, more) in relation_and_names(),
    ) {
        let mut values: Vec<ScalarValue> = Vec::new();
        for (rel, names) in [(&r, &names), (&s, &more)] {
            values.push(ScalarValue::Relation(rel.clone()));
            for t in rel.tuples() {
                values.extend(t.values().values().cloned());
            }
            if let Ok(g) = rel.group(&strs(names), "x") {
                values.extend(g.tuples().map(|t| t.values()["x"].clone()));
            }
        }
        for x in &values {
            for y in &values {
                prop_assert_eq!(x.cmp(y) == std::cmp::Ordering::Equal, x == y);
                prop_assert_eq!(x.cmp(y), y.cmp(x).reverse());
            }
        }
    }

    #[test]
    fn rename_round_trips((r, sources) in relation_and_names()) {
        // Distinct fresh targets: always a valid renaming.
        let pairs: Vec<(&str, &str)> = sources
            .iter()
            .map(String::as_str)
            .zip(["x", "y", "z", "w"])
            .collect();
        let back: Vec<(&str, &str)> = pairs.iter().map(|(f, t)| (*t, *f)).collect();
        let renamed = r.rename(&pairs).unwrap();
        prop_assert_eq!(renamed.cardinality(), r.cardinality());
        prop_assert_eq!(&renamed.rename(&back).unwrap(), &r);
    }
}

// ---------------------------------------------------------------------------
// Error cases (TTM: these operations are undefined, so they must not
// produce a relation)
// ---------------------------------------------------------------------------

fn single(name: &str, ty: ScalarType) -> Relation {
    Relation::new(RelationType::new(TupleType::new().with_attribute(name, ty)))
}

#[test]
fn join_with_conflicting_attribute_types_is_rejected() {
    assert!(
        single("a", ScalarType::Int)
            .join(&single("a", ScalarType::Bool))
            .is_err()
    );
}

#[test]
fn project_on_unknown_attribute_is_rejected() {
    assert!(single("a", ScalarType::Int).project(&["b"]).is_err());
}

#[test]
fn rename_onto_existing_attribute_is_rejected() {
    let r = Relation::new(RelationType::new(
        TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int),
    ));
    assert!(r.rename(&[("a", "b")]).is_err());
}

#[test]
fn ungroup_with_colliding_inner_attribute_is_rejected() {
    // {a, b} GROUP {a} AS g, then b RENAME AS a: {a, g: {a}}.
    let mut r = Relation::new(RelationType::new(
        TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int),
    ));
    r.insert(
        Tuple::new(
            r.relation_type().heading().clone(),
            [
                ("a".to_string(), ScalarValue::Int(1)),
                ("b".to_string(), ScalarValue::Int(2)),
            ],
        )
        .unwrap(),
    )
    .unwrap();
    let g = r.group(&["a"], "g").unwrap().rename(&[("b", "a")]).unwrap();
    assert!(g.ungroup("g").is_err());
}
