//! Relational Boids (Flocking Simulation)
//!
//! This module implements Craig Reynolds' Boids flocking simulation using
//! purely relational algebra operations (Cross Join, Extend, Summarize).
//!
//! # Concept
//!
//! Boids have three primary behaviors:
//! 1. **Separation**: Steer to avoid crowding local flockmates.
//! 2. **Alignment**: Steer towards the average heading of local flockmates.
//! 3. **Cohesion**: Steer to move toward the average position of local flockmates.
//!
//! We can implement this in a relational engine!
//! - **boids**: Relation `(id: Int, x: Float, y: Float, vx: Float, vy: Float)`
//!
//! For each step:
//! 1. `theta_join` boids with themselves (as neighbors) where distance < perception_radius.
//! 2. `summarize` the joined relation to compute the average position (cohesion) and average velocity (alignment) of neighbors.
//! 3. `extend` to compute new velocity based on the rules.
//! 4. `extend` to update positions.
//!
//! # Example
//!
//! ```
//! use relvar_core::{types::{TupleType, RelationType, ScalarType}, values::Relation, tuple};
//! use relvar::experimental::boids::BoidsSimulation;
//!
//! let heading = TupleType::new()
//!     .with_attribute("id", ScalarType::Int)
//!     .with_attribute("x", ScalarType::Float)
//!     .with_attribute("y", ScalarType::Float)
//!     .with_attribute("vx", ScalarType::Float)
//!     .with_attribute("vy", ScalarType::Float);
//! let mut boids = Relation::new(RelationType::new(heading));
//! boids.insert(tuple! { id: 1i64, x: 10.0, y: 10.0, vx: 1.0, vy: 0.0 }).unwrap();
//! boids.insert(tuple! { id: 2i64, x: 12.0, y: 10.0, vx: 1.0, vy: 1.0 }).unwrap();
//!
//! let mut sim = BoidsSimulation::new(boids);
//! sim.step().unwrap();
//! ```

use relvar_core::{
    algebra::Aggregation,
    types::ScalarType,
    values::{Relation, ScalarValue},
};
use std::error::Error;

/// A relational Boids simulation.
pub struct BoidsSimulation {
    /// The current state of the boids.
    pub boids: Relation,
    /// The perception radius for flockmates.
    pub perception_radius: f64,
    /// Separation radius.
    pub separation_radius: f64,
    /// Separation weight.
    pub separation_weight: f64,
    /// Alignment weight.
    pub alignment_weight: f64,
    /// Cohesion weight.
    pub cohesion_weight: f64,
    /// Max speed.
    pub max_speed: f64,
    /// Delta time.
    pub dt: f64,
}

impl BoidsSimulation {
    /// Create a new simulation.
    pub fn new(boids: Relation) -> Self {
        Self {
            boids,
            perception_radius: 50.0,
            separation_radius: 20.0,
            separation_weight: 1.5,
            alignment_weight: 1.0,
            cohesion_weight: 1.0,
            max_speed: 4.0,
            dt: 1.0,
        }
    }

