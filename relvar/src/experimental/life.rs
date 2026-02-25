//! Relational Conway's Game of Life.
//!
//! This module implements Conway's Game of Life using pure relational algebra.
//! The grid is infinite and sparse, modeled as a relation of live cells `(x, y)`.
//!
//! # Rules
//!
//! 1. **Survival**: A live cell with 2 or 3 live neighbors stays alive.
//! 2. **Birth**: A dead cell with exactly 3 live neighbors becomes alive.
//! 3. **Death**: All other live cells die (under/over-population).
//!
//! # Relational Implementation
//!
//! The algorithm computes the next generation using the following steps:
//!
//! 1. **Find Candidates**: Compute all cells that have at least one neighbor.
//!    - Join current `cells` with `neighbors` offsets (Cartesian Product).
//!    - Extend to calculate neighbor coordinates `(nx, ny)`.
//! 2. **Count Neighbors**: Summarize candidates by `(nx, ny)` to get `count`.
//! 3. **Identify Survivors**:
//!    - Join `neighbor_counts` with `cells` (Intersection of location).
//!    - Restrict where `count == 2 || count == 3`.
//! 4. **Identify Births**:
//!    - Restrict `neighbor_counts` where `count == 3`.
//!    - Difference `cells` (remove existing live cells).
//! 5. **Union**: Combine Survivors and Births.

use relvar_core::algebra::summarize::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

/// A wrapper for the Game of Life simulation.
pub struct Life {
    cells: Relation,
    neighbors: Relation,
}

impl Life {
    /// Creates a new Game of Life simulation with the given initial live cells.
    ///
    /// The input relation must have attributes "x" and "y" of type Int.
    pub fn new(cells: Relation) -> Self {
        // Create the constant neighbor offset relation
        let heading = TupleType::new()
            .with_attribute("dx", ScalarType::Int)
            .with_attribute("dy", ScalarType::Int);

        let mut neighbors = Relation::new(RelationType::new(heading));

        // 8 Neighbors
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                neighbors
                    .insert(tuple! { dx: dx as i64, dy: dy as i64 })
                    .unwrap();
            }
        }

        Self { cells, neighbors }
    }

    /// Computes the next generation of cells.
    pub fn step(&self) -> Result<Relation, DatabaseError> {
        // 1. Generate Neighbor Coordinates (Candidates)
        // Cross Join: cells (x, y) x neighbors (dx, dy) -> (x, y, dx, dy)
        let joined = self.cells.join(&self.neighbors)?;

        // Extend: nx = x + dx, ny = y + dy
        let with_coords = joined
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

        // 2. Count Neighbors per Cell location
        // Group by (nx, ny), Count(*) -> (nx, ny, count)
        let counts = with_coords
            .summarize(&["nx", "ny"], &[Aggregation::count("count")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Identify Survivors: (nx, ny) in counts AND (x, y) in cells AND (count == 2 || count == 3)
        // We need to join counts with cells.
        // Rename counts (nx, ny) -> (x, y) to join with cells
        let rename_map = vec![("nx", "x"), ("ny", "y")];
        let counts_renamed = counts.rename(&rename_map);

        // Intersection (Join) with current live cells
        // Result has (x, y, count) for existing cells
        let existing_with_counts = counts_renamed.join(&self.cells)?;

        // Filter: count == 2 || count == 3
        let survivors_with_count = existing_with_counts.restrict(|t| {
            let c = t.get_typed::<i64>("count").unwrap();
            c == 2 || c == 3
        });

        let survivors = survivors_with_count.project(&["x", "y"]);

        // 4. Identify Births: (x, y) NOT in cells AND (count == 3)
        // Filter counts_renamed where count == 3
        let birth_candidates_with_count = counts_renamed.restrict(|t| {
            let c = t.get_typed::<i64>("count").unwrap();
            c == 3
        });

        let birth_candidates = birth_candidates_with_count.project(&["x", "y"]);

        // Difference: candidates - cells
        // Note: Relation::difference requires exact tuple match.
        // Both have schema (x, y).
        let born = birth_candidates
            .difference(&self.cells)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Union Survivors and Born
        survivors
            .union(&born)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    /// Returns the current state.
    pub fn current(&self) -> &Relation {
        &self.cells
    }

    /// Advances the simulation by N generations.
    pub fn advance(&mut self, generations: usize) -> Result<(), DatabaseError> {
        for _ in 0..generations {
            self.cells = self.step()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_world(coords: &[(i64, i64)]) -> Relation {
        let heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int);
        let mut rel = Relation::new(RelationType::new(heading));

        for (x, y) in coords {
            rel.insert(tuple! { x: *x, y: *y }).unwrap();
        }
        rel
    }

    fn check_world(rel: &Relation, expected: &[(i64, i64)]) {
        assert_eq!(rel.cardinality(), expected.len());
        for (x, y) in expected {
            let t = tuple! { x: *x, y: *y };
            assert!(rel.contains(&t), "Missing cell at ({}, {})", x, y);
        }
    }

    #[test]
    fn test_block_still_life() {
        // Block (2x2 square) is stable
        // (0,0), (1,0)
        // (0,1), (1,1)
        let initial = vec![(0, 0), (1, 0), (0, 1), (1, 1)];
        let rel = create_world(&initial);
        let mut life = Life::new(rel);

        life.advance(1).unwrap();
        check_world(life.current(), &initial);

        life.advance(5).unwrap();
        check_world(life.current(), &initial);
    }

    #[test]
    fn test_blinker_oscillator() {
        // Blinker (Period 2)
        // Gen 0: Horizontal (0,0), (1,0), (2,0)
        // Gen 1: Vertical (1,-1), (1,0), (1,1)
        let initial = vec![(0, 0), (1, 0), (2, 0)];
        let rel = create_world(&initial);
        let mut life = Life::new(rel);

        // Step 1: Should become vertical
        life.advance(1).unwrap();
        let expected_gen1 = vec![(1, -1), (1, 0), (1, 1)];
        check_world(life.current(), &expected_gen1);

        // Step 2: Should become horizontal again
        life.advance(1).unwrap();
        check_world(life.current(), &initial);
    }

    #[test]
    fn test_glider() {
        // Glider moves (1, 1) every 4 generations
        // 0,1
        // 1,2
        // 2,0  2,1  2,2
        let initial = vec![(0, 1), (1, 2), (2, 0), (2, 1), (2, 2)];
        let rel = create_world(&initial);
        let mut life = Life::new(rel);

        life.advance(4).unwrap();

        // Expected: same shape, shifted by (1, 1)
        let expected: Vec<(i64, i64)> = initial.iter().map(|(x, y)| (x + 1, y + 1)).collect();
        check_world(life.current(), &expected);
    }
}
