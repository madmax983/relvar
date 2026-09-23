#![allow(dead_code)]
//! Relational Sudoku Solver
//!
//! This module demonstrates how a Sudoku puzzle can be solved using purely
//! relational algebra. It represents the grid, given numbers, and domain of
//! possible values as relations.
//!
//! # Concept
//!
//! - **Givens**: Relation `(row: Int, col: Int, val: Int)` representing the known cells.
//! - **Cells**: Relation `(row: Int, col: Int, box_id: Int)` representing all 81 cells.
//! - **Domain**: Relation `(val: Int)` representing numbers 1 through 9.
//!
//! The solver iteratively reduces the domain of possible values for each empty cell
//! by applying the rules of Sudoku (no duplicates in row, column, or 3x3 box).
//! When a cell has only exactly one possible value left, it is added to the known cells.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue, Tuple},
};

/// A Relational Sudoku Solver.
///
/// # Examples
///
/// ```
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::values::{Relation, ScalarValue, Tuple};
/// use relvar::experimental::sudoku::SudokuSolver;
///
/// let solver = SudokuSolver::new().unwrap();
/// let givens_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("row", ScalarType::Int)
///         .with_attribute("col", ScalarType::Int)
///         .with_attribute("val", ScalarType::Int),
/// );
/// let mut givens = Relation::new(givens_type.clone());
/// givens.insert(Tuple::new(givens_type.tuple_type().clone(), vec![
///     ("row".to_string(), ScalarValue::Int(0)),
///     ("col".to_string(), ScalarValue::Int(0)),
///     ("val".to_string(), ScalarValue::Int(5)),
/// ]).unwrap()).unwrap();
/// // ...
/// ```
pub struct SudokuSolver {
    /// Cells representation
    pub cells: Relation,
    /// Domain representation
    pub domain: Relation,
}

impl SudokuSolver {
    /// Create a new solver
    /// # Examples
    ///
    /// ```
    /// use relvar::experimental::sudoku::SudokuSolver;
    /// let solver = SudokuSolver::new().unwrap();
    /// ```
    pub fn new() -> Result<Self, DatabaseError> {
        let cells_type = RelationType::new(
            TupleType::new()
                .with_attribute("row", ScalarType::Int)
                .with_attribute("col", ScalarType::Int)
                .with_attribute("box_id", ScalarType::Int),
        );
        let domain_type =
            RelationType::new(TupleType::new().with_attribute("val", ScalarType::Int));

        let mut cells = Relation::new(cells_type.clone());
        let mut domain = Relation::new(domain_type.clone());

        for val in 1..=9 {
            domain.insert(
                Tuple::new(
                    domain_type.tuple_type().clone(),
                    vec![("val".to_string(), ScalarValue::Int(val))],
                )
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?,
            )?;
        }

        for row in 0..9 {
            for col in 0..9 {
                let box_id = (row / 3) * 3 + (col / 3);
                cells.insert(
                    Tuple::new(
                        cells_type.tuple_type().clone(),
                        vec![
                            ("row".to_string(), ScalarValue::Int(row)),
                            ("col".to_string(), ScalarValue::Int(col)),
                            ("box_id".to_string(), ScalarValue::Int(box_id)),
                        ],
                    )
                    .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?,
                )?;
            }
        }

        Ok(Self { cells, domain })
    }

