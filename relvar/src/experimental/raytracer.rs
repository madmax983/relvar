//! Relational Raytracer
//!
//! This module implements a basic raytracer using pure relational algebra.
//! It demonstrates that a 3D scene can be modeled as a relation of objects
//! (spheres), rays can be modeled as a relation of vectors, and rendering
//! is simply an intersection join, a distance extend, and an aggregation to
//! find the closest hit.
//!
//! # Example
//!
//! ```
//! use relvar::experimental::raytracer::Scene;
//! use relvar_core::values::ScalarValue;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut scene = Scene::new();
//!
//!     // Add a sphere at (0, 0, 5) with radius 1.0 and color (255, 0, 0)
//!     scene.add_sphere(0.0, 0.0, 5.0, 1.0, 255, 0, 0)?;
//!
//!     // Add a sphere at (2, 0, 5) with radius 1.0 and color (0, 255, 0)
//!     scene.add_sphere(2.0, 0.0, 5.0, 1.0, 0, 255, 0)?;
//!
//!     // Render a 10x10 image
//!     let pixels = scene.render(10, 10)?;
//!
//!     // The result is a relation of (x, y, r, g, b)
//!     assert_eq!(pixels.cardinality(), 100);
//!
//!     Ok(())
//! }
//! ```
//!
//! # How it works
//!
//! 1. **Scene Generation**: Spheres are stored in a relation: `(id, cx, cy, cz, radius, r, g, b)`.
//! 2. **Ray Generation**: For each pixel (x, y), we generate a ray origin `(ox, oy, oz)` and direction `(dx, dy, dz)`.
//!    These are stored in a `rays` relation: `(x, y, ox, oy, oz, dx, dy, dz)`.
//! 3. **Intersection**: We perform a Cartesian product (via natural join on a dummy attribute) of `rays` and `spheres`.
//! 4. **Distance Calculation**: We `extend` the relation to calculate the distance `t` to intersection using the quadratic formula.
//! 5. **Filtering**: We `restrict` to intersections where `t > 0.0`.
//! 6. **Visibility**: We `summarize` by `(x, y)` finding the minimum `t` to get the closest hit for each pixel.
//! 7. **Color Mapping**: We join the closest hits back with the calculated intersections to get the sphere's color.
//! 8. **Background**: Pixels with no hits are given a background color.

use relvar_core::algebra::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

/// A 3D scene containing spheres to be rendered.
pub struct Scene {
    spheres: Relation,
    next_id: i64,
}

impl Scene {
    /// Creates a new, empty scene.
    pub fn new() -> Self {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("cx", ScalarType::Float)
            .with_attribute("cy", ScalarType::Float)
            .with_attribute("cz", ScalarType::Float)
            .with_attribute("radius", ScalarType::Float)
            .with_attribute("r", ScalarType::Int)
            .with_attribute("g", ScalarType::Int)
            .with_attribute("b", ScalarType::Int);

        Self {
            spheres: Relation::new(RelationType::new(heading)),
            next_id: 1,
        }
    }

    /// Adds a sphere to the scene.
    ///
    /// # Arguments
    ///
    /// * `cx`, `cy`, `cz` - Center coordinates of the sphere.
    /// * `radius` - Radius of the sphere.
    /// * `r`, `g`, `b` - Color of the sphere (0-255).
    #[allow(clippy::too_many_arguments)]
    pub fn add_sphere(
        &mut self,
        cx: f64,
        cy: f64,
        cz: f64,
        radius: f64,
        r: i64,
        g: i64,
        b: i64,
    ) -> Result<(), DatabaseError> {
        let id = self.next_id;
        self.next_id += 1;

        self.spheres.insert(tuple! {
            id: id,
            cx: cx,
            cy: cy,
            cz: cz,
            radius: radius,
            r: r,
            g: g,
            b: b
        })?;

        Ok(())
    }

    /// Renders the scene to a relation of pixels.
    ///
    /// Returns a relation with heading `(x, y, r, g, b)`.
    pub fn render(&self, width: i64, height: i64) -> Result<Relation, DatabaseError> {
        // 1. Generate Rays Relation: (x, y, ox, oy, oz, dx, dy, dz, dummy_join)
        let ray_heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int)
            .with_attribute("ox", ScalarType::Float)
            .with_attribute("oy", ScalarType::Float)
            .with_attribute("oz", ScalarType::Float)
            .with_attribute("dx", ScalarType::Float)
            .with_attribute("dy", ScalarType::Float)
            .with_attribute("dz", ScalarType::Float)
            .with_attribute("dummy_join", ScalarType::Int);

        let mut rays = Relation::new(RelationType::new(ray_heading));

        // Camera setup (simple perspective)
        let origin_x = 0.0;
        let origin_y = 0.0;
        let origin_z = 0.0;

