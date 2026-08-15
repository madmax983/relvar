//! Relational Physics Engine
//!
//! This module implements an N-body physics simulation using pure relational
//! algebra operations (Join, Extend, Summarize, Restrict).
//!
//! The simulation represents the state as a single relation of `particles`
//! with the schema `(id: Int, x: Float, y: Float, vx: Float, vy: Float, mass: Float)`.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A relational N-Body Physics Simulation.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::physics::PhysicsEngine;
/// // Note: This is a placeholder example
/// ```
pub struct PhysicsEngine {
    /// The current state of the particles.
    /// Schema: `(id: Int, x: Float, y: Float, vx: Float, vy: Float, mass: Float)`
    pub particles: Relation,
    /// The universal gravitational constant
    pub g: f64,
    /// The time step
    pub dt: f64,
}

impl PhysicsEngine {
    /// Creates a new PhysicsEngine.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::physics::PhysicsEngine;
    /// // Note: This is a placeholder example
    /// ```
    pub fn new(particles: Relation, g: f64, dt: f64) -> Self {
        Self { particles, g, dt }
    }

    /// Computes the next state of the simulation.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::physics::PhysicsEngine;
    /// // Note: This is a placeholder example
    /// ```
    pub fn next_step(&self) -> Result<Relation, DatabaseError> {
        let with_forces = self.compute_pairwise_forces()?;
        let all_particles_with_forces = self.summarize_net_forces(&with_forces)?;
        self.update_kinematics_and_project(&all_particles_with_forces)
    }

