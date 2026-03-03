//! Relational Cellular Automata (Game of Life).
//!
//! This module implements Conway's Game of Life using pure relational algebra.
//!
//! # Concept
//!
//! Spatial simulations often rely on 2D arrays and nested loops. By modeling
//! the grid as a relation `{x: Int, y: Int}` of alive cells, we can apply
//! relational operations like `Join`, `Extend`, and `Summarize` to compute
//! the next generation entirely within the relational engine.
//!
//! # Example
//!
//! ```
//! use relvar::experimental::cellular_automata::GameOfLife;
//!
//! let mut game = GameOfLife::new();
//! // Create a blinker
//! game.insert(1, 0).unwrap();
//! game.insert(1, 1).unwrap();
//! game.insert(1, 2).unwrap();
//!
//! // Evolve
//! game.next_generation().unwrap();
//! ```

use relvar_core::algebra::Aggregation;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue, Tuple};

/// Conway's Game of Life implemented using purely relational algebra.
///
/// This demonstrates how complex spatial rules can be expressed
/// as a series of relational operations (Join, Extend, Summarize, Restrict).
pub struct GameOfLife {
    /// The current state of the board. A relation with heading `{x: Int, y: Int}`.
    pub cells: Relation,
}

impl Default for GameOfLife {
    fn default() -> Self {
        Self::new()
    }
}

impl GameOfLife {
    /// Creates a new empty Game of Life board.
    pub fn new() -> Self {
        let heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int);
        Self {
            cells: Relation::new(RelationType::new(heading)),
        }
    }

    /// Inserts a living cell at coordinates (x, y).
    pub fn insert(&mut self, x: i64, y: i64) -> Result<(), Box<dyn std::error::Error>> {
        self.cells.insert(tuple! { x: x, y: y })?;
        Ok(())
    }

    /// Computes the next generation of cells using relational algebra.
    pub fn next_generation(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Generate Deltas (-1, -1) to (1, 1) excluding (0, 0)
        let delta_heading = TupleType::new()
            .with_attribute("dx", ScalarType::Int)
            .with_attribute("dy", ScalarType::Int);
        let mut deltas = Relation::new(RelationType::new(delta_heading));
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx != 0 || dy != 0 {
                    deltas.insert(tuple! { dx: dx as i64, dy: dy as i64 })?;
                }
            }
        }

        // 2. Compute Neighbors of Alive Cells
        // Rename current cells to (cx, cy) to distinguish from target neighbor coords (x, y)
        let c = self.cells.rename(&[("x", "cx"), ("y", "cy")]);

        // Cartesian Product (Cross Join) with Deltas:
        // { cx, cy, dx, dy }
        let cd = c.join(&deltas).unwrap();

        // Extend to calculate target neighbor coordinates (x = cx + dx, y = cy + dy)
        let cde_x = cd
            .extend("x", ScalarType::Int, |t: &Tuple| {
                let cx = t.get_typed::<i64>("cx").unwrap();
                let dx = t.get_typed::<i64>("dx").unwrap();
                ScalarValue::Int(cx + dx)
            })
            .unwrap();

        let cde = cde_x
            .extend("y", ScalarType::Int, |t: &Tuple| {
                let cy = t.get_typed::<i64>("cy").unwrap();
                let dy = t.get_typed::<i64>("dy").unwrap();
                ScalarValue::Int(cy + dy)
            })
            .unwrap();

        // Neighbors(x, y, cx, cy)
        // A tuple here means "The cell at (x, y) has a living neighbor at (cx, cy)"
        let neighbors = cde.project(&["x", "y", "cx", "cy"]);

        // 3. Count Neighbors
        // Summarize by target coordinates (x, y) to count living neighbors
        let neighbor_counts = neighbors
            .summarize(&["x", "y"], &[Aggregation::count("count")])
            .unwrap();

        // 4. Apply Rules
        // Rule 1 & 2: Any live cell with 2 or 3 live neighbors survives
        // Join AliveCells with NeighborCounts (Natural Join on x, y)
        let alive_with_counts = self.cells.join(&neighbor_counts).unwrap();
        let surviving = alive_with_counts
            .restrict(|t: &Tuple| {
                let count = t.get_typed::<i64>("count").unwrap();
                count == 2 || count == 3
            })
            .project(&["x", "y"]);

        // Rule 3: Any dead cell with exactly 3 live neighbors becomes a live cell
        // We need counts for cells that are currently dead.
        // First, get the counts for currently alive cells.
        let alive_counts_only = alive_with_counts.project(&["x", "y", "count"]);

        // Then, subtract them from ALL neighbor counts to get counts for DEAD cells.
        let dead_counts = neighbor_counts.difference(&alive_counts_only).unwrap();

        // Restrict to exactly 3 neighbors
        let new_cells = dead_counts
            .restrict(|t: &Tuple| {
                let count = t.get_typed::<i64>("count").unwrap();
                count == 3
            })
            .project(&["x", "y"]);

        // 5. Union surviving and new cells
        self.cells = surviving.union(&new_cells).unwrap();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blinker() {
        let mut game = GameOfLife::new();

        // Vertical blinker
        game.insert(1, 0).unwrap();
        game.insert(1, 1).unwrap();
        game.insert(1, 2).unwrap();

        assert_eq!(game.cells.cardinality(), 3);

        game.next_generation().unwrap();

        // Should become horizontal blinker
        assert_eq!(game.cells.cardinality(), 3);

        let horizontal_blinker = game.cells.clone();

        let target_heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int);
        let mut target = Relation::new(RelationType::new(target_heading));
        target.insert(tuple! { x: 0i64, y: 1i64 }).unwrap();
        target.insert(tuple! { x: 1i64, y: 1i64 }).unwrap();
        target.insert(tuple! { x: 2i64, y: 1i64 }).unwrap();

        assert_eq!(horizontal_blinker, target);

        // Step again
        game.next_generation().unwrap();

        // Should become vertical again
        assert_eq!(game.cells.cardinality(), 3);
        let vertical_target_heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int);
        let mut vertical_target = Relation::new(RelationType::new(vertical_target_heading));
        vertical_target.insert(tuple! { x: 1i64, y: 0i64 }).unwrap();
        vertical_target.insert(tuple! { x: 1i64, y: 1i64 }).unwrap();
        vertical_target.insert(tuple! { x: 1i64, y: 2i64 }).unwrap();

        assert_eq!(game.cells, vertical_target);
    }
}
