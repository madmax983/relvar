//! Relational Boids (Flocking Simulation)
//!
//! This module implements Craig Reynolds' Boids flocking simulation using pure
//! relational algebra operations (Join, Extend, Summarize, Restrict).
//!
//! The simulation represents the flock as a single relation of `boids`
//! with the schema `(id: Int, x: Float, y: Float, vx: Float, vy: Float)`.
//!
//! # Rules
//! 1. **Separation:** Steer to avoid crowding local flockmates.
//! 2. **Alignment:** Steer towards the average heading of local flockmates.
//! 3. **Cohesion:** Steer to move toward the average position of local flockmates.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational Boids Flocking Simulator.
///
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar_core::tuple;
/// use relvar::experimental::boids::BoidsSimulator;
///
/// let heading = TupleType::new()
///     .with_attribute("id", ScalarType::Int)
///     .with_attribute("x", ScalarType::Float)
///     .with_attribute("y", ScalarType::Float)
///     .with_attribute("vx", ScalarType::Float)
///     .with_attribute("vy", ScalarType::Float);
///
/// let mut boids = Relation::new(RelationType::new(heading));
/// boids.insert(tuple! { id: 1i64, x: 0.0f64, y: 0.0f64, vx: 1.0f64, vy: 0.0f64 }).unwrap();
/// boids.insert(tuple! { id: 2i64, x: 1.0f64, y: 0.0f64, vx: 0.0f64, vy: 1.0f64 }).unwrap();
///
/// let sim = BoidsSimulator::new(boids, 2.0, 1.0, 1.0, 1.0, 1.0);
/// let next_state = sim.next_step().unwrap();
/// assert_eq!(next_state.cardinality(), 2);
/// ```
pub struct BoidsSimulator {
    /// The current state of the boids.
    /// Schema: `(id: Int, x: Float, y: Float, vx: Float, vy: Float)`
    pub boids: Relation,
    /// The perception radius
    pub perception_radius: f64,
    /// Separation weight
    pub separation_weight: f64,
    /// Alignment weight
    pub alignment_weight: f64,
    /// Cohesion weight
    pub cohesion_weight: f64,
    /// Time step
    pub dt: f64,
}

