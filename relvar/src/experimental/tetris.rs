//! Relational Tetris Engine
//!
//! This module models the game of Tetris purely using relational algebra.
//! The game board and the falling piece are represented as relations.
//! Game mechanics like collision detection, piece locking, and line clearing
//! are evaluated declaratively using Joins, Semidifferences, and Summarizations.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue},
};

/// A Relational Tetris Engine.
/// # Examples
///
/// ```
/// use relvar::experimental::tetris::TetrisEngine;
/// // Placeholder example
/// ```
pub struct TetrisEngine {
    /// The game board. Schema: (x: Int, y: Int, color: String)
    pub board: Relation,
    /// Width of the board
    pub width: i64,
    /// Height of the board
    pub height: i64,
}

impl TetrisEngine {
    /// Creates a new Tetris engine.
    pub fn new(width: i64, height: i64) -> Self {
        let heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int)
            .with_attribute("color", ScalarType::String);
        let board = Relation::new(RelationType::new(heading));

        Self {
            board,
            width,
            height,
        }
    }

    /// Checks if a piece collides with the board or boundaries.
    pub fn check_collision(&self, piece: &Relation) -> Result<bool, DatabaseError> {
        // 1. Boundary collision
        let width = self.width;
        let height = self.height;
        let out_of_bounds = piece.restrict(move |t: &relvar_core::values::Tuple| {
            let x = t.get_typed::<i64>("x").unwrap();
            let y = t.get_typed::<i64>("y").unwrap();
            x < 0 || x >= width || y >= height
        });

        if out_of_bounds.cardinality() > 0 {
            return Ok(true);
        }

        // 2. Board collision
        let piece_coords = piece.project(&["x", "y"]);
        let board_coords = self.board.project(&["x", "y"]);

        let intersection = piece_coords
            .intersect(&board_coords)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(intersection.cardinality() > 0)
    }

    /// Returns a new piece relation shifted down by 1.
    pub fn move_piece_down(&self, piece: &Relation) -> Result<Relation, DatabaseError> {
        let moved = piece
            .extend(
                "new_y",
                ScalarType::Int,
                |t: &relvar_core::values::Tuple| {
                    let y = t.get_typed::<i64>("y").unwrap();
                    ScalarValue::Int(y + 1)
                },
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["x", "new_y", "color"])
            .rename(&[("new_y", "y")]);

        Ok(moved)
    }

    /// Locks a piece into the board and clears full lines.
    pub fn lock_piece(&mut self, piece: &Relation) -> Result<(), DatabaseError> {
        self.board = self
            .board
            .union(piece)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        self.clear_lines()
    }

    fn clear_lines(&mut self) -> Result<(), DatabaseError> {
        if self.board.cardinality() == 0 {
            return Ok(());
        }

        // 1. Count blocks per row
        let row_counts = self
            .board
            .summarize(&["y"], &[Aggregation::count("blocks")]);

        // 2. Identify full lines
        let width = self.width;
        let full_lines = row_counts
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .restrict(move |t: &relvar_core::values::Tuple| {
                t.get_typed::<i64>("blocks").unwrap() == width
            });

        if full_lines.cardinality() == 0 {
            return Ok(());
        }

        // 3. Remove full lines from board
        let full_y = full_lines.project(&["y"]);
        let remaining_board = self.board.semidifference(&full_y);

        if remaining_board.cardinality() == 0 {
            self.board = remaining_board;
            return Ok(());
        }

        let full_y_renamed = full_y.rename(&[("y", "full_y")]);

        // 4. Shift blocks above down
        let theta_joined = remaining_board.theta_join(
            &full_y_renamed,
            |b: &relvar_core::values::Tuple, l: &relvar_core::values::Tuple| {
                let by = b.get_typed::<i64>("y").unwrap();
                let ly = l.get_typed::<i64>("full_y").unwrap();
                by < ly
            },
        );

        let shifts = theta_joined.summarize(&["x", "y", "color"], &[Aggregation::count("shift")]);

        let shifts_rel = shifts.map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let shifted_coords = shifts_rel.project(&["x", "y", "color"]);
        let no_shift = remaining_board.semidifference(&shifted_coords);

        let no_shift_extended = no_shift
            .extend("shift", ScalarType::Int, |_| ScalarValue::Int(0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let all_blocks = shifts_rel
            .union(&no_shift_extended)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        self.board = all_blocks
            .extend(
                "new_y",
                ScalarType::Int,
                |t: &relvar_core::values::Tuple| {
                    let y = t.get_typed::<i64>("y").unwrap();
                    let shift = t.get_typed::<i64>("shift").unwrap();
                    ScalarValue::Int(y + shift)
                },
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["x", "new_y", "color"])
            .rename(&[("new_y", "y")]);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;

    fn create_piece(coords: &[(i64, i64)]) -> Relation {
        let heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int)
            .with_attribute("color", ScalarType::String);
        let mut piece = Relation::new(RelationType::new(heading));

        for &(x, y) in coords {
            piece.insert(tuple! { x: x, y: y, color: "red" }).unwrap();
        }
        piece
    }

    #[test]
    fn test_collision_and_movement() {
        let engine = TetrisEngine::new(10, 20);
        let piece = create_piece(&[(0, 0), (1, 0), (0, 1), (1, 1)]); // O piece

        // No collision initially
        assert!(!engine.check_collision(&piece).unwrap());

        // Move down
        let moved = engine.move_piece_down(&piece).unwrap();

        let t = moved.tuples().find(|t| {
            t.get_typed::<i64>("x").unwrap() == 0 && t.get_typed::<i64>("y").unwrap() == 1
        });
        assert!(t.is_some());

        // Out of bounds
        let out_bounds_piece = create_piece(&[(0, 20)]);
        assert!(engine.check_collision(&out_bounds_piece).unwrap());
    }

    #[test]
    fn test_line_clear() {
        let mut engine = TetrisEngine::new(4, 5); // small board

        // Add a full line at y=4
        let line4 = create_piece(&[(0, 4), (1, 4), (2, 4), (3, 4)]);
        engine.lock_piece(&line4).unwrap();
        assert_eq!(engine.board.cardinality(), 0); // Should be cleared!

        // Add a block at y=3 and a full line at y=4
        let block = create_piece(&[(1, 3)]);
        engine.lock_piece(&block).unwrap();

        let line4 = create_piece(&[(0, 4), (1, 4), (2, 4), (3, 4)]);
        engine.lock_piece(&line4).unwrap();

        // Block should have fallen to y=4
        assert_eq!(engine.board.cardinality(), 1);
        let t = engine.board.tuples().next().unwrap();
        assert_eq!(t.get_typed::<i64>("x").unwrap(), 1);
        assert_eq!(t.get_typed::<i64>("y").unwrap(), 4);
    }
}
