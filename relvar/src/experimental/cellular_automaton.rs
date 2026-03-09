//! Experimental Relational Cellular Automaton (Game of Life).
//!
//! Demonstrates evaluating Conway's Game of Life purely through
//! relational algebra: Cartesian products, Joins, Grouping, and Summarization.

use relvar_core::algebra::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

/// Conway's Game of Life simulated via Relational Algebra.
pub struct GameOfLife {
    /// Relation of alive cells, heading {x: Int, y: Int}
    alive_cells: Relation,
    /// Relation of neighbor deltas, heading {dx: Int, dy: Int}
    neighbor_deltas: Relation,
}

impl GameOfLife {
    /// Creates a new Game of Life simulator.
    ///
    /// # Arguments
    ///
    /// * `initial_cells` - A relation with heading {x: Int, y: Int} representing the initial live cells.
    pub fn new(initial_cells: Relation) -> Self {
        // Initialize neighbor_deltas
        let delta_type = RelationType::new(
            TupleType::new()
                .with_attribute("dx", ScalarType::Int)
                .with_attribute("dy", ScalarType::Int),
        );
        let mut deltas = Relation::new(delta_type);
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx != 0 || dy != 0 {
                    deltas
                        .insert(tuple! { dx: dx as i64, dy: dy as i64 })
                        .unwrap();
                }
            }
        }
        Self {
            alive_cells: initial_cells,
            neighbor_deltas: deltas,
        }
    }

    /// Advances the simulation by one tick.
    pub fn tick(&mut self) -> Result<(), DatabaseError> {
        // 1. Generate all neighbor coordinates for every alive cell.
        // Natural join acts as Cartesian product since there are no common attributes.
        let cell_deltas = self.alive_cells.join(&self.neighbor_deltas)?;

        // 2. Extend to compute actual neighbor coordinates: nx = x + dx, ny = y + dy
        let neighbors = cell_deltas
            .extend("nx", ScalarType::Int, |t| {
                let x = t.get_typed::<i64>("x").unwrap();
                let dx = t.get_typed::<i64>("dx").unwrap();
                ScalarValue::Int(x + dx)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("ny", ScalarType::Int, |t| {
                let y = t.get_typed::<i64>("y").unwrap();
                let dy = t.get_typed::<i64>("dy").unwrap();
                ScalarValue::Int(y + dy)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Summarize to get neighbor counts.
        let neighbor_counts = neighbors
            .summarize(&["nx", "ny"], &[Aggregation::count("n_count")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Rename nx -> x, ny -> y to match alive_cells
        let counts_renamed = neighbor_counts.rename(&[("nx", "x"), ("ny", "y")]);

        // 4. Determine survivors: alive AND (count == 2 OR count == 3)
        let alive_with_counts = self.alive_cells.join(&counts_renamed)?;
        let survivors = alive_with_counts
            .restrict(|t| {
                let c = t.get_typed::<i64>("n_count").unwrap();
                c == 2 || c == 3
            })
            .project(&["x", "y"]);

        // 5. Determine births: NOT alive AND (count == 3)
        let potential_births = counts_renamed
            .restrict(|t| {
                let c = t.get_typed::<i64>("n_count").unwrap();
                c == 3
            })
            .project(&["x", "y"]);

        let births = potential_births
            .difference(&self.alive_cells)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 6. Next generation = survivors UNION births
        self.alive_cells = survivors
            .union(&births)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(())
    }

    /// Gets the current living cells.
    pub fn get_cells(&self) -> &Relation {
        &self.alive_cells
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_of_life_blinker() {
        let cell_type = RelationType::new(
            TupleType::new()
                .with_attribute("x", ScalarType::Int)
                .with_attribute("y", ScalarType::Int),
        );
        let mut initial = Relation::new(cell_type);
        // Horizontal blinker
        initial.insert(tuple! { x: 0i64, y: 0i64 }).unwrap();
        initial.insert(tuple! { x: 1i64, y: 0i64 }).unwrap();
        initial.insert(tuple! { x: 2i64, y: 0i64 }).unwrap();

        let mut game = GameOfLife::new(initial);

        // Tick 1: should become vertical blinker
        game.tick().unwrap();
        let cells = game.get_cells();
        assert_eq!(cells.cardinality(), 3);

        // Expected: (1, -1), (1, 0), (1, 1)
        assert!(
            cells
                .tuples()
                .any(|t| t.get_typed::<i64>("x") == Some(1) && t.get_typed::<i64>("y") == Some(-1))
        );
        assert!(
            cells
                .tuples()
                .any(|t| t.get_typed::<i64>("x") == Some(1) && t.get_typed::<i64>("y") == Some(0))
        );
        assert!(
            cells
                .tuples()
                .any(|t| t.get_typed::<i64>("x") == Some(1) && t.get_typed::<i64>("y") == Some(1))
        );

        // Tick 2: should become horizontal blinker again
        game.tick().unwrap();
        let cells2 = game.get_cells();
        assert_eq!(cells2.cardinality(), 3);
        assert!(
            cells2
                .tuples()
                .any(|t| t.get_typed::<i64>("x") == Some(0) && t.get_typed::<i64>("y") == Some(0))
        );
        assert!(
            cells2
                .tuples()
                .any(|t| t.get_typed::<i64>("x") == Some(1) && t.get_typed::<i64>("y") == Some(0))
        );
        assert!(
            cells2
                .tuples()
                .any(|t| t.get_typed::<i64>("x") == Some(2) && t.get_typed::<i64>("y") == Some(0))
        );
    }
}
