//! Conway's Game of Life modeled using relational algebra.
//!
//! This module demonstrates how Conway's Game of Life can be implemented
//! by modeling the grid as a sparse relation of coordinates `(x, y)` and
//! computing generations purely via relational operators (Join, Extend,
//! Summarize, Restrict, Union).
//!
//! # Example
//! ```
//! use relvar::{Relation, RelationType, ScalarType, TupleType, tuple};
//! use relvar::experimental::life::Life;
//!
//! # fn main() -> Result<(), String> {
//! let heading = TupleType::new()
//!     .with_attribute("x", ScalarType::Int)
//!     .with_attribute("y", ScalarType::Int);
//! let mut initial = Relation::new(RelationType::new(heading));
//!
//! // Vertical blinker
//! initial.insert(tuple! { x: 0i64, y: -1i64 }).unwrap();
//! initial.insert(tuple! { x: 0i64, y: 0i64 }).unwrap();
//! initial.insert(tuple! { x: 0i64, y: 1i64 }).unwrap();
//!
//! let life = Life::new(initial)?;
//! let next = life.next_generation()?;
//!
//! assert_eq!(next.grid.cardinality(), 3);
//! # Ok(())
//! # }
//! ```

use relvar_core::algebra::summarize::{Aggregation, AggregationFn};
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue, Tuple};

/// A relational representation of Conway's Game of Life grid.
pub struct Life {
    /// A relation with heading `{x: Int, y: Int}` representing alive cells.
    pub grid: Relation,
}

impl Life {
    /// Creates a new Game of Life state from a relation of live cells.
    ///
    /// The input relation must have the heading `{x: Int, y: Int}`.
    pub fn new(grid: Relation) -> Result<Self, String> {
        let expected = RelationType::new(
            TupleType::new()
                .with_attribute("x", ScalarType::Int)
                .with_attribute("y", ScalarType::Int),
        );
        if *grid.relation_type() != expected {
            return Err("Grid relation must have heading {x: Int, y: Int}".to_string());
        }
        Ok(Self { grid })
    }

    /// Computes the next generation of the game purely relationally.
    pub fn next_generation(&self) -> Result<Life, String> {
        // Step 1: Generate neighborhood deltas: (-1..=1, -1..=1) \ (0, 0)
        let delta_heading = TupleType::new()
            .with_attribute("dx", ScalarType::Int)
            .with_attribute("dy", ScalarType::Int);
        let mut deltas = Relation::new(RelationType::new(delta_heading));
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                deltas
                    .insert(tuple! { dx: dx as i64, dy: dy as i64 })
                    .map_err(|e| e.to_string())?;
            }
        }

        // Step 2: Cartesian product (join on empty intersection) of live cells and deltas
        let adjacent = self.grid.join(&deltas).map_err(|e| e.to_string())?;

        // Step 3: Compute actual neighbor coordinates (nx, ny)
        let neighbor_coords = adjacent
            .extend("nx", ScalarType::Int, |t| {
                let x = t.get_typed::<i64>("x").unwrap();
                let dx = t.get_typed::<i64>("dx").unwrap();
                ScalarValue::Int(x + dx)
            })
            .map_err(|e| e.to_string())?
            .extend("ny", ScalarType::Int, |t| {
                let y = t.get_typed::<i64>("y").unwrap();
                let dy = t.get_typed::<i64>("dy").unwrap();
                ScalarValue::Int(y + dy)
            })
            .map_err(|e| e.to_string())?;

        // Step 4: Summarize to count neighbors for each (nx, ny)
        let neighbor_counts = neighbor_coords
            .summarize(
                &["nx", "ny"],
                &[Aggregation {
                    result_name: "count".to_string(),
                    result_type: ScalarType::Int,
                    function: AggregationFn::Count,
                }],
            )
            .map_err(|e| e.to_string())?;

        // Rename back to (x, y) to match the original grid
        let counts: Relation = neighbor_counts.rename(&[("nx", "x"), ("ny", "y")]);

        // Step 5: Apply Conway's rules
        // Rule 1: Any live cell with 2 or 3 live neighbors survives.
        // Rule 2: Any dead cell with exactly 3 live neighbors becomes a live cell.
        // Equivalent to:
        // Survivors: grid JOIN (counts RESTRICT count = 2 or count = 3) ... actually it's easier to split.

        // Cells with exactly 3 neighbors: either survive or are born
        let three_neighbors = counts.restrict(|t: &Tuple| t.get_typed::<i64>("count") == Some(3));

        // Cells with exactly 2 neighbors: must be alive to survive
        let two_neighbors = counts.restrict(|t: &Tuple| t.get_typed::<i64>("count") == Some(2));
        let alive_with_two = self.grid.join(&two_neighbors).map_err(|e| e.to_string())?;

        // Step 6: Union the two sets
        let next_with_count = three_neighbors
            .union(&alive_with_two)
            .map_err(|e| e.to_string())?;

        // Step 7: Project back to {x, y}
        let next_grid = next_with_count.project(&["x", "y"]);

        Ok(Life { grid: next_grid })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blinker() -> Result<(), String> {
        let heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int);
        let mut initial = Relation::new(RelationType::new(heading));
        // Vertical blinker
        initial.insert(tuple! { x: 0i64, y: -1i64 }).unwrap();
        initial.insert(tuple! { x: 0i64, y: 0i64 }).unwrap();
        initial.insert(tuple! { x: 0i64, y: 1i64 }).unwrap();

        let life = Life::new(initial)?;
        let next = life.next_generation()?;

        // Should become horizontal blinker
        assert_eq!(next.grid.cardinality(), 3);

        let has_cell = |x: i64, y: i64| {
            next.grid
                .tuples()
                .any(|t| t.get_typed::<i64>("x") == Some(x) && t.get_typed::<i64>("y") == Some(y))
        };

        assert!(has_cell(-1, 0));
        assert!(has_cell(0, 0));
        assert!(has_cell(1, 0));

        let next2 = next.next_generation()?;
        // Should go back to vertical blinker
        assert_eq!(next2.grid.cardinality(), 3);
        let has_cell2 = |x: i64, y: i64| {
            next2
                .grid
                .tuples()
                .any(|t| t.get_typed::<i64>("x") == Some(x) && t.get_typed::<i64>("y") == Some(y))
        };
        assert!(has_cell2(0, -1));
        assert!(has_cell2(0, 0));
        assert!(has_cell2(0, 1));

        Ok(())
    }
}
