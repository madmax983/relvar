//! Relational Image Processing (RIP)
//!
//! This module demonstrates how image processing algorithms (like convolution)
//! can be implemented using pure relational algebra.
//!
//! An image is modeled as a relation `(x: Int, y: Int, r: Int, g: Int, b: Int)`.
//! Filters are applied by joining the image with itself (or shifted versions)
//! and aggregating the results.

use relvar_core::algebra::Aggregation;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

/// A kernel tap definition for convolution.
///
/// Represents a weight at a specific offset `(dx, dy)`.
#[derive(Debug, Clone, Copy)]
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// // Note: This is a placeholder example
/// ```
pub struct KernelTap {
    /// Horizontal offset from the center pixel.
    pub dx: i64,
    /// Vertical offset from the center pixel.
    pub dy: i64,
    /// Weight of this tap (integer for fixed-point arithmetic).
    pub weight: i64,
}

/// Converts a raw RGB buffer into a relation.
///
/// The resulting relation has heading `(x: Int, y: Int, r: Int, g: Int, b: Int)`.
///
/// # Arguments
///
/// * `width` - Image width
/// * `height` - Image height
/// * `data` - RGB pixel data (flat buffer: r, g, b, r, g, b, ...)
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// // Note: This is a placeholder example
/// ```
pub fn load(width: usize, height: usize, data: &[u8]) -> Relation {
    let heading = TupleType::new()
        .with_attribute("x", ScalarType::Int)
        .with_attribute("y", ScalarType::Int)
        .with_attribute("r", ScalarType::Int)
        .with_attribute("g", ScalarType::Int)
        .with_attribute("b", ScalarType::Int);

    if width.saturating_mul(height) > 10_000_000 {
        return Relation::new(RelationType::new(heading));
    }

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 3;
            if idx + 2 < data.len() {
                let r = data[idx] as i64;
                let g = data[idx + 1] as i64;
                let b = data[idx + 2] as i64;

                // We can use insert here. For large images, batch loading would be better.
                relation
                    .insert(tuple! {
                        x: x as i64,
                        y: y as i64,
                        r: r,
                        g: g,
                        b: b
                    })
                    .unwrap();
            }
        }
    }

    relation
}

/// Converts a relation back into a raw RGB buffer.
///
/// # Returns
///
/// A tuple `(width, height, data)`.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// // Note: This is a placeholder example
/// ```
pub fn save(relation: &Relation) -> (usize, usize, Vec<u8>) {
    if relation.is_empty() {
        return (0, 0, Vec::new());
    }

    // 1. Find dimensions
    let mut min_x = i64::MAX;
    let mut max_x = i64::MIN;
    let mut min_y = i64::MAX;
    let mut max_y = i64::MIN;

    for tuple in relation.tuples() {
        let x = tuple.get_typed::<i64>("x").unwrap_or(0);
        let y = tuple.get_typed::<i64>("y").unwrap_or(0);

        if x < min_x {
            min_x = x;
        }
        if x > max_x {
            max_x = x;
        }
        if y < min_y {
            min_y = y;
        }
        if y > max_y {
            max_y = y;
        }
    }

    let width = (max_x.saturating_sub(min_x).saturating_add(1)) as usize;
    let height = (max_y.saturating_sub(min_y).saturating_add(1)) as usize;

    if width.saturating_mul(height) > 10_000_000 {
        return (0, 0, Vec::new());
    }

    let mut data = vec![0u8; width * height * 3];

    for tuple in relation.tuples() {
        let x = tuple.get_typed::<i64>("x").unwrap_or(0);
        let y = tuple.get_typed::<i64>("y").unwrap_or(0);
        let r = tuple.get_typed::<i64>("r").unwrap_or(0).clamp(0, 255) as u8;
        let g = tuple.get_typed::<i64>("g").unwrap_or(0).clamp(0, 255) as u8;
        let b = tuple.get_typed::<i64>("b").unwrap_or(0).clamp(0, 255) as u8;

        let img_x = (x - min_x) as usize;
        let img_y = (y - min_y) as usize;

        if img_x < width && img_y < height {
            let idx = (img_y * width + img_x) * 3;
            data[idx] = r;
            data[idx + 1] = g;
            data[idx + 2] = b;
        }
    }

    (width, height, data)
}