        let aspect_ratio = width as f64 / height as f64;
        let fov = std::f64::consts::PI / 4.0; // 45 degrees
        let viewport_height = 2.0 * (fov / 2.0).tan();
        let viewport_width = aspect_ratio * viewport_height;

        for y in 0..height {
            for x in 0..width {
                // Map pixel to NDC (Normalized Device Coordinates) [-1, 1]
                // Note: y is inverted so +y is up in world space
                let ndc_x = (x as f64 + 0.5) / width as f64 * 2.0 - 1.0;
                let ndc_y = 1.0 - (y as f64 + 0.5) / height as f64 * 2.0;

                // Ray direction
                let dx = ndc_x * viewport_width / 2.0;
                let dy = ndc_y * viewport_height / 2.0;
                let dz = 1.0; // Looking down +Z

                // Normalize direction
                let len = (dx * dx + dy * dy + dz * dz).sqrt();
                let nx = dx / len;
                let ny = dy / len;
                let nz = dz / len;

                rays.insert(tuple! {
                    x: x,
                    y: y,
                    ox: origin_x,
                    oy: origin_y,
                    oz: origin_z,
                    dx: nx,
                    dy: ny,
                    dz: nz,
                    dummy_join: 1i64
                })?;
            }
        }

        // 2. Prepare Spheres for Cross Join
        // We use extend to add `dummy_join: 1` so natural join acts as a cross join.
        let spheres_prepared = self
            .spheres
            .extend("dummy_join", ScalarType::Int, |_| ScalarValue::Int(1))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Cross Join: All rays against all spheres
        let combinations = rays.join(&spheres_prepared)?;