    fn compute_pairwise_forces(&self) -> Result<Relation, DatabaseError> {
        // 1. Cross join particles with themselves to compute pairwise forces.
        // Rename attributes to distinguish particle 1 and particle 2.
        let p1 = self.particles.rename(&[
            ("id", "id1"),
            ("x", "x1"),
            ("y", "y1"),
            ("vx", "vx1"),
            ("vy", "vy1"),
            ("mass", "m1"),
        ]);

        let p2 = self.particles.rename(&[
            ("id", "id2"),
            ("x", "x2"),
            ("y", "y2"),
            ("vx", "vx2"),
            ("vy", "vy2"),
            ("mass", "m2"),
        ]);

        let pairs = p1.join(&p2)?;

        // 2. Filter out self-interactions (id1 == id2)
        let interactions = pairs.restrict(|t| {
            let id1 = t.get_typed::<i64>("id1").unwrap();
            let id2 = t.get_typed::<i64>("id2").unwrap();
            id1 != id2
        });

        // 3. Compute forces for each pair
        let g = self.g;
        interactions
            .extend("fx", ScalarType::Float, move |t| {
                let (fx, _) = calculate_force_components(t, g);
                ScalarValue::Float(fx)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("fy", ScalarType::Float, move |t| {
                let (_, fy) = calculate_force_components(t, g);
                ScalarValue::Float(fy)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    fn summarize_net_forces(&self, with_forces: &Relation) -> Result<Relation, DatabaseError> {
        // 4. Summarize to get net forces for each particle
        let net_forces = with_forces
            .summarize(
                &["id1"],
                &[
                    Aggregation::sum_float("net_fx", "fx"),
                    Aggregation::sum_float("net_fy", "fy"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. If there is only one particle, or a particle has no other particles,
        // it won't be in net_forces. We need to handle particles with 0 net force.
        // We'll take the original particles and join with net_forces. But it's an inner join.
        // Relational algebra lacks an easy outer join. We compute difference and union.

        let p_base = self.particles.rename(&[("id", "id1")]);
        let particles_with_forces = p_base.join(&net_forces)?;

        let p_ids = self.particles.project(&["id"]);
        let force_ids = net_forces.project(&["id1"]).rename(&[("id1", "id")]);
        let missing_ids = p_ids
            .difference(&force_ids)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Add 0 force to missing particles
        let missing_with_zero_forces = self
            .particles
            .join(&missing_ids)?
            .rename(&[("id", "id1")])
            .extend("net_fx", ScalarType::Float, |_| ScalarValue::Float(0.0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("net_fy", ScalarType::Float, |_| ScalarValue::Float(0.0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        particles_with_forces
            .union(&missing_with_zero_forces)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    fn update_kinematics_and_project(
        &self,
        all_particles_with_forces: &Relation,
    ) -> Result<Relation, DatabaseError> {
        // 6. Update kinematics: v = v + a*dt, p = p + v*dt
        let dt = self.dt;
        let updated = all_particles_with_forces
            .extend("new_vx", ScalarType::Float, move |t| {
                let vx = t.get_typed::<f64>("vx").unwrap();
                let m = t.get_typed::<f64>("mass").unwrap();
                let fx = t.get_typed::<f64>("net_fx").unwrap();
                let ax = fx / m;
                ScalarValue::Float(vx + ax * dt)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_vy", ScalarType::Float, move |t| {
                let vy = t.get_typed::<f64>("vy").unwrap();
                let m = t.get_typed::<f64>("mass").unwrap();
                let fy = t.get_typed::<f64>("net_fy").unwrap();
                let ay = fy / m;
                ScalarValue::Float(vy + ay * dt)
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

        // 7. Project back to original schema and rename
        let next_state = updated
            .project(&["id1", "new_x", "new_y", "new_vx", "new_vy", "mass"])
            .rename(&[
                ("id1", "id"),
                ("new_x", "x"),
                ("new_y", "y"),
                ("new_vx", "vx"),
                ("new_vy", "vy"),
            ]);

        Ok(next_state)
    }
}

fn calculate_force_components(t: &relvar_core::values::Tuple, g: f64) -> (f64, f64) {
    let x1 = t.get_typed::<f64>("x1").unwrap();
    let y1 = t.get_typed::<f64>("y1").unwrap();
    let x2 = t.get_typed::<f64>("x2").unwrap();
    let y2 = t.get_typed::<f64>("y2").unwrap();
    let m1 = t.get_typed::<f64>("m1").unwrap();
    let m2 = t.get_typed::<f64>("m2").unwrap();

    let dx = x2 - x1;
    let dy = y2 - y1;
    let dist_sq = dx * dx + dy * dy;

    if dist_sq < 1e-10 {
        return (0.0, 0.0);
    }

    let dist = dist_sq.sqrt();
    let f = g * m1 * m2 / dist_sq;
    let fx = f * (dx / dist);
    let fy = f * (dy / dist);

    (fx, fy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_two_body_gravity() {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("x", ScalarType::Float)
            .with_attribute("y", ScalarType::Float)
            .with_attribute("vx", ScalarType::Float)
            .with_attribute("vy", ScalarType::Float)
            .with_attribute("mass", ScalarType::Float);

        let mut particles = Relation::new(RelationType::new(heading));

        // Particle 1: origin, motionless, mass 100
        particles
            .insert(tuple! {
                id: 1i64, x: 0.0f64, y: 0.0f64, vx: 0.0f64, vy: 0.0f64, mass: 100.0f64
            })
            .unwrap();

        // Particle 2: at x=10, motionless, mass 100
        particles
            .insert(tuple! {
                id: 2i64, x: 10.0f64, y: 0.0f64, vx: 0.0f64, vy: 0.0f64, mass: 100.0f64
            })
            .unwrap();

        let engine = PhysicsEngine::new(particles, 1.0, 1.0); // G=1.0, dt=1.0

        // F = G * m1 * m2 / r^2 = 1.0 * 100 * 100 / 100 = 100
        // a1 = F / m1 = 100 / 100 = 1.0
        // a2 = F / m2 = -100 / 100 = -1.0
        // v_new = v_old + a * dt
        // p_new = p_old + v_new * dt

        let next_state = engine.next_step().unwrap();

        assert_eq!(next_state.cardinality(), 2);

        let p1 = next_state
            .tuples()
            .find(|t| t.get_typed::<i64>("id") == Some(1))
            .unwrap();
        assert_eq!(p1.get_typed::<f64>("vx"), Some(1.0));
        assert_eq!(p1.get_typed::<f64>("x"), Some(1.0));

        let p2 = next_state
            .tuples()
            .find(|t| t.get_typed::<i64>("id") == Some(2))
            .unwrap();
        assert_eq!(p2.get_typed::<f64>("vx"), Some(-1.0));
        assert_eq!(p2.get_typed::<f64>("x"), Some(9.0));
    }
}