/// Applies a convolution kernel to the image relation.
///
/// This demonstrates the power of Relational Algebra:
/// Convolution is implemented as a set of Shifts (Extend), Unions, and Aggregations (Summarize).
///
/// # Algorithm
///
/// 1. For each kernel tap `(dx, dy, w)`:
///    - Create a "shifted" view of the image where `target_x = x - dx`, `target_y = y - dy`.
///    - Scale pixel values by weight `w`.
///    - Tag with a unique kernel index to preserve provenance (set semantics would merge identical values).
/// 2. Union all shifted views.
/// 3. Summarize (Group By) `target_x, target_y`.
/// 4. Sum the weighted values.
/// 5. Normalize by total weight.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// // Note: This is a placeholder example
/// ```
pub fn apply_kernel(relation: &Relation, kernel: &[KernelTap]) -> Relation {
    if kernel.is_empty() {
        return relation.clone();
    }

    let total_weight: i64 = kernel.iter().map(|k| k.weight).sum();
    if total_weight == 0 {
        return relation.clone(); // Avoid division by zero
    }

    let contributions = compute_kernel_contributions(relation, kernel);
    let unioned = union_contributions(&contributions);

    summarize_and_normalize(&unioned, total_weight)
}

fn compute_kernel_contributions(relation: &Relation, kernel: &[KernelTap]) -> Vec<Relation> {
    let mut contributions = Vec::with_capacity(kernel.len());

    for (i, tap) in kernel.iter().enumerate() {
        let with_coords = extend_target_coordinates(relation, tap.dx, tap.dy);
        let with_weights = extend_weighted_colors(&with_coords, tap.weight);
        let renamed = extend_kernel_idx_and_project(&with_weights, i as i64);

        contributions.push(renamed);
    }

    contributions
}

fn extend_target_coordinates(relation: &Relation, dx: i64, dy: i64) -> Relation {
    relation
        .extend("tx", ScalarType::Int, move |t| {
            let x = t.get_typed::<i64>("x").unwrap();
            ScalarValue::Int(x + dx)
        })
        .unwrap()
        .extend("ty", ScalarType::Int, move |t| {
            let y = t.get_typed::<i64>("y").unwrap();
            ScalarValue::Int(y + dy)
        })
        .unwrap()
}

fn extend_weighted_colors(relation: &Relation, weight: i64) -> Relation {
    relation
        .extend("wr", ScalarType::Int, move |t| {
            let v = t.get_typed::<i64>("r").unwrap();
            ScalarValue::Int(v * weight)
        })
        .unwrap()
        .extend("wg", ScalarType::Int, move |t| {
            let v = t.get_typed::<i64>("g").unwrap();
            ScalarValue::Int(v * weight)
        })
        .unwrap()
        .extend("wb", ScalarType::Int, move |t| {
            let v = t.get_typed::<i64>("b").unwrap();
            ScalarValue::Int(v * weight)
        })
        .unwrap()
}

fn extend_kernel_idx_and_project(relation: &Relation, k_idx: i64) -> Relation {
    let with_idx = relation
        .extend("k_idx", ScalarType::Int, move |_| ScalarValue::Int(k_idx))
        .unwrap();

    let rename_map = vec![
        ("tx", "x"),
        ("ty", "y"),
        ("wr", "r"),
        ("wg", "g"),
        ("wb", "b"),
    ];

    with_idx
        .project(&["tx", "ty", "wr", "wg", "wb", "k_idx"])
        .rename(&rename_map)
}

fn union_contributions(contributions: &[Relation]) -> Relation {
    // Step 2: Union
    // Start with the first contribution
    let mut unioned = contributions[0].clone();
    for other in contributions.iter().skip(1) {
        unioned = unioned.union(other).unwrap();
    }
    unioned
}

