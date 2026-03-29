//! Relational Cellular Automaton (Conway's Game of Life)
//!
//! This module demonstrates how Conway's Game of Life can be implemented using purely
//! relational algebra operations. It treats the grid as a relation of active cells `(x, y)`,
//! and computing the next generation is performed via `Extend`, `Join`, `Summarize`,
//! and `Restrict`.
//!
//! # Concept
//!
//! We represent the board as a relation of alive cells: `(x, y)`.
//!
//! To compute the next generation:
//! 1. Generate all neighbors for each alive cell using a Cartesian Product (Join) with a static offsets relation.
//! 2. Group by cell coordinates and count the number of alive neighbors.
//! 3. Apply the Game of Life rules:
//!    - Survival: an alive cell with 2 or 3 neighbors survives.
//!    - Birth: an empty cell with exactly 3 neighbors becomes alive.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue, Tuple},
};

/// Computes the next generation of the cellular automaton (Game of Life).
///
/// # Arguments
///
/// * `cells` - A relation with heading `(x: Int, y: Int)` representing alive cells.
pub fn next_generation(cells: &Relation) -> Result<Relation, DatabaseError> {
    // 1. Create neighbor offsets relation: (dx, dy)
    let offset_heading = TupleType::new()
        .with_attribute("dx".to_string(), ScalarType::Int)
        .with_attribute("dy".to_string(), ScalarType::Int);
    let mut offsets = Relation::new(RelationType::new(offset_heading.clone()));

    for dx in -1i64..=1 {
        for dy in -1i64..=1 {
            if dx != 0 || dy != 0 {
                let mut vals = std::collections::BTreeMap::new();
                vals.insert("dx".to_string(), ScalarValue::Int(dx));
                vals.insert("dy".to_string(), ScalarValue::Int(dy));
                offsets.insert(Tuple::new(offset_heading.clone(), vals).unwrap())?;
            }
        }
    }

    // 2. Cartesian product of cells and offsets
    // Since they share no common attributes, a natural join acts as a cartesian product.
    let cartesian = cells.join(&offsets)?;

    // 3. Compute neighbor coordinates: nx = x + dx, ny = y + dy
    let neighbors = cartesian
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

    // 4. Summarize (count) the frequencies of (nx, ny)
    // Note: Summarize must be applied BEFORE Project to correctly count duplicate neighbors!
    let neighbor_counts = neighbors
        .summarize(&["nx", "ny"], &[Aggregation::count("n_count")])
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Rename back to x, y
    let rename_map = vec![("nx", "x"), ("ny", "y")];
    let counts_xy = neighbor_counts.rename(&rename_map);

    // 5. Apply Game of Life rules

    // Find surviving cells: alive cells with 2 or 3 neighbors
    // Join with cells effectively acts as an intersection/filter for "is alive"
    let surviving = counts_xy
        .join(cells)?
        .restrict(|t| {
            let count = t.get_typed::<i64>("n_count").unwrap();
            count == 2 || count == 3
        })
        .project(&["x", "y"]);

    // Find new cells (birth): dead cells with exactly 3 neighbors
    let exactly_three = counts_xy
        .restrict(|t| t.get_typed::<i64>("n_count").unwrap() == 3)
        .project(&["x", "y"]);

    // Dead cells are cells with exactly 3 neighbors minus the currently alive cells
    let births = exactly_three
        .difference(cells)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Union surviving and births
    let next_gen = surviving
        .union(&births)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    Ok(next_gen)
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;

    #[test]
    fn test_blinker() {
        let heading = TupleType::new()
            .with_attribute("x".to_string(), ScalarType::Int)
            .with_attribute("y".to_string(), ScalarType::Int);
        let mut cells = Relation::new(RelationType::new(heading));

        // Horizontal blinker
        cells.insert(tuple! { x: 0i64, y: 1i64 }).unwrap();
        cells.insert(tuple! { x: 1i64, y: 1i64 }).unwrap();
        cells.insert(tuple! { x: 2i64, y: 1i64 }).unwrap();

        // Generation 1 (Vertical blinker)
        let gen1 = next_generation(&cells).unwrap();
        assert_eq!(gen1.cardinality(), 3);

        let has_cell = |x, y| {
            gen1.tuples().any(|t| {
                t.get_typed::<i64>("x").unwrap() == x && t.get_typed::<i64>("y").unwrap() == y
            })
        };

        assert!(has_cell(1, 0));
        assert!(has_cell(1, 1));
        assert!(has_cell(1, 2));

        // Generation 2 (Horizontal blinker again)
        let gen2 = next_generation(&gen1).unwrap();

        assert_eq!(gen2.cardinality(), 3);

        let has_cell2 = |x, y| {
            gen2.tuples().any(|t| {
                t.get_typed::<i64>("x").unwrap() == x && t.get_typed::<i64>("y").unwrap() == y
            })
        };

        assert!(has_cell2(0, 1));
        assert!(has_cell2(1, 1));
        assert!(has_cell2(2, 1));
    }
}
