//! The value layer of the relational model: types, values, headings,
//! tuples and relations — the "math shadow" of `relvar-core`'s
//! `ScalarType`/`ScalarValue`/`TupleType`/`Tuple`/`RelationType`/`Relation`.
//!
//! Everything here is a `spec` definition. The proofs in this module are the
//! value-level TTM proscriptions; the operator proofs live in `algebra.rs`.
//!
//! Representation choices (each one *is* a TTM rule, made structural):
//!
//! | Model                         | TTM rule                                   |
//! |-------------------------------|--------------------------------------------|
//! | `Val` has no null variant     | no nulls (RM Proscription 4)               |
//! | `Tuple = Map<Name, Val>`      | no attribute ordering (RM Proscription 1)  |
//! | `Heading = Map<Name, Ty>`     | no attribute ordering (RM Proscription 1)  |
//! | `body: Set<Tuple>`            | no tuple ordering, no duplicates (RM Proscriptions 2, 3) |
//! | `Val::Rel` carries its heading | relation-valued attributes (RM Prescription 7) |
//!
//! Because vstd's `Set` and `Map` are finite and accept recursive types, a
//! relation value can be an attribute value (RVAs), and every relation in the
//! model is finite, as TTM requires.
use vstd::prelude::*;

verus! {

broadcast use vstd::map_lib::group_map_union;

/// Attribute names. Opaque on purpose: attributes are identified by name
/// only (RM Proscription 1), so the model gives names no structure at all.
pub type Name = int;

/// A type: a scalar type (identified by an opaque id) or a relation type
/// `RELATION { heading }` (RM Prescription 7, the RELATION type generator).
pub enum Ty {
    Scalar(int),
    Rel(Map<Name, Ty>),
}

/// A heading: a set of `<attribute name, type>` pairs with unique names
/// (RM Prescription 9). A `Map` has unique keys and no order.
pub type Heading = Map<Name, Ty>;

/// A value. There is deliberately no `Null` variant: every value is a value
/// of some type (RM Prescription 2, RM Proscription 4).
pub enum Val {
    /// A scalar value: its declared type and its (opaque) content.
    Scalar { ty: int, bits: int },
    /// A relation value used as an attribute value (an RVA).
    Rel { heading: Map<Name, Ty>, body: Set<Map<Name, Val>> },
}

/// A tuple: a set of `<attribute name, value>` pairs (RM Prescription 9).
pub type Tuple = Map<Name, Val>;

/// The type of a value. Every value has exactly one type (RM Prescription 2).
pub open spec fn type_of(v: Val) -> Ty {
    match v {
        Val::Scalar { ty, .. } => Ty::Scalar(ty),
        Val::Rel { heading, .. } => Ty::Rel(heading),
    }
}

/// `t` conforms to heading `h`: exactly the heading's attributes, each with
/// a value of the attribute's declared type. This is what makes a tuple
/// NULL-free: an attribute of the heading cannot be missing from the tuple.
pub open spec fn conforms(t: Tuple, h: Heading) -> bool {
    &&& t.dom() == h.dom()
    &&& forall|a: Name| #[trigger] h.dom().contains(a) ==> type_of(t[a]) == h[a]
}

/// A relation value: a heading and a body (RM Prescription 10).
pub struct Relation {
    pub heading: Heading,
    pub body: Set<Tuple>,
}

/// Well-formedness: every tuple of the body conforms to the heading.
/// Every operator in `algebra.rs` is proved to preserve this.
pub open spec fn wf(r: Relation) -> bool {
    forall|t: Tuple| #[trigger] r.body.contains(t) ==> conforms(t, r.heading)
}

/// The degree of a relation (number of attributes).
pub open spec fn degree(r: Relation) -> nat {
    r.heading.dom().len()
}

/// The cardinality of a relation (number of tuples).
pub open spec fn cardinality(r: Relation) -> nat {
    r.body.len()
}

/// The empty relation with heading `h`.
pub open spec fn empty(h: Heading) -> Relation {
    Relation { heading: h, body: Set::empty() }
}

/// `TABLE_DEE`: the degree-0 relation with one (empty) tuple — "true".
pub open spec fn table_dee() -> Relation {
    Relation { heading: Map::empty(), body: Set::empty().insert(Map::empty()) }
}

/// `TABLE_DUM`: the degree-0 relation with no tuples — "false".
pub open spec fn table_dum() -> Relation {
    Relation { heading: Map::empty(), body: Set::empty() }
}

/// Inserting a tuple into a relation value (set semantics).
pub open spec fn insert(r: Relation, t: Tuple) -> Relation {
    Relation { heading: r.heading, body: r.body.insert(t) }
}

/// Deleting a tuple from a relation value.
pub open spec fn delete(r: Relation, t: Tuple) -> Relation {
    Relation { heading: r.heading, body: r.body.remove(t) }
}

/// Projection of a single tuple onto a set of attribute names.
pub open spec fn tuple_project(t: Tuple, names: Set<Name>) -> Tuple {
    t.restrict(names)
}

// ---------------------------------------------------------------------------
// RM Proscription 4 — no nulls.
// ---------------------------------------------------------------------------

/// Every attribute of a well-formed relation's heading has a value, of the
/// declared type, in every tuple of the body. There is no "missing" value.
pub proof fn lemma_no_nulls(r: Relation, t: Tuple, a: Name)
    requires
        wf(r),
        r.body.contains(t),
        r.heading.dom().contains(a),
    ensures
        t.dom().contains(a),
        type_of(t[a]) == r.heading[a],
{
}

/// A tuple that lacks an attribute of the heading can never be in the body
/// of a well-formed relation (the dual of `lemma_no_nulls`).
pub proof fn lemma_missing_attribute_rejected(r: Relation, t: Tuple, a: Name)
    requires
        wf(r),
        r.heading.dom().contains(a),
        !t.dom().contains(a),
    ensures
        !r.body.contains(t),
{
}

/// Inserting a non-conforming tuple breaks well-formedness, so a checked
/// insert must reject it (the runtime does: `TupleError::MissingValue`,
/// `TupleError::TypeMismatch`, `RelationError`).
pub proof fn lemma_insert_requires_conformance(r: Relation, t: Tuple)
    requires
        wf(insert(r, t)),
    ensures
        conforms(t, r.heading),
{
    assert(insert(r, t).body.contains(t));
}

/// Inserting a conforming tuple preserves well-formedness.
pub proof fn lemma_insert_wf(r: Relation, t: Tuple)
    requires
        wf(r),
        conforms(t, r.heading),
    ensures
        wf(insert(r, t)),
{
}

/// Deleting preserves well-formedness.
pub proof fn lemma_delete_wf(r: Relation, t: Tuple)
    requires
        wf(r),
    ensures
        wf(delete(r, t)),
{
}

// ---------------------------------------------------------------------------
// RM Proscription 3 — no duplicate tuples.
// ---------------------------------------------------------------------------

/// Inserting a tuple that is already present leaves the relation unchanged:
/// a relation cannot hold the same tuple twice.
pub proof fn lemma_no_duplicates(r: Relation, t: Tuple)
    requires
        r.body.contains(t),
    ensures
        insert(r, t) == r,
{
    assert(r.body.insert(t) =~= r.body);
}

/// Cardinality counts distinct tuples: inserting a duplicate does not
/// change it, inserting a new tuple increases it by exactly one.
pub proof fn lemma_cardinality_counts_distinct_tuples(r: Relation, t: Tuple)
    ensures
        r.body.contains(t) ==> cardinality(insert(r, t)) == cardinality(r),
        !r.body.contains(t) ==> cardinality(insert(r, t)) == cardinality(r) + 1,
{
    if r.body.contains(t) {
        lemma_no_duplicates(r, t);
    }
}

// ---------------------------------------------------------------------------
// RM Proscription 2 — no tuple ordering.
// ---------------------------------------------------------------------------

/// The order in which tuples are inserted is not part of the value.
pub proof fn lemma_insertion_order_irrelevant(r: Relation, t1: Tuple, t2: Tuple)
    ensures
        insert(insert(r, t1), t2) == insert(insert(r, t2), t1),
{
    assert(r.body.insert(t1).insert(t2) =~= r.body.insert(t2).insert(t1));
}

/// Relation equality is extensional: two relations are equal exactly when
/// they have the same heading and the same tuples. There is no position,
/// cursor or "first tuple" that could tell them apart (RM Prescription 8).
pub proof fn lemma_relation_equality_is_extensional(r1: Relation, r2: Relation)
    requires
        r1.heading == r2.heading,
        forall|t: Tuple| r1.body.contains(t) <==> r2.body.contains(t),
    ensures
        r1 == r2,
{
    assert(r1.body =~= r2.body);
}

// ---------------------------------------------------------------------------
// RM Proscription 1 — no attribute ordering.
// ---------------------------------------------------------------------------

/// The order in which attributes are given is not part of a tuple.
pub proof fn lemma_tuple_attribute_order_irrelevant(t: Tuple, a: Name, x: Val, b: Name, y: Val)
    requires
        a != b,
    ensures
        t.insert(a, x).insert(b, y) == t.insert(b, y).insert(a, x),
{
    assert(t.insert(a, x).insert(b, y) =~= t.insert(b, y).insert(a, x));
}

/// The order in which attributes are declared is not part of a heading.
pub proof fn lemma_heading_attribute_order_irrelevant(h: Heading, a: Name, x: Ty, b: Name, y: Ty)
    requires
        a != b,
    ensures
        h.insert(a, x).insert(b, y) == h.insert(b, y).insert(a, x),
{
    assert(h.insert(a, x).insert(b, y) =~= h.insert(b, y).insert(a, x));
}

/// Tuple equality is extensional: same attribute names, same values.
pub proof fn lemma_tuple_equality_is_extensional(t1: Tuple, t2: Tuple)
    requires
        t1.dom() == t2.dom(),
        forall|a: Name| #[trigger] t1.dom().contains(a) ==> t1[a] == t2[a],
    ensures
        t1 == t2,
{
    assert(t1 =~= t2);
}

// ---------------------------------------------------------------------------
// RM Proscription 5 — no nullological mistakes (TABLE_DEE / TABLE_DUM).
// ---------------------------------------------------------------------------

/// `TABLE_DEE` and `TABLE_DUM` are well-formed relations of degree 0, and
/// they are different values with cardinalities 1 and 0.
pub proof fn lemma_dee_dum()
    ensures
        wf(table_dee()),
        wf(table_dum()),
        degree(table_dee()) == 0,
        degree(table_dum()) == 0,
        cardinality(table_dee()) == 1,
        cardinality(table_dum()) == 0,
        table_dee() != table_dum(),
{
    assert forall|t: Tuple| #[trigger] table_dee().body.contains(t) implies conforms(
        t,
        table_dee().heading,
    ) by {
        assert(t == Map::<Name, Val>::empty());
        assert(t.dom() =~= Map::<Name, Ty>::empty().dom());
    }
    assert(table_dee().body.contains(Map::empty()));
    assert(!table_dum().body.contains(Map::<Name, Val>::empty()));
}

