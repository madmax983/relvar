//! Float equality, hashing and ordering in `ScalarValue`
//! (relvar-core/src/values/scalar/mod.rs): RM Prescription 8 (equality is
//! "same value") and RM Proscription 3 (a set needs an equality that is an
//! equivalence relation and a hash that respects it).
//!
//! `ScalarValue::Float` compares by bit pattern with every NaN equal to every
//! NaN, hashes every NaN as `f64::NAN.to_bits()`, and orders NaN above all
//! numbers with `-0.0 < 0.0`. These definitions mirror that code; the exec
//! functions below are line-for-line transcriptions of `PartialEq`,
//! `Hash` and `cmp_floats`, verified against the spec.
use vstd::prelude::*;

verus! {

pub const EXP_MASK: u64 = 0x7ff0_0000_0000_0000;
pub const MANT_MASK: u64 = 0x000f_ffff_ffff_ffff;
pub const SIGN_BIT: u64 = 0x8000_0000_0000_0000;
/// `f64::NAN.to_bits()`.
pub const CANONICAL_NAN: u64 = 0x7ff8_0000_0000_0000;

pub open spec fn is_nan(b: u64) -> bool {
    (b & EXP_MASK) == EXP_MASK && (b & MANT_MASK) != 0
}

pub open spec fn float_eq(a: u64, b: u64) -> bool {
    (is_nan(a) && is_nan(b)) || a == b
}

pub open spec fn hash_key(a: u64) -> u64 {
    if is_nan(a) { CANONICAL_NAN } else { a }
}

/// The total-order key: NaNs above everything, then IEEE order on numbers
/// with `-0.0` just below `0.0`.
pub open spec fn order_key(a: u64) -> int {
    if is_nan(a) {
        (0xffff_ffff_ffff_ffffu64 as int) + 1
    } else if a >= SIGN_BIT {
        (0xffff_ffff_ffff_ffffu64 as int) - (a as int)
    } else {
        (a as int) + (0x8000_0000_0000_0000u64 as int)
    }
}

pub open spec fn sgn(x: int) -> int {
    if x < 0 { -1 } else if x == 0 { 0 } else { 1 }
}

pub proof fn lemma_canonical_nan_is_nan()
    ensures
        is_nan(CANONICAL_NAN),
{
    assert(((0x7ff8_0000_0000_0000u64 & 0x7ff0_0000_0000_0000u64) == 0x7ff0_0000_0000_0000u64) && ((
    0x7ff8_0000_0000_0000u64 & 0x000f_ffff_ffff_ffffu64) != 0)) by (bit_vector);
}

/// Float equality is an equivalence relation (reflexive even for NaN, which
/// IEEE `==` is not — a set of floats would otherwise break).
pub proof fn lemma_float_eq_is_equivalence(a: u64, b: u64, c: u64)
    ensures
        float_eq(a, a),
        float_eq(a, b) ==> float_eq(b, a),
        float_eq(a, b) && float_eq(b, c) ==> float_eq(a, c),
{
}

/// The hash respects equality, and is exact: equal hash keys mean equal
/// values. So a hash set never stores two equal floats and never merges two
/// different ones.
pub proof fn lemma_hash_coherent(a: u64, b: u64)
    ensures
        float_eq(a, b) <==> hash_key(a) == hash_key(b),
{
    lemma_canonical_nan_is_nan();
}

/// The order is total and consistent with equality.
pub proof fn lemma_order_consistent_with_eq(a: u64, b: u64)
    ensures
        order_key(a) == order_key(b) <==> float_eq(a, b),
{
}

pub proof fn lemma_order_total(a: u64, b: u64, c: u64)
    ensures
        sgn(order_key(a) - order_key(b)) == -sgn(order_key(b) - order_key(a)),
        order_key(a) <= order_key(b) && order_key(b) <= order_key(c) ==> order_key(a) <= order_key(c),
        order_key(a) <= order_key(b) || order_key(b) <= order_key(a),
{
}

/// `-0.0 < 0.0`, and they are different values.
pub proof fn lemma_signed_zeros()
    ensures
        order_key(SIGN_BIT) < order_key(0),
        !float_eq(SIGN_BIT, 0),
{
    assert(!is_nan(SIGN_BIT) && !is_nan(0)) by {
        assert((0x8000_0000_0000_0000u64 & 0x7ff0_0000_0000_0000u64) != 0x7ff0_0000_0000_0000u64)
            by (bit_vector);
        assert((0u64 & 0x7ff0_0000_0000_0000u64) != 0x7ff0_0000_0000_0000u64) by (bit_vector);
    }
}

// ---------------------------------------------------------------------------
// Executable transcriptions of the runtime code.
// ---------------------------------------------------------------------------

pub fn is_nan_bits(b: u64) -> (r: bool)
    ensures
        r == is_nan(b),
{
    (b & EXP_MASK) == EXP_MASK && (b & MANT_MASK) != 0
}

/// `ScalarValue::eq` on `Float`.
pub fn float_bits_eq(a: u64, b: u64) -> (r: bool)
    ensures
        r == float_eq(a, b),
{
    if is_nan_bits(a) && is_nan_bits(b) {
        true
    } else {
        a == b
    }
}

/// `ScalarValue::hash` on `Float`: the bits that get hashed.
pub fn float_hash_bits(a: u64) -> (r: u64)
    ensures
        r == hash_key(a),
{
    if is_nan_bits(a) {
        CANONICAL_NAN
    } else {
        a
    }
}

/// `ScalarValue::cmp_floats`, returning -1 / 0 / 1.
pub fn cmp_float_bits(a: u64, b: u64) -> (r: i8)
    ensures
        r as int == sgn(order_key(a) - order_key(b)),
{
    let an = is_nan_bits(a);
    let bn = is_nan_bits(b);
    if an && bn {
        0
    } else if an {
        1
    } else if bn {
        -1
    } else {
        let a_sign = a >> 63u64;
        let b_sign = b >> 63u64;
        assert(a_sign == (if a >= SIGN_BIT { 1u64 } else { 0u64 })) by (bit_vector)
            requires a_sign == a >> 63u64;
        assert(b_sign == (if b >= SIGN_BIT { 1u64 } else { 0u64 })) by (bit_vector)
            requires b_sign == b >> 63u64;
        if a_sign != b_sign {
            if a_sign == 1 {
                -1
            } else {
                1
            }
        } else if a_sign == 0 {
            // Both positive: larger bits are larger.
            if a < b {
                -1
            } else if a == b {
                0
            } else {
                1
            }
        } else {
            // Both negative: larger bits are more negative.
            if a > b {
                -1
            } else if a == b {
                0
            } else {
                1
            }
        }
    }
}

} // verus!
