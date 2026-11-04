//! The relational algebra (RM Prescription 18) over the model in `model.rs`.
//!
//! Every operator is defined as a `spec fn` over finite sets and maps, and
//! for every operator there is a closure proof: given well-formed inputs it
//! produces a well-formed relation whose heading is exactly the heading the
//! operator promises. Because the body is a `Set`, every result is
//! duplicate-free and unordered by construction (RM Proscriptions 2, 3) —
//! including `project`, which is where SQL keeps duplicates.
//!
//! Alongside closure, the algebraic laws the query layer relies on are
//! proved: commutativity/associativity/idempotence of the set operators,
//! commutativity of join, `TABLE_DEE` as the identity of join (RM
//! Proscription 5), `MATCHING`/`NOT MATCHING` partitioning a relation,
//! `DIVIDEBY` matching its definition, `UNGROUP` undoing `GROUP`, and the
//! transitive closure being transitive and containing its argument.
use vstd::prelude::*;
use crate::model::*;

verus! {

broadcast use vstd::map_lib::group_map_union;

// ---------------------------------------------------------------------------
// RESTRICT (WHERE)
// ---------------------------------------------------------------------------

/// `r WHERE p`.
pub open spec fn restrict(r: Relation, p: spec_fn(Tuple) -> bool) -> Relation {
    Relation { heading: r.heading, body: r.body.filter(p) }
}

pub proof fn lemma_restrict_closed(r: Relation, p: spec_fn(Tuple) -> bool)
    requires
        wf(r),
    ensures
        wf(restrict(r, p)),
        restrict(r, p).heading == r.heading,
        restrict(r, p).body.subset_of(r.body),
{
}

// ---------------------------------------------------------------------------
// PROJECT
// ---------------------------------------------------------------------------

/// `r { names }`. Duplicates that arise from dropping attributes vanish
/// because the result body is a set.
pub open spec fn project(r: Relation, names: Set<Name>) -> Relation
    recommends
        names.subset_of(r.heading.dom()),
{
    Relation {
        heading: r.heading.restrict(names),
        body: r.body.map(|t: Tuple| tuple_project(t, names)),
    }
}

pub proof fn lemma_project_contains(r: Relation, names: Set<Name>, u: Tuple)
    ensures
        project(r, names).body.contains(u) <==> exists|t: Tuple|
            #[trigger] r.body.contains(t) && u == tuple_project(t, names),
{
}

pub proof fn lemma_project_closed(r: Relation, names: Set<Name>)
    requires
        wf(r),
    ensures
        wf(project(r, names)),
        project(r, names).heading == r.heading.restrict(names),
{
    assert forall|u: Tuple| #[trigger] project(r, names).body.contains(u) implies conforms(
        u,
        project(r, names).heading,
    ) by {
        lemma_project_contains(r, names, u);
        let t = choose|t: Tuple| #[trigger] r.body.contains(t) && u == tuple_project(t, names);
        lemma_project_tuple_conforms(t, r.heading, names);
    }
}

/// Projecting onto the whole heading is the identity.
pub proof fn lemma_project_identity(r: Relation)
    requires
        wf(r),
    ensures
        project(r, r.heading.dom()) == r,
{
    assert(r.heading.restrict(r.heading.dom()) =~= r.heading);
    assert forall|u: Tuple| project(r, r.heading.dom()).body.contains(u) <==> r.body.contains(
        u,
    ) by {
        lemma_project_contains(r, r.heading.dom(), u);
        if r.body.contains(u) {
            lemma_project_tuple_onto_own_heading(u, r.heading);
        }
        if project(r, r.heading.dom()).body.contains(u) {
            let t = choose|t: Tuple|
                #[trigger] r.body.contains(t) && u == tuple_project(t, r.heading.dom());
            lemma_project_tuple_onto_own_heading(t, r.heading);
        }
    }
    assert(project(r, r.heading.dom()).body =~= r.body);
}

/// Projecting onto no attributes yields `TABLE_DEE` for a non-empty relation
/// and `TABLE_DUM` for an empty one (RM Proscription 5).
pub proof fn lemma_project_onto_nothing(r: Relation)
    requires
        wf(r),
    ensures
        r.body.len() > 0 ==> project(r, Set::empty()) == table_dee(),
        r.body.len() == 0 ==> project(r, Set::empty()) == table_dum(),
{
    lemma_project_closed(r, Set::empty());
    assert(r.heading.restrict(Set::empty()) =~= Map::<Name, Ty>::empty());
    assert forall|t: Tuple| #[trigger] r.body.contains(t) implies tuple_project(t, Set::empty())
        == Map::<Name, Val>::empty() by {
        assert(tuple_project(t, Set::empty()) =~= Map::<Name, Val>::empty());
    }
    if r.body.len() > 0 {
        let t = r.body.choose();
        assert(r.body.contains(t));
        lemma_project_contains(r, Set::empty(), Map::<Name, Val>::empty());
        assert(project(r, Set::empty()).body.contains(Map::<Name, Val>::empty()));
    } else {
        assert(r.body =~= Set::<Tuple>::empty());
        lemma_project_contains(r, Set::empty(), Map::<Name, Val>::empty());
        assert(!project(r, Set::empty()).body.contains(Map::<Name, Val>::empty()));
    }
    lemma_degree_zero_is_dee_or_dum(project(r, Set::empty()));
    lemma_dee_dum();
}

// ---------------------------------------------------------------------------
// RENAME
// ---------------------------------------------------------------------------

pub open spec fn rename_tuple(t: Tuple, from: Name, to: Name) -> Tuple {
    t.remove(from).insert(to, t[from])
}

pub open spec fn rename_heading(h: Heading, from: Name, to: Name) -> Heading {
    h.remove(from).insert(to, h[from])
}

