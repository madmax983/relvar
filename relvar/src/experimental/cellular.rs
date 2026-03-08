//! Relational Cellular Automata (Conway's Game of Life)
//!
//! This module demonstrates how complex spatial simulations like cellular automata
//! can be implemented using pure relational algebra without imperative 2D array loops.
//!
//! # Concept
//!
//! The universe is simply a relation of `Alive(x: Int, y: Int)`.
//! To compute the next generation:
//! 1. **Cross Join** `Alive` with an `Offsets` relation to generate all neighbor coordinates.
//! 2. **Extend** to compute absolute neighbor coordinates `(nx, ny)`.
//! 3. **Summarize** to count alive neighbors for each coordinate `(nx, ny)`.
//! 4. **Join** and **Difference** to apply the rules of survival and birth.
//! 5. **Union** the surviving and newly born cells.

use relvar_core::algebra::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

/// A relational implementation of Conway's Game of Life.
pub struct GameOfLife {
    /// The relation containing currently alive cells: (x: Int, y: Int).
    pub alive: Relation,
    /// The relation containing the 8 neighbor offsets: (dx: Int, dy: Int).
    offsets: Relation,
}

impl GameOfLife {
    /// Creates a new Game of Life simulation with the given initial alive cells.
    ///
    /// The `initial_state` must have the heading `(x: Int, y: Int)`.
    pub fn new(initial_state: Relation) -> Result<Self, DatabaseError> {
        // Validate heading
        let heading = initial_state.relation_type().heading();
        if heading.get_attribute_type("x") != Some(&ScalarType::Int)
            || heading.get_attribute_type("y") != Some(&ScalarType::Int)
            || heading.attributes().len() != 2
        {
            return Err(DatabaseError::AlgebraError(
                "Initial state must have heading (x: Int, y: Int)".to_string(),
            ));
        }

        // Create offsets relation
        let offsets_heading = TupleType::new()
            .with_attribute("dx", ScalarType::Int)
            .with_attribute("dy", ScalarType::Int);
        let mut offsets = Relation::new(RelationType::new(offsets_heading));

        let deltas = [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ];

        for (dx, dy) in deltas {
            offsets.insert(tuple! { dx: dx as i64, dy: dy as i64 })?;
        }

        Ok(Self {
            alive: initial_state,
            offsets,
        })
    }

    /// Computes the next generation of the simulation.
    pub fn tick(&mut self) -> Result<(), DatabaseError> {
        if self.alive.is_empty() {
            return Ok(());
        }

        // 1. Cartesian product of Alive(x, y) and Offsets(dx, dy)
        // Since attributes are disjoint, join performs a cartesian product.
        let cartesian = self.alive.join(&self.offsets)?;

        // 2. Extend to compute nx = x + dx, ny = y + dy
        let extended = cartesian
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

        // 3. Summarize by (nx, ny) to count neighbors
        let neighbor_counts = extended
            .summarize(&["nx", "ny"], &[Aggregation::count("alive_count")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Rename nx->x, ny->y so it matches the Alive relation
        let counts_renamed = neighbor_counts.rename(&[("nx", "x"), ("ny", "y")]);

        // 5. Compute Surviving cells
        // Join with Alive to get only currently alive cells with their neighbor counts.
        let alive_with_counts = counts_renamed.join(&self.alive)?;

        let surviving = alive_with_counts
            .restrict(|t| {
                let count = t.get_typed::<i64>("alive_count").unwrap();
                count == 2 || count == 3
            })
            .project(&["x", "y"]);

        // 6. Compute Born cells
        // Restrict neighbor counts to exactly 3.
        let exactly_3 =
            counts_renamed.restrict(|t| t.get_typed::<i64>("alive_count").unwrap() == 3);

        // Project to (x, y) to make heading compatible for Difference
        let born_candidates = exactly_3.project(&["x", "y"]);

        // Born = BornCandidates MINUS Alive
        let born = born_candidates
            .difference(&self.alive)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 7. Next generation = Surviving UNION Born
        self.alive = surviving
            .union(&born)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blinker() {
        // A blinker is a horizontal line of 3 cells that becomes vertical, then horizontal again.
        let heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int);
        let mut initial = Relation::new(RelationType::new(heading));

        // Horizontal line at y = 0: (-1, 0), (0, 0), (1, 0)
        initial.insert(tuple! { x: -1i64, y: 0i64 }).unwrap();
        initial.insert(tuple! { x: 0i64, y: 0i64 }).unwrap();
        initial.insert(tuple! { x: 1i64, y: 0i64 }).unwrap();

        let mut game = GameOfLife::new(initial).unwrap();

        // Tick 1: should become vertical at x = 0: (0, -1), (0, 0), (0, 1)
        game.tick().unwrap();

        assert_eq!(game.alive.cardinality(), 3);

        let has_cell = |g: &GameOfLife, x: i64, y: i64| {
            g.alive.tuples().any(|t| {
                t.get_typed::<i64>("x").unwrap() == x && t.get_typed::<i64>("y").unwrap() == y
            })
        };

        assert!(has_cell(&game, 0, -1));
        assert!(has_cell(&game, 0, 0));
        assert!(has_cell(&game, 0, 1));

        // Tick 2: should become horizontal again
        game.tick().unwrap();

        assert_eq!(game.alive.cardinality(), 3);
        assert!(has_cell(&game, -1, 0));
        assert!(has_cell(&game, 0, 0));
        assert!(has_cell(&game, 1, 0));
    }

    #[test]
    fn test_block() {
        // A block is a 2x2 square that never changes.
        let heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int);
        let mut initial = Relation::new(RelationType::new(heading));

        initial.insert(tuple! { x: 0i64, y: 0i64 }).unwrap();
        initial.insert(tuple! { x: 1i64, y: 0i64 }).unwrap();
        initial.insert(tuple! { x: 0i64, y: 1i64 }).unwrap();
        initial.insert(tuple! { x: 1i64, y: 1i64 }).unwrap();

        let mut game = GameOfLife::new(initial).unwrap();

        game.tick().unwrap();

        assert_eq!(game.alive.cardinality(), 4);

        let has_cell = |g: &GameOfLife, x: i64, y: i64| {
            g.alive.tuples().any(|t| {
                t.get_typed::<i64>("x").unwrap() == x && t.get_typed::<i64>("y").unwrap() == y
            })
        };

        assert!(has_cell(&game, 0, 0));
        assert!(has_cell(&game, 1, 0));
        assert!(has_cell(&game, 0, 1));
        assert!(has_cell(&game, 1, 1));
    }
}
