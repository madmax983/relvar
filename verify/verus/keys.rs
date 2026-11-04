//! Candidate keys and foreign keys (RM Prescription 15, referential
//! constraints), and the key index that enforces key uniqueness in
//! `relvar-core/src/database/key_index.rs`.
use vstd::prelude::*;
use crate::model::*;

verus! {

/// `k` is a superkey of the relation value `r`: no two distinct tuples
/// agree on `k`.
pub open spec fn is_superkey(r: Relation, k: Set<Name>) -> bool {
    &&& k.subset_of(r.heading.dom())
    &&& forall|t1: Tuple, t2: Tuple|
        #![trigger r.body.contains(t1), r.body.contains(t2)]
        r.body.contains(t1) && r.body.contains(t2) && tuple_project(t1, k) == tuple_project(t2, k)
            ==> t1 == t2
}

/// `k` is a candidate key of `r`: a superkey none of whose proper subsets is
/// a superkey (irreducibility).
pub open spec fn is_candidate_key(r: Relation, k: Set<Name>) -> bool {
    &&& is_superkey(r, k)
    &&& forall|a: Name| #[trigger] k.contains(a) ==> !is_superkey(r, k.remove(a))
}

/// The whole heading is always a superkey. This is RM Proscription 3 (no
/// duplicate tuples) restated: because the body is a set, a relvar with no
/// declared key still has one.
pub proof fn lemma_heading_is_superkey(r: Relation)
    requires
        wf(r),
    ensures
        is_superkey(r, r.heading.dom()),
{
    assert forall|t1: Tuple, t2: Tuple|
        #![trigger r.body.contains(t1), r.body.contains(t2)]
        r.body.contains(t1) && r.body.contains(t2) && tuple_project(t1, r.heading.dom())
            == tuple_project(t2, r.heading.dom()) implies t1 == t2 by {
        lemma_project_tuple_onto_own_heading(t1, r.heading);
        lemma_project_tuple_onto_own_heading(t2, r.heading);
    }
}

/// Every superkey contains a candidate key (by removing attributes while
/// the remainder is still a superkey; the heading is finite, so this ends).
pub proof fn lemma_superkey_contains_candidate_key(r: Relation, k: Set<Name>)
    requires
        is_superkey(r, k),
    ensures
        exists|c: Set<Name>| #[trigger] is_candidate_key(r, c) && c.subset_of(k),
    decreases k.len(),
{
    if is_candidate_key(r, k) {
        assert(is_candidate_key(r, k) && k.subset_of(k));
    } else {
        let a = choose|a: Name| #[trigger] k.contains(a) && is_superkey(r, k.remove(a));
        assert(k.remove(a).len() < k.len());
        lemma_superkey_contains_candidate_key(r, k.remove(a));
        let c = choose|c: Set<Name>| #[trigger] is_candidate_key(r, c) && c.subset_of(k.remove(a));
        assert(c.subset_of(k));
    }
}

/// RM Prescription 15: every relation has at least one candidate key.
pub proof fn lemma_candidate_key_exists(r: Relation)
    requires
        wf(r),
    ensures
        exists|c: Set<Name>| #[trigger] is_candidate_key(r, c),
{
    lemma_heading_is_superkey(r);
    lemma_superkey_contains_candidate_key(r, r.heading.dom());
}

/// The empty key is legal: a relvar with key `{}` holds at most one tuple
/// (the runtime must accept `{}` as a key and enforce this).
pub proof fn lemma_empty_key_means_at_most_one_tuple(r: Relation)
    requires
        is_superkey(r, Set::empty()),
    ensures
        r.body.len() <= 1,
{
    if r.body.len() > 1 {
        let t1 = r.body.choose();
        assert(r.body.contains(t1));
        assert(r.body.remove(t1).len() > 0);
        let t2 = r.body.remove(t1).choose();
        assert(r.body.remove(t1).contains(t2));
        assert(r.body.contains(t2) && t1 != t2);
        assert(tuple_project(t1, Set::empty()) =~= tuple_project(t2, Set::empty()));
    }
}

// ---------------------------------------------------------------------------
// Key-checked insert: what `Database::insert` does.
// ---------------------------------------------------------------------------

/// Inserting `t` respects key `k`: every existing tuple with the same key
/// value is `t` itself.
pub open spec fn insert_respects_key(r: Relation, k: Set<Name>, t: Tuple) -> bool {
    forall|u: Tuple| #[trigger] r.body.contains(u) && tuple_project(u, k) == tuple_project(t, k) ==> u == t
}

/// A checked insert preserves well-formedness and every declared key.
pub proof fn lemma_checked_insert_preserves_key(r: Relation, k: Set<Name>, t: Tuple)
    requires
        wf(r),
        is_superkey(r, k),
        conforms(t, r.heading),
        insert_respects_key(r, k, t),
    ensures
        wf(insert(r, t)),
        is_superkey(insert(r, t), k),
{
    let r2 = insert(r, t);
    assert forall|t1: Tuple, t2: Tuple|
        #![trigger r2.body.contains(t1), r2.body.contains(t2)]
        r2.body.contains(t1) && r2.body.contains(t2) && tuple_project(t1, k) == tuple_project(t2, k)
            implies t1 == t2 by {
        if t1 != t && t2 != t {
            assert(r.body.contains(t1) && r.body.contains(t2));
        } else if t1 == t && t2 != t {
            assert(r.body.contains(t2));
        } else if t2 == t && t1 != t {
            assert(r.body.contains(t1));
        }
    }
}

/// If the check fails the insert would break the key, so rejecting it is
/// required, not merely allowed.
pub proof fn lemma_failed_check_would_violate_key(r: Relation, k: Set<Name>, t: Tuple)
    requires
        !insert_respects_key(r, k, t),
    ensures
        !is_superkey(insert(r, t), k),
{
    let u = choose|u: Tuple| #[trigger] r.body.contains(u) && tuple_project(u, k) == tuple_project(t, k) && u != t;
    assert(insert(r, t).body.contains(u) && insert(r, t).body.contains(t));
}

/// Deleting tuples never breaks a key.
pub proof fn lemma_delete_preserves_key(r: Relation, k: Set<Name>, t: Tuple)
    requires
        is_superkey(r, k),
    ensures
        is_superkey(delete(r, t), k),
{
    let r2 = delete(r, t);
    assert forall|t1: Tuple, t2: Tuple|
        #![trigger r2.body.contains(t1), r2.body.contains(t2)]
        r2.body.contains(t1) && r2.body.contains(t2) && tuple_project(t1, k) == tuple_project(t2, k)
            implies t1 == t2 by {
        assert(r.body.contains(t1) && r.body.contains(t2));
    }
}

// ---------------------------------------------------------------------------
// The key index (database/key_index.rs): a set of key values per key.
// ---------------------------------------------------------------------------

/// The index for key `k` holds exactly the key values of the body.
pub open spec fn index_inv(r: Relation, k: Set<Name>, idx: Set<Tuple>) -> bool {
    idx == r.body.map(|t: Tuple| tuple_project(t, k))
}

/// Under the index invariant and a valid key, the O(1) index probe the
/// runtime does ("is this key value already present?") is equivalent to
/// the full scan in `insert_respects_key`, for a tuple not yet present.
pub proof fn lemma_index_probe_is_exact(r: Relation, k: Set<Name>, idx: Set<Tuple>, t: Tuple)
    requires
        index_inv(r, k, idx),
        !r.body.contains(t),
    ensures
        idx.contains(tuple_project(t, k)) <==> !insert_respects_key(r, k, t),
{
    if idx.contains(tuple_project(t, k)) {
        let u = choose|u: Tuple| #[trigger] r.body.contains(u) && tuple_project(t, k) == tuple_project(u, k);
        assert(u != t);
    }
    if !insert_respects_key(r, k, t) {
        let u = choose|u: Tuple| #[trigger] r.body.contains(u) && tuple_project(u, k) == tuple_project(t, k) && u != t;
        assert(r.body.map(|t: Tuple| tuple_project(t, k)).contains(tuple_project(u, k)));
    }
}

/// The runtime treats inserting a tuple that is already present as a key
/// violation when a key is declared. That is conservative, never unsound:
/// the probe finds the value, and the database state is left unchanged.
pub proof fn lemma_index_probe_for_present_tuple(r: Relation, k: Set<Name>, idx: Set<Tuple>, t: Tuple)
    requires
        index_inv(r, k, idx),
        r.body.contains(t),
    ensures
        idx.contains(tuple_project(t, k)),
        insert(r, t) == r,
{
    assert(r.body.map(|t: Tuple| tuple_project(t, k)).contains(tuple_project(t, k)));
    lemma_no_duplicates(r, t);
}

/// Maintaining the index on insert: add the new key value.
pub proof fn lemma_index_insert(r: Relation, k: Set<Name>, idx: Set<Tuple>, t: Tuple)
    requires
        index_inv(r, k, idx),
    ensures
        index_inv(insert(r, t), k, idx.insert(tuple_project(t, k))),
{
    assert(insert(r, t).body.map(|t: Tuple| tuple_project(t, k)) =~= idx.insert(tuple_project(t, k)));
}

/// Maintaining the index on delete under a valid key: remove the deleted
/// tuple's key value (no other tuple shares it).
pub proof fn lemma_index_delete(r: Relation, k: Set<Name>, idx: Set<Tuple>, t: Tuple)
    requires
        index_inv(r, k, idx),
        is_superkey(r, k),
        r.body.contains(t),
    ensures
        index_inv(delete(r, t), k, idx.remove(tuple_project(t, k))),
{
    let lhs = delete(r, t).body.map(|t: Tuple| tuple_project(t, k));
    let rhs = idx.remove(tuple_project(t, k));
    assert forall|v: Tuple| lhs.contains(v) <==> rhs.contains(v) by {
        if lhs.contains(v) {
            let u = choose|u: Tuple| #[trigger] delete(r, t).body.contains(u) && v == tuple_project(u, k);
            assert(r.body.contains(u));
            assert(r.body.map(|t: Tuple| tuple_project(t, k)).contains(v));
        }
        if rhs.contains(v) {
            let u = choose|u: Tuple| #[trigger] r.body.contains(u) && v == tuple_project(u, k);
            assert(u != t);
            assert(delete(r, t).body.contains(u));
        }
    }
    assert(lhs =~= rhs);
}

// ---------------------------------------------------------------------------
// Foreign keys: an inclusion dependency onto a candidate key.
// ---------------------------------------------------------------------------

/// Every `fk`-value of the referencing relation `r1` is a `k`-value of the
/// referenced relation `r2` (attribute names already aligned by renaming).
pub open spec fn fk_holds(r1: Relation, fk: Set<Name>, r2: Relation) -> bool {
    forall|t: Tuple| #[trigger] r1.body.contains(t) ==> exists|u: Tuple| #[trigger]
        r2.body.contains(u) && tuple_project(u, fk) == tuple_project(t, fk)
}

/// Inserting into the referencing relvar preserves the FK exactly when the
/// new tuple has a match.
pub proof fn lemma_fk_insert_referencing(r1: Relation, fk: Set<Name>, r2: Relation, t: Tuple)
    requires
        fk_holds(r1, fk, r2),
    ensures
        fk_holds(insert(r1, t), fk, r2) <==> exists|u: Tuple| #[trigger]
            r2.body.contains(u) && tuple_project(u, fk) == tuple_project(t, fk),
{
    assert(insert(r1, t).body.contains(t));
}

/// Deleting from the referenced relvar preserves the FK when no remaining
/// referencing tuple used the deleted value. Note the check is against the
/// state AFTER the statement — this is what makes self-referencing FKs and
/// multiple assignment come out right.
pub proof fn lemma_fk_delete_referenced(r1: Relation, fk: Set<Name>, r2: Relation, u: Tuple)
    requires
        fk_holds(r1, fk, r2),
        forall|t: Tuple| #[trigger] r1.body.contains(t) ==> exists|w: Tuple| #[trigger]
            delete(r2, u).body.contains(w) && tuple_project(w, fk) == tuple_project(t, fk),
    ensures
        fk_holds(r1, fk, delete(r2, u)),
{
}

/// A foreign key must reference a candidate key: otherwise "the referenced
/// tuple" is not unique. Under a key, the referenced tuple is unique.
pub proof fn lemma_fk_target_unique(r2: Relation, fk: Set<Name>, t: Tuple, u1: Tuple, u2: Tuple)
    requires
        is_superkey(r2, fk),
        r2.body.contains(u1),
        r2.body.contains(u2),
        tuple_project(u1, fk) == tuple_project(t, fk),
        tuple_project(u2, fk) == tuple_project(t, fk),
    ensures
        u1 == u2,
{
}

} // verus!
