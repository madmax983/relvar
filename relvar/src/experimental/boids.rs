//! Relational Boids Flocking Simulator
//!
//! This module demonstrates how a flocking simulation (Boids) can be implemented
//! using pure relational algebra. Each boid is represented as a tuple with its
//! position and velocity vectors. The forces of Separation, Alignment, and
//! Cohesion are computed via Relational Cross Joins, Restrictions (to find
//! neighbors within a certain radius), Extensions (to compute vectors), and
//! Summarizations (to aggregate forces per boid).
//!
//! # Concept
//!
//! - **Boids**: Relation `(id: Int, x: Float, y: Float, vx: Float, vy: Float)`
//!   representing the state of each boid.
//!
//! The simulation step process:
//! 1. Cross join the Boids relation with itself to compute pairwise distances.
//! 2. Restrict to find neighbors within the visual range, excluding self.
//! 3. Extend to compute individual vectors for Separation, Alignment, and Cohesion.
//! 4. Summarize to get the net force vector for each boid.
//! 5. Extend the original Boids relation joined with the net forces to update
//!    velocities and positions according to the time step.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue, Tuple},
};

/// A Relational Boids Flocking Simulator.
///
/// # Examples
///
/// ```
/// use relvar_core::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::boids::BoidsSimulator;
/// // Note: This is a placeholder example
/// ```
pub struct BoidsSimulator {
    /// The state of all boids.
    /// Schema: (id: Int, x: Float, y: Float, vx: Float, vy: Float)
    pub boids: Relation,
    /// The visual range (radius) within which a boid can perceive other boids.
    pub visual_range: f64,
    /// Weight applied to the separation force.
    pub separation_weight: f64,
    /// Weight applied to the alignment force.
    pub alignment_weight: f64,
    /// Weight applied to the cohesion force.
    pub cohesion_weight: f64,
    /// The time step for each simulation update.
    pub dt: f64,
}

impl BoidsSimulator {
    /// Creates a new BoidsSimulator.
    pub fn new(
        boids: Relation,
        visual_range: f64,
        separation_weight: f64,
        alignment_weight: f64,
        cohesion_weight: f64,
        dt: f64,
    ) -> Self {
        Self {
            boids,
            visual_range,
            separation_weight,
            alignment_weight,
            cohesion_weight,
            dt,
        }
    }