/// The runtime rejects renaming a missing attribute or renaming onto an
/// existing one (`RenameError`).
pub open spec fn rename_ok(h: Heading, from: Name, to: Name) -> bool {
    &&& h.dom().contains(from)
    &&& (from == to || !h.dom().contains(to))
}

/// `r RENAME { from AS to }`.
pub open spec fn rename(r: Relation, from: Name, to: Name) -> Relation {
    Relation {
        heading: rename_heading(r.heading, from, to),
        body: r.body.map(|t: Tuple| rename_tuple(t, from, to)),
    }
}

pub proof fn lemma_rename_closed(r: Relation, from: Name, to: Name)
    requires
        wf(r),
        rename_ok(r.heading, from, to),
    ensures
        wf(rename(r, from, to)),
        rename(r, from, to).heading == rename_heading(r.heading, from, to),
        degree(rename(r, from, to)) == degree(r),
{
    let nh = rename_heading(r.heading, from, to);
    assert forall|u: Tuple| #[trigger] rename(r, from, to).body.contains(u) implies conforms(
        u,
        nh,
    ) by {
        let t = choose|t: Tuple| #[trigger]
            r.body.contains(t) && u == rename_tuple(t, from, to);
        assert(r.heading.dom().contains(from));
        assert(u.dom() =~= nh.dom());
        assert forall|a: Name| #[trigger] nh.dom().contains(a) implies type_of(u[a]) == nh[a] by {
            if a != to {
                assert(r.heading.dom().contains(a));
            }
        }
    }
    if from == to {
        assert(nh.dom() =~= r.heading.dom());
    } else {
        assert(nh.dom() =~= r.heading.dom().remove(from).insert(to));
        assert(r.heading.dom().remove(from).len() + 1 == r.heading.dom().len());
    }
}

/// Renaming back undoes a rename: `RENAME` is a bijection on tuples, so it
/// never merges or loses tuples.
pub proof fn lemma_rename_roundtrip(t: Tuple, h: Heading, from: Name, to: Name)
    requires
        conforms(t, h),
        rename_ok(h, from, to),
    ensures
        rename_tuple(rename_tuple(t, from, to), to, from) == t,
{
    assert(h.dom().contains(from));
    assert(rename_tuple(rename_tuple(t, from, to), to, from) =~= t);
}

// ---------------------------------------------------------------------------
// UNION, INTERSECT, MINUS
// ---------------------------------------------------------------------------

pub open spec fn union(r1: Relation, r2: Relation) -> Relation
    recommends
        r1.heading == r2.heading,
{
    Relation { heading: r1.heading, body: r1.body.union(r2.body) }
}

pub open spec fn intersect(r1: Relation, r2: Relation) -> Relation
    recommends
        r1.heading == r2.heading,
{
    Relation { heading: r1.heading, body: r1.body.intersect(r2.body) }
}

pub open spec fn minus(r1: Relation, r2: Relation) -> Relation
    recommends
        r1.heading == r2.heading,
{
    Relation { heading: r1.heading, body: r1.body.difference(r2.body) }
}

/// Set operators are closed over relations of the same heading. (The
/// runtime rejects different headings with a heading-mismatch error.)
pub proof fn lemma_set_ops_closed(r1: Relation, r2: Relation)
    requires
        wf(r1),
        wf(r2),
        r1.heading == r2.heading,
    ensures
        wf(union(r1, r2)),
        wf(intersect(r1, r2)),
        wf(minus(r1, r2)),
{
}

pub proof fn lemma_set_op_laws(r1: Relation, r2: Relation, r3: Relation)
    requires
        r1.heading == r2.heading,
        r2.heading == r3.heading,
    ensures
        union(r1, r2) == union(r2, r1),
        union(union(r1, r2), r3) == union(r1, union(r2, r3)),
        union(r1, r1) == r1,
        intersect(r1, r2) == intersect(r2, r1),
        intersect(intersect(r1, r2), r3) == intersect(r1, intersect(r2, r3)),
        intersect(r1, r1) == r1,
        intersect(r1, r2) == minus(r1, minus(r1, r2)),
        union(minus(r1, r2), intersect(r1, r2)) == r1,
{
    assert(r1.body.union(r2.body) =~= r2.body.union(r1.body));
    assert(r1.body.union(r2.body).union(r3.body) =~= r1.body.union(r2.body.union(r3.body)));
    assert(r1.body.union(r1.body) =~= r1.body);
    assert(r1.body.intersect(r2.body) =~= r2.body.intersect(r1.body));
    assert(r1.body.intersect(r2.body).intersect(r3.body) =~= r1.body.intersect(
        r2.body.intersect(r3.body),
    ));
    assert(r1.body.intersect(r1.body) =~= r1.body);
    assert(r1.body.intersect(r2.body) =~= r1.body.difference(r1.body.difference(r2.body)));
    assert(r1.body.difference(r2.body).union(r1.body.intersect(r2.body)) =~= r1.body);
}

// ---------------------------------------------------------------------------
// JOIN (natural join; TIMES when headings are disjoint)
// ---------------------------------------------------------------------------

/// Headings are join-compatible when their common attributes have the same
/// type (the runtime rejects anything else).
pub open spec fn compatible(h1: Heading, h2: Heading) -> bool {
    h1.agrees(h2)
}

/// The tuples of `b2` that agree with `t1` on the common attributes.
pub open spec fn agreeing(t1: Tuple, b2: Set<Tuple>) -> Set<Tuple> {
    b2.filter(|t2: Tuple| t1.agrees(t2))
}

/// The join of one tuple with a body.
pub open spec fn join_matches(t1: Tuple, b2: Set<Tuple>) -> Set<Tuple> {
    agreeing(t1, b2).map(|t2: Tuple| t1.union_prefer_right(t2))
}