impl BoidsSimulator {
    /// Creates a new BoidsSimulator.
    pub fn new(
        boids: Relation,
        perception_radius: f64,
        separation_weight: f64,
        alignment_weight: f64,
        cohesion_weight: f64,
        dt: f64,
    ) -> Self {
        Self {
            boids,
            perception_radius,
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

        let pairs = b1.join(&b2)?;

        let radius_sq = self.perception_radius * self.perception_radius;
        let neighbors = pairs.restrict(move |t| {
            let id1 = t.get_typed::<i64>("id1").unwrap();
            let id2 = t.get_typed::<i64>("id2").unwrap();
            if id1 == id2 {
                return false;
            }
            let x1 = t.get_typed::<f64>("x1").unwrap();
            let y1 = t.get_typed::<f64>("y1").unwrap();
            let x2 = t.get_typed::<f64>("x2").unwrap();
            let y2 = t.get_typed::<f64>("y2").unwrap();

            let dist_sq = (x1 - x2) * (x1 - x2) + (y1 - y2) * (y1 - y2);
            dist_sq < radius_sq
        });

        let extended = neighbors
            .extend("sep_x", ScalarType::Float, |t| {
                let x1 = t.get_typed::<f64>("x1").unwrap();
                let x2 = t.get_typed::<f64>("x2").unwrap();
                let y1 = t.get_typed::<f64>("y1").unwrap();
                let y2 = t.get_typed::<f64>("y2").unwrap();
                let dx = x1 - x2;
                let dy = y1 - y2;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq > 0.0001 {
                    ScalarValue::Float(dx / dist_sq)
                } else {
                    ScalarValue::Float(0.0)
                }
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sep_y", ScalarType::Float, |t| {
                let x1 = t.get_typed::<f64>("x1").unwrap();
                let x2 = t.get_typed::<f64>("x2").unwrap();
                let y1 = t.get_typed::<f64>("y1").unwrap();
                let y2 = t.get_typed::<f64>("y2").unwrap();
                let dx = x1 - x2;
                let dy = y1 - y2;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq > 0.0001 {
                    ScalarValue::Float(dy / dist_sq)
                } else {
                    ScalarValue::Float(0.0)
                }
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let summarized = extended
            .summarize(
                &["id1", "x1", "y1", "vx1", "vy1"],
                &[
                    Aggregation::sum_float("sum_sep_x", "sep_x"),
                    Aggregation::sum_float("sum_sep_y", "sep_y"),
                    Aggregation::sum_float("sum_coh_x", "x2"),
                    Aggregation::sum_float("sum_coh_y", "y2"),
                    Aggregation::sum_float("sum_align_vx", "vx2"),
                    Aggregation::sum_float("sum_align_vy", "vy2"),
                    Aggregation::count("n"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let sep_w = self.separation_weight;
        let align_w = self.alignment_weight;
        let coh_w = self.cohesion_weight;
        let dt = self.dt;

        let updated_neighbors = summarized
            .extend("new_vx", ScalarType::Float, move |t| {
                let n = t.get_typed::<i64>("n").unwrap() as f64;
                let vx = t.get_typed::<f64>("vx1").unwrap();
                let sum_sep_x = t.get_typed::<f64>("sum_sep_x").unwrap();
                let sum_coh_x = t.get_typed::<f64>("sum_coh_x").unwrap();
                let sum_align_vx = t.get_typed::<f64>("sum_align_vx").unwrap();
                let x1 = t.get_typed::<f64>("x1").unwrap();

                let sep = sum_sep_x;
                let align = (sum_align_vx / n) - vx;
                let coh = (sum_coh_x / n) - x1;

                ScalarValue::Float(vx + (sep * sep_w + align * align_w + coh * coh_w) * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_vy", ScalarType::Float, move |t| {
                let n = t.get_typed::<i64>("n").unwrap() as f64;
                let vy = t.get_typed::<f64>("vy1").unwrap();
                let sum_sep_y = t.get_typed::<f64>("sum_sep_y").unwrap();
                let sum_coh_y = t.get_typed::<f64>("sum_coh_y").unwrap();
                let sum_align_vy = t.get_typed::<f64>("sum_align_vy").unwrap();
                let y1 = t.get_typed::<f64>("y1").unwrap();

                let sep = sum_sep_y;
                let align = (sum_align_vy / n) - vy;
                let coh = (sum_coh_y / n) - y1;

                ScalarValue::Float(vy + (sep * sep_w + align * align_w + coh * coh_w) * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_x", ScalarType::Float, move |t| {
                let x = t.get_typed::<f64>("x1").unwrap();
                let nvx = t.get_typed::<f64>("new_vx").unwrap();
                ScalarValue::Float(x + nvx * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_y", ScalarType::Float, move |t| {
                let y = t.get_typed::<f64>("y1").unwrap();
                let nvy = t.get_typed::<f64>("new_vy").unwrap();
                ScalarValue::Float(y + nvy * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let updated_proj = updated_neighbors
            .project(&["id1", "new_x", "new_y", "new_vx", "new_vy"])
            .rename(&[
                ("id1", "id"),
                ("new_x", "x"),
                ("new_y", "y"),
                ("new_vx", "vx"),
                ("new_vy", "vy"),
            ]);

        let all_ids = self.boids.project(&["id"]);
        let neigh_ids = updated_proj.project(&["id"]);
        let no_neigh_ids = all_ids
            .difference(&neigh_ids)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let no_neigh_boids = self.boids.join(&no_neigh_ids)?;

        let no_neigh_updated = no_neigh_boids
            .extend("new_x", ScalarType::Float, move |t| {
                let x = t.get_typed::<f64>("x").unwrap();
                let vx = t.get_typed::<f64>("vx").unwrap();
                ScalarValue::Float(x + vx * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_y", ScalarType::Float, move |t| {
                let y = t.get_typed::<f64>("y").unwrap();
                let vy = t.get_typed::<f64>("vy").unwrap();
                ScalarValue::Float(y + vy * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["id", "new_x", "new_y", "vx", "vy"])
            .rename(&[("new_x", "x"), ("new_y", "y")]);

        updated_proj
            .union(&no_neigh_updated)
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

        // Boid 1 at origin, moving right
        boids
            .insert(tuple! { id: 1i64, x: 0.0f64, y: 0.0f64, vx: 1.0f64, vy: 0.0f64 })
            .unwrap();

        // Boid 2 close to Boid 1, moving up
        boids
            .insert(tuple! { id: 2i64, x: 1.0f64, y: 0.0f64, vx: 0.0f64, vy: 1.0f64 })
            .unwrap();

        // Boid 3 far away, won't be perceived
        boids
            .insert(tuple! { id: 3i64, x: 100.0f64, y: 100.0f64, vx: 1.0f64, vy: 1.0f64 })
            .unwrap();

        let sim = BoidsSimulator::new(boids, 5.0, 1.0, 1.0, 1.0, 1.0);
        let next_state = sim.next_step().unwrap();

        assert_eq!(next_state.cardinality(), 3);

        let b1 = next_state
            .tuples()
            .find(|t| t.get_typed::<i64>("id") == Some(1))
            .unwrap();
        // Boid 1 should have adjusted its velocity based on Boid 2.
        // It's a complex formula but vx should be different from 1.0 and vy > 0.0
        assert!(b1.get_typed::<f64>("vy").unwrap() > 0.0);

        let b3 = next_state
            .tuples()
            .find(|t| t.get_typed::<i64>("id") == Some(3))
            .unwrap();
        // Boid 3 has no neighbors, so its velocity should be unchanged
        assert_eq!(b3.get_typed::<f64>("vx"), Some(1.0));
        assert_eq!(b3.get_typed::<f64>("vy"), Some(1.0));
        // And its position updated linearly
        assert_eq!(b3.get_typed::<f64>("x"), Some(101.0));
        assert_eq!(b3.get_typed::<f64>("y"), Some(101.0));
    }
}
