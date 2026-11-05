//! `Ord` / `Eq` / `Hash` coherence for [`ScalarValue`].
//!
//! `Ord` must be a total order consistent with `Eq` (`a.cmp(b) == Equal`
//! exactly when `a == b`), and `Hash` must agree with `Eq`. Sorted
//! containers (`BTreeMap`/`BTreeSet` keys, `sort` + `dedup`) and hash
//! containers both rely on this; an `Ord` that calls two different values
//! `Equal` silently merges them.

use super::*;
use crate::collections::new_hasher;
use crate::types::{RelationType, TupleType};
use crate::values::{Relation, Tuple};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::hash::{Hash, Hasher};
use proptest::prelude::*;

fn hash_of<T: Hash>(value: &T) -> u64 {
    let mut hasher = new_hasher();
    value.hash(&mut hasher);
    hasher.finish()
}

/// A relation with the single attribute `name` holding `values`.
fn unary_relation(name: &str, scalar_type: ScalarType, values: Vec<ScalarValue>) -> Relation {
    let heading = TupleType::new().with_attribute(name, scalar_type);
    let shared = Arc::new(heading.clone());
    let tuples = values.into_iter().map(|value| {
        Tuple::new(shared.clone(), [(name.to_string(), value)])
            .expect("test values match the heading")
    });
    Relation::from_tuples(RelationType::new(heading), tuples).expect("tuples match the heading")
}

fn int_relation(name: &str, values: &[i64]) -> ScalarValue {
    ScalarValue::Relation(unary_relation(
        name,
        ScalarType::Int,
        values.iter().copied().map(ScalarValue::Int).collect(),
    ))
}

// ---------------------------------------------------------------------------
// Deterministic regressions
// ---------------------------------------------------------------------------

#[test]
fn different_relations_of_same_cardinality_and_degree_are_not_cmp_equal() {
    let a = int_relation("x", &[1]);
    let b = int_relation("x", &[2]);
    assert_ne!(a, b);
    assert_ne!(a.cmp(&b), Ordering::Equal);
    assert_eq!(a.cmp(&b), b.cmp(&a).reverse());
}

#[test]
fn relations_with_different_headings_of_same_degree_are_not_cmp_equal() {
    let a = int_relation("x", &[1]);
    let b = int_relation("y", &[1]);
    assert_ne!(a, b);
    assert_ne!(a.cmp(&b), Ordering::Equal);
}

#[test]
fn equal_relations_are_cmp_equal_regardless_of_insertion_order() {
    let a = int_relation("x", &[1, 2, 3]);
    let b = int_relation("x", &[3, 1, 2]);
    assert_eq!(a, b);
    assert_eq!(a.cmp(&b), Ordering::Equal);
    assert_eq!(hash_of(&a), hash_of(&b));
}

#[test]
fn relation_order_refines_cardinality_order() {
    // The previous (cardinality, degree) order is kept as the primary key.
    let small = int_relation("x", &[9]);
    let large = int_relation("x", &[1, 2]);
    assert_eq!(small.cmp(&large), Ordering::Less);
}

#[test]
fn btree_set_keeps_distinct_relation_values() {
    // `insert` (not `collect`, which dedups with `==`) relies on `Ord` alone.
    let mut set = alloc::collections::BTreeSet::new();
    assert!(set.insert(int_relation("x", &[1])));
    assert!(set.insert(int_relation("x", &[2])));
    assert!(set.insert(int_relation("y", &[1])));
    assert!(!set.insert(int_relation("x", &[1])));
    assert_eq!(set.len(), 3);
}

// ---------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------

/// Floats biased towards the cases bit equality and the NaN rule care about.
fn float_strategy() -> impl Strategy<Value = f64> {
    prop_oneof![
        any::<f64>(),
        Just(0.0),
        Just(-0.0),
        Just(1.0),
        Just(f64::NAN),
        Just(-f64::NAN),
        Just(f64::INFINITY),
        Just(f64::NEG_INFINITY),
        // NaNs with arbitrary sign and payload (mantissa forced non-zero).
        (any::<bool>(), any::<u64>()).prop_map(|(negative, payload)| {
            let sign = if negative { 1u64 << 63 } else { 0 };
            f64::from_bits(sign | 0x7ff0_0000_0000_0000 | (payload & 0x000f_ffff_ffff_ffff) | 1)
        }),
    ]
}

fn user_defined_type(name: &str, representation: ScalarType) -> ScalarType {
    ScalarType::user_defined(name, representation)
}

