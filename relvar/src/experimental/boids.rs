//! Relational Boids Flocking Simulation
//!
//! Models Craig Reynolds' Boids algorithm purely using relational algebra.
//!
//! # Concept
//!
//! The Boids algorithm relies on three core rules:
//! - **Separation**: Steer to avoid crowding local flockmates.
//! - **Alignment**: Steer towards the average heading of local flockmates.
//! - **Cohesion**: Steer to move towards the average position of local flockmates.
//!
//! We express these mathematically as relational queries.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational Boids Engine.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::boids::BoidsEngine;
/// // Note: This is a placeholder example
/// ```
pub struct BoidsEngine {
    /// The boids relation. Schema: `(id: Int, x: Float, y: Float, vx: Float, vy: Float)`
    pub boids: Relation,
    /// Perception radius
    pub vision_radius: f64,
    /// Separation radius
    pub separation_radius: f64,
    /// Weight for separation
    pub separation_weight: f64,
    /// Weight for alignment
    pub alignment_weight: f64,
    /// Weight for cohesion
    pub cohesion_weight: f64,
    /// The time step
    pub dt: f64,
}

impl BoidsEngine {
    /// Creates a new BoidsEngine.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::boids::BoidsEngine;
    /// // Note: This is a placeholder example
    /// ```
    pub fn new(
        boids: Relation,
        vision_radius: f64,
        separation_radius: f64,
        separation_weight: f64,
        alignment_weight: f64,
        cohesion_weight: f64,
        dt: f64,
    ) -> Self {
        Self {
            boids,
            vision_radius,
            separation_radius,
            separation_weight,
            alignment_weight,
            cohesion_weight,
            dt,
        }
    }

    /// Computes the next state of the flocking simulation.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::boids::BoidsEngine;
    /// // Note: This is a placeholder example
    /// ```
    pub fn next_step(&self) -> Result<Relation, DatabaseError> {
        let pairs = self.compute_pairs()?;

        let local_flockmates = self.filter_by_radius(&pairs, self.vision_radius)?;
        let too_close = self.filter_by_radius(&pairs, self.separation_radius)?;

        let sep_force = self.compute_separation(&too_close)?;
        let align_force = self.compute_alignment(&local_flockmates)?;
        let coh_force = self.compute_cohesion(&local_flockmates)?;

        let forces = self.combine_forces(&sep_force, &align_force, &coh_force)?;

        self.apply_forces(&forces)
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

        let joined = b1.join(&b2)?;

        // Exclude self
        Ok(joined.restrict(|t| {
            let id1 = t.get_typed::<i64>("id1").unwrap();
            let id2 = t.get_typed::<i64>("id2").unwrap();
            id1 != id2
        }))
    }

    fn filter_by_radius(&self, pairs: &Relation, radius: f64) -> Result<Relation, DatabaseError> {
        let r_sq = radius * radius;
        Ok(pairs.restrict(move |t| {
            let x1 = t.get_typed::<f64>("x1").unwrap();
            let y1 = t.get_typed::<f64>("y1").unwrap();
            let x2 = t.get_typed::<f64>("x2").unwrap();
            let y2 = t.get_typed::<f64>("y2").unwrap();

            let dx = x2 - x1;
            let dy = y2 - y1;

            dx * dx + dy * dy < r_sq
        }))
    }

