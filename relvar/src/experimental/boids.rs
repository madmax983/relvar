//! Relational Boids Simulation
//!
//! This module demonstrates how a flocking algorithm (Boids) can be modeled
//! purely using relational algebra operations (Join, Extend, Summarize, Difference).
//!
//! # Concept
//!
//! The simulation consists of a `boids` relation:
//! `(id: Int, x: Float, y: Float, vx: Float, vy: Float)`
//!
//! Flocking behavior is determined by three rules:
//! 1. **Separation**: Steer to avoid crowding local flockmates.
//! 2. **Alignment**: Steer towards the average heading of local flockmates.
//! 3. **Cohesion**: Steer to move towards the average position of local flockmates.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue, Tuple},
};

/// A Relational Boids Simulation.
pub struct BoidsSimulation {
    /// The boids in the simulation.
    /// Schema: (id: Int, x: Float, y: Float, vx: Float, vy: Float)
    pub boids: Relation,
    /// The radius within which boids consider other boids as neighbors.
    pub sight_radius: f64,
    /// The radius within which boids try to avoid each other.
    pub separation_radius: f64,
    /// Weight of the cohesion behavior.
    pub cohesion_weight: f64,
    /// Weight of the alignment behavior.
    pub alignment_weight: f64,
    /// Weight of the separation behavior.
    pub separation_weight: f64,
    /// The time step.
    pub dt: f64,
}

impl BoidsSimulation {
    /// Creates a new BoidsSimulation.
    pub fn new(
        boids: Relation,
        sight_radius: f64,
        separation_radius: f64,
        cohesion_weight: f64,
        alignment_weight: f64,
        separation_weight: f64,
        dt: f64,
    ) -> Self {
        Self {
            boids,
            sight_radius,
            separation_radius,
            cohesion_weight,
            alignment_weight,
            separation_weight,
            dt,
        }
    }

