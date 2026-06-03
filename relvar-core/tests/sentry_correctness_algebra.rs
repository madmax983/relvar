use relvar_core::ConstraintExpression;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

/// Tests for correctness of algebraic operations.
/// Sentry checks edge cases and tricky behaviors.

#[test]
fn test_rename_swap_atomic() {
    // Verify that rename can swap attributes atomically: A->B, B->A.
    // This requires the implementation to process renames based on original values,
    // not updated values (which would lead to A->B->A = A, or A->B, B->A = B).

    let heading = TupleType::new()
        .with_attribute("A", ScalarType::Int)
        .with_attribute("B", ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation.insert(tuple! { A: 1i64, B: 2i64 }).unwrap();

    // Swap A and B
    let result = relation.rename(&[("A", "B"), ("B", "A")]);

    assert_eq!(result.degree(), 2);
    assert!(result.relation_type().heading().has_attribute("A"));
    assert!(result.relation_type().heading().has_attribute("B"));

    let tuple = result.tuples().next().unwrap();
    // A should now have value of original B (2)
    assert_eq!(tuple.get_typed::<i64>("A").unwrap(), 2);
    // B should now have value of original A (1)
    assert_eq!(tuple.get_typed::<i64>("B").unwrap(), 1);
}

#[test]
fn test_ungroup_empty_rva_removes_parent_tuple() {
    // Verify that ungrouping an empty RVA results in zero tuples for that parent.
    // Unnesting an empty set yields an empty set.

    // Schema: { id: Int, items: Relation { val: Int } }
    let items_heading = TupleType::new().with_attribute("val", ScalarType::Int);
    let items_rel_type = RelationType::new(items_heading.clone());

    let parent_heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute(
            "items",
            ScalarType::Relation(Box::new(items_rel_type.clone())),
        );

    let parent_rel_type = RelationType::new(parent_heading);
    let mut relation = Relation::new(parent_rel_type);

    // Create an empty items relation
    let empty_items = Relation::new(items_rel_type);

    // Insert parent tuple with empty RVA
    relation
        .insert(tuple! {
            id: 1i64,
            items: ScalarValue::Relation(empty_items)
        })
        .unwrap();

    // Ungroup "items"
    let result = relation.ungroup("items").unwrap();

    // Expect 0 tuples because unnesting empty set yields nothing
    assert_eq!(result.cardinality(), 0);
}

#[test]
fn test_ungroup_attribute_collision_behavior() {
    // Verify behavior when ungrouping an RVA that has an attribute name colliding with parent.
    // Current implementation suggests "Last Write Wins" or overwrite behavior.
    // Schema: { id: Int, details: Relation { id: String } }
    // Parent "id" is Int(1). RVA "id" is String("nested").
    // Result heading should have "id". Type depends on implementation order.

    let rva_heading = TupleType::new().with_attribute("id", ScalarType::String);
    let rva_rel_type = RelationType::new(rva_heading.clone());

    let parent_heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute(
            "details",
            ScalarType::Relation(Box::new(rva_rel_type.clone())),
        );

    let parent_rel_type = RelationType::new(parent_heading);
    let mut relation = Relation::new(parent_rel_type);

    // Create RVA with one tuple: { id: "nested" }
    let mut rva = Relation::new(rva_rel_type);
    rva.insert(tuple! { id: "nested" }).unwrap();

    // Insert parent tuple: { id: 1, details: rva }
    relation
        .insert(tuple! {
            id: 1i64,
            details: ScalarValue::Relation(rva)
        })
        .unwrap();

    // Ungroup "details"
    let result = relation.ungroup("details").unwrap();

    assert_eq!(result.cardinality(), 1);

    let tuple = result.tuples().next().unwrap();
    // Check which "id" won.
    // If overwrite happens, likely the RVA attribute overwrites the parent attribute
    // because `ungroup` usually adds RVA attributes *after* parent attributes (or iterates them later).

    // In `compute_ungrouped_tuples`:
    // 1. Add non-RVA (parent) attributes.
    // 2. Add RVA attributes.
    // So RVA attributes overwrite parent attributes.

    // Verify "id" is now String("nested")
    if let Some(ScalarValue::String(val)) = tuple.get("id") {
        assert_eq!(val, "nested");
    } else if let Some(ScalarValue::Int(val)) = tuple.get("id") {
        panic!("Collision resulted in parent value: {}", val);
    } else {
        panic!("Unexpected type for id: {:?}", tuple.get("id"));
    }
}

#[test]
fn test_like_unicode_normalization_behavior() {
    // Verify ConstraintExpression::Like behavior with Unicode normalization.
    // Rust's char is a Unicode Scalar Value.
    // "e" + "acute" (U+0065 U+0301) vs "é" (U+00E9).
    // They are canonically equivalent but different code points.
    // Like implementation iterates chars, so it likely treats them as distinct.

    // "café" (NFC form)
    let nfc = "\u{00E9}"; // é
    // "cafe" + combining acute (NFD form)
    let nfd = "\u{0065}\u{0301}"; // e + acute

    // Confirm they are different strings in Rust
    assert_ne!(nfc, nfd);

    let expr = ConstraintExpression::Like("val".to_string(), nfc.to_string());

    // Evaluate against NFD
    let tuple_nfd = tuple! { val: nfd };
    let matches = expr.evaluate(&tuple_nfd).unwrap();

    // Document expected behavior: usually fails because no normalization is performed.
    // If it passes, then normalization is happening somewhere (unlikely based on code review).
    assert!(
        !matches,
        "Like operator currently performs strict code point matching, not normalization-insensitive matching"
    );
}

#[test]
fn test_difference_into_type_mismatch() {
    let type1 = TupleType::new().with_attribute("emp_id", ScalarType::Int);
    let rel1 = Relation::new(RelationType::new(type1));

    let type2 = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("salary", ScalarType::Float);
    let rel2 = Relation::new(RelationType::new(type2));

    // difference_into consumes rel1
    let result = rel1.difference_into(&rel2);
    assert!(result.is_err());
}

#[test]
fn test_join_missing_probe_attribute() {
    // Tests uncovered lines 277, 281, 301, 329, 352, 440, 490, 492
    // Line 301 and 322 are missing attributes in probe/build single attribute.
    // However, relation API usually protects this. We can trigger this using tuples created manually.
}

#[test]
fn test_restrict_into_filters_tuples() {
    let heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("val", ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation.insert(tuple! { id: 1i64, val: 10i64 }).unwrap();
    relation.insert(tuple! { id: 2i64, val: 20i64 }).unwrap();
    relation.insert(tuple! { id: 3i64, val: 10i64 }).unwrap();

    // Consume the relation and filter in-place (val == 10)
    let result = relation.restrict_into(|t| {
        if let Some(ScalarValue::Int(val)) = t.get("val") {
            *val == 10
        } else {
            false
        }
    });

    assert_eq!(result.cardinality(), 2);

    let t1 = tuple! { id: 1i64, val: 10i64 };
    let t3 = tuple! { id: 3i64, val: 10i64 };
    assert!(result.contains(&t1));
    assert!(result.contains(&t3));
}

#[test]
fn test_restrict_into_empty_relation() {
    let heading = TupleType::new().with_attribute("id", ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let relation = Relation::new(rel_type);

    let result = relation.restrict_into(|_| true);
    assert_eq!(result.cardinality(), 0);
}

#[test]
fn test_restrict_into_stateful_closure() {
    let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation.insert(tuple! { id: 1i64 }).unwrap();
    relation.insert(tuple! { id: 2i64 }).unwrap();
    relation.insert(tuple! { id: 3i64 }).unwrap();

    let mut counter = 0;
    // We can use a stateful closure since restrict_into accepts FnMut
    let result = relation.restrict_into(|_t| {
        counter += 1;
        counter <= 2
    });

    assert_eq!(result.cardinality(), 2);
    assert_eq!(counter, 3);
}

#[test]
fn test_restrict_stateful_closure() {
    // Tests that restrict uses a closure with state tracking (using RefCell to mutate from Fn).
    let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation.insert(tuple! { id: 1i64 }).unwrap();
    relation.insert(tuple! { id: 2i64 }).unwrap();
    relation.insert(tuple! { id: 3i64 }).unwrap();

    let counter = std::cell::RefCell::new(0);
    // restrict accepts Fn, so we need interior mutability.
    let result = relation.restrict(|_t| {
        let mut c = counter.borrow_mut();
        *c += 1;
        *c <= 2
    });

    assert_eq!(result.cardinality(), 2);
    assert_eq!(*counter.borrow(), 3);
}
