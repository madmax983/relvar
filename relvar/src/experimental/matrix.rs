//! Experimental Relational Matrix Math.
//!
//! This module demonstrates how to implement mathematical operations on sparse matrices
//! using pure Relational Algebra primitives (Join, Extend, Summarize, Union).
//!
//! # Philosophy
//!
//! A sparse matrix can be naturally modeled as a relation: `(row, col, val)`.
//! Linear algebra operations then become elegant relational expressions:
//! * Addition: Union matching cells, then Summarize (sum values).
//! * Multiplication: Join on A.col = B.row, Extend with A.val * B.val, Summarize (sum values).
//! * Transpose: Rename `row` to `col` and `col` to `row`.

use relvar_core::algebra::summarize::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue, Tuple};

/// A sparse matrix backed by a relation.
pub struct Matrix {
    /// The relation storing the matrix data. Schema: (row: Int, col: Int, val: Float)
    pub relation: Relation,
}

impl Default for Matrix {
    fn default() -> Self {
        Self::new()
    }
}

impl Matrix {
    /// Creates a new, empty Matrix.
    pub fn new() -> Self {
        let heading = TupleType::new()
            .with_attribute("row", ScalarType::Int)
            .with_attribute("col", ScalarType::Int)
            .with_attribute("val", ScalarType::Float);

        let rel_type = RelationType::new(heading);
        Self {
            relation: Relation::new(rel_type),
        }
    }

    /// Sets a value in the matrix.
    pub fn set(&mut self, row: i64, col: i64, val: f64) -> Result<(), DatabaseError> {
        let mut tuple_values = std::collections::BTreeMap::new();
        tuple_values.insert("row".to_string(), ScalarValue::Int(row));
        tuple_values.insert("col".to_string(), ScalarValue::Int(col));
        tuple_values.insert("val".to_string(), ScalarValue::Float(val));

        let heading = self.relation.relation_type().heading().clone();
        let tuple = Tuple::new(heading, tuple_values)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Relation doesn't have a mutable delete method that takes a closure directly.
        // We can use difference, or we can just filter out the tuple we want to replace and create a new relation.
        // Since this is experimental, creating a new relation is fine.
        let without_cell = self.relation.restrict(|t| {
            !(t.get_typed::<i64>("row") == Some(row) && t.get_typed::<i64>("col") == Some(col))
        });

        self.relation = without_cell;

        // Add only non-zero values
        if val != 0.0 {
            self.relation.insert(tuple)?;
        }
        Ok(())
    }

    /// Gets a value from the matrix, returning 0.0 if not found.
    pub fn get(&self, row: i64, col: i64) -> f64 {
        for t in self.relation.tuples() {
            if t.get_typed::<i64>("row") == Some(row) && t.get_typed::<i64>("col") == Some(col) {
                return t.get_typed::<f64>("val").unwrap_or(0.0);
            }
        }
        0.0
    }