    fn compute_separation(&self, too_close: &Relation) -> Result<Relation, DatabaseError> {
        // vector away from neighbors
        let repel = too_close
            .extend("sep_dx", ScalarType::Float, |t| {
                let x1 = t.get_typed::<f64>("x1").unwrap();
                let x2 = t.get_typed::<f64>("x2").unwrap();
                ScalarValue::Float(x1 - x2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("sep_dy", ScalarType::Float, |t| {
                let y1 = t.get_typed::<f64>("y1").unwrap();
                let y2 = t.get_typed::<f64>("y2").unwrap();
                ScalarValue::Float(y1 - y2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        repel
            .summarize(
                &["id1"],
                &[
                    Aggregation::sum_float("sep_x", "sep_dx"),
                    Aggregation::sum_float("sep_y", "sep_dy"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    fn compute_alignment(&self, local: &Relation) -> Result<Relation, DatabaseError> {
        local
            .summarize(
                &["id1"],
                &[
                    Aggregation::avg("avg_vx", "vx2"),
                    Aggregation::avg("avg_vy", "vy2"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    fn compute_cohesion(&self, local: &Relation) -> Result<Relation, DatabaseError> {
        local
            .summarize(
                &["id1"],
                &[
                    Aggregation::avg("avg_x", "x2"),
                    Aggregation::avg("avg_y", "y2"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    fn combine_forces(
        &self,
        sep: &Relation,
        align: &Relation,
        coh: &Relation,
    ) -> Result<Relation, DatabaseError> {
        let b_ids = self.boids.project(&["id"]).rename(&[("id", "id1")]);

        let ensure_all =
            |rel: &Relation, cols: &[(&str, f64)]| -> Result<Relation, DatabaseError> {
                let p = b_ids.join(rel)?;
                let missing = b_ids
                    .difference(&p.project(&["id1"]))
                    .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

                let mut result = missing;
                for (col, val) in cols {
                    result = result
                    .extend(col, ScalarType::Float, move |_| ScalarValue::Float(*val))
                        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
                }
                p.union(&result)
                    .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
            };

        let sep_full = ensure_all(sep, &[("sep_x", 0.0), ("sep_y", 0.0)])?;
        let align_full = ensure_all(align, &[("avg_vx", 0.0), ("avg_vy", 0.0)])?;
        let coh_full = ensure_all(coh, &[("avg_x", 0.0), ("avg_y", 0.0)])?;

        let j1 = sep_full.join(&align_full)?;
        j1.join(&coh_full)
    }

    fn apply_forces(&self, forces: &Relation) -> Result<Relation, DatabaseError> {
        let b = self.boids.rename(&[("id", "id1")]);
        let combined = b.join(forces)?;

        let sep_w = self.separation_weight;
        let ali_w = self.alignment_weight;
        let coh_w = self.cohesion_weight;
        let dt = self.dt;

        let new_boids = combined
            .extend("new_vx", ScalarType::Float, move |t| {
                let vx = t.get_typed::<f64>("vx").unwrap();
                let sep_x = t.get_typed::<f64>("sep_x").unwrap();

                let avg_vx = t.get_typed::<f64>("avg_vx").unwrap();
                let ali_x = if avg_vx == 0.0 { 0.0 } else { avg_vx - vx };

                let avg_x = t.get_typed::<f64>("avg_x").unwrap();
                let x = t.get_typed::<f64>("x").unwrap();
                let coh_x = if avg_x == 0.0 { 0.0 } else { avg_x - x };

                let force_x = sep_x * sep_w + ali_x * ali_w + coh_x * coh_w;
                ScalarValue::Float(vx + force_x * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_vy", ScalarType::Float, move |t| {
                let vy = t.get_typed::<f64>("vy").unwrap();
                let sep_y = t.get_typed::<f64>("sep_y").unwrap();

                let avg_vy = t.get_typed::<f64>("avg_vy").unwrap();
                let ali_y = if avg_vy == 0.0 { 0.0 } else { avg_vy - vy };

                let avg_y = t.get_typed::<f64>("avg_y").unwrap();
                let y = t.get_typed::<f64>("y").unwrap();
                let coh_y = if avg_y == 0.0 { 0.0 } else { avg_y - y };

                let force_y = sep_y * sep_w + ali_y * ali_w + coh_y * coh_w;
                ScalarValue::Float(vy + force_y * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_x", ScalarType::Float, move |t| {
                let x = t.get_typed::<f64>("x").unwrap();
                let nvx = t.get_typed::<f64>("new_vx").unwrap();
                ScalarValue::Float(x + nvx * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_y", ScalarType::Float, move |t| {
                let y = t.get_typed::<f64>("y").unwrap();
                let nvy = t.get_typed::<f64>("new_vy").unwrap();
                ScalarValue::Float(y + nvy * dt)
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
        Ok(new_boids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

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
            .insert(tuple! { id: 2i64, x: 1.0f64, y: 0.0f64, vx: 0.0f64, vy: 1.0f64 })
            .unwrap();

        let engine = BoidsEngine::new(
            boids, 5.0, // vision_radius
            2.0, // separation_radius
            1.0, // separation_weight
            1.0, // alignment_weight
            1.0, // cohesion_weight
            0.1, // dt
        );

        let next_state = engine.next_step().unwrap();
        assert_eq!(next_state.cardinality(), 2);
    }
}