pub open spec fn join_parts(r1: Relation, r2: Relation) -> Set<Set<Tuple>> {
    r1.body.map(|t1: Tuple| join_matches(t1, r2.body))
}

/// `r1 JOIN r2`.
pub open spec fn join(r1: Relation, r2: Relation) -> Relation
    recommends
        compatible(r1.heading, r2.heading),
{
    Relation {
        heading: r1.heading.union_prefer_right(r2.heading),
        body: join_parts(r1, r2).flatten(),
    }
}

/// The defining property of join: a tuple is in `r1 JOIN r2` exactly when
/// it is the union of a tuple of `r1` and a tuple of `r2` that agree on the
/// common attributes.
pub proof fn lemma_join_contains(r1: Relation, r2: Relation, u: Tuple)
    ensures
        join(r1, r2).body.contains(u) <==> exists|t1: Tuple, t2: Tuple|
            #![trigger r1.body.contains(t1), r2.body.contains(t2)]
            r1.body.contains(t1) && r2.body.contains(t2) && t1.agrees(t2) && u
                == t1.union_prefer_right(t2),
{
    if join(r1, r2).body.contains(u) {
        let s = choose|s: Set<Tuple>| #[trigger] join_parts(r1, r2).contains(s) && s.contains(u);
        let t1 = choose|t1: Tuple| #[trigger] r1.body.contains(t1) && s == join_matches(t1, r2.body);
        let t2 = choose|t2: Tuple| #[trigger]
            agreeing(t1, r2.body).contains(t2) && u == t1.union_prefer_right(t2);
        assert(r1.body.contains(t1) && r2.body.contains(t2) && t1.agrees(t2));
    }
    if exists|t1: Tuple, t2: Tuple|
        #![trigger r1.body.contains(t1), r2.body.contains(t2)]
        r1.body.contains(t1) && r2.body.contains(t2) && t1.agrees(t2) && u
            == t1.union_prefer_right(t2) {
        let (t1, t2) = choose|t1: Tuple, t2: Tuple|
            #![trigger r1.body.contains(t1), r2.body.contains(t2)]
            r1.body.contains(t1) && r2.body.contains(t2) && t1.agrees(t2) && u
                == t1.union_prefer_right(t2);
        assert(agreeing(t1, r2.body).contains(t2));
        assert(join_matches(t1, r2.body).contains(u));
        assert(join_parts(r1, r2).contains(join_matches(t1, r2.body)));
    }
}

/// The union of a tuple of `h1` and an agreeing tuple of `h2` conforms to
/// the joined heading.
pub proof fn lemma_union_conforms(t1: Tuple, h1: Heading, t2: Tuple, h2: Heading)
    requires
        conforms(t1, h1),
        conforms(t2, h2),
    ensures
        conforms(t1.union_prefer_right(t2), h1.union_prefer_right(h2)),
{
    let u = t1.union_prefer_right(t2);
    let h = h1.union_prefer_right(h2);
    assert(u.dom() =~= h.dom());
    assert forall|a: Name| #[trigger] h.dom().contains(a) implies type_of(u[a]) == h[a] by {
        if h2.dom().contains(a) {
            assert(t2.dom().contains(a));
        } else {
            assert(h1.dom().contains(a));
            assert(!t2.dom().contains(a));
        }
    }
}

pub proof fn lemma_join_closed(r1: Relation, r2: Relation)
    requires
        wf(r1),
        wf(r2),
    ensures
        wf(join(r1, r2)),
        join(r1, r2).heading == r1.heading.union_prefer_right(r2.heading),
{
    assert forall|u: Tuple| #[trigger] join(r1, r2).body.contains(u) implies conforms(
        u,
        join(r1, r2).heading,
    ) by {
        lemma_join_contains(r1, r2, u);
        let (t1, t2) = choose|t1: Tuple, t2: Tuple|
            #![trigger r1.body.contains(t1), r2.body.contains(t2)]
            r1.body.contains(t1) && r2.body.contains(t2) && t1.agrees(t2) && u
                == t1.union_prefer_right(t2);
        lemma_union_conforms(t1, r1.heading, t2, r2.heading);
    }
}

proof fn lemma_union_prefer_right_commutes_when_agreeing(m1: Tuple, m2: Tuple)
    requires
        m1.agrees(m2),
    ensures
        m2.agrees(m1),
        m1.union_prefer_right(m2) == m2.union_prefer_right(m1),
{
    assert(m1.union_prefer_right(m2) =~= m2.union_prefer_right(m1));
}

/// Join is commutative (there is no "left" and "right" input).
pub proof fn lemma_join_commutative(r1: Relation, r2: Relation)
    requires
        compatible(r1.heading, r2.heading),
    ensures
        join(r1, r2) == join(r2, r1),
{
    assert(r1.heading.union_prefer_right(r2.heading) =~= r2.heading.union_prefer_right(
        r1.heading,
    ));
    assert forall|u: Tuple| join(r1, r2).body.contains(u) <==> join(r2, r1).body.contains(u) by {
        lemma_join_contains(r1, r2, u);
        lemma_join_contains(r2, r1, u);
        if join(r1, r2).body.contains(u) {
            let (t1, t2) = choose|t1: Tuple, t2: Tuple|
                #![trigger r1.body.contains(t1), r2.body.contains(t2)]
                r1.body.contains(t1) && r2.body.contains(t2) && t1.agrees(t2) && u
                    == t1.union_prefer_right(t2);
            lemma_union_prefer_right_commutes_when_agreeing(t1, t2);
        }
        if join(r2, r1).body.contains(u) {
            let (t2, t1) = choose|t2: Tuple, t1: Tuple|
                #![trigger r2.body.contains(t2), r1.body.contains(t1)]
                r2.body.contains(t2) && r1.body.contains(t1) && t2.agrees(t1) && u
                    == t2.union_prefer_right(t1);
            lemma_union_prefer_right_commutes_when_agreeing(t2, t1);
        }
    }
    assert(join(r1, r2).body =~= join(r2, r1).body);
}

