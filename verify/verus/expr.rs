//! Relational expressions and the closure theorem: ANY well-typed
//! relational expression, nested arbitrarily deep, evaluates to a
//! well-formed relation whose heading is the statically inferred one
//! (RM Prescriptions 18 and 19: the algebra is closed and every relational
//! expression has a known heading; RM Prescription 14: a virtual relvar's
//! value is its defining expression's value, so it is always well-formed
//! and of its declared heading).
use vstd::prelude::*;
use crate::model::*;
use crate::algebra::*;
use crate::database::*;

verus! {

pub enum Expr {
    Var(RelvarName),
    Restrict(Box<Expr>, spec_fn(Tuple) -> bool),
    Project(Box<Expr>, Set<Name>),
    Rename(Box<Expr>, Name, Name),
    Union(Box<Expr>, Box<Expr>),
    Intersect(Box<Expr>, Box<Expr>),
    Minus(Box<Expr>, Box<Expr>),
    Join(Box<Expr>, Box<Expr>),
    Matching(Box<Expr>, Box<Expr>),
    NotMatching(Box<Expr>, Box<Expr>),
    Extend(Box<Expr>, Name, Ty, spec_fn(Tuple) -> Val),
    Divide(Box<Expr>, Box<Expr>),
}

/// Set operators need equal headings.
pub open spec fn same_heading(a: Option<Heading>, b: Option<Heading>) -> Option<Heading> {
    match (a, b) {
        (Some(h1), Some(h2)) => if h1 == h2 { Some(h1) } else { None },
        _ => None,
    }
}

/// `MATCHING`/`NOT MATCHING` keep the left heading; common attributes must
/// agree in type.
pub open spec fn left_if_compatible(a: Option<Heading>, b: Option<Heading>) -> Option<Heading> {
    match (a, b) {
        (Some(h1), Some(h2)) => if compatible(h1, h2) { Some(h1) } else { None },
        _ => None,
    }
}

/// Static heading inference (type checking). `None` = type error, which the
/// runtime reports as an error instead of evaluating.
pub open spec fn heading_of(e: &Expr, env: Map<RelvarName, Heading>) -> Option<Heading>
    decreases e,
{
    match e {
        Expr::Var(n) => if env.dom().contains(*n) { Some(env[*n]) } else { None },
        Expr::Restrict(a, _) => heading_of(a, env),
        Expr::Project(a, names) => match heading_of(a, env) {
            Some(h) => if names.subset_of(h.dom()) { Some(h.restrict(*names)) } else { None },
            None => None,
        },
        Expr::Rename(a, from, to) => match heading_of(a, env) {
            Some(h) => if rename_ok(h, *from, *to) { Some(rename_heading(h, *from, *to)) } else { None },
            None => None,
        },
        Expr::Union(a, b) => same_heading(heading_of(a, env), heading_of(b, env)),
        Expr::Intersect(a, b) => same_heading(heading_of(a, env), heading_of(b, env)),
        Expr::Minus(a, b) => same_heading(heading_of(a, env), heading_of(b, env)),
        Expr::Join(a, b) => match (heading_of(a, env), heading_of(b, env)) {
            (Some(h1), Some(h2)) => if compatible(h1, h2) { Some(h1.union_prefer_right(h2)) } else { None },
            _ => None,
        },
        Expr::Matching(a, b) => left_if_compatible(heading_of(a, env), heading_of(b, env)),
        Expr::NotMatching(a, b) => left_if_compatible(heading_of(a, env), heading_of(b, env)),
        Expr::Extend(a, name, ty, f) => match heading_of(a, env) {
            Some(h) => if !h.dom().contains(*name) && (forall|t: Tuple| type_of(#[trigger] (*f)(t)) == *ty) {
                Some(h.insert(*name, *ty))
            } else {
                None
            },
            None => None,
        },
        Expr::Divide(a, b) => match (heading_of(a, env), heading_of(b, env)) {
            (Some(h1), Some(h2)) => if h2.submap_of(h1) { Some(h1.restrict(h1.dom().difference(h2.dom()))) } else { None },
            _ => None,
        },
    }
}

pub open spec fn eval(e: &Expr, db: Db) -> Relation
    decreases e,
{
    match e {
        Expr::Var(n) => db.relvars[*n],
        Expr::Restrict(a, p) => restrict(eval(a, db), *p),
        Expr::Project(a, names) => project(eval(a, db), *names),
        Expr::Rename(a, from, to) => rename(eval(a, db), *from, *to),
        Expr::Union(a, b) => union(eval(a, db), eval(b, db)),
        Expr::Intersect(a, b) => intersect(eval(a, db), eval(b, db)),
        Expr::Minus(a, b) => minus(eval(a, db), eval(b, db)),
        Expr::Join(a, b) => join(eval(a, db), eval(b, db)),
        Expr::Matching(a, b) => semijoin(eval(a, db), eval(b, db)),
        Expr::NotMatching(a, b) => semiminus(eval(a, db), eval(b, db)),
        Expr::Extend(a, name, ty, f) => extend(eval(a, db), *name, *ty, *f),
        Expr::Divide(a, b) => divide(eval(a, db), eval(b, db)),
    }
}

/// The database matches the environment the expression was typed in.
pub open spec fn db_matches(db: Db, env: Map<RelvarName, Heading>) -> bool {
    &&& db.relvars.dom() == env.dom()
    &&& forall|n: RelvarName| #[trigger] db.relvars.dom().contains(n) ==> wf(db.relvars[n])
        && db.relvars[n].heading == env[n]
}

/// THE CLOSURE THEOREM. A well-typed expression over a well-formed database
/// evaluates to a well-formed relation of exactly its inferred heading:
/// every result is NULL-free, duplicate-free, unordered and correctly typed,
/// however the operators are nested.
pub proof fn theorem_closure(e: &Expr, db: Db, env: Map<RelvarName, Heading>)
    requires
        db_matches(db, env),
        heading_of(e, env) is Some,
    ensures
        wf(eval(e, db)),
        eval(e, db).heading == heading_of(e, env)->Some_0,
    decreases e,
{
    match e {
        Expr::Var(n) => {
            assert(db.relvars.dom().contains(*n));
        },
        Expr::Restrict(a, p) => {
            theorem_closure(a, db, env);
            lemma_restrict_closed(eval(a, db), *p);
        },
        Expr::Project(a, names) => {
            theorem_closure(a, db, env);
            lemma_project_closed(eval(a, db), *names);
        },
        Expr::Rename(a, from, to) => {
            theorem_closure(a, db, env);
            lemma_rename_closed(eval(a, db), *from, *to);
        },
        Expr::Union(a, b) => {
            theorem_closure(a, db, env);
            theorem_closure(b, db, env);
            lemma_set_ops_closed(eval(a, db), eval(b, db));
        },
        Expr::Intersect(a, b) => {
            theorem_closure(a, db, env);
            theorem_closure(b, db, env);
            lemma_set_ops_closed(eval(a, db), eval(b, db));
        },
        Expr::Minus(a, b) => {
            theorem_closure(a, db, env);
            theorem_closure(b, db, env);
            lemma_set_ops_closed(eval(a, db), eval(b, db));
        },
        Expr::Join(a, b) => {
            theorem_closure(a, db, env);
            theorem_closure(b, db, env);
            lemma_join_closed(eval(a, db), eval(b, db));
        },
        Expr::Matching(a, b) => {
            theorem_closure(a, db, env);
            theorem_closure(b, db, env);
            lemma_semijoin_closed_and_partition(eval(a, db), eval(b, db));
        },
        Expr::NotMatching(a, b) => {
            theorem_closure(a, db, env);
            theorem_closure(b, db, env);
            lemma_semijoin_closed_and_partition(eval(a, db), eval(b, db));
        },
        Expr::Extend(a, name, ty, f) => {
            theorem_closure(a, db, env);
            lemma_extend_closed(eval(a, db), *name, *ty, *f);
        },
        Expr::Divide(a, b) => {
            theorem_closure(a, db, env);
            theorem_closure(b, db, env);
            lemma_divide_closed(eval(a, db), eval(b, db));
        },
    }
}

/// A virtual relvar (view) defined by a well-typed expression always has a
/// well-formed value of its declared heading, in every consistent database.
pub proof fn theorem_virtual_relvar_well_formed(e: &Expr, db: Db, s: Schema)
    requires
        consistent(db, s),
        heading_of(e, s.headings) is Some,
    ensures
        wf(eval(e, db)),
        eval(e, db).heading == heading_of(e, s.headings)->Some_0,
{
    theorem_closure(e, db, s.headings);
}

} // verus!