        // 4. Calculate Intersections
        // Mathematical derivation:
        // Ray: P(t) = O + t*D
        // Sphere: (P - C) \cdot (P - C) = r^2
        // Subbing P(t) into Sphere equation gives:
        // (D \cdot D)*t^2 + 2*(D \cdot (O - C))*t + (O - C) \cdot (O - C) - r^2 = 0
        //
        // Let a = D \cdot D (which is 1 since D is normalized)
        // Let half_b = D \cdot (O - C)
        // Let c = (O - C) \cdot (O - C) - r^2
        // discriminant = half_b^2 - a*c
        let intersections = combinations
            .extend("t", ScalarType::Float, |tup| {
                let ox = tup.get_typed::<f64>("ox").unwrap_or(0.0);
                let oy = tup.get_typed::<f64>("oy").unwrap_or(0.0);
                let oz = tup.get_typed::<f64>("oz").unwrap_or(0.0);
                let dx = tup.get_typed::<f64>("dx").unwrap_or(0.0);
                let dy = tup.get_typed::<f64>("dy").unwrap_or(0.0);
                let dz = tup.get_typed::<f64>("dz").unwrap_or(0.0);
                let cx = tup.get_typed::<f64>("cx").unwrap_or(0.0);
                let cy = tup.get_typed::<f64>("cy").unwrap_or(0.0);
                let cz = tup.get_typed::<f64>("cz").unwrap_or(0.0);
                let radius = tup.get_typed::<f64>("radius").unwrap_or(0.0);

                let oc_x = ox - cx;
                let oc_y = oy - cy;
                let oc_z = oz - cz;

                let a = dx * dx + dy * dy + dz * dz; // Should be ~1.0
                let half_b = dx * oc_x + dy * oc_y + dz * oc_z;
                let c = (oc_x * oc_x + oc_y * oc_y + oc_z * oc_z) - radius * radius;

                let discriminant = half_b * half_b - a * c;

                if discriminant < 0.0 {
                    // No intersection, return negative distance
                    ScalarValue::Float(-1.0)
                } else {
                    // Two solutions, we want the smallest positive one
                    let sqrtd = discriminant.sqrt();
                    let t1 = (-half_b - sqrtd) / a;
                    let t2 = (-half_b + sqrtd) / a;

                    if t1 > 0.001 {
                        ScalarValue::Float(t1)
                    } else if t2 > 0.001 {
                        ScalarValue::Float(t2)
                    } else {
                        ScalarValue::Float(-1.0)
                    }
                }
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Filter Hits
        let hits = intersections.restrict(|t| {
            let dist = t.get_typed::<f64>("t").unwrap_or(-1.0);
            dist > 0.0
        });

        // 6. Find Closest Hits (Z-Buffer)
        // If there are hits, group by (x, y) and find min(t)
        // We use min on 't' to find the closest intersection per ray.
        let mut closest_hits_base = Relation::new(RelationType::new(
            TupleType::new()
                .with_attribute("x", ScalarType::Int)
                .with_attribute("y", ScalarType::Int)
                .with_attribute("min_t", ScalarType::Float),
        ));

        if hits.cardinality() > 0 {
            closest_hits_base = hits
                .summarize(
                    &["x", "y"],
                    &[Aggregation::min("min_t", "t", ScalarType::Float)],
                )
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        }

        // 7. Map Colors
        // Rename min_t to t in closest_hits to natural join it back with hits.
        // This acts like an INNER JOIN hits h ON h.x=c.x AND h.y=c.y AND h.t=c.min_t
        let closest_renamed = closest_hits_base.rename(&[("min_t", "t")]);

        // Note: Multiple spheres could theoretically be exactly at distance t.
        // Set semantics handle duplicates, but we could get multiple colors if
        // different spheres occupy the exact same spot. For a basic raytracer, this is fine.
        let visible_pixels = closest_renamed.join(&hits)?;

        // Project down to final image attributes
        let rendered_hits = visible_pixels.project(&["x", "y", "r", "g", "b"]);

        // 8. Background Color
        // Find rays that didn't hit anything: all rays MINUS rays that hit something
        let all_pixels = rays.project(&["x", "y"]);
        let hit_pixels = rendered_hits.project(&["x", "y"]);

        let background_pixels = all_pixels
            .difference(&hit_pixels)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Extend background with default sky blue color (135, 206, 235)
        let background_colored = background_pixels
            .extend("r", ScalarType::Int, |_| ScalarValue::Int(135))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("g", ScalarType::Int, |_| ScalarValue::Int(206))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("b", ScalarType::Int, |_| ScalarValue::Int(235))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Combine hits and background
        let final_image = rendered_hits
            .union(&background_colored)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(final_image)
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raytracer_empty_scene() {
        let scene = Scene::new();
        let pixels = scene.render(4, 4).unwrap();

        // 4x4 image = 16 pixels
        assert_eq!(pixels.cardinality(), 16);

        // All pixels should be background color (135, 206, 235)
        for tuple in pixels.tuples() {
            assert_eq!(tuple.get_typed::<i64>("r").unwrap(), 135);
            assert_eq!(tuple.get_typed::<i64>("g").unwrap(), 206);
            assert_eq!(tuple.get_typed::<i64>("b").unwrap(), 235);
        }
    }

    #[test]
    fn test_raytracer_with_sphere() {
        let mut scene = Scene::new();
        // Sphere directly in front of camera
        scene.add_sphere(0.0, 0.0, 5.0, 1.0, 255, 0, 0).unwrap();

        let pixels = scene.render(10, 10).unwrap();

        assert_eq!(pixels.cardinality(), 100);

        // The center pixel (4, 4) or (5, 5) should be red (255, 0, 0)
        let center_pixel = pixels
            .tuples()
            .find(|t| {
                t.get_typed::<i64>("x").unwrap() == 5 && t.get_typed::<i64>("y").unwrap() == 5
            })
            .unwrap();

        assert_eq!(center_pixel.get_typed::<i64>("r").unwrap(), 255);
        assert_eq!(center_pixel.get_typed::<i64>("g").unwrap(), 0);
        assert_eq!(center_pixel.get_typed::<i64>("b").unwrap(), 0);

        // A corner pixel (0, 0) should definitely be background
        let corner_pixel = pixels
            .tuples()
            .find(|t| {
                t.get_typed::<i64>("x").unwrap() == 0 && t.get_typed::<i64>("y").unwrap() == 0
            })
            .unwrap();

        assert_eq!(corner_pixel.get_typed::<i64>("r").unwrap(), 135);
        assert_eq!(corner_pixel.get_typed::<i64>("g").unwrap(), 206);
        assert_eq!(corner_pixel.get_typed::<i64>("b").unwrap(), 235);
    }

    #[test]
    fn test_raytracer_z_buffer() {
        let mut scene = Scene::new();
        // Red sphere further away
        scene.add_sphere(0.0, 0.0, 5.0, 2.0, 255, 0, 0).unwrap();
        // Green sphere closer, blocking the red one
        scene.add_sphere(0.0, 0.0, 2.0, 1.0, 0, 255, 0).unwrap();

        let pixels = scene.render(10, 10).unwrap();

        // The center pixel should be green because it's closer
        let center_pixel = pixels
            .tuples()
            .find(|t| {
                t.get_typed::<i64>("x").unwrap() == 5 && t.get_typed::<i64>("y").unwrap() == 5
            })
            .unwrap();

        assert_eq!(center_pixel.get_typed::<i64>("r").unwrap(), 0);
        assert_eq!(center_pixel.get_typed::<i64>("g").unwrap(), 255);
        assert_eq!(center_pixel.get_typed::<i64>("b").unwrap(), 0);
    }
}