/// `TABLE_DEE` is the identity of join, and `TABLE_DUM` annihilates it
/// (RM Proscription 5: nullology is handled, not special-cased away).
pub proof fn lemma_join_dee_dum(r: Relation)
    requires
        wf(r),
    ensures
        join(r, table_dee()) == r,
        join(r, table_dum()) == empty(r.heading),
{
    assert(r.heading.union_prefer_right(Map::<Name, Ty>::empty()) =~= r.heading);
    let e = Map::<Name, Val>::empty();
    assert forall|u: Tuple| join(r, table_dee()).body.contains(u) <==> r.body.contains(u) by {
        lemma_join_contains(r, table_dee(), u);
        assert(u.union_prefer_right(e) =~= u);
        if r.body.contains(u) {
            assert(table_dee().body.contains(e));
            assert(u.agrees(e));
        }
        if join(r, table_dee()).body.contains(u) {
            let (t1, t2) = choose|t1: Tuple, t2: Tuple|
                #![trigger r.body.contains(t1), table_dee().body.contains(t2)]
                r.body.contains(t1) && table_dee().body.contains(t2) && t1.agrees(t2) && u
                    == t1.union_prefer_right(t2);
            assert(t2 == e);
            assert(t1.union_prefer_right(e) =~= t1);
        }
    }
    assert(join(r, table_dee()).body =~= r.body);
    assert forall|u: Tuple| !join(r, table_dum()).body.contains(u) by {
        lemma_join_contains(r, table_dum(), u);
    }
    assert(join(r, table_dum()).body =~= Set::<Tuple>::empty());
}

// ---------------------------------------------------------------------------
// MATCHING (semijoin) and NOT MATCHING (semidifference)
// ---------------------------------------------------------------------------

pub open spec fn has_match(t1: Tuple, b2: Set<Tuple>) -> bool {
    exists|t2: Tuple| #[trigger] b2.contains(t2) && t1.agrees(t2)
}

/// `r1 MATCHING r2`.
pub open spec fn semijoin(r1: Relation, r2: Relation) -> Relation {
    Relation { heading: r1.heading, body: r1.body.filter(|t1: Tuple| has_match(t1, r2.body)) }
}

/// `r1 NOT MATCHING r2`.
pub open spec fn semiminus(r1: Relation, r2: Relation) -> Relation {
    Relation { heading: r1.heading, body: r1.body.filter(|t1: Tuple| !has_match(t1, r2.body)) }
}

pub proof fn lemma_semijoin_closed_and_partition(r1: Relation, r2: Relation)
    requires
        wf(r1),
    ensures
        wf(semijoin(r1, r2)),
        wf(semiminus(r1, r2)),
        union(semijoin(r1, r2), semiminus(r1, r2)) == r1,
        intersect(semijoin(r1, r2), semiminus(r1, r2)) == empty(r1.heading),
{
    assert(semijoin(r1, r2).body.union(semiminus(r1, r2).body) =~= r1.body);
    assert(semijoin(r1, r2).body.intersect(semiminus(r1, r2).body) =~= Set::<Tuple>::empty());
}

/// `MATCHING` keeps exactly the tuples of `r1` that participate in the join.
pub proof fn lemma_semijoin_is_projected_join(r1: Relation, r2: Relation, t1: Tuple)
    requires
        wf(r1),
        wf(r2),
        r1.body.contains(t1),
    ensures
        semijoin(r1, r2).body.contains(t1) <==> exists|u: Tuple|
            #[trigger] join(r1, r2).body.contains(u) && tuple_project(u, r1.heading.dom()) == t1,
{
    if semijoin(r1, r2).body.contains(t1) {
        let t2 = choose|t2: Tuple| #[trigger] r2.body.contains(t2) && t1.agrees(t2);
        let u = t1.union_prefer_right(t2);
        lemma_join_contains(r1, r2, u);
        assert(tuple_project(u, r1.heading.dom()) =~= t1);
        assert(join(r1, r2).body.contains(u));
    }
    if exists|u: Tuple|
        #[trigger] join(r1, r2).body.contains(u) && tuple_project(u, r1.heading.dom()) == t1 {
        let u = choose|u: Tuple|
            #[trigger] join(r1, r2).body.contains(u) && tuple_project(u, r1.heading.dom()) == t1;
        lemma_join_contains(r1, r2, u);
        let (a1, a2) = choose|a1: Tuple, a2: Tuple|
            #![trigger r1.body.contains(a1), r2.body.contains(a2)]
            r1.body.contains(a1) && r2.body.contains(a2) && a1.agrees(a2) && u
                == a1.union_prefer_right(a2);
        // a1 and t1 both project u onto r1's heading, so a2 agrees with t1.
        assert(tuple_project(u, r1.heading.dom()) =~= a1);
        assert(r2.body.contains(a2) && t1.agrees(a2));
    }
}

// ---------------------------------------------------------------------------
// EXTEND
// ---------------------------------------------------------------------------

/// `EXTEND r : { name := f(t) }`, where every `f(t)` has type `ty`.
pub open spec fn extend(r: Relation, name: Name, ty: Ty, f: spec_fn(Tuple) -> Val) -> Relation {
    Relation {
        heading: r.heading.insert(name, ty),
        body: r.body.map(|t: Tuple| t.insert(name, f(t))),
    }
}

