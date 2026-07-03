//! Relational Boids Flocking Simulator
//!
//! This module demonstrates how a flocking simulation (Boids) can be implemented
//! using purely relational algebra operations.
//!
//! # Concept
//!
//! - **Boids**: Relation `(id: Int, px: Float, py: Float, vx: Float, vy: Float)`.
//!
//! The simulation computes the three classic Boids rules:
//! 1. **Separation**: Steer to avoid crowding local flockmates.
//! 2. **Alignment**: Steer towards the average heading of local flockmates.
//! 3. **Cohesion**: Steer to move towards the average position of local flockmates.
//!
//! This is achieved by taking the cross product of the boids relation with itself,
//! computing distances, restricting to a neighborhood radius, and summarizing
//! the neighbor forces using relational aggregation.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue, Tuple},
};

/// A Relational Boids Simulator.
pub struct BoidsSimulator {
    /// The current state of the boids.
    /// Schema: (id: Int, px: Float, py: Float, vx: Float, vy: Float)
    pub boids: Relation,
    /// Neighborhood radius
    pub perception_radius: f64,
}

impl BoidsSimulator {
    /// Creates a new BoidsSimulator.
    pub fn new(boids: Relation, perception_radius: f64) -> Self {
        Self {
            boids,
            perception_radius,
        }
    }