    /// Advance the simulation by one step.
    pub fn step(&mut self) -> Result<(), Box<dyn Error>> {
        let perception_sq = self.perception_radius * self.perception_radius;
        let sep_sq = self.separation_radius * self.separation_radius;

        // Step 1: Join boids with themselves to find neighbors
        let boids_a = self
            .boids
            .rename(&[("x", "ax"), ("y", "ay"), ("vx", "avx"), ("vy", "avy")]);

        let boids_b = self.boids.rename(&[
            ("id", "nid"),
            ("x", "bx"),
            ("y", "by"),
            ("vx", "bvx"),
            ("vy", "bvy"),
        ]);

        // Find neighbors within perception radius, including themselves for cohesion/alignment averages.
        let neighbors = boids_a.theta_join(&boids_b, |a, b| {
            let ax = a.get_typed::<f64>("ax").unwrap();
            let ay = a.get_typed::<f64>("ay").unwrap();
            let bx = b.get_typed::<f64>("bx").unwrap();
            let by = b.get_typed::<f64>("by").unwrap();

            let dx = bx - ax;
            let dy = by - ay;
            dx * dx + dy * dy < perception_sq
        });

        // Compute separation components. If within separation radius and not self, push away.
        let neighbors = neighbors
            .extend("sep_dx", ScalarType::Float, move |t| {
                let id = t.get_typed::<i64>("id").unwrap();
                let nid = t.get_typed::<i64>("nid").unwrap();
                if id == nid {
                    return ScalarValue::Float(0.0);
                }

                let ax = t.get_typed::<f64>("ax").unwrap();
                let ay = t.get_typed::<f64>("ay").unwrap();
                let bx = t.get_typed::<f64>("bx").unwrap();
                let by = t.get_typed::<f64>("by").unwrap();

                let dx = ax - bx; // Vector pointing AWAY from neighbor
                let dy = ay - by;
                let d_sq = dx * dx + dy * dy;
                if d_sq > 0.0 && d_sq < sep_sq {
                    // Weight by 1/distance
                    let dist = d_sq.sqrt();
                    ScalarValue::Float(dx / dist)
                } else {
                    ScalarValue::Float(0.0)
                }
            })?
            .extend("sep_dy", ScalarType::Float, move |t| {
                let id = t.get_typed::<i64>("id").unwrap();
                let nid = t.get_typed::<i64>("nid").unwrap();
                if id == nid {
                    return ScalarValue::Float(0.0);
                }

                let ax = t.get_typed::<f64>("ax").unwrap();
                let ay = t.get_typed::<f64>("ay").unwrap();
                let bx = t.get_typed::<f64>("bx").unwrap();
                let by = t.get_typed::<f64>("by").unwrap();

                let dx = ax - bx;
                let dy = ay - by;
                let d_sq = dx * dx + dy * dy;
                if d_sq > 0.0 && d_sq < sep_sq {
                    let dist = d_sq.sqrt();
                    ScalarValue::Float(dy / dist)
                } else {
                    ScalarValue::Float(0.0)
                }
            })?;

        // Summarize to get averages for alignment, cohesion, and total separation
        let summarized = neighbors.summarize(
            &["id", "ax", "ay", "avx", "avy"],
            &[
                Aggregation::sum_float("sum_sep_dx", "sep_dx"),
                Aggregation::sum_float("sum_sep_dy", "sep_dy"),
                Aggregation::avg("avg_vx", "bvx"),
                Aggregation::avg("avg_vy", "bvy"),
                Aggregation::avg("avg_x", "bx"),
                Aggregation::avg("avg_y", "by"),
            ],
        )?;

        let sep_w = self.separation_weight;
        let ali_w = self.alignment_weight;
        let coh_w = self.cohesion_weight;
        let max_s = self.max_speed;
        let dt = self.dt;

        // Apply weights and calculate new velocity and position
        let updated = summarized
            .extend("new_vx", ScalarType::Float, move |t| {
                let avx = t.get_typed::<f64>("avx").unwrap();
                let sum_sep_dx = t.get_typed::<f64>("sum_sep_dx").unwrap();
                let avg_vx = t.get_typed::<f64>("avg_vx").unwrap();
                let avg_x = t.get_typed::<f64>("avg_x").unwrap();
                let ax = t.get_typed::<f64>("ax").unwrap();

                let ali = avg_vx - avx;
                let coh = avg_x - ax;

                let nvx = avx + (sum_sep_dx * sep_w + ali * ali_w + coh * coh_w) * dt;
                ScalarValue::Float(nvx)
            })?
            .extend("new_vy", ScalarType::Float, move |t| {
                let avy = t.get_typed::<f64>("avy").unwrap();
                let sum_sep_dy = t.get_typed::<f64>("sum_sep_dy").unwrap();
                let avg_vy = t.get_typed::<f64>("avg_vy").unwrap();
                let avg_y = t.get_typed::<f64>("avg_y").unwrap();
                let ay = t.get_typed::<f64>("ay").unwrap();

                let ali = avg_vy - avy;
                let coh = avg_y - ay;

                let nvy = avy + (sum_sep_dy * sep_w + ali * ali_w + coh * coh_w) * dt;
                ScalarValue::Float(nvy)
            })?
            .extend("limited_vx", ScalarType::Float, move |t| {
                let vx = t.get_typed::<f64>("new_vx").unwrap();
                let vy = t.get_typed::<f64>("new_vy").unwrap();
                let speed = (vx * vx + vy * vy).sqrt();
                if speed > max_s {
                    ScalarValue::Float((vx / speed) * max_s)
                } else {
                    ScalarValue::Float(vx)
                }
            })?
            .extend("limited_vy", ScalarType::Float, move |t| {
                let vx = t.get_typed::<f64>("new_vx").unwrap();
                let vy = t.get_typed::<f64>("new_vy").unwrap();
                let speed = (vx * vx + vy * vy).sqrt();
                if speed > max_s {
                    ScalarValue::Float((vy / speed) * max_s)
                } else {
                    ScalarValue::Float(vy)
                }
            })?
            .extend("new_x", ScalarType::Float, move |t| {
                let ax = t.get_typed::<f64>("ax").unwrap();
                let vx = t.get_typed::<f64>("limited_vx").unwrap();
                ScalarValue::Float(ax + vx * dt)
            })?
            .extend("new_y", ScalarType::Float, move |t| {
                let ay = t.get_typed::<f64>("ay").unwrap();
                let vy = t.get_typed::<f64>("limited_vy").unwrap();
                ScalarValue::Float(ay + vy * dt)
            })?;

        // Finally, project and rename to match original relation
        self.boids = updated
            .project(&["id", "new_x", "new_y", "limited_vx", "limited_vy"])
            .rename(&[
                ("new_x", "x"),
                ("new_y", "y"),
                ("limited_vx", "vx"),
                ("limited_vy", "vy"),
            ]);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::{
        tuple,
        types::{RelationType, ScalarType, TupleType},
        values::Relation,
    };

    #[test]
    fn test_boids_step() {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("x", ScalarType::Float)
            .with_attribute("y", ScalarType::Float)
            .with_attribute("vx", ScalarType::Float)
            .with_attribute("vy", ScalarType::Float);

        let mut boids = Relation::new(RelationType::new(heading));
        boids
            .insert(tuple! { id: 1i64, x: 10.0, y: 10.0, vx: 1.0, vy: 0.0 })
            .unwrap();
        boids
            .insert(tuple! { id: 2i64, x: 12.0, y: 10.0, vx: -1.0, vy: 0.0 })
            .unwrap();

        let mut sim = BoidsSimulation::new(boids);
        assert!(sim.step().is_ok());

        assert_eq!(sim.boids.cardinality(), 2);
    }
}
