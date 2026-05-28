//! Relational Boids (Flocking) Simulation
//!
//! This module implements Craig Reynolds' Boids flocking algorithm using purely
//! relational algebra operations. It demonstrates that complex multi-agent
//! emergent behavior can be modeled entirely within a database engine.
//!
//! # Concept
//!
//! The simulation revolves around a single core relation:
//! - **boids**: `(id: Int, x: Float, y: Float, vx: Float, vy: Float)`
//!
//! During each tick, three main rules are evaluated declaratively:
//! - **Separation**: Steer to avoid crowding local flockmates.
//! - **Alignment**: Steer towards the average heading of local flockmates.
//! - **Cohesion**: Steer to move towards the average position of local flockmates.
//!
//! # Evaluation
//!
//! 1. **Cross Join**: Find all pairs of boids (`id1`, `id2`).
//! 2. **Restrict**: Filter pairs to those within the perception radius, excluding self (`id1 != id2`).
//! 3. **Extend**: Calculate distances and rule forces (separation, alignment, cohesion vectors).
//! 4. **Summarize**: Aggregate the forces per `id1` to compute a single adjustment vector per boid.
//! 5. **Join & Extend**: Apply the aggregated adjustments to the original boids relation to update velocities and positions.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue, Tuple},
};

/// A Relational Boids (Flocking) Simulation.
///
/// # Examples
///
/// ```
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::values::{Relation, ScalarValue, Tuple};
/// use relvar::experimental::boids::BoidSimulation;
///
/// let boid_type = TupleType::new()
///     .with_attribute("id", ScalarType::Int)
///     .with_attribute("x", ScalarType::Float)
///     .with_attribute("y", ScalarType::Float)
///     .with_attribute("vx", ScalarType::Float)
///     .with_attribute("vy", ScalarType::Float);
/// let mut boids = Relation::new(RelationType::new(boid_type));
///
/// let mut sim = BoidSimulation::new(boids, 10.0, 1.0, 1.0, 1.0, 1.0, 5.0, 100.0, 100.0);
/// sim.tick().unwrap();
/// ```
pub struct BoidSimulation {
    /// The state of the boids. Schema: `(id: Int, x: Float, y: Float, vx: Float, vy: Float)`
    pub boids: Relation,
    /// The maximum distance a boid can perceive other boids.
    pub perception_radius: f64,
    /// Separation rule weight.
    pub w_sep: f64,
    /// Alignment rule weight.
    pub w_ali: f64,
    /// Cohesion rule weight.
    pub w_coh: f64,
    /// Maximum acceleration per tick.
    pub max_accel: f64,
    /// Maximum velocity magnitude.
    pub max_speed: f64,
    /// Width of the toroidal space (wrap-around).
    pub width: f64,
    /// Height of the toroidal space (wrap-around).
    pub height: f64,
}