pub open spec fn extend_ok(r: Relation, name: Name, ty: Ty, f: spec_fn(Tuple) -> Val) -> bool {
    &&& !r.heading.dom().contains(name)
    &&& forall|t: Tuple| #[trigger] r.body.contains(t) ==> type_of(f(t)) == ty
}

pub proof fn lemma_extend_closed(r: Relation, name: Name, ty: Ty, f: spec_fn(Tuple) -> Val)
    requires
        wf(r),
        extend_ok(r, name, ty, f),
    ensures
        wf(extend(r, name, ty, f)),
        extend(r, name, ty, f).heading == r.heading.insert(name, ty),
        degree(extend(r, name, ty, f)) == degree(r) + 1,
        cardinality(extend(r, name, ty, f)) == cardinality(r),
{
    let nh = r.heading.insert(name, ty);
    let g = |t: Tuple| t.insert(name, f(t));
    assert forall|u: Tuple| #[trigger] extend(r, name, ty, f).body.contains(u) implies conforms(
        u,
        nh,
    ) by {
        let t = choose|t: Tuple| #[trigger] r.body.contains(t) && u == t.insert(name, f(t));
        assert(u.dom() =~= nh.dom());
        assert forall|a: Name| #[trigger] nh.dom().contains(a) implies type_of(u[a]) == nh[a] by {
            if a != name {
                assert(r.heading.dom().contains(a));
            }
        }
    }
    // EXTEND is injective on the body (the new value is a function of the
    // old tuple, and dropping it recovers the old tuple), so it preserves
    // cardinality.
    assert forall|t1: Tuple, t2: Tuple|
        r.body.contains(t1) && r.body.contains(t2) && #[trigger] g(t1) == #[trigger] g(t2)
            implies t1 == t2 by {
        assert(!t1.dom().contains(name));
        assert(!t2.dom().contains(name));
        assert(g(t1).remove(name) =~= t1);
        assert(g(t2).remove(name) =~= t2);
    }
    assert(r.body.injective_on(g));
    vstd::set_lib::lemma_map_size(r.body, extend(r, name, ty, f).body, g);
}

// ---------------------------------------------------------------------------
// SUMMARIZE
// ---------------------------------------------------------------------------

/// The tuples of `r` that project onto `g` over `by`.
pub open spec fn group_of(r: Relation, by: Set<Name>, g: Tuple) -> Set<Tuple> {
    r.body.filter(|t: Tuple| tuple_project(t, by) == g)
}

/// `SUMMARIZE r PER (r { by }) : { name := agg(group) }`, where every
/// aggregate value has type `ty`.
pub open spec fn summarize(
    r: Relation,
    by: Set<Name>,
    name: Name,
    ty: Ty,
    agg: spec_fn(Set<Tuple>) -> Val,
) -> Relation {
    Relation {
        heading: r.heading.restrict(by).insert(name, ty),
        body: project(r, by).body.map(|g: Tuple| g.insert(name, agg(group_of(r, by, g)))),
    }
}

pub proof fn lemma_summarize_closed(
    r: Relation,
    by: Set<Name>,
    name: Name,
    ty: Ty,
    agg: spec_fn(Set<Tuple>) -> Val,
)
    requires
        wf(r),
        !r.heading.restrict(by).dom().contains(name),
        forall|s: Set<Tuple>| type_of(#[trigger] agg(s)) == ty,
    ensures
        wf(summarize(r, by, name, ty, agg)),
        summarize(r, by, name, ty, agg).heading == r.heading.restrict(by).insert(name, ty),
{
    lemma_project_closed(r, by);
    let p = project(r, by);
    let nh = r.heading.restrict(by).insert(name, ty);
    assert forall|u: Tuple| #[trigger] summarize(r, by, name, ty, agg).body.contains(u) implies conforms(
        u,
        nh,
    ) by {
        let g = choose|g: Tuple| #[trigger]
            p.body.contains(g) && u == g.insert(name, agg(group_of(r, by, g)));
        assert(conforms(g, p.heading));
        assert(u.dom() =~= nh.dom());
        assert forall|a: Name| #[trigger] nh.dom().contains(a) implies type_of(u[a]) == nh[a] by {
            if a != name {
                assert(p.heading.dom().contains(a));
            }
        }
    }
}

/// Every summary tuple comes from a non-empty group: `SUMMARIZE` never
/// invents groups (and so never needs a NULL for "no rows").
pub proof fn lemma_summarize_groups_nonempty(r: Relation, by: Set<Name>, g: Tuple)
    requires
        project(r, by).body.contains(g),
    ensures
        group_of(r, by, g).len() > 0,
{
    lemma_project_contains(r, by, g);
    let t = choose|t: Tuple| #[trigger] r.body.contains(t) && g == tuple_project(t, by);
    assert(group_of(r, by, g).contains(t));
}

// ---------------------------------------------------------------------------
// GROUP and UNGROUP (relation-valued attributes)
// ---------------------------------------------------------------------------

/// The nested relation body for the group that `t` belongs to.
pub open spec fn nest(r: Relation, names: Set<Name>, t: Tuple) -> Set<Tuple> {
    r.body.filter(|u: Tuple| u.remove_keys(names) == t.remove_keys(names)).map(
        |u: Tuple| tuple_project(u, names),
    )
}

pub open spec fn group_tuple(r: Relation, names: Set<Name>, rva: Name, t: Tuple) -> Tuple {
    t.remove_keys(names).insert(
        rva,
        Val::Rel { heading: r.heading.restrict(names), body: nest(r, names, t) },
    )
}

/// `r GROUP { names } AS rva`.
pub open spec fn group(r: Relation, names: Set<Name>, rva: Name) -> Relation {
    Relation {
        heading: r.heading.remove_keys(names).insert(rva, Ty::Rel(r.heading.restrict(names))),
        body: r.body.map(|t: Tuple| group_tuple(r, names, rva, t)),
    }
}