    /// Adds another matrix to this one.
    ///
    /// The logic uses Relational Union and Summarize.
    /// 1. Extend both matrices with a "source" attribute (1 for `self`, 2 for `other`).
    ///    This is necessary because Union is a set operation, so if both matrices
    ///    have the EXACT same `(row, col, val)`, the union would discard the duplicate.
    ///    Adding a source attribute preserves multiplicity.
    /// 2. Union the two relations together.
    /// 3. Summarize (Group by) `row` and `col`, and sum the `val`.
    /// 4. This automatically drops the `source` attribute, leaving `(row, col, val)`.
    pub fn add(&self, other: &Matrix) -> Result<Matrix, DatabaseError> {
        let left = self
            .relation
            .extend("source", ScalarType::Int, |_| ScalarValue::Int(1))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let right = other
            .relation
            .extend("source", ScalarType::Int, |_| ScalarValue::Int(2))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let unioned = left
            .union(&right)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let summarized = unioned
            .summarize(&["row", "col"], &[Aggregation::sum_float("val", "val")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(Matrix {
            relation: summarized,
        })
    }

    /// Multiplies this matrix by another matrix.
    ///
    /// The logic uses Relational Join, Extend, and Summarize.
    /// 1. Rename columns in both matrices to prepare for Join.
    ///    A: (r1, c1, v1)
    ///    B: (r2, c2, v2)
    /// 2. Join on A.c1 = B.r2 (which requires renaming B's `row` to `c1` to trigger the natural join).
    /// 3. Extend to compute `v1 * v2`.
    /// 4. Summarize (Group by) `r1` and `c2`, summing the product.
    /// 5. Rename back to `row`, `col`, `val`.
    pub fn multiply(&self, other: &Matrix) -> Result<Matrix, DatabaseError> {
        // A: row -> r1, col -> c1, val -> v1
        let a_renamed = self
            .relation
            .rename(&[("row", "r1"), ("col", "c1"), ("val", "v1")]);

        // B: row -> c1, col -> c2, val -> v2
        let b_renamed = other
            .relation
            .rename(&[("row", "c1"), ("col", "c2"), ("val", "v2")]);

        // Natural Join on c1
        let joined = a_renamed.join(&b_renamed)?;

        // Extend: p = v1 * v2
        let extended = joined
            .extend("p", ScalarType::Float, |t| {
                let v1 = t.get_typed::<f64>("v1").unwrap_or(0.0);
                let v2 = t.get_typed::<f64>("v2").unwrap_or(0.0);
                ScalarValue::Float(v1 * v2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Summarize: Group by r1, c2. sum(p) as v
        let summarized = extended
            .summarize(&["r1", "c2"], &[Aggregation::sum_float("val", "p")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Rename: r1 -> row, c2 -> col
        let final_relation = summarized.rename(&[("r1", "row"), ("c2", "col")]);

        Ok(Matrix {
            relation: final_relation,
        })
    }

    /// Transposes the matrix.
    ///
    /// Simply renames `row` to `col` and `col` to `row`.
    pub fn transpose(&self) -> Result<Matrix, DatabaseError> {
        // We can't rename row -> col and col -> row simultaneously directly if it creates a conflict
        // during the intermediate step depending on the implementation.
        // The rename operator in relvar processes sequentially or simultaneously?
        // Let's use an intermediate name to be safe.
        // row -> temp, col -> row, temp -> col
        let step1 = self.relation.rename(&[("row", "temp")]);
        let step2 = step1.rename(&[("col", "row")]);
        let step3 = step2.rename(&[("temp", "col")]);

        Ok(Matrix { relation: step3 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_add() {
        let mut m1 = Matrix::new();
        m1.set(0, 0, 1.0).unwrap();
        m1.set(0, 1, 2.0).unwrap();

        let mut m2 = Matrix::new();
        m2.set(0, 1, 3.0).unwrap();
        m2.set(1, 0, 4.0).unwrap();

        let m3 = m1.add(&m2).unwrap();

        assert_eq!(m3.get(0, 0), 1.0);
        assert_eq!(m3.get(0, 1), 5.0); // 2.0 + 3.0
        assert_eq!(m3.get(1, 0), 4.0);
        assert_eq!(m3.get(1, 1), 0.0);
    }

    #[test]
    fn test_matrix_add_identical() {
        let mut m1 = Matrix::new();
        m1.set(0, 0, 5.0).unwrap();

        let mut m2 = Matrix::new();
        m2.set(0, 0, 5.0).unwrap();

        let m3 = m1.add(&m2).unwrap();

        assert_eq!(m3.get(0, 0), 10.0); // 5.0 + 5.0
    }

    #[test]
    fn test_matrix_multiply() {
        // [1 2]
        // [3 4]
        let mut m1 = Matrix::new();
        m1.set(0, 0, 1.0).unwrap();
        m1.set(0, 1, 2.0).unwrap();
        m1.set(1, 0, 3.0).unwrap();
        m1.set(1, 1, 4.0).unwrap();

        // [2 0]
        // [1 2]
        let mut m2 = Matrix::new();
        m2.set(0, 0, 2.0).unwrap();
        m2.set(0, 1, 0.0).unwrap();
        m2.set(1, 0, 1.0).unwrap();
        m2.set(1, 1, 2.0).unwrap();

        // [1*2+2*1, 1*0+2*2] = [4, 4]
        // [3*2+4*1, 3*0+4*2] = [10, 8]
        let m3 = m1.multiply(&m2).unwrap();

        assert_eq!(m3.get(0, 0), 4.0);
        assert_eq!(m3.get(0, 1), 4.0);
        assert_eq!(m3.get(1, 0), 10.0);
        assert_eq!(m3.get(1, 1), 8.0);
    }

    #[test]
    fn test_matrix_transpose() {
        let mut m1 = Matrix::new();
        m1.set(0, 1, 2.0).unwrap();
        m1.set(1, 0, 3.0).unwrap();
        m1.set(2, 2, 4.0).unwrap();

        let m2 = m1.transpose().unwrap();

        assert_eq!(m2.get(1, 0), 2.0); // was 0, 1
        assert_eq!(m2.get(0, 1), 3.0); // was 1, 0
        assert_eq!(m2.get(2, 2), 4.0); // was 2, 2
    }
}