impl BoidSimulation {
    /// Creates a new Boid Simulation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        boids: Relation,
        perception_radius: f64,
        w_sep: f64,
        w_ali: f64,
        w_coh: f64,
        max_accel: f64,
        max_speed: f64,
        width: f64,
        height: f64,
    ) -> Self {
        Self {
            boids,
            perception_radius,
            w_sep,
            w_ali,
            w_coh,
            max_accel,
            max_speed,
            width,
            height,
        }
    }

    /// Advances the simulation by one tick using relational operations.
    pub fn tick(&mut self) -> Result<(), DatabaseError> {
        let perception_radius = self.perception_radius;

        // 1. Rename to create disjoint sets for the cross join
        let b1 = self.boids.rename(&[
            ("id", "id1"),
            ("x", "x1"),
            ("y", "y1"),
            ("vx", "vx1"),
            ("vy", "vy1"),
        ]);
        let b2 = self.boids.rename(&[
            ("id", "id2"),
            ("x", "x2"),
            ("y", "y2"),
            ("vx", "vx2"),
            ("vy", "vy2"),
        ]);

        // 2. Cross join all boids
        let pairs = b1.join(&b2)?;

        // 3. Extend with distance squared, and handle torus wrapping if needed, but for simplicity we'll just do Euclidean here for neighbors.
        let width = self.width;
        let height = self.height;

        let pairs_with_dist2 = pairs
            .extend("dist2", ScalarType::Float, move |t: &Tuple| {
                let x1 = t.get_typed::<f64>("x1").unwrap();
                let y1 = t.get_typed::<f64>("y1").unwrap();
                let x2 = t.get_typed::<f64>("x2").unwrap();
                let y2 = t.get_typed::<f64>("y2").unwrap();

                // Toroidal distance
                let mut dx = (x1 - x2).abs();
                if dx > width / 2.0 {
                    dx = width - dx;
                }
                let mut dy = (y1 - y2).abs();
                if dy > height / 2.0 {
                    dy = height - dy;
                }

                ScalarValue::Float(dx * dx + dy * dy)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Restrict to neighbors: id1 != id2 AND dist2 < r^2 AND dist2 > 0
        let r2 = perception_radius * perception_radius;
        let neighbors = pairs_with_dist2.restrict(move |t: &Tuple| {
            let id1 = t.get_typed::<i64>("id1").unwrap();
            let id2 = t.get_typed::<i64>("id2").unwrap();
            let dist2 = t.get_typed::<f64>("dist2").unwrap();
            id1 != id2 && dist2 > 0.0 && dist2 < r2
        });

        // 5. Extend with rule components: Separation, Alignment, Cohesion terms
        let w_sep = self.w_sep;
        let w_ali = self.w_ali;
        let w_coh = self.w_coh;

        // Separation: push away from neighbors, weighted by 1/dist. For simplicity we just use dx, dy (unnormalized)
        let with_forces = neighbors
            .extend("sep_x", ScalarType::Float, move |t: &Tuple| {
                let x1 = t.get_typed::<f64>("x1").unwrap();
                let x2 = t.get_typed::<f64>("x2").unwrap();
                let dist2 = t.get_typed::<f64>("dist2").unwrap();

                let mut dx = x1 - x2;
                if dx > width / 2.0 {
                    dx -= width;
                } else if dx < -width / 2.0 {
                    dx += width;
                }

                ScalarValue::Float(w_sep * (dx / dist2)) // Separation force inversely proportional to distance squared
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sep_y", ScalarType::Float, move |t: &Tuple| {
                let y1 = t.get_typed::<f64>("y1").unwrap();
                let y2 = t.get_typed::<f64>("y2").unwrap();
                let dist2 = t.get_typed::<f64>("dist2").unwrap();

                let mut dy = y1 - y2;
                if dy > height / 2.0 {
                    dy -= height;
                } else if dy < -height / 2.0 {
                    dy += height;
                }

                ScalarValue::Float(w_sep * (dy / dist2))
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            // Alignment: match velocities
            .extend("ali_x", ScalarType::Float, move |t: &Tuple| {
                let vx1 = t.get_typed::<f64>("vx1").unwrap();
                let vx2 = t.get_typed::<f64>("vx2").unwrap();
                ScalarValue::Float(w_ali * (vx2 - vx1))
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("ali_y", ScalarType::Float, move |t: &Tuple| {
                let vy1 = t.get_typed::<f64>("vy1").unwrap();
                let vy2 = t.get_typed::<f64>("vy2").unwrap();
                ScalarValue::Float(w_ali * (vy2 - vy1))
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            // Cohesion: move towards center of mass
            .extend("coh_x", ScalarType::Float, move |t: &Tuple| {
                let x1 = t.get_typed::<f64>("x1").unwrap();
                let x2 = t.get_typed::<f64>("x2").unwrap();
                let mut dx = x2 - x1;
                if dx > width / 2.0 {
                    dx -= width;
                } else if dx < -width / 2.0 {
                    dx += width;
                }
                ScalarValue::Float(w_coh * dx)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("coh_y", ScalarType::Float, move |t: &Tuple| {
                let y1 = t.get_typed::<f64>("y1").unwrap();
                let y2 = t.get_typed::<f64>("y2").unwrap();
                let mut dy = y2 - y1;
                if dy > height / 2.0 {
                    dy -= height;
                } else if dy < -height / 2.0 {
                    dy += height;
                }
                ScalarValue::Float(w_coh * dy)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 6. Summarize to get total force per boid (id1)
        // Note: For boids with no neighbors, they will drop out of the join, so we handle them later
        let forces = with_forces
            .summarize(
                &["id1"],
                &[
                    Aggregation::sum_float("sum_sep_x", "sep_x"),
                    Aggregation::sum_float("sum_sep_y", "sep_y"),
                    Aggregation::sum_float("sum_ali_x", "ali_x"),
                    Aggregation::sum_float("sum_ali_y", "ali_y"),
                    Aggregation::sum_float("sum_coh_x", "coh_x"),
                    Aggregation::sum_float("sum_coh_y", "coh_y"),
                    Aggregation::count("n_neighbors"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 7. Join back with original boids (outer join pattern: we need to handle boids with no neighbors)
        // We do this by projecting the forces to rename id1 -> id, and then we will union the updated ones and the unchanged ones.
        let forces_renamed = forces.rename(&[("id1", "id")]);

        let boids_with_neighbors = self.boids.join(&forces_renamed)?;

        // Find boids with NO neighbors
        let boids_ids_with_neighbors = boids_with_neighbors.project(&["id"]);
        let all_boids_ids = self.boids.project(&["id"]);
        let boids_ids_without_neighbors = all_boids_ids
            .difference(&boids_ids_with_neighbors)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let boids_without_neighbors = self.boids.join(&boids_ids_without_neighbors)?;

        // 8. Update velocities and positions for boids with neighbors
        let max_accel = self.max_accel;
        let max_speed = self.max_speed;

        let updated_boids_with_neighbors = boids_with_neighbors
            .extend("new_vx", ScalarType::Float, move |t: &Tuple| {
                let vx = t.get_typed::<f64>("vx").unwrap();
                let sep = t.get_typed::<f64>("sum_sep_x").unwrap();
                let ali = t.get_typed::<f64>("sum_ali_x").unwrap();
                let coh = t.get_typed::<f64>("sum_coh_x").unwrap();
                let n = t.get_typed::<i64>("n_neighbors").unwrap() as f64;

                let mut ax = sep + (ali / n) + (coh / n);
                // Clamp accel
                let a_mag = ax.abs();
                if a_mag > max_accel {
                    ax = ax / a_mag * max_accel;
                }

                let new_v = vx + ax;
                ScalarValue::Float(new_v)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_vy", ScalarType::Float, move |t: &Tuple| {
                let vy = t.get_typed::<f64>("vy").unwrap();
                let sep = t.get_typed::<f64>("sum_sep_y").unwrap();
                let ali = t.get_typed::<f64>("sum_ali_y").unwrap();
                let coh = t.get_typed::<f64>("sum_coh_y").unwrap();
                let n = t.get_typed::<i64>("n_neighbors").unwrap() as f64;

                let mut ay = sep + (ali / n) + (coh / n);
                let a_mag = ay.abs();
                if a_mag > max_accel {
                    ay = ay / a_mag * max_accel;
                }

                let new_v = vy + ay;
                ScalarValue::Float(new_v)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Limit speed for updated boids
        let speed_limited_boids = updated_boids_with_neighbors
            .extend("clamped_vx", ScalarType::Float, move |t: &Tuple| {
                let vx = t.get_typed::<f64>("new_vx").unwrap();
                let vy = t.get_typed::<f64>("new_vy").unwrap();
                let speed = (vx * vx + vy * vy).sqrt();
                if speed > max_speed {
                    ScalarValue::Float(vx / speed * max_speed)
                } else {
                    ScalarValue::Float(vx)
                }
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("clamped_vy", ScalarType::Float, move |t: &Tuple| {
                let vx = t.get_typed::<f64>("new_vx").unwrap();
                let vy = t.get_typed::<f64>("new_vy").unwrap();
                let speed = (vx * vx + vy * vy).sqrt();
                if speed > max_speed {
                    ScalarValue::Float(vy / speed * max_speed)
                } else {
                    ScalarValue::Float(vy)
                }
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Apply physics to updated boids
        let advanced_updated = speed_limited_boids
            .extend("new_x", ScalarType::Float, move |t: &Tuple| {
                let x = t.get_typed::<f64>("x").unwrap();
                let vx = t.get_typed::<f64>("clamped_vx").unwrap();
                let mut nx = x + vx;
                // Torus wrap
                if nx < 0.0 {
                    nx += width;
                }
                if nx >= width {
                    nx -= width;
                }
                ScalarValue::Float(nx)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_y", ScalarType::Float, move |t: &Tuple| {
                let y = t.get_typed::<f64>("y").unwrap();
                let vy = t.get_typed::<f64>("clamped_vy").unwrap();
                let mut ny = y + vy;
                // Torus wrap
                if ny < 0.0 {
                    ny += height;
                }
                if ny >= height {
                    ny -= height;
                }
                ScalarValue::Float(ny)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Apply physics to isolated boids
        let advanced_isolated = boids_without_neighbors
            .extend("new_x", ScalarType::Float, move |t: &Tuple| {
                let x = t.get_typed::<f64>("x").unwrap();
                let vx = t.get_typed::<f64>("vx").unwrap();
                let mut nx = x + vx;
                if nx < 0.0 {
                    nx += width;
                }
                if nx >= width {
                    nx -= width;
                }
                ScalarValue::Float(nx)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_y", ScalarType::Float, move |t: &Tuple| {
                let y = t.get_typed::<f64>("y").unwrap();
                let vy = t.get_typed::<f64>("vy").unwrap();
                let mut ny = y + vy;
                if ny < 0.0 {
                    ny += height;
                }
                if ny >= height {
                    ny -= height;
                }
                ScalarValue::Float(ny)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 9. Format them back to standard schema and union
        let final_updated = advanced_updated
            .project(&["id", "new_x", "new_y", "clamped_vx", "clamped_vy"])
            .rename(&[
                ("new_x", "x"),
                ("new_y", "y"),
                ("clamped_vx", "vx"),
                ("clamped_vy", "vy"),
            ]);

        let final_isolated = advanced_isolated
            .project(&["id", "new_x", "new_y", "vx", "vy"])
            .rename(&[("new_x", "x"), ("new_y", "y")]);

        let next_boids = final_updated
            .union(&final_isolated)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 10. Save new state
        self.boids = next_boids;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    fn get_boid(boids: &Relation, id: i64) -> Tuple {
        boids
            .restrict(move |t| t.get_typed::<i64>("id").unwrap() == id)
            .tuples()
            .next()
            .unwrap()
            .clone()
    }

    #[test]
    fn test_boid_simulation() {
        let boid_type = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("x", ScalarType::Float)
            .with_attribute("y", ScalarType::Float)
            .with_attribute("vx", ScalarType::Float)
            .with_attribute("vy", ScalarType::Float);

        let mut boids = Relation::new(RelationType::new(boid_type));

        // Boid 1: at origin, moving right
        boids
            .insert(tuple! { id: 1i64, x: 10.0f64, y: 10.0f64, vx: 1.0f64, vy: 0.0f64 })
            .unwrap();
        // Boid 2: slightly ahead and above, moving right and up
        boids
            .insert(tuple! { id: 2i64, x: 12.0f64, y: 12.0f64, vx: 1.0f64, vy: 1.0f64 })
            .unwrap();
        // Boid 3: far away (isolated)
        boids
            .insert(tuple! { id: 3i64, x: 80.0f64, y: 80.0f64, vx: -1.0f64, vy: 0.0f64 })
            .unwrap();

        let mut sim = BoidSimulation::new(boids, 10.0, 1.0, 1.0, 1.0, 1.0, 5.0, 100.0, 100.0);

        sim.tick().unwrap();

        assert_eq!(sim.boids.cardinality(), 3);

        // Boid 3 should just move linearly since it has no neighbors
        let b3 = get_boid(&sim.boids, 3);
        assert!((b3.get_typed::<f64>("x").unwrap() - 79.0).abs() < 1e-6);
        assert!((b3.get_typed::<f64>("y").unwrap() - 80.0).abs() < 1e-6);
        assert!((b3.get_typed::<f64>("vx").unwrap() - -1.0).abs() < 1e-6);
        assert!((b3.get_typed::<f64>("vy").unwrap() - 0.0).abs() < 1e-6);

        // Boids 1 and 2 should have influenced each other
        let b1 = get_boid(&sim.boids, 1);
        let _b2 = get_boid(&sim.boids, 2);

        // Boid 1 vx/vy should be modified by alignment and cohesion (pulling towards 2)
        assert!(b1.get_typed::<f64>("vy").unwrap() > 0.0); // alignment/cohesion pulls Y velocity up
    }
}