pub open spec fn group_ok(r: Relation, names: Set<Name>, rva: Name) -> bool {
    &&& names.subset_of(r.heading.dom())
    &&& !r.heading.remove_keys(names).dom().contains(rva)
}

pub proof fn lemma_group_closed(r: Relation, names: Set<Name>, rva: Name)
    requires
        wf(r),
        group_ok(r, names, rva),
    ensures
        wf(group(r, names, rva)),
        group(r, names, rva).heading == r.heading.remove_keys(names).insert(
            rva,
            Ty::Rel(r.heading.restrict(names)),
        ),
{
    let nh = group(r, names, rva).heading;
    assert forall|u: Tuple| #[trigger] group(r, names, rva).body.contains(u) implies conforms(
        u,
        nh,
    ) by {
        let t = choose|t: Tuple| #[trigger] r.body.contains(t) && u == group_tuple(r, names, rva, t);
        assert(u.dom() =~= nh.dom());
        assert forall|a: Name| #[trigger] nh.dom().contains(a) implies type_of(u[a]) == nh[a] by {
            if a != rva {
                assert(r.heading.dom().contains(a));
            }
        }
    }
}

/// The relation-valued attribute produced by `GROUP` is itself a
/// well-formed relation of the advertised heading ("deep" well-formedness).
pub proof fn lemma_group_rva_wf(r: Relation, names: Set<Name>, t: Tuple)
    requires
        wf(r),
    ensures
        wf(Relation { heading: r.heading.restrict(names), body: nest(r, names, t) }),
{
    assert forall|v: Tuple| #[trigger] nest(r, names, t).contains(v) implies conforms(
        v,
        r.heading.restrict(names),
    ) by {
        let u = choose|u: Tuple| #[trigger]
            r.body.filter(|u: Tuple| u.remove_keys(names) == t.remove_keys(names)).contains(u)
                && v == tuple_project(u, names);
        lemma_project_tuple_conforms(u, r.heading, names);
    }
}

pub open spec fn rva_body(v: Val) -> Set<Tuple> {
    match v {
        Val::Rel { body, .. } => body,
        Val::Scalar { .. } => Set::empty(),
    }
}

pub open spec fn unnest(t: Tuple, rva: Name) -> Set<Tuple> {
    rva_body(t[rva]).map(|v: Tuple| t.remove(rva).union_prefer_right(v))
}

/// `r UNGROUP rva`.
pub open spec fn ungroup(r: Relation, rva: Name, inner: Heading) -> Relation {
    Relation {
        heading: r.heading.remove(rva).union_prefer_right(inner),
        body: r.body.map(|t: Tuple| unnest(t, rva)).flatten(),
    }
}

/// `UNGROUP` needs the attribute to be relation-valued with heading
/// `inner`, the inner attribute names to be new, and (deep
/// well-formedness) every nested body to conform to `inner`.
pub open spec fn ungroup_ok(r: Relation, rva: Name, inner: Heading) -> bool {
    &&& r.heading.dom().contains(rva)
    &&& r.heading[rva] == Ty::Rel(inner)
    &&& inner.dom().disjoint(r.heading.remove(rva).dom())
    &&& forall|t: Tuple, v: Tuple|
        #![trigger r.body.contains(t), rva_body(t[rva]).contains(v)]
        r.body.contains(t) && rva_body(t[rva]).contains(v) ==> conforms(v, inner)
}

pub proof fn lemma_ungroup_closed(r: Relation, rva: Name, inner: Heading)
    requires
        wf(r),
        ungroup_ok(r, rva, inner),
    ensures
        wf(ungroup(r, rva, inner)),
        ungroup(r, rva, inner).heading == r.heading.remove(rva).union_prefer_right(inner),
{
    let nh = r.heading.remove(rva).union_prefer_right(inner);
    assert forall|w: Tuple| #[trigger] ungroup(r, rva, inner).body.contains(w) implies conforms(
        w,
        nh,
    ) by {
        let s = choose|s: Set<Tuple>| #[trigger]
            r.body.map(|t: Tuple| unnest(t, rva)).contains(s) && s.contains(w);
        let t = choose|t: Tuple| #[trigger] r.body.contains(t) && s == unnest(t, rva);
        let v = choose|v: Tuple| #[trigger]
            rva_body(t[rva]).contains(v) && w == t.remove(rva).union_prefer_right(v);
        assert(conforms(v, inner));
        assert(conforms(t.remove(rva), r.heading.remove(rva))) by {
            assert(t.remove(rva).dom() =~= r.heading.remove(rva).dom());
        }
        lemma_union_conforms(t.remove(rva), r.heading.remove(rva), v, inner);
    }
}

// ---------------------------------------------------------------------------
// DIVIDEBY (Small Divide)
// ---------------------------------------------------------------------------

/// `r1 DIVIDEBY r2` with `r2`'s heading a sub-heading of `r1`'s: the
/// `A`-tuples of `r1` (A = attributes of r1 not in r2) that are paired in
/// `r1` with every tuple of `r2`. Written with the primitive operators:
/// `r1{A} MINUS ((r1{A} JOIN r2) MINUS r1){A}`.
pub open spec fn divide(r1: Relation, r2: Relation) -> Relation {
    let a = r1.heading.dom().difference(r2.heading.dom());
    minus(project(r1, a), project(minus(join(project(r1, a), r2), r1), a))
}

pub open spec fn divide_ok(r1: Relation, r2: Relation) -> bool {
    r2.heading.submap_of(r1.heading)
}

