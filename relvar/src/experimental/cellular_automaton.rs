//! Relational Cellular Automaton (Conway's Game of Life).
//!
//! This module demonstrates how to implement cellular automata like Conway's Game of Life
//! purely using relational algebra primitives (Join, Extend, Summarize, Difference, Union).
//!
//! # Concept
//!
//! We represent active cells as a relation `alive_cells` with heading `(x: Int, y: Int)`.
//!
//! Using relational operators, we can:
//! 1. Define an `offsets` relation for the 8 neighbors of a cell.
//! 2. `Join` alive cells with offsets (Cartesian Product) to get all neighbor coordinates.
//! 3. `Summarize` to count how many alive cells each coordinate touches.
//! 4. Apply rules (Birth = 3 neighbors, Survival = 2 neighbors + already alive).

use relvar_core::algebra::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

/// Conway's Game of Life simulator operating on relations.
pub struct Conway {
    /// The relation containing alive cells. Schema: (x: Int, y: Int)
    alive_cells: Relation,
    x_attr: String,
    y_attr: String,
}

impl Conway {
    /// Creates a new Conway instance.
    ///
    /// # Arguments
    ///
    /// * `alive_cells` - The relation containing alive cells. Must have two integer attributes.
    /// * `x_attr` - The attribute name for the x coordinate.
    /// * `y_attr` - The attribute name for the y coordinate.
    pub fn new(alive_cells: Relation, x_attr: &str, y_attr: &str) -> Self {
        Self {
            alive_cells,
            x_attr: x_attr.to_string(),
            y_attr: y_attr.to_string(),
        }
    }

    /// Advances the automaton by one step.
    ///
    /// Returns a new relation representing the next generation of alive cells.
    pub fn step(&self) -> Result<Relation, DatabaseError> {
        // 1. Create Offsets Relation
        let offset_heading = TupleType::new()
            .with_attribute("dx", ScalarType::Int)
            .with_attribute("dy", ScalarType::Int);
        let mut offsets = Relation::new(RelationType::new(offset_heading));

        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue; // Skip self
                }
                offsets
                    .insert(relvar_core::tuple! { dx: dx as i64, dy: dy as i64 })
                    .unwrap();
            }
        }

        // 2. Cartesian Product of alive_cells and offsets
        // To do a cartesian product in this model, we join with no common attributes.
        // alive_cells: (x, y)
        // offsets: (dx, dy)
        let neighbor_pairs = self.alive_cells.join(&offsets)?;

        // 3. Extend to compute actual neighbor coordinates (nx = x + dx, ny = y + dy)
        let extended = neighbor_pairs
            .extend("nx", ScalarType::Int, |t| {
                let x = t.get_typed::<i64>(&self.x_attr).unwrap_or(0);
                let dx = t.get_typed::<i64>("dx").unwrap_or(0);
                ScalarValue::Int(x + dx)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("ny", ScalarType::Int, |t| {
                let y = t.get_typed::<i64>(&self.y_attr).unwrap_or(0);
                let dy = t.get_typed::<i64>("dy").unwrap_or(0);
                ScalarValue::Int(y + dy)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Summarize (Group by nx, ny) to count neighbors
        let counts = extended
            .summarize(&["nx", "ny"], &[Aggregation::count("n_count")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Rename nx -> x, ny -> y for the resulting candidates
        let rename_map = vec![("nx", self.x_attr.as_str()), ("ny", self.y_attr.as_str())];
        let candidates = counts.rename(&rename_map);

        // 5. Apply Rules
        // Rule 1: Birth (exactly 3 neighbors)
        let birth_candidates = candidates.restrict(|t| t.get_typed::<i64>("n_count") == Some(3));

        // Remove n_count column
        let born = birth_candidates.project(&[self.x_attr.as_str(), self.y_attr.as_str()]);

        // Only cells that are NOT currently alive can be born
        let actually_born = born
            .difference(&self.alive_cells)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Rule 2: Survival (2 or 3 neighbors, and already alive)
        // To find survivals, we join candidates with alive_cells to find cells that are alive.
        let surviving_candidates = candidates.join(&self.alive_cells)?;
        let survivors = surviving_candidates.restrict(|t| {
            let count = t.get_typed::<i64>("n_count");
            count == Some(2) || count == Some(3)
        });

        // Remove n_count column
        let actually_survived = survivors.project(&[self.x_attr.as_str(), self.y_attr.as_str()]);

        // 6. Next generation = Born UNION Survived
        let next_gen = actually_born
            .union(&actually_survived)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(next_gen)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;

    #[test]
    fn test_blinker() {
        // Schema: (x: Int, y: Int)
        let heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int);

        let mut initial_state = Relation::new(RelationType::new(heading));

        // Blinker (Vertical)
        // (1, 0)
        // (1, 1)
        // (1, 2)
        initial_state.insert(tuple! { x: 1i64, y: 0i64 }).unwrap();
        initial_state.insert(tuple! { x: 1i64, y: 1i64 }).unwrap();
        initial_state.insert(tuple! { x: 1i64, y: 2i64 }).unwrap();

        let conway = Conway::new(initial_state, "x", "y");
        let next_state = conway.step().unwrap();

        // After one step, it should be horizontal:
        // (0, 1), (1, 1), (2, 1)
        assert_eq!(next_state.cardinality(), 3);

        let mut has_0_1 = false;
        let mut has_1_1 = false;
        let mut has_2_1 = false;

        for t in next_state.tuples() {
            let x = t.get_typed::<i64>("x").unwrap();
            let y = t.get_typed::<i64>("y").unwrap();

            if x == 0 && y == 1 {
                has_0_1 = true;
            } else if x == 1 && y == 1 {
                has_1_1 = true;
            } else if x == 2 && y == 1 {
                has_2_1 = true;
            }
        }

        assert!(has_0_1, "Missing (0, 1)");
        assert!(has_1_1, "Missing (1, 1)");
        assert!(has_2_1, "Missing (2, 1)");
    }
}