/// Non-relation values over small domains, so equal pairs are common.
fn leaf_strategy() -> impl Strategy<Value = ScalarValue> {
    prop_oneof![
        (-2i64..=2).prop_map(ScalarValue::Int),
        float_strategy().prop_map(ScalarValue::Float),
        prop::sample::select(vec!["", "a", "b"]).prop_map(|s| ScalarValue::String(s.into())),
        any::<bool>().prop_map(ScalarValue::Bool),
        prop::collection::vec(0u8..2, 0..3).prop_map(ScalarValue::Bytes),
        // Same representation, different type names: distinct types.
        (
            prop::sample::select(vec!["WidgetId", "SupplierId"]),
            -1i64..=1
        )
            .prop_map(|(name, v)| {
                ScalarValue::select(
                    &user_defined_type(name, ScalarType::Int),
                    ScalarValue::Int(v),
                )
                .expect("Int representation")
            }),
        // Nested user-defined over a float (NaN payloads inside a POSSREP).
        float_strategy().prop_map(|f| {
            let inner = user_defined_type("Celsius", ScalarType::Float);
            let outer = user_defined_type("Reading", inner.clone());
            let inner_value =
                ScalarValue::select(&inner, ScalarValue::Float(f)).expect("Float representation");
            ScalarValue::select(&outer, inner_value).expect("Celsius representation")
        }),
    ]
}

/// Flat relation values: headings `{x: Int}`, `{y: Int}` (same degree,
/// different name) and `{f: Float}` (NaN / signed-zero tuples).
fn flat_relation_strategy() -> impl Strategy<Value = Relation> {
    prop_oneof![
        prop::collection::vec(-1i64..=1, 0..3).prop_map(|xs| {
            unary_relation(
                "x",
                ScalarType::Int,
                xs.into_iter().map(ScalarValue::Int).collect(),
            )
        }),
        prop::collection::vec(-1i64..=1, 0..3).prop_map(|ys| {
            unary_relation(
                "y",
                ScalarType::Int,
                ys.into_iter().map(ScalarValue::Int).collect(),
            )
        }),
        prop::collection::vec(float_strategy(), 0..3).prop_map(|fs| {
            unary_relation(
                "f",
                ScalarType::Float,
                fs.into_iter().map(ScalarValue::Float).collect(),
            )
        }),
    ]
}

/// Relation-valued attributes: `{r: RELATION {x: Int}}`.
fn nested_relation_strategy() -> impl Strategy<Value = Relation> {
    let inner_type = RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int));
    prop::collection::vec(prop::collection::vec(-1i64..=1, 0..3), 0..3).prop_map(move |bodies| {
        let values = bodies
            .into_iter()
            .map(|xs| {
                ScalarValue::Relation(unary_relation(
                    "x",
                    ScalarType::Int,
                    xs.into_iter().map(ScalarValue::Int).collect(),
                ))
            })
            .collect();
        unary_relation(
            "r",
            ScalarType::Relation(Box::new(inner_type.clone())),
            values,
        )
    })
}

fn value_strategy() -> impl Strategy<Value = ScalarValue> {
    prop_oneof![
        3 => leaf_strategy(),
        2 => flat_relation_strategy().prop_map(ScalarValue::Relation),
        1 => nested_relation_strategy().prop_map(ScalarValue::Relation),
    ]
}

proptest! {
    #[test]
    fn prop_cmp_equal_iff_eq(a in value_strategy(), b in value_strategy()) {
        prop_assert_eq!(a.cmp(&b) == Ordering::Equal, a == b);
        prop_assert_eq!(a.cmp(&a), Ordering::Equal);
    }

    #[test]
    fn prop_cmp_antisymmetric(a in value_strategy(), b in value_strategy()) {
        prop_assert_eq!(a.cmp(&b), b.cmp(&a).reverse());
        prop_assert_eq!(a.partial_cmp(&b), Some(a.cmp(&b)));
    }

    #[test]
    fn prop_cmp_transitive(a in value_strategy(), b in value_strategy(), c in value_strategy()) {
        if a <= b && b <= c {
            prop_assert!(a <= c);
        }
        if a < b && b < c {
            prop_assert!(a < c);
        }
    }

    /// Transitivity exercised densely: after sorting, every pair is ordered.
    #[test]
    fn prop_sorted_sequence_is_pairwise_ordered(mut values in prop::collection::vec(value_strategy(), 0..8)) {
        values.sort();
        for i in 0..values.len() {
            for j in i..values.len() {
                prop_assert_ne!(values[i].cmp(&values[j]), Ordering::Greater);
            }
        }
    }

    #[test]
    fn prop_eq_implies_equal_hash(a in value_strategy(), b in value_strategy()) {
        if a == b {
            prop_assert_eq!(hash_of(&a), hash_of(&b));
        }
        let copy = a.clone();
        prop_assert_eq!(hash_of(&a), hash_of(&copy));
    }
}
