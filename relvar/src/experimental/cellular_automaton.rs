//! Relational Cellular Automaton (Conway's Game of Life)
//!
//! This module implements Conway's Game of Life using pure relational algebra.
//! It demonstrates how complex state transitions in a grid can be modeled
//! using relational operations like Join, Extend, Summarize, and Restrict,
//! completely avoiding imperative loop-based grid updates.
//!
//! # The Relational Approach
//!
//! In a standard imperative implementation, you iterate over a 2D array, counting
//! neighbors for each cell. In the relational model, we represent the board
//! as a relation of active (alive) cells: `AliveCells { x: Int, y: Int }`.
//!
//! To calculate the next generation:
//! 1. **Neighbor Generation**: We cross-join the alive cells with a static relation
//!    of neighbor offsets (dx: -1..1, dy: -1..1) to generate all neighbor coordinates.
//! 2. **Aggregation**: We summarize the generated neighbor coordinates to count
//!    how many alive cells are adjacent to every cell on the board (even empty ones).
//! 3. **Rules Application**: We join the neighbor counts with the current alive cells
//!    and use relational restriction (filtering) to apply Conway's rules:
//!    - Birth: exactly 3 neighbors
//!    - Survival: exactly 2 or 3 neighbors

use relvar_core::algebra::Aggregation;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

/// A simulator for Conway's Game of Life using relational algebra.
pub struct CellularAutomaton {
    /// The current state of the board, represented as a relation of alive cells.
    /// Heading: { x: Int, y: Int }
    board: Relation,

    /// A static relation containing the 8 neighbor offsets.
    /// Heading: { dx: Int, dy: Int }
    offsets: Relation,
}

impl CellularAutomaton {
    /// Creates a new Cellular Automaton with an empty board.
    pub fn new() -> Self {
        let board_heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int);

        let board = Relation::new(RelationType::new(board_heading));

        let offsets_heading = TupleType::new()
            .with_attribute("dx", ScalarType::Int)
            .with_attribute("dy", ScalarType::Int);

        let mut offsets = Relation::new(RelationType::new(offsets_heading));

        // Insert the 8 surrounding neighbor offsets
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx != 0 || dy != 0 {
                    offsets
                        .insert(tuple! { dx: dx as i64, dy: dy as i64 })
                        .unwrap();
                }
            }
        }

        Self { board, offsets }
    }

    /// Returns the current state of the board.
    pub fn state(&self) -> &Relation {
        &self.board
    }

    /// Seed the board with initial alive cells.
    pub fn seed(&mut self, cells: &[(i64, i64)]) {
        for &(x, y) in cells {
            self.board.insert(tuple! { x: x, y: y }).unwrap();
        }
    }

    /// Advances the simulation by one generation using pure relational algebra.
    pub fn tick(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Generate all neighbor coordinates for every alive cell.
        // We use a Cartesian Product (Join with no common attributes) to pair each alive cell with all 8 offsets.
        // Result heading: { x: Int, y: Int, dx: Int, dy: Int }
        let cell_offsets = self.board.join(&self.offsets)?;

        // 2. Extend the relation to calculate the actual neighbor coordinate (nx = x + dx, ny = y + dy).
        // Result heading: { x: Int, y: Int, dx: Int, dy: Int, nx: Int, ny: Int }
        let neighbor_coords = cell_offsets
            .extend("nx", ScalarType::Int, |t: &relvar_core::values::Tuple| {
                let x = t.get_typed::<i64>("x").unwrap_or(0);
                let dx = t.get_typed::<i64>("dx").unwrap_or(0);
                ScalarValue::Int(x + dx)
            })?
            .extend("ny", ScalarType::Int, |t: &relvar_core::values::Tuple| {
                let y = t.get_typed::<i64>("y").unwrap_or(0);
                let dy = t.get_typed::<i64>("dy").unwrap_or(0);
                ScalarValue::Int(y + dy)
            })?;

        // 3. Summarize to count how many times each (nx, ny) coordinate appears.
        // We group by (nx, ny) and count the number of occurrences.
        // We MUST summarize before projecting away identifying attributes, as project eliminates duplicates.
        let counts = neighbor_coords.summarize(&["nx", "ny"], &[Aggregation::count("n_count")])?;

        // 4. Rename nx->x, ny->y to match the board heading.
        let neighbor_counts = counts.rename(&[("nx", "x"), ("ny", "y")]);

        // 5. To apply rules, we need to know which cells are currently alive.
        // We extend the current board with an "alive" flag = true.
        let current_alive = self
            .board
            .extend("alive", ScalarType::Bool, |_| ScalarValue::Bool(true))?;

        // 6. Cells with 3 neighbors are born, regardless of current state.
        let births = neighbor_counts
            .restrict(|t: &relvar_core::values::Tuple| {
                t.get_typed::<i64>("n_count").unwrap_or(0) == 3
            })
            .project(&["x", "y"]);

        // 7. Cells with 2 neighbors survive IF they are currently alive.
        // We join neighbor_counts with current_alive (this acts as an intersection of coordinates).
        let survivors = neighbor_counts
            .join(&current_alive)?
            .restrict(|t: &relvar_core::values::Tuple| {
                t.get_typed::<i64>("n_count").unwrap_or(0) == 2
            })
            .project(&["x", "y"]);

        // 8. The new board is the union of births and survivors.
        self.board = births.union(&survivors)?;

        Ok(())
    }
}

impl Default for CellularAutomaton {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blinker() {
        let mut sim = CellularAutomaton::new();

        // Horizontal blinker
        sim.seed(&[(0, 0), (1, 0), (2, 0)]);

        sim.tick().unwrap();

        // Should become vertical blinker
        let state = sim.state();
        assert_eq!(state.cardinality(), 3);

        // Verify coords (1,-1), (1,0), (1,1)
        let mut found = 0;
        for t in state.tuples() {
            let x = t.get_typed::<i64>("x").unwrap();
            let y = t.get_typed::<i64>("y").unwrap();
            if x == 1 && (y == -1 || y == 0 || y == 1) {
                found += 1;
            }
        }
        assert_eq!(found, 3);
    }
}