/// Over the empty heading there are exactly two relations: every tuple that
/// conforms to the empty heading is the empty tuple, so a well-formed
/// degree-0 relation is `TABLE_DEE` or `TABLE_DUM`.
pub proof fn lemma_degree_zero_is_dee_or_dum(r: Relation)
    requires
        wf(r),
        r.heading == Map::<Name, Ty>::empty(),
    ensures
        r == table_dee() || r == table_dum(),
{
    assert forall|t: Tuple| #[trigger] r.body.contains(t) implies t == Map::<Name, Val>::empty() by {
        assert(t =~= Map::<Name, Val>::empty());
    }
    if r.body.contains(Map::empty()) {
        assert(r.body =~= table_dee().body);
    } else {
        assert(r.body =~= table_dum().body);
    }
}

/// The empty relation of any heading is well-formed.
pub proof fn lemma_empty_wf(h: Heading)
    ensures
        wf(empty(h)),
        cardinality(empty(h)) == 0,
{
}

/// A tuple conforming to a heading, projected onto that heading's names, is
/// itself (projection onto everything is the identity).
pub proof fn lemma_project_tuple_onto_own_heading(t: Tuple, h: Heading)
    requires
        conforms(t, h),
    ensures
        tuple_project(t, h.dom()) == t,
{
    assert(tuple_project(t, h.dom()) =~= t);
}

/// A projected tuple conforms to the projected heading.
pub proof fn lemma_project_tuple_conforms(t: Tuple, h: Heading, names: Set<Name>)
    requires
        conforms(t, h),
    ensures
        conforms(tuple_project(t, names), h.restrict(names)),
{
    assert(tuple_project(t, names).dom() =~= h.restrict(names).dom());
}

} // verus!
