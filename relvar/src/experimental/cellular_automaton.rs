//! Relational Cellular Automaton (Game of Life).
//!
//! This module demonstrates how iterative grid-based simulations can be modeled
//! entirely using relational algebra (Join, Extend, Summarize, Restrict, Union, Difference).

use relvar_core::{
    algebra::Aggregation,
    tuple,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue, Tuple},
};

/// A relational implementation of Cellular Automata (like Conway's Game of Life).
///
/// This demonstrates how iterative grid-based simulations can be modeled
/// entirely using relational algebra (Join, Extend, Summarize, Restrict, Union, Difference).
#[derive(Debug, Clone)]
pub struct GameOfLife {
    /// The current state of the board, represented as a relation of alive cells.
    /// Schema: { x: Int, y: Int }
    pub alive_cells: Relation,

    /// The relation representing the 8 neighbor offsets.
    /// Schema: { dx: Int, dy: Int }
    neighbor_offsets: Relation,
}

impl GameOfLife {
    /// Creates a new Game of Life simulator with the given initial state of alive cells.
    ///
    /// The `alive_cells` relation must have the schema { x: Int, y: Int }.
    pub fn new(alive_cells: Relation) -> Result<Self, String> {
        // Build the neighbor offsets relation (-1..=1, -1..=1 excluding 0,0)
        let mut offsets = Relation::new(RelationType::new(
            TupleType::new()
                .with_attribute("dx", ScalarType::Int)
                .with_attribute("dy", ScalarType::Int),
        ));

        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                offsets
                    .insert(tuple! { dx: dx as i64, dy: dy as i64 })
                    .map_err(|e| e.to_string())?;
            }
        }

        Ok(Self {
            alive_cells,
            neighbor_offsets: offsets,
        })
    }

    /// Computes the next generation of the cellular automaton using relational algebra.
    pub fn next_generation(&mut self) -> Result<(), String> {
        // 1. Cross join alive cells with neighbor offsets
        let crossed = self
            .alive_cells
            .join(&self.neighbor_offsets)
            .map_err(|e| e.to_string())?;

        // 2. Extend to calculate absolute neighbor coordinates (nx = x + dx, ny = y + dy)
        let extended = crossed
            .extend("nx", ScalarType::Int, |t| {
                let x = t.get_typed::<i64>("x").unwrap_or(0);
                let dx = t.get_typed::<i64>("dx").unwrap_or(0);
                ScalarValue::Int(x + dx)
            })
            .map_err(|e| e.to_string())?
            .extend("ny", ScalarType::Int, |t| {
                let y = t.get_typed::<i64>("y").unwrap_or(0);
                let dy = t.get_typed::<i64>("dy").unwrap_or(0);
                ScalarValue::Int(y + dy)
            })
            .map_err(|e| e.to_string())?;

        // 3. Summarize to count alive neighbors per cell location
        // We group by (nx, ny) and count the number of alive neighbors.
        let counts = extended
            .summarize(&["nx", "ny"], &[Aggregation::count("n_count")])
            .map_err(|e| e.to_string())?;

        // Rename nx -> x and ny -> y to match our standard grid schema
        let counts = counts.rename(&[("nx", "x"), ("ny", "y")]);

        // 4. Rule 1: Survivors
        // Alive cells with 2 or 3 alive neighbors survive.
        let survivors = self
            .alive_cells
            .join(&counts)
            .map_err(|e| e.to_string())?
            .restrict(|t: &Tuple| {
                let c = t.get_typed::<i64>("n_count").unwrap_or(0);
                c == 2 || c == 3
            })
            .project(&["x", "y"]);

        // 5. Rule 2: Births
        // Dead cells (cells in `counts` that are NOT in `alive_cells`) with exactly 3 alive neighbors become alive.
        let alive_with_count = self.alive_cells.join(&counts).map_err(|e| e.to_string())?;
        let dead_with_count = counts
            .difference(&alive_with_count)
            .map_err(|e| e.to_string())?;
        let births = dead_with_count
            .restrict(|t: &Tuple| {
                let c = t.get_typed::<i64>("n_count").unwrap_or(0);
                c == 3
            })
            .project(&["x", "y"]);

        // 6. The next generation is the union of survivors and births
        self.alive_cells = survivors.union(&births).map_err(|e| e.to_string())?;

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

    fn make_grid_relation() -> Relation {
        Relation::new(RelationType::new(
            TupleType::new()
                .with_attribute("x", ScalarType::Int)
                .with_attribute("y", ScalarType::Int),
        ))
    }

    #[test]
    fn test_glider() {
        // Setup initial glider pattern
        let mut initial = make_grid_relation();
        // . O .
        // . . O
        // O O O
        initial.insert(tuple! { x: 1i64, y: 0i64 }).unwrap();
        initial.insert(tuple! { x: 2i64, y: 1i64 }).unwrap();
        initial.insert(tuple! { x: 0i64, y: 2i64 }).unwrap();
        initial.insert(tuple! { x: 1i64, y: 2i64 }).unwrap();
        initial.insert(tuple! { x: 2i64, y: 2i64 }).unwrap();

        let mut game = GameOfLife::new(initial).unwrap();

        // Next step should be:
        // . . .
        // O . O
        // . O O
        // . O .
        game.next_generation().unwrap();

        let mut expected = make_grid_relation();
        expected.insert(tuple! { x: 0i64, y: 1i64 }).unwrap();
        expected.insert(tuple! { x: 2i64, y: 1i64 }).unwrap();
        expected.insert(tuple! { x: 1i64, y: 2i64 }).unwrap();
        expected.insert(tuple! { x: 2i64, y: 2i64 }).unwrap();
        expected.insert(tuple! { x: 1i64, y: 3i64 }).unwrap();

        assert_eq!(game.alive_cells, expected);
        assert_eq!(game.alive_cells.cardinality(), 5);

        // Another step
        // . . .
        // . . O
        // O . O
        // . O O
        game.next_generation().unwrap();
        let mut expected_step2 = make_grid_relation();
        expected_step2.insert(tuple! { x: 2i64, y: 1i64 }).unwrap();
        expected_step2.insert(tuple! { x: 0i64, y: 2i64 }).unwrap();
        expected_step2.insert(tuple! { x: 2i64, y: 2i64 }).unwrap();
        expected_step2.insert(tuple! { x: 1i64, y: 3i64 }).unwrap();
        expected_step2.insert(tuple! { x: 2i64, y: 3i64 }).unwrap();

        assert_eq!(game.alive_cells, expected_step2);
    }
}