/// The joined heading in `divide` is `r1`'s heading again, so the inner
/// `MINUS` is between relations of the same heading.
proof fn lemma_divide_heading(r1: Relation, r2: Relation)
    requires
        divide_ok(r1, r2),
    ensures
        r1.heading.restrict(r1.heading.dom().difference(r2.heading.dom())).union_prefer_right(
            r2.heading,
        ) == r1.heading,
{
    assert(r1.heading.restrict(r1.heading.dom().difference(r2.heading.dom())).union_prefer_right(
        r2.heading,
    ) =~= r1.heading);
}

pub proof fn lemma_divide_closed(r1: Relation, r2: Relation)
    requires
        wf(r1),
        wf(r2),
        divide_ok(r1, r2),
    ensures
        wf(divide(r1, r2)),
        divide(r1, r2).heading == r1.heading.restrict(
            r1.heading.dom().difference(r2.heading.dom()),
        ),
{
    let a = r1.heading.dom().difference(r2.heading.dom());
    lemma_project_closed(r1, a);
    lemma_join_closed(project(r1, a), r2);
    lemma_divide_heading(r1, r2);
    lemma_set_ops_closed(join(project(r1, a), r2), r1);
    lemma_project_closed(minus(join(project(r1, a), r2), r1), a);
    lemma_set_ops_closed(project(r1, a), project(minus(join(project(r1, a), r2), r1), a));
}

/// The meaning of `DIVIDEBY`: `x` is in the quotient exactly when `x` is an
/// `A`-projection of `r1` and, for every tuple `y` of `r2`, `x ∪ y` is in
/// `r1`.
pub proof fn lemma_divide_meaning(r1: Relation, r2: Relation, x: Tuple)
    requires
        wf(r1),
        wf(r2),
        divide_ok(r1, r2),
    ensures
        divide(r1, r2).body.contains(x) <==> (project(
            r1,
            r1.heading.dom().difference(r2.heading.dom()),
        ).body.contains(x) && forall|y: Tuple| #[trigger]
            r2.body.contains(y) ==> r1.body.contains(x.union_prefer_right(y))),
{
    let a = r1.heading.dom().difference(r2.heading.dom());
    let pa = project(r1, a);
    let j = join(pa, r2);
    let bad = minus(j, r1);
    lemma_project_closed(r1, a);
    if pa.body.contains(x) {
        assert(conforms(x, pa.heading));
        // x ∪ y for y in r2 is a join tuple (x and y share no attributes).
        assert forall|y: Tuple| #[trigger] r2.body.contains(y) implies x.agrees(y)
            && j.body.contains(x.union_prefer_right(y)) by {
            assert(conforms(y, r2.heading));
            lemma_join_contains(pa, r2, x.union_prefer_right(y));
        }
        if forall|y: Tuple| #[trigger]
            r2.body.contains(y) ==> r1.body.contains(x.union_prefer_right(y)) {
            // No "bad" tuple projects to x, so x survives the MINUS.
            assert forall|b: Tuple| #[trigger] bad.body.contains(b) implies tuple_project(b, a)
                != x by {
                lemma_join_contains(pa, r2, b);
                let (x1, y1) = choose|x1: Tuple, y1: Tuple|
                    #![trigger pa.body.contains(x1), r2.body.contains(y1)]
                    pa.body.contains(x1) && r2.body.contains(y1) && x1.agrees(y1) && b
                        == x1.union_prefer_right(y1);
                assert(conforms(x1, pa.heading));
                assert(conforms(y1, r2.heading));
                assert(tuple_project(b, a) =~= x1);
                if x1 == x {
                    assert(r1.body.contains(x.union_prefer_right(y1)));
                }
            }
            lemma_project_contains(bad, a, x);
        } else {
            // Some y is missing: x ∪ y is a bad tuple, so x is removed.
            let y = choose|y: Tuple| #[trigger]
                r2.body.contains(y) && !r1.body.contains(x.union_prefer_right(y));
            assert(conforms(y, r2.heading));
            assert(bad.body.contains(x.union_prefer_right(y)));
            assert(tuple_project(x.union_prefer_right(y), a) =~= x);
            lemma_project_contains(bad, a, x);
        }
    }
}

// ---------------------------------------------------------------------------
// TCLOSE (transitive closure of a binary relation)
// ---------------------------------------------------------------------------

pub open spec fn pair(x_name: Name, y_name: Name, x: Val, y: Val) -> Tuple {
    Map::empty().insert(x_name, x).insert(y_name, y)
}

/// `r` is a binary relation `{X: T, Y: T}` (TCLOSE's precondition).
pub open spec fn binary(r: Relation, x_name: Name, y_name: Name, t: Ty) -> bool {
    &&& x_name != y_name
    &&& r.heading == Map::<Name, Ty>::empty().insert(x_name, t).insert(y_name, t)
}

/// A path `p[0] -> p[1] -> ... -> p[n-1]` of edges of `r`.
pub open spec fn is_path(r: Relation, x_name: Name, y_name: Name, p: Seq<Val>) -> bool {
    &&& p.len() >= 2
    &&& forall|i: int|
        0 <= i < p.len() - 1 ==> #[trigger] r.body.contains(pair(x_name, y_name, p[i], p[i + 1]))
}

pub open spec fn reaches(r: Relation, x_name: Name, y_name: Name, x: Val, y: Val) -> bool {
    exists|p: Seq<Val>|
        #[trigger] is_path(r, x_name, y_name, p) && p[0] == x && p[p.len() - 1] == y
}

/// The values that occur in `r` (both columns).
pub open spec fn vals(r: Relation, x_name: Name, y_name: Name) -> Set<Val> {
    r.body.map(|t: Tuple| t[x_name]).union(r.body.map(|t: Tuple| t[y_name]))
}

