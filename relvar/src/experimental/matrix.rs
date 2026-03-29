//! Relational Linear Algebra.
//!
//! This module demonstrates how sparse matrices and linear algebra operations
//! (like addition and multiplication) can be implemented using purely relational
//! algebra operations.
//!
//! # Concept
//!
//! A sparse matrix is modeled as a relation with heading `(row: Int, col: Int, val: Float)`.
//! Only non-zero entries are stored.
//!
//! - **Matrix Addition ($A + B$)**: Performed via a Full Outer Join (simulated with Union and Join)
//!   followed by aggregation to sum the values.
//! - **Matrix Multiplication ($A \times B$)**: Performed via a Join on $A.col = B.row$,
//!   extending with $A.val \times B.val$, and summarizing by $(A.row, B.col)$ to sum the products.

use relvar_core::algebra::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::types::ScalarType;
use relvar_core::values::{Relation, ScalarValue};

/// Adds another matrix to this one.
///
/// Computes $C = A + B$.
///
/// The algorithm uses relational algebra:
/// 1. Union $A$ and $B$.
/// 2. Summarize (Group By) `row` and `col`, taking the sum of `val`.
///
/// The input relations must have heading `(row: Int, col: Int, val: Float)`.
pub fn add_matrices(a: &Relation, b: &Relation) -> Result<Relation, DatabaseError> {
    // Union A and B. Since they have the same heading, this works.
    // However, union deduplicates exact tuples. If A and B have the same (row, col, val),
    // union will keep one. If they have different vals, both are kept.
    // Wait, union deduplication is fine, but if we just union and then group by row/col and sum,
    // it works perfectly for matrix addition!
    // But what if A and B have EXACTLY the same (row, col, val)?
    // E.g., A = (1, 1, 5.0), B = (1, 1, 5.0).
    // Union gives (1, 1, 5.0).
    // Group by (1, 1), sum(val) gives 5.0. BUT it should be 10.0!
    // This is a classic relational algebra vs linear algebra mismatch.
    // Sets don't preserve multiplicity.

    // Solution: We need to tag the sources to preserve multiplicity before unioning!

    let a_tagged = a
        .extend("source", ScalarType::String, |_| {
            ScalarValue::String("A".to_string())
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    let b_tagged = b
        .extend("source", ScalarType::String, |_| {
            ScalarValue::String("B".to_string())
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    let unioned = a_tagged
        .union(&b_tagged)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    let summed = unioned
        .summarize(&["row", "col"], &[Aggregation::sum_float("sum_val", "val")])
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Rename sum_val back to val
    Ok(summed.rename(&[("sum_val", "val")]))
}

/// Multiplies this matrix by another.
///
/// Computes $C = A \times B$.
///
/// The algorithm uses relational algebra:
/// 1. Rename attributes to prepare for join: $A(row, k, val\_a)$, $B(k, col, val\_b)$.
/// 2. Natural Join on $k$.
/// 3. Extend with $product = val\_a \times val\_b$.
/// 4. Summarize by $row, col$, summing the products.
///
/// The input relations must have heading `(row: Int, col: Int, val: Float)`.
pub fn multiply_matrices(a: &Relation, b: &Relation) -> Result<Relation, DatabaseError> {
    // Prepare A: (row, k, val_a)
    let a_renamed = a.rename(&[("col", "k"), ("val", "val_a")]);

    // Prepare B: (k, col, val_b)
    let b_renamed = b.rename(&[("row", "k"), ("val", "val_b")]);

    // Join on k
    // Schema: (row, k, val_a, col, val_b)
    let joined = a_renamed
        .join(&b_renamed)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Extend: product = val_a * val_b
    let extended = joined
        .extend("product", ScalarType::Float, |t| {
            let va = t.get_typed::<f64>("val_a").unwrap();
            let vb = t.get_typed::<f64>("val_b").unwrap();
            ScalarValue::Float(va * vb)
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Summarize by row, col: sum(product)
    let summarized = extended
        .summarize(&["row", "col"], &[Aggregation::sum_float("val", "product")])
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // The resulting relation has (row, col, val) exactly as needed!
    Ok(summarized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    fn get_val(matrix: &Relation, row: i64, col: i64) -> f64 {
        for t in matrix.tuples() {
            if t.get_typed::<i64>("row") == Some(row) && t.get_typed::<i64>("col") == Some(col) {
                return t.get_typed::<f64>("val").unwrap();
            }
        }
        0.0 // sparse matrix default
    }

    #[test]
    fn test_matrix_addition() {
        let heading = TupleType::new()
            .with_attribute("row", ScalarType::Int)
            .with_attribute("col", ScalarType::Int)
            .with_attribute("val", ScalarType::Float);
        let mut a = Relation::new(RelationType::new(heading.clone()));
        a.insert(tuple! { row: 1, col: 1, val: 5.0 }).unwrap();
        a.insert(tuple! { row: 1, col: 2, val: 3.0 }).unwrap();

        let mut b = Relation::new(RelationType::new(heading));
        b.insert(tuple! { row: 1, col: 1, val: 5.0 }).unwrap(); // same (row, col) & same val! Tests multiplicity.
        b.insert(tuple! { row: 2, col: 1, val: 7.0 }).unwrap();

        let c = add_matrices(&a, &b).unwrap();

        assert_eq!(get_val(&c, 1, 1), 10.0); // 5 + 5
        assert_eq!(get_val(&c, 1, 2), 3.0); // 3 + 0
        assert_eq!(get_val(&c, 2, 1), 7.0); // 0 + 7
        assert_eq!(get_val(&c, 2, 2), 0.0); // 0 + 0
    }

    #[test]
    fn test_matrix_multiplication() {
        // A = | 1 2 |
        //     | 3 4 |
        let heading = TupleType::new()
            .with_attribute("row", ScalarType::Int)
            .with_attribute("col", ScalarType::Int)
            .with_attribute("val", ScalarType::Float);
        let mut a = Relation::new(RelationType::new(heading.clone()));
        a.insert(tuple! { row: 1, col: 1, val: 1.0 }).unwrap();
        a.insert(tuple! { row: 1, col: 2, val: 2.0 }).unwrap();
        a.insert(tuple! { row: 2, col: 1, val: 3.0 }).unwrap();
        a.insert(tuple! { row: 2, col: 2, val: 4.0 }).unwrap();

        // B = | 2 0 |
        //     | 1 2 |
        let mut b = Relation::new(RelationType::new(heading));
        b.insert(tuple! { row: 1, col: 1, val: 2.0 }).unwrap();
        b.insert(tuple! { row: 2, col: 1, val: 1.0 }).unwrap();
        b.insert(tuple! { row: 2, col: 2, val: 2.0 }).unwrap();
        // (1, 2) is sparse 0.0

        // C = A * B = | (1*2 + 2*1)  (1*0 + 2*2) | = | 4 4 |
        //             | (3*2 + 4*1)  (3*0 + 4*2) |   | 10 8 |
        let c = multiply_matrices(&a, &b).unwrap();

        assert_eq!(get_val(&c, 1, 1), 4.0);
        assert_eq!(get_val(&c, 1, 2), 4.0);
        assert_eq!(get_val(&c, 2, 1), 10.0);
        assert_eq!(get_val(&c, 2, 2), 8.0);
    }
}