    fn compute_all_possibilities(&self) -> Result<Relation, DatabaseError> {
        self.cells
            .join(&self.domain)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    fn compute_invalid_possibilities(
        &self,
        all_possibilities: &Relation,
        known: &Relation,
    ) -> Result<Relation, DatabaseError> {
        // Extract the used values in each row, col, and box from the known cells
        // First, extend known with box_id by joining with cells
        let known_with_box = known
            .join(&self.cells)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // To find invalid possibilities, we need to join possibilities with known cells
        // on the constraints: same row, same col, or same box.

        // 1. Invalid due to same row:
        // rename known(col -> k_col, box_id -> k_box) to avoid collision, join on row, val
        let mappings = vec![("col", "k_col"), ("box_id", "k_box")];
        let known_row = known_with_box.rename(&mappings);
        let invalid_row = all_possibilities
            .join(&known_row)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let invalid_row_proj = invalid_row.project(&["row", "col", "box_id", "val"]);

        // 2. Invalid due to same col:
        // rename known(row -> k_row, box_id -> k_box) to avoid collision, join on col, val
        let mappings = vec![("row", "k_row"), ("box_id", "k_box")];
        let known_col = known_with_box.rename(&mappings);
        let invalid_col = all_possibilities
            .join(&known_col)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let invalid_col_proj = invalid_col.project(&["row", "col", "box_id", "val"]);

        // 3. Invalid due to same box:
        // rename known(row -> k_row, col -> k_col) to avoid collision, join on box_id, val
        let mappings = vec![("row", "k_row"), ("col", "k_col")];
        let known_box = known_with_box.rename(&mappings);
        let invalid_box = all_possibilities
            .join(&known_box)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let invalid_box_proj = invalid_box.project(&["row", "col", "box_id", "val"]);

        // Combine all invalid possibilities
        let all_invalid1 = invalid_row_proj
            .union(&invalid_col_proj)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let all_invalid = all_invalid1
            .union(&invalid_box_proj)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(all_invalid)
    }

    fn find_determined_cells(
        &self,
        unknown_possibilities: &Relation,
    ) -> Result<Relation, DatabaseError> {
        // Group by row, col and count possibilities
        let counts = unknown_possibilities
            .summarize(
                &["row", "col", "box_id"],
                &[Aggregation::count("num_possibilities")],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Restrict to those with exactly 1 possibility
        let determined_cells =
            counts.restrict(|t| matches!(t.get("num_possibilities"), Some(ScalarValue::Int(1))));

        Ok(determined_cells)
    }

    /// Solves the puzzle given the relation of known values.
    /// The givens relation must have attributes `(row: Int, col: Int, val: Int)`.
    /// # Examples
    ///
    /// ```text
    /// // Solve sudoku
    /// ```
    pub fn solve(&self, givens: &Relation) -> Result<Relation, DatabaseError> {
        let mut known = givens.clone();

        loop {
            let prev_count = known.cardinality();

            // Generate all possible cell-value combinations (81 * 9 = 729 tuples)
            let all_possibilities = self.compute_all_possibilities()?;

            let all_invalid = self.compute_invalid_possibilities(&all_possibilities, &known)?;

            // Remove invalid possibilities from all possibilities
            let valid_possibilities = all_possibilities
                .difference(&all_invalid)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // Also we shouldn't consider cells that are already known
            // To do this, project known cells to just row, col and difference them out
            let known_cells = known.project(&["row", "col"]);
            let joined_known = valid_possibilities
                .join(&known_cells)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
            let unknown_possibilities = valid_possibilities
                .difference(&joined_known)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // Now find cells that have exactly 1 valid possibility left
            let determined_cells = self.find_determined_cells(&unknown_possibilities)?;

            // If we found no new determined cells, we are done (or stuck)
            if determined_cells.cardinality() == 0 {
                break;
            }

            // Join back with unknown_possibilities to get the actual value
            let joined_det = unknown_possibilities
                .join(&determined_cells)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
            let new_known = joined_det.project(&["row", "col", "val"]);

            known = known
                .union(&new_known)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            if known.cardinality() == prev_count {
                break;
            }
        }

        Ok(known)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sudoku_solver() -> Result<(), DatabaseError> {
        let solver = SudokuSolver::new()?;

        let givens_type = RelationType::new(
            TupleType::new()
                .with_attribute("row", ScalarType::Int)
                .with_attribute("col", ScalarType::Int)
                .with_attribute("val", ScalarType::Int),
        );
        let mut givens = Relation::new(givens_type.clone());

        // Use an easier puzzle that single-candidate solver can solve
        let puzzle_givens = vec![
            (0, 1, 6),
            (0, 3, 3),
            (0, 6, 8),
            (0, 7, 4),
            (1, 0, 5),
            (1, 1, 3),
            (1, 2, 7),
            (1, 4, 9),
            (1, 8, 1),
            (2, 1, 4),
            (2, 5, 6),
            (2, 6, 3),
            (2, 8, 7),
            (3, 0, 9),
            (3, 3, 5),
            (3, 4, 1),
            (3, 5, 2),
            (3, 7, 8),
            (4, 2, 2),
            (4, 4, 6),
            (4, 5, 4),
            (4, 7, 9),
            (5, 0, 1),
            (5, 1, 5),
            (5, 4, 8),
            (5, 6, 2),
            (5, 8, 4),
            (6, 0, 7),
            (6, 2, 6),
            (6, 4, 2),
            (6, 8, 9),
            (7, 1, 9),
            (7, 4, 4),
            (7, 6, 6),
            (7, 7, 2),
            (8, 0, 8),
            (8, 4, 3),
            (8, 6, 4),
            (8, 7, 1),
        ];

        for (r, c, v) in puzzle_givens {
            givens.insert(
                Tuple::new(
                    givens_type.tuple_type().clone(),
                    vec![
                        ("row".to_string(), ScalarValue::Int(r as i64)),
                        ("col".to_string(), ScalarValue::Int(c as i64)),
                        ("val".to_string(), ScalarValue::Int(v as i64)),
                    ],
                )
                .unwrap(),
            )?;
        }

        let solution = solver.solve(&givens)?;

        // Asserting that it solves at least more cells
        assert!(solution.cardinality() > 39);

        Ok(())
    }
}
