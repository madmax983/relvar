use relvar_core::values::Relation;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::tuple;
use relvar::experimental::image;

#[test]
fn test_image_save_span_overflow() {
    let heading = TupleType::new()
        .with_attribute("x", ScalarType::Int)
        .with_attribute("y", ScalarType::Int)
        .with_attribute("r", ScalarType::Int)
        .with_attribute("g", ScalarType::Int)
        .with_attribute("b", ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    // Create a scenario where max_x - min_x > i64::MAX
    // If not protected, max_x.saturating_sub(min_x) would cap at i64::MAX
    // but without overflow protection on pixel calculations, this could
    // result in incorrect bounds or overflow panics.
    relation.insert(tuple! { x: i64::MAX, y: i64::MAX, r: 0i64, g: 0i64, b: 0i64 }).unwrap();
    relation.insert(tuple! { x: i64::MIN, y: i64::MIN, r: 0i64, g: 0i64, b: 0i64 }).unwrap();

    let (w, h, data) = image::save(&relation);

    // It should safely return an empty image due to exceeding MAX_IMAGE_PIXELS
    assert_eq!(w, 0);
    assert_eq!(h, 0);
    assert_eq!(data.len(), 0);
}