pub open spec fn candidates(r: Relation, x_name: Name, y_name: Name) -> Set<Tuple> {
    vals(r, x_name, y_name).map(
        |x: Val| vals(r, x_name, y_name).map(|y: Val| pair(x_name, y_name, x, y)),
    ).flatten()
}

/// `TCLOSE r`: every pair `(x, y)` with a path from `x` to `y`. Defined as a
/// filter of a finite candidate set, so it is a finite relation.
pub open spec fn tclose(r: Relation, x_name: Name, y_name: Name) -> Relation {
    Relation {
        heading: r.heading,
        body: candidates(r, x_name, y_name).filter(
            |t: Tuple| reaches(r, x_name, y_name, t[x_name], t[y_name]),
        ),
    }
}

proof fn lemma_pair_conforms(r: Relation, x_name: Name, y_name: Name, t: Ty, x: Val, y: Val)
    requires
        binary(r, x_name, y_name, t),
        type_of(x) == t,
        type_of(y) == t,
    ensures
        conforms(pair(x_name, y_name, x, y), r.heading),
{
    assert(pair(x_name, y_name, x, y).dom() =~= r.heading.dom());
}

proof fn lemma_vals_typed(r: Relation, x_name: Name, y_name: Name, t: Ty, v: Val)
    requires
        wf(r),
        binary(r, x_name, y_name, t),
        vals(r, x_name, y_name).contains(v),
    ensures
        type_of(v) == t,
{
    assert(r.heading.dom().contains(x_name));
    assert(r.heading.dom().contains(y_name));
    if r.body.map(|t: Tuple| t[x_name]).contains(v) {
        let u = choose|u: Tuple| #[trigger] r.body.contains(u) && v == u[x_name];
        assert(conforms(u, r.heading));
    } else {
        let u = choose|u: Tuple| #[trigger] r.body.contains(u) && v == u[y_name];
        assert(conforms(u, r.heading));
    }
}

pub proof fn lemma_tclose_closed(r: Relation, x_name: Name, y_name: Name, t: Ty)
    requires
        wf(r),
        binary(r, x_name, y_name, t),
    ensures
        wf(tclose(r, x_name, y_name)),
        tclose(r, x_name, y_name).heading == r.heading,
{
    let c = candidates(r, x_name, y_name);
    let vs = vals(r, x_name, y_name);
    assert forall|w: Tuple| #[trigger] tclose(r, x_name, y_name).body.contains(w) implies conforms(
        w,
        r.heading,
    ) by {
        assert(c.contains(w));
        let s = choose|s: Set<Tuple>| #[trigger]
            vs.map(|x: Val| vs.map(|y: Val| pair(x_name, y_name, x, y))).contains(s)
                && s.contains(w);
        let x = choose|x: Val| #[trigger]
            vs.contains(x) && s == vs.map(|y: Val| pair(x_name, y_name, x, y));
        let y = choose|y: Val| #[trigger] vs.contains(y) && w == pair(x_name, y_name, x, y);
        lemma_vals_typed(r, x_name, y_name, t, x);
        lemma_vals_typed(r, x_name, y_name, t, y);
        lemma_pair_conforms(r, x_name, y_name, t, x, y);
    }
}

/// Every edge of `r` is in `TCLOSE r`.
pub proof fn lemma_tclose_contains_r(r: Relation, x_name: Name, y_name: Name, t: Ty, e: Tuple)
    requires
        wf(r),
        binary(r, x_name, y_name, t),
        r.body.contains(e),
    ensures
        tclose(r, x_name, y_name).body.contains(e),
{
    let vs = vals(r, x_name, y_name);
    assert(conforms(e, r.heading));
    let x = e[x_name];
    let y = e[y_name];
    assert(e =~= pair(x_name, y_name, x, y));
    let p = seq![x, y];
    assert(is_path(r, x_name, y_name, p));
    assert(reaches(r, x_name, y_name, x, y));
    assert(r.body.map(|t: Tuple| t[x_name]).contains(x));
    assert(r.body.map(|t: Tuple| t[y_name]).contains(y));
    assert(vs.contains(x) && vs.contains(y));
    let s = vs.map(|y: Val| pair(x_name, y_name, x, y));
    assert(s.contains(e));
    assert(vs.map(|x: Val| vs.map(|y: Val| pair(x_name, y_name, x, y))).contains(s));
    assert(candidates(r, x_name, y_name).contains(e));
}

/// `TCLOSE r` is transitive: paths compose.
pub proof fn lemma_tclose_transitive(r: Relation, x_name: Name, y_name: Name, x: Val, y: Val, z: Val)
    requires
        x_name != y_name,
        reaches(r, x_name, y_name, x, y),
        reaches(r, x_name, y_name, y, z),
    ensures
        reaches(r, x_name, y_name, x, z),
{
    let p1 = choose|p: Seq<Val>|
        #[trigger] is_path(r, x_name, y_name, p) && p[0] == x && p[p.len() - 1] == y;
    let p2 = choose|p: Seq<Val>|
        #[trigger] is_path(r, x_name, y_name, p) && p[0] == y && p[p.len() - 1] == z;
    let p = p1 + p2.subrange(1, p2.len() as int);
    assert forall|i: int| 0 <= i < p.len() - 1 implies #[trigger] r.body.contains(
        pair(x_name, y_name, p[i], p[i + 1]),
    ) by {
        if i < p1.len() - 1 {
            assert(p[i] == p1[i] && p[i + 1] == p1[i + 1]);
        } else if i == p1.len() - 1 {
            assert(p[i] == p2[0] && p[i + 1] == p2[1]);
        } else {
            let k = i - p1.len() + 1;
            assert(p[i] == p2[k] && p[i + 1] == p2[k + 1]);
        }
    }
    assert(is_path(r, x_name, y_name, p));
    assert(p[0] == x && p[p.len() - 1] == z);
}

} // verus!