    /// Computes the next state of the simulation.
    pub fn next_step(&self) -> Result<Relation, DatabaseError> {
        let b1 = self.boids.clone().rename(&[
            ("id", "id1"),
            ("x", "x1"),
            ("y", "y1"),
            ("vx", "vx1"),
            ("vy", "vy1"),
        ]);
        let b2 = self.boids.clone().rename(&[
            ("id", "id2"),
            ("x", "x2"),
            ("y", "y2"),
            ("vx", "vx2"),
            ("vy", "vy2"),
        ]);

        let pairs = b1
            .join(&b2)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let sr2 = self.sight_radius * self.sight_radius;
        let sep2 = self.separation_radius * self.separation_radius;

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
            let dist_sq = (x1 - x2) * (x1 - x2) + (y1 - y2) * (y1 - y2);
            dist_sq < sr2
        });

        let with_sep = neighbors
            .extend("sep_x", ScalarType::Float, move |t: &Tuple| {
                let x1 = t.get_typed::<f64>("x1").unwrap();
                let y1 = t.get_typed::<f64>("y1").unwrap();
                let x2 = t.get_typed::<f64>("x2").unwrap();
                let y2 = t.get_typed::<f64>("y2").unwrap();
                let dist_sq = (x1 - x2) * (x1 - x2) + (y1 - y2) * (y1 - y2);
                if dist_sq < sep2 && dist_sq > 0.0001 {
                    ScalarValue::Float((x1 - x2) / dist_sq)
                } else {
                    ScalarValue::Float(0.0)
                }
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sep_y", ScalarType::Float, move |t: &Tuple| {
                let x1 = t.get_typed::<f64>("x1").unwrap();
                let y1 = t.get_typed::<f64>("y1").unwrap();
                let x2 = t.get_typed::<f64>("x2").unwrap();
                let y2 = t.get_typed::<f64>("y2").unwrap();
                let dist_sq = (x1 - x2) * (x1 - x2) + (y1 - y2) * (y1 - y2);
                if dist_sq < sep2 && dist_sq > 0.0001 {
                    ScalarValue::Float((y1 - y2) / dist_sq)
                } else {
                    ScalarValue::Float(0.0)
                }
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let summaries = with_sep
            .summarize(
                &["id1"],
                &[
                    Aggregation::count("n_count"),
                    Aggregation::sum_float("sum_x2", "x2"),
                    Aggregation::sum_float("sum_y2", "y2"),
                    Aggregation::sum_float("sum_vx2", "vx2"),
                    Aggregation::sum_float("sum_vy2", "vy2"),
                    Aggregation::sum_float("sum_sep_x", "sep_x"),
                    Aggregation::sum_float("sum_sep_y", "sep_y"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let base_boids = self.boids.clone().rename(&[("id", "id1")]);
        let joined = base_boids
            .join(&summaries)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let missing = base_boids
            .difference(&base_boids.clone().semijoin(&summaries.project(&["id1"])))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let missing_zeros = missing
            .extend("n_count", ScalarType::Int, |_t: &Tuple| ScalarValue::Int(0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sum_x2", ScalarType::Float, |_t: &Tuple| {
                ScalarValue::Float(0.0)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sum_y2", ScalarType::Float, |_t: &Tuple| {
                ScalarValue::Float(0.0)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sum_vx2", ScalarType::Float, |_t: &Tuple| {
                ScalarValue::Float(0.0)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sum_vy2", ScalarType::Float, |_t: &Tuple| {
                ScalarValue::Float(0.0)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sum_sep_x", ScalarType::Float, |_t: &Tuple| {
                ScalarValue::Float(0.0)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sum_sep_y", ScalarType::Float, |_t: &Tuple| {
                ScalarValue::Float(0.0)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let all_stats = joined
            .union(&missing_zeros)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let cw = self.cohesion_weight;
        let aw = self.alignment_weight;
        let sw = self.separation_weight;
        let dt = self.dt;

        let new_state = all_stats
            .extend("coh_x", ScalarType::Float, move |t: &Tuple| {
                let count = t.get_typed::<i64>("n_count").unwrap();
                if count == 0 {
                    return ScalarValue::Float(0.0);
                }
                let sum_x2 = t.get_typed::<f64>("sum_x2").unwrap();
                let x = t.get_typed::<f64>("x").unwrap();
                ScalarValue::Float(((sum_x2 / count as f64) - x) * cw)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("coh_y", ScalarType::Float, move |t: &Tuple| {
                let count = t.get_typed::<i64>("n_count").unwrap();
                if count == 0 {
                    return ScalarValue::Float(0.0);
                }
                let sum_y2 = t.get_typed::<f64>("sum_y2").unwrap();
                let y = t.get_typed::<f64>("y").unwrap();
                ScalarValue::Float(((sum_y2 / count as f64) - y) * cw)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("align_x", ScalarType::Float, move |t: &Tuple| {
                let count = t.get_typed::<i64>("n_count").unwrap();
                if count == 0 {
                    return ScalarValue::Float(0.0);
                }
                let sum_vx2 = t.get_typed::<f64>("sum_vx2").unwrap();
                let vx = t.get_typed::<f64>("vx").unwrap();
                ScalarValue::Float(((sum_vx2 / count as f64) - vx) * aw)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("align_y", ScalarType::Float, move |t: &Tuple| {
                let count = t.get_typed::<i64>("n_count").unwrap();
                if count == 0 {
                    return ScalarValue::Float(0.0);
                }
                let sum_vy2 = t.get_typed::<f64>("sum_vy2").unwrap();
                let vy = t.get_typed::<f64>("vy").unwrap();
                ScalarValue::Float(((sum_vy2 / count as f64) - vy) * aw)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_vx", ScalarType::Float, move |t: &Tuple| {
                let vx = t.get_typed::<f64>("vx").unwrap();
                let sum_sep_x = t.get_typed::<f64>("sum_sep_x").unwrap();
                let coh_x = t.get_typed::<f64>("coh_x").unwrap();
                let align_x = t.get_typed::<f64>("align_x").unwrap();
                ScalarValue::Float(vx + (sum_sep_x * sw + coh_x + align_x) * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_vy", ScalarType::Float, move |t: &Tuple| {
                let vy = t.get_typed::<f64>("vy").unwrap();
                let sum_sep_y = t.get_typed::<f64>("sum_sep_y").unwrap();
                let coh_y = t.get_typed::<f64>("coh_y").unwrap();
                let align_y = t.get_typed::<f64>("align_y").unwrap();
                ScalarValue::Float(vy + (sum_sep_y * sw + coh_y + align_y) * dt)
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
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(new_state
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
    fn test_boids_simulation() {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("x", ScalarType::Float)
            .with_attribute("y", ScalarType::Float)
            .with_attribute("vx", ScalarType::Float)
            .with_attribute("vy", ScalarType::Float);

        let mut boids = Relation::new(RelationType::new(heading));
        boids
            .insert(tuple! { id: 1i64, x: 0.0f64, y: 0.0f64, vx: 1.0f64, vy: 0.0f64 })
            .unwrap();
        boids
            .insert(tuple! { id: 2i64, x: 1.0f64, y: 0.0f64, vx: 0.0f64, vy: 1.0f64 })
            .unwrap();

        let sim = BoidsSimulation {
            boids,
            sight_radius: 5.0,
            separation_radius: 2.0,
            cohesion_weight: 0.1,
            alignment_weight: 0.1,
            separation_weight: 1.0,
            dt: 1.0,
        };

        let next = sim.next_step().unwrap();
        assert_eq!(next.cardinality(), 2);
    }
}
