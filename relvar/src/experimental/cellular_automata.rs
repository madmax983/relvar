//! Relational Cellular Automata (Conway's Game of Life).
//!
//! This module demonstrates how cellular automata can be implemented using purely
//! relational algebra operations.
//!
//! # Concept
//!
//! The board is modeled as a relation of live cells with heading `(x: Int, y: Int)`.
//! To compute the next generation:
//! 1. Cartesian product of the board with a relation of 8 neighbor coordinate deltas.
//! 2. Extend to compute the absolute neighbor coordinates `nx = x + dx, ny = y + dy`.
//! 3. Summarize grouping by `(nx, ny)` to count live neighbors for every affected cell.
//! 4. Rename `nx` to `x` and `ny` to `y`.
//! 5. Natural Join with the original board to find surviving cells (neighbors == 2).
//! 6. Restrict to find all cells with exactly 3 neighbors (births + survivals).
//! 7. Union the sets to produce the next generation.

use relvar_core::algebra::Aggregation;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue, Tuple};

/// A Game of Life board, represented relationally.
pub struct Board {
    /// The relation storing the live cells.
    /// Heading: `(x: Int, y: Int)`
    pub live_cells: Relation,
}

impl Board {
    /// Creates a new empty board.
    pub fn new() -> Self {
        let heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int);

        Self {
            live_cells: Relation::new(RelationType::new(heading)),
        }
    }

    /// Creates a board from a relation of live cells.
    pub fn from_relation(live_cells: Relation) -> Self {
        Self { live_cells }
    }

    /// Computes the next generation of the board using pure relational algebra.
    pub fn next_generation(&self) -> Result<Self, Box<dyn std::error::Error>> {
        let board = &self.live_cells;

        // If the board is empty, the next generation is also empty.
        if board.cardinality() == 0 {
            return Ok(Self::new());
        }

        // 1. Create Deltas relation: (dx: Int, dy: Int)
        let delta_heading = TupleType::new()
            .with_attribute("dx", ScalarType::Int)
            .with_attribute("dy", ScalarType::Int);
        let mut deltas = Relation::new(RelationType::new(delta_heading));

        let d_vals = [
            (-1, -1),
            (-1, 0),
            (-1, 1),
            (0, -1),
            (0, 1),
            (1, -1),
            (1, 0),
            (1, 1),
        ];

        for (dx, dy) in d_vals {
            deltas.insert(tuple! { dx: dx as i64, dy: dy as i64 })?;
        }

        // 2. Cartesian product (Join with disjoint headings)
        let board_cross_deltas = board.join(&deltas)?;

        // 3. Extend with neighbor coordinates
        let neighbor_coords = board_cross_deltas
            .extend("nx", ScalarType::Int, |t: &Tuple| {
                let x = t.get_typed::<i64>("x").unwrap();
                let dx = t.get_typed::<i64>("dx").unwrap();
                ScalarValue::Int(x + dx)
            })?
            .extend("ny", ScalarType::Int, |t: &Tuple| {
                let y = t.get_typed::<i64>("y").unwrap();
                let dy = t.get_typed::<i64>("dy").unwrap();
                ScalarValue::Int(y + dy)
            })?;

        // 4. Summarize to count neighbors
        let neighbor_counts =
            neighbor_coords.summarize(&["nx", "ny"], &[Aggregation::count("neighbors")])?;

        // 5. Rename nx->x, ny->y
        let counts = neighbor_counts.rename(&[("nx", "x"), ("ny", "y")]);

        // 6. Find survivals: alive AND 2 neighbors
        // Natural join with board filters down to only cells that are currently alive.
        let alive_counts = counts.join(board)?;
        let survivals = alive_counts
            .restrict(|t: &Tuple| t.get_typed::<i64>("neighbors").unwrap() == 2)
            .project(&["x", "y"]);

        // 7. Find births & survivals with 3 neighbors
        // Any cell (dead or alive) with exactly 3 neighbors will be alive next gen.
        let threes = counts
            .restrict(|t: &Tuple| t.get_typed::<i64>("neighbors").unwrap() == 3)
            .project(&["x", "y"]);

        // 8. Union them for the final next generation
        let next_board = survivals.union(&threes)?;

        Ok(Self::from_relation(next_board))
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blinker() -> Result<(), Box<dyn std::error::Error>> {
        let mut board = Board::new();
        // Vertical blinker
        board.live_cells.insert(tuple! { x: 0i64, y: 0i64 })?;
        board.live_cells.insert(tuple! { x: 0i64, y: 1i64 })?;
        board.live_cells.insert(tuple! { x: 0i64, y: -1i64 })?;

        let gen1 = board.next_generation()?;

        // Should become a horizontal blinker
        assert_eq!(gen1.live_cells.cardinality(), 3);

        let mut expected = Relation::new(board.live_cells.relation_type().clone());
        expected.insert(tuple! { x: -1i64, y: 0i64 })?;
        expected.insert(tuple! { x: 0i64, y: 0i64 })?;
        expected.insert(tuple! { x: 1i64, y: 0i64 })?;

        assert_eq!(gen1.live_cells, expected);

        let gen2 = gen1.next_generation()?;
        // Should revert to vertical
        assert_eq!(gen2.live_cells, board.live_cells);

        Ok(())
    }
}
