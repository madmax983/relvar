//! # relvar TTM verification (Verus)
//!
//! Machine-checked proofs that the relational model `relvar` implements
//! satisfies The Third Manifesto's invariants. See
//! `docs/ttm-invariants.md` for the catalog mapping every prescription and
//! proscription to its proof here, its runtime enforcement, and the tests
//! that check the runtime against this model.
//!
//! Verify with (Verus release pinned in `.github/workflows/verify.yml`):
//!
//! ```text
//! verus --crate-type=lib verify/verus/lib.rs
//! ```
//!
//! No assume, no admit, no external-body escapes and no axioms are used
//! anywhere in this crate; CI fails if one appears.
#![allow(unused_imports)]

pub mod model;
pub mod algebra;
pub mod keys;
pub mod database;
pub mod expr;
pub mod mvcc;
pub mod scalar;
