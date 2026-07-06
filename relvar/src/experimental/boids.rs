//! Relational Boids Flocking Simulation
//!
//! This module demonstrates how to implement a Boids flocking simulation
//! (Reynolds, 1987) using purely relational algebra.
//!
//! # Concept
//!
//! The simulation consists of agents (boids) that move according to three rules:
//! 1. **Cohesion**: Steer towards the average position of local flockmates.
//! 2. **Alignment**: Steer towards the average heading of local flockmates.
//! 3. **Separation**: Steer to avoid crowding local flockmates.
//!
//! We evaluate these rules relationally by:
//! 1. Cross joining the boids relation with itself to form pairs.
//! 2. Filtering pairs to those within a perception radius.
//! 3. Using aggregation (Summarize) to compute local centers of mass, average velocities, and separation vectors.
//! 4. Joining the aggregations back to the original boids and extending to update positions and velocities.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational Boids Simulation Engine.
pub struct BoidsEngine {
    /// Boids relation. Schema: (id: Int, x: Float, y: Float, vx: Float, vy: Float)
    pub boids: Relation,
    /// Perception radius for boids to see their neighbors.
    pub perception_radius: f64,
    /// Separation distance to avoid crowding.
    pub separation_radius: f64,
    /// Cohesion factor (how much to steer towards center of mass).
    pub cohesion_factor: f64,
    /// Alignment factor (how much to match velocity).
    pub alignment_factor: f64,
    /// Separation factor (how much to repel).
    pub separation_factor: f64,
    /// The time step
    pub dt: f64,
}

impl BoidsEngine {
    /// Creates a new BoidsEngine.
    pub fn new(
        boids: Relation,
        perception_radius: f64,
        separation_radius: f64,
        cohesion_factor: f64,
        alignment_factor: f64,
        separation_factor: f64,
        dt: f64,
    ) -> Self {
        Self {
            boids,
            perception_radius,
            separation_radius,
            cohesion_factor,
            alignment_factor,
            separation_factor,
            dt,
        }
    }

    /// Computes the next state of the simulation.
    pub fn next_step(&self) -> Result<Relation, DatabaseError> {
        let pairs = self.compute_pairs()?;
        let neighbors = self.filter_neighbors(&pairs)?;
        let summaries = self.summarize_flockmates(&neighbors)?;
        self.update_kinematics(&summaries)
    }

    fn compute_pairs(&self) -> Result<Relation, DatabaseError> {
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

        b1.join(&b2)
    }