    /// Computes the next state of the flocking simulation.
    pub fn next_step(&self) -> Result<Relation, DatabaseError> {
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

        // 1. Cross join to evaluate all pairs
        let pairs = b1.join(&b2)?;

        // 2. Restrict to neighbors within visual range (and not self)
        let visual_range = self.visual_range;
        let neighbors = pairs.restrict(move |t: &Tuple| {
            let id1 = t.get_typed::<i64>("id1").unwrap();
            let id2 = t.get_typed::<i64>("id2").unwrap();
            if id1 == id2 {
                return false;
            }
            let x1 = t.get_typed::<f64>("x1").unwrap();
            let y1 = t.get_typed::<f64>("y1").unwrap();
            let x2 = t.get_typed::<f64>("x2").unwrap();
            let y2 = t.get_typed::<f64>("y2").unwrap();

            let dx = x2 - x1;
            let dy = y2 - y1;
            let dist_sq = dx * dx + dy * dy;

            dist_sq < visual_range * visual_range
        });

        // 3. Compute forces for each neighbor pair
        // Separation: steer away from neighbors
        // Alignment: steer towards average velocity of neighbors
        // Cohesion: steer towards average position of neighbors
        let forces = neighbors
            .extend("sep_fx", ScalarType::Float, move |t: &Tuple| {
                let x1 = t.get_typed::<f64>("x1").unwrap();
                let x2 = t.get_typed::<f64>("x2").unwrap();
                ScalarValue::Float(x1 - x2) // Away from neighbor
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sep_fy", ScalarType::Float, move |t: &Tuple| {
                let y1 = t.get_typed::<f64>("y1").unwrap();
                let y2 = t.get_typed::<f64>("y2").unwrap();
                ScalarValue::Float(y1 - y2) // Away from neighbor
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            // Note: Alignment force just needs the neighbor's velocity. We will average it in the summarize step.
            .extend("align_fx", ScalarType::Float, move |t: &Tuple| {
                let vx2 = t.get_typed::<f64>("vx2").unwrap();
                ScalarValue::Float(vx2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("align_fy", ScalarType::Float, move |t: &Tuple| {
                let vy2 = t.get_typed::<f64>("vy2").unwrap();
                ScalarValue::Float(vy2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            // Cohesion force needs neighbor's position to find average position (center of mass).
            .extend("coh_x", ScalarType::Float, move |t: &Tuple| {
                let x2 = t.get_typed::<f64>("x2").unwrap();
                ScalarValue::Float(x2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("coh_y", ScalarType::Float, move |t: &Tuple| {
                let y2 = t.get_typed::<f64>("y2").unwrap();
                ScalarValue::Float(y2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Summarize net forces per boid
        let aggregated_forces = forces
            .summarize(
                &["id1"],
                &[
                    // Separation: sum of (pos1 - pos2)
                    Aggregation::sum_float("sum_sep_fx", "sep_fx"),
                    Aggregation::sum_float("sum_sep_fy", "sep_fy"),
                    // Alignment: avg of neighbor velocities
                    Aggregation::avg("avg_align_fx", "align_fx"),
                    Aggregation::avg("avg_align_fy", "align_fy"),
                    // Cohesion: avg of neighbor positions
                    Aggregation::avg("avg_coh_x", "coh_x"),
                    Aggregation::avg("avg_coh_y", "coh_y"),
                    // Need count to know if there were any neighbors (though summarize on empty group drops the group, so presence means count > 0)
                    Aggregation::count("neighbor_count"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Join aggregated forces with original boids to apply updates
        let boids_base = self.boids.rename(&[("id", "id1")]);
        let boids_with_forces = boids_base.join(&aggregated_forces)?;

        let boids_ids = self.boids.project(&["id"]);
        let force_ids = aggregated_forces.project(&["id1"]).rename(&[("id1", "id")]);

        // Handle isolated boids (no neighbors)
        let isolated_ids = boids_ids
            .difference(&force_ids)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let isolated_boids = self.boids.join(&isolated_ids)?;

        // Advance isolated boids
        let dt = self.dt;
        let advanced_isolated = isolated_boids
            .extend("new_x", ScalarType::Float, move |t: &Tuple| {
                let x = t.get_typed::<f64>("x").unwrap();
                let vx = t.get_typed::<f64>("vx").unwrap();
                ScalarValue::Float(x + vx * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_y", ScalarType::Float, move |t: &Tuple| {
                let y = t.get_typed::<f64>("y").unwrap();
                let vy = t.get_typed::<f64>("vy").unwrap();
                ScalarValue::Float(y + vy * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["id", "new_x", "new_y", "vx", "vy"])
            .rename(&[("new_x", "x"), ("new_y", "y")]);

        // Advance flocking boids
        let sep_w = self.separation_weight;
        let ali_w = self.alignment_weight;
        let coh_w = self.cohesion_weight;

        let advanced_flocking = boids_with_forces
            .extend("new_vx", ScalarType::Float, move |t: &Tuple| {
                let vx = t.get_typed::<f64>("vx").unwrap();
                let x = t.get_typed::<f64>("x").unwrap();

                let sum_sep_fx = t.get_typed::<f64>("sum_sep_fx").unwrap();
                let avg_align_fx = t.get_typed::<f64>("avg_align_fx").unwrap();
                let avg_coh_x = t.get_typed::<f64>("avg_coh_x").unwrap();

                let sep_force = sum_sep_fx * sep_w;
                let ali_force = (avg_align_fx - vx) * ali_w;
                let coh_force = (avg_coh_x - x) * coh_w;

                ScalarValue::Float(vx + sep_force + ali_force + coh_force)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_vy", ScalarType::Float, move |t: &Tuple| {
                let vy = t.get_typed::<f64>("vy").unwrap();
                let y = t.get_typed::<f64>("y").unwrap();

                let sum_sep_fy = t.get_typed::<f64>("sum_sep_fy").unwrap();
                let avg_align_fy = t.get_typed::<f64>("avg_align_fy").unwrap();
                let avg_coh_y = t.get_typed::<f64>("avg_coh_y").unwrap();

                let sep_force = sum_sep_fy * sep_w;
                let ali_force = (avg_align_fy - vy) * ali_w;
                let coh_force = (avg_coh_y - y) * coh_w;

                ScalarValue::Float(vy + sep_force + ali_force + coh_force)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_x", ScalarType::Float, move |t: &Tuple| {
                let x = t.get_typed::<f64>("x").unwrap();
                let new_vx = t.get_typed::<f64>("new_vx").unwrap();
                ScalarValue::Float(x + new_vx * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_y", ScalarType::Float, move |t: &Tuple| {
                let y = t.get_typed::<f64>("y").unwrap();
                let new_vy = t.get_typed::<f64>("new_vy").unwrap();
                ScalarValue::Float(y + new_vy * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["id1", "new_x", "new_y", "new_vx", "new_vy"])
            .rename(&[
                ("id1", "id"),
                ("new_x", "x"),
                ("new_y", "y"),
                ("new_vx", "vx"),
                ("new_vy", "vy"),
            ]);

        advanced_isolated
            .union(&advanced_flocking)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_boids_flocking() {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("x", ScalarType::Float)
            .with_attribute("y", ScalarType::Float)
            .with_attribute("vx", ScalarType::Float)
            .with_attribute("vy", ScalarType::Float);

        let mut boids = Relation::new(RelationType::new(heading));

        // Boid 1: at origin, moving right
        boids
            .insert(tuple! {
                id: 1i64, x: 0.0f64, y: 0.0f64, vx: 1.0f64, vy: 0.0f64
            })
            .unwrap();

        // Boid 2: slightly ahead and above, moving right
        boids
            .insert(tuple! {
                id: 2i64, x: 1.0f64, y: 1.0f64, vx: 1.0f64, vy: 0.0f64
            })
            .unwrap();

        // Boid 3: far away (isolated)
        boids
            .insert(tuple! {
                id: 3i64, x: 100.0f64, y: 100.0f64, vx: 0.0f64, vy: -1.0f64
            })
            .unwrap();

        let simulator = BoidsSimulator::new(
            boids, 5.0, // visual range
            0.1, // separation
            0.1, // alignment
            0.1, // cohesion
            1.0, // dt
        );

        let next_state = simulator.next_step().unwrap();

        assert_eq!(next_state.cardinality(), 3);

        // Boid 3 should just move by its velocity
        let b3 = next_state
            .tuples()
            .find(|t| t.get_typed::<i64>("id") == Some(3))
            .unwrap();
        assert_eq!(b3.get_typed::<f64>("x"), Some(100.0));
        assert_eq!(b3.get_typed::<f64>("y"), Some(99.0)); // 100 + (-1 * 1.0)

        // Boid 1 and 2 should influence each other
        let b1 = next_state
            .tuples()
            .find(|t| t.get_typed::<i64>("id") == Some(1))
            .unwrap();
        let b2 = next_state
            .tuples()
            .find(|t| t.get_typed::<i64>("id") == Some(2))
            .unwrap();

        // We just ensure they have updated positions based on complex logic
        assert!(b1.get_typed::<f64>("x").unwrap() > 0.0);
        assert!(b2.get_typed::<f64>("x").unwrap() > 1.0);
    }
}