fn summarize_and_normalize(unioned: &Relation, total_weight: i64) -> Relation {
    // Step 3: Summarize
    // Group by (x, y)
    // Sum (r, g, b) -> (sum_r, sum_g, sum_b)
    let summarized = unioned
        .summarize(
            &["x", "y"],
            &[
                Aggregation::sum("sum_r", "r"),
                Aggregation::sum("sum_g", "g"),
                Aggregation::sum("sum_b", "b"),
            ],
        )
        .unwrap();

    // Step 4: Normalize
    // Extend with final values: sum / total_weight
    let normalized = summarized
        .extend("final_r", ScalarType::Int, move |t| {
            let s = t.get_typed::<i64>("sum_r").unwrap();
            ScalarValue::Int(s / total_weight)
        })
        .unwrap()
        .extend("final_g", ScalarType::Int, move |t| {
            let s = t.get_typed::<i64>("sum_g").unwrap();
            ScalarValue::Int(s / total_weight)
        })
        .unwrap()
        .extend("final_b", ScalarType::Int, move |t| {
            let s = t.get_typed::<i64>("sum_b").unwrap();
            ScalarValue::Int(s / total_weight)
        })
        .unwrap();

    // Step 5: Final Project and Rename
    // Keep (x, y, final_r, final_g, final_b)
    // Rename final_r->r, etc.
    let rename_map = vec![("final_r", "r"), ("final_g", "g"), ("final_b", "b")];

    normalized
        .project(&["x", "y", "final_r", "final_g", "final_b"])
        .rename(&rename_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_save_identity() {
        let width = 2;
        let height = 2;
        // 2x2 image: Red, Green, Blue, White
        let data: Vec<u8> = vec![
            255, 0, 0, // Red
            0, 255, 0, // Green
            0, 0, 255, // Blue
            255, 255, 255, // White
        ];

        let relation = load(width, height, &data);
        assert_eq!(relation.cardinality(), 4);

        let (w, h, saved_data) = save(&relation);
        assert_eq!(w, width);
        assert_eq!(h, height);
        assert_eq!(saved_data, data);
    }

    #[test]
    fn test_box_blur() {
        // 3x3 image with a center white pixel, everything else black
        let width = 3;
        let height = 3;
        let mut data = vec![0u8; width * height * 3];
        // Set center (1, 1) to White
        let center_idx = (width + 1) * 3;
        data[center_idx] = 255;
        data[center_idx + 1] = 255;
        data[center_idx + 2] = 255;

        let relation = load(width, height, &data);

        // 3x3 Box Blur Kernel (all 1s)
        // Sum of weights = 9.
        // Center pixel contributes 255 to itself and all neighbors.
        // After normalization, every pixel in the 3x3 grid should be 255 / 9 = 28.
        let mut kernel = Vec::new();
        for dy in -1..=1 {
            for dx in -1..=1 {
                kernel.push(KernelTap { dx, dy, weight: 1 });
            }
        }

        let blurred = apply_kernel(&relation, &kernel);

        // Verify result
        // Only check pixels that are within the original 3x3 bounds.
        // The convolution expands the domain, so we filter.
        let relevant_tuples = blurred.tuples().filter(|t| {
            let x = t.get_typed::<i64>("x").unwrap();
            let y = t.get_typed::<i64>("y").unwrap();
            x >= 0 && x < width as i64 && y >= 0 && y < height as i64
        });

        let mut count = 0;
        for tuple in relevant_tuples {
            count += 1;
            let r = tuple.get_typed::<i64>("r").unwrap();
            assert_eq!(
                r,
                28,
                "Pixel at ({}, {}) should be 255/9 = 28",
                tuple.get_typed::<i64>("x").unwrap(),
                tuple.get_typed::<i64>("y").unwrap()
            );
        }
        assert_eq!(count, 9, "Should have 9 pixels in the 3x3 grid");
    }

    #[test]
    fn test_shift() {
        // 2x2 image: Top-Left is White, rest Black
        let width = 2;
        let height = 2;
        let mut data = vec![0u8; width * height * 3];
        data[0] = 255;
        data[1] = 255;
        data[2] = 255;

        let relation = load(width, height, &data);

        // Kernel: Shift Right by 1 (dx=1, dy=0, w=1)
        let kernel = vec![KernelTap {
            dx: 1,
            dy: 0,
            weight: 1,
        }];

        let shifted = apply_kernel(&relation, &kernel);

        // Expected: (1, 0) should be White. (0, 0) should be Black (or gone if no contribution).
        // Our logic preserves all contributions.
        // Original (0,0) moves to (1,0).

        // Let's check (1, 0)
        let found = shifted.tuples().find(|t| {
            t.get_typed::<i64>("x").unwrap() == 1 && t.get_typed::<i64>("y").unwrap() == 0
        });

        assert!(found.is_some());
        let t = found.unwrap();
        assert_eq!(t.get_typed::<i64>("r").unwrap(), 255);
    }
}