    /// Performs a single simulation step and returns the updated boids relation.
    pub fn step(&self) -> Result<Relation, DatabaseError> {
        let b1 = self.boids.rename(&[
            ("id", "id1"),
            ("px", "px1"),
            ("py", "py1"),
            ("vx", "vx1"),
            ("vy", "vy1"),
        ]);

        let b2 = self.boids.rename(&[
            ("id", "id2"),
            ("px", "px2"),
            ("py", "py2"),
            ("vx", "vx2"),
            ("vy", "vy2"),
        ]);

        // Join on disjoint schemas is equivalent to Cartesian Product / Cross Join
        let pairs = b1.join(&b2)?;

        let radius = self.perception_radius;
        let neighbors = pairs.restrict(move |t: &Tuple| {
            let id1 = t.get_typed::<i64>("id1").unwrap_or(0);
            let id2 = t.get_typed::<i64>("id2").unwrap_or(0);
            if id1 == id2 {
                return false;
            }

            let px1 = t.get_typed::<f64>("px1").unwrap_or(0.0);
            let py1 = t.get_typed::<f64>("py1").unwrap_or(0.0);
            let px2 = t.get_typed::<f64>("px2").unwrap_or(0.0);
            let py2 = t.get_typed::<f64>("py2").unwrap_or(0.0);

            let dx = px2 - px1;
            let dy = py2 - py1;
            let dist_sq = dx * dx + dy * dy;

            dist_sq < radius * radius
        });

        let with_sep = neighbors
            .extend("sep_x", ScalarType::Float, |t: &Tuple| {
                let px1 = t.get_typed::<f64>("px1").unwrap_or(0.0);
                let px2 = t.get_typed::<f64>("px2").unwrap_or(0.0);
                ScalarValue::Float(px1 - px2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sep_y", ScalarType::Float, |t: &Tuple| {
                let py1 = t.get_typed::<f64>("py1").unwrap_or(0.0);
                let py2 = t.get_typed::<f64>("py2").unwrap_or(0.0);
                ScalarValue::Float(py1 - py2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let summary = with_sep
            .summarize(
                &["id1"],
                &[
                    Aggregation::avg("avg_px", "px2"),
                    Aggregation::avg("avg_py", "py2"),
                    Aggregation::avg("avg_vx", "vx2"),
                    Aggregation::avg("avg_vy", "vy2"),
                    Aggregation::sum_float("sum_sep_x", "sep_x"),
                    Aggregation::sum_float("sum_sep_y", "sep_y"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let summary_renamed = summary.rename(&[("id1", "id")]);

        let boids_with_neighbors = self.boids.join(&summary_renamed)?;
        let ids_with_neighbors = summary_renamed.project(&["id"]);
        let missing_ids = self
            .boids
            .project(&["id"])
            .difference(&ids_with_neighbors)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let boids_no_neighbors = self.boids.join(&missing_ids)?;

        let updated_boids_with_neighbors = boids_with_neighbors
            .extend("new_vx", ScalarType::Float, |t: &Tuple| {
                let vx = t.get_typed::<f64>("vx").unwrap_or(0.0);
                let px = t.get_typed::<f64>("px").unwrap_or(0.0);

                let avg_vx = t.get_typed::<f64>("avg_vx").unwrap_or(0.0);
                let avg_px = t.get_typed::<f64>("avg_px").unwrap_or(0.0);
                let sum_sep_x = t.get_typed::<f64>("sum_sep_x").unwrap_or(0.0);

                let alignment = (avg_vx - vx) * 0.05;
                let cohesion = (avg_px - px) * 0.01;
                let separation = sum_sep_x * 0.05;

                ScalarValue::Float(vx + alignment + cohesion + separation)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_vy", ScalarType::Float, |t: &Tuple| {
                let vy = t.get_typed::<f64>("vy").unwrap_or(0.0);
                let py = t.get_typed::<f64>("py").unwrap_or(0.0);

                let avg_vy = t.get_typed::<f64>("avg_vy").unwrap_or(0.0);
                let avg_py = t.get_typed::<f64>("avg_py").unwrap_or(0.0);
                let sum_sep_y = t.get_typed::<f64>("sum_sep_y").unwrap_or(0.0);

                let alignment = (avg_vy - vy) * 0.05;
                let cohesion = (avg_py - py) * 0.01;
                let separation = sum_sep_y * 0.05;

                ScalarValue::Float(vy + alignment + cohesion + separation)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let moved_boids_with_neighbors = updated_boids_with_neighbors
            .extend("new_px", ScalarType::Float, |t: &Tuple| {
                let px = t.get_typed::<f64>("px").unwrap_or(0.0);
                let nvx = t.get_typed::<f64>("new_vx").unwrap_or(0.0);
                ScalarValue::Float(px + nvx)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_py", ScalarType::Float, |t: &Tuple| {
                let py = t.get_typed::<f64>("py").unwrap_or(0.0);
                let nvy = t.get_typed::<f64>("new_vy").unwrap_or(0.0);
                ScalarValue::Float(py + nvy)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let final_with_neighbors = moved_boids_with_neighbors
            .project(&["id", "new_px", "new_py", "new_vx", "new_vy"])
            .rename(&[
                ("new_px", "px"),
                ("new_py", "py"),
                ("new_vx", "vx"),
                ("new_vy", "vy"),
            ]);

        let moved_no_neighbors = boids_no_neighbors
            .extend("new_px", ScalarType::Float, |t: &Tuple| {
                let px = t.get_typed::<f64>("px").unwrap_or(0.0);
                let vx = t.get_typed::<f64>("vx").unwrap_or(0.0);
                ScalarValue::Float(px + vx)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_py", ScalarType::Float, |t: &Tuple| {
                let py = t.get_typed::<f64>("py").unwrap_or(0.0);
                let vy = t.get_typed::<f64>("vy").unwrap_or(0.0);
                ScalarValue::Float(py + vy)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["id", "new_px", "new_py", "vx", "vy"])
            .rename(&[("new_px", "px"), ("new_py", "py")]);

        let next_generation = final_with_neighbors
            .union(&moved_no_neighbors)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(next_generation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_boids_step() {
        let boid_type = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("px", ScalarType::Float)
            .with_attribute("py", ScalarType::Float)
            .with_attribute("vx", ScalarType::Float)
            .with_attribute("vy", ScalarType::Float);

        let mut boids = Relation::new(RelationType::new(boid_type));

        boids
            .insert(tuple! { id: 1i64, px: 0.0, py: 0.0, vx: 1.0, vy: 0.0 })
            .unwrap();
        boids
            .insert(tuple! { id: 2i64, px: 0.0, py: 1.0, vx: 1.0, vy: 0.0 })
            .unwrap();
        boids
            .insert(tuple! { id: 3i64, px: 100.0, py: 100.0, vx: 0.0, vy: 1.0 })
            .unwrap();

        let sim = BoidsSimulator::new(boids, 5.0);
        let next_boids = sim.step().unwrap();

        assert_eq!(next_boids.cardinality(), 3);

        let boid3_rel = next_boids.restrict(|t: &Tuple| t.get_typed::<i64>("id").unwrap() == 3);
        let boid3 = boid3_rel.tuples().next().unwrap();
        assert_eq!(boid3.get_typed::<f64>("px").unwrap(), 100.0);
        assert_eq!(boid3.get_typed::<f64>("py").unwrap(), 101.0);
    }
}