    fn filter_neighbors(&self, pairs: &Relation) -> Result<Relation, DatabaseError> {
        let p_rad = self.perception_radius;
        let s_rad = self.separation_radius;
        let p_rad_sq = p_rad * p_rad;

        // Compute distance squared and separation vectors for valid neighbors (id1 != id2)
        pairs
            .restrict(move |t| {
                let id1 = t.get_typed::<i64>("id1").unwrap();
                let id2 = t.get_typed::<i64>("id2").unwrap();
                id1 != id2
            })
            .extend("dist_sq", ScalarType::Float, move |t| {
                let x1 = t.get_typed::<f64>("x1").unwrap();
                let y1 = t.get_typed::<f64>("y1").unwrap();
                let x2 = t.get_typed::<f64>("x2").unwrap();
                let y2 = t.get_typed::<f64>("y2").unwrap();
                let dx = x2 - x1;
                let dy = y2 - y1;
                ScalarValue::Float(dx * dx + dy * dy)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .restrict(move |t| {
                let dist_sq = t.get_typed::<f64>("dist_sq").unwrap();
                dist_sq < p_rad_sq && dist_sq > 0.0001 // prevent self or exactly overlapping issues just in case
            })
            .extend("sep_x", ScalarType::Float, move |t| {
                let x1 = t.get_typed::<f64>("x1").unwrap();
                let x2 = t.get_typed::<f64>("x2").unwrap();
                let dist_sq = t.get_typed::<f64>("dist_sq").unwrap();

                if dist_sq < s_rad * s_rad {
                    let dist = dist_sq.sqrt();
                    ScalarValue::Float((x1 - x2) / dist)
                } else {
                    ScalarValue::Float(0.0)
                }
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sep_y", ScalarType::Float, move |t| {
                let y1 = t.get_typed::<f64>("y1").unwrap();
                let y2 = t.get_typed::<f64>("y2").unwrap();
                let dist_sq = t.get_typed::<f64>("dist_sq").unwrap();

                if dist_sq < s_rad * s_rad {
                    let dist = dist_sq.sqrt();
                    ScalarValue::Float((y1 - y2) / dist)
                } else {
                    ScalarValue::Float(0.0)
                }
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    fn summarize_flockmates(&self, neighbors: &Relation) -> Result<Relation, DatabaseError> {
        let summaries = neighbors
            .summarize(
                &["id1"],
                &[
                    Aggregation::count("n_count"),
                    Aggregation::sum_float("sum_x", "x2"),
                    Aggregation::sum_float("sum_y", "y2"),
                    Aggregation::sum_float("sum_vx", "vx2"),
                    Aggregation::sum_float("sum_vy", "vy2"),
                    Aggregation::sum_float("sum_sep_x", "sep_x"),
                    Aggregation::sum_float("sum_sep_y", "sep_y"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Not all boids have neighbors. We join with the original boids to ensure everyone is Ok(updated.
        let base_boids = self.boids.rename(&[("id", "id1")]);
        let joined = base_boids.join(&summaries)?;

        let p_ids = self.boids.project(&["id"]);
        let s_ids = summaries.project(&["id1"]).rename(&[("id1", "id")]);
        let missing_ids = p_ids
            .difference(&s_ids)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let missing_boids = self
            .boids
            .join(&missing_ids)?
            .rename(&[("id", "id1")])
            .extend("n_count", ScalarType::Int, |_| ScalarValue::Int(0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sum_x", ScalarType::Float, |_| ScalarValue::Float(0.0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sum_y", ScalarType::Float, |_| ScalarValue::Float(0.0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sum_vx", ScalarType::Float, |_| ScalarValue::Float(0.0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sum_vy", ScalarType::Float, |_| ScalarValue::Float(0.0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sum_sep_x", ScalarType::Float, |_| ScalarValue::Float(0.0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sum_sep_y", ScalarType::Float, |_| ScalarValue::Float(0.0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        joined
            .union(&missing_boids)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    fn update_kinematics(&self, summaries: &Relation) -> Result<Relation, DatabaseError> {
        let coh_f = self.cohesion_factor;
        let ali_f = self.alignment_factor;
        let sep_f = self.separation_factor;
        let dt = self.dt;

        let updated = summaries
            .extend("new_vx", ScalarType::Float, move |t| {
                let vx = t.get_typed::<f64>("vx").unwrap();
                let count = t.get_typed::<i64>("n_count").unwrap();
                if count == 0 {
                    return ScalarValue::Float(vx);
                }

                let c = count as f64;
                let x = t.get_typed::<f64>("x").unwrap();
                let sum_x = t.get_typed::<f64>("sum_x").unwrap();
                let sum_vx = t.get_typed::<f64>("sum_vx").unwrap();
                let sum_sep_x = t.get_typed::<f64>("sum_sep_x").unwrap();

                // Cohesion (steer towards center of mass)
                let center_x = sum_x / c;
                let v_coh = (center_x - x) * coh_f;

                // Alignment (steer towards avg velocity)
                let avg_vx = sum_vx / c;
                let v_ali = (avg_vx - vx) * ali_f;

                // Separation
                let v_sep = sum_sep_x * sep_f;

                ScalarValue::Float(vx + (v_coh + v_ali + v_sep) * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_vy", ScalarType::Float, move |t| {
                let vy = t.get_typed::<f64>("vy").unwrap();
                let count = t.get_typed::<i64>("n_count").unwrap();
                if count == 0 {
                    return ScalarValue::Float(vy);
                }

                let c = count as f64;
                let y = t.get_typed::<f64>("y").unwrap();
                let sum_y = t.get_typed::<f64>("sum_y").unwrap();
                let sum_vy = t.get_typed::<f64>("sum_vy").unwrap();
                let sum_sep_y = t.get_typed::<f64>("sum_sep_y").unwrap();

                // Cohesion
                let center_y = sum_y / c;
                let v_coh = (center_y - y) * coh_f;

                // Alignment
                let avg_vy = sum_vy / c;
                let v_ali = (avg_vy - vy) * ali_f;

                // Separation
                let v_sep = sum_sep_y * sep_f;

                ScalarValue::Float(vy + (v_coh + v_ali + v_sep) * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_x", ScalarType::Float, move |t| {
                let x = t.get_typed::<f64>("x").unwrap();
                let new_vx = t.get_typed::<f64>("new_vx").unwrap();
                ScalarValue::Float(x + new_vx * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_y", ScalarType::Float, move |t| {
                let y = t.get_typed::<f64>("y").unwrap();
                let new_vy = t.get_typed::<f64>("new_vy").unwrap();
                ScalarValue::Float(y + new_vy * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(updated
            .project(&["id1", "new_x", "new_y", "new_vx", "new_vy"])
            .rename(&[
                ("id1", "id"),
                ("new_x", "x"),
                ("new_y", "y"),
                ("new_vx", "vx"),
                ("new_vy", "vy"),
            ]))
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

        // Two boids close to each other
        boids
            .insert(tuple! { id: 1i64, x: 0.0f64, y: 0.0f64, vx: 1.0f64, vy: 0.0f64 })
            .unwrap();
        boids
            .insert(tuple! { id: 2i64, x: 2.0f64, y: 0.0f64, vx: 0.0f64, vy: 1.0f64 })
            .unwrap();

        let engine = BoidsEngine::new(
            boids, 10.0, // perception_radius
            1.0,  // separation_radius
            0.1,  // cohesion_factor
            0.1,  // alignment_factor
            0.5,  // separation_factor
            1.0,  // dt
        );

        let next_state = engine.next_step().unwrap();
        assert_eq!(next_state.cardinality(), 2);

        let b1 = next_state
            .tuples()
            .find(|t| t.get_typed::<i64>("id") == Some(1))
            .unwrap();
        let _b2 = next_state
            .tuples()
            .find(|t| t.get_typed::<i64>("id") == Some(2))
            .unwrap();

        // Boid 1: (0, 0), v=(1, 0)
        // Neighbors: Boid 2 at (2, 0), v=(0, 1)
        // Center of mass: (2, 0). Cohesion steering: (2 - 0)*0.1 = 0.2 in x.
        // Avg v: (0, 1). Alignment steering: (0 - 1)*0.1 = -0.1 in x, (1 - 0)*0.1 = 0.1 in y.
        // Dist is 2. separation_radius is 1.0, so no separation.
        // new_vx = 1.0 + (0.2 - 0.1) = 1.1
        // new_vy = 0.0 + (0.0 + 0.1) = 0.1
        // new_x = 0.0 + 1.1 = 1.1
        // new_y = 0.0 + 0.1 = 0.1

        assert!((b1.get_typed::<f64>("vx").unwrap() - 1.1).abs() < 1e-6);
        assert!((b1.get_typed::<f64>("vy").unwrap() - 0.1).abs() < 1e-6);
        assert!((b1.get_typed::<f64>("x").unwrap() - 1.1).abs() < 1e-6);
        assert!((b1.get_typed::<f64>("y").unwrap() - 0.1).abs() < 1e-6);
    }
}
