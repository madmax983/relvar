//! Relational Cellular Automata (Conway's Game of Life)
//!
//! This module demonstrates how to implement a cellular automaton entirely
//! using relational algebra (Cross Join, Extend, Summarize, Restrict, Union).
//!
//! # Concept
//!
//! In a relational cellular automaton, the grid is not a 2D array, but a relation
//! of active cells: `(x, y)`.
//!
//! To compute the next generation:
//! 1. We cross-join the live cells with a static relation of 8 neighbor offsets `(dx, dy)`.
//! 2. We use `Extend` to calculate the coordinates of all neighbors: `nx = x + dx`, `ny = y + dy`.
//! 3. We project to `(x, y, nx, ny)` and summarize by `(nx, ny)` to count how many live neighbors each coordinate has.
//! 4. We apply Conway's rules using relational logic:
//!    - Next Gen = (Cells with exactly 3 neighbors) UNION (Live cells with exactly 2 neighbors)
//!
//! # The Spark
//!
//! Relational databases are rarely used for simulations. By modeling space as relations and
//! rules as relational algebra operations, we can simulate complex emergent behavior without loops!

use relvar_core::algebra::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

/// Computes the next generation of Conway's Game of Life.
///
/// # Arguments
/// * `live_cells` - A relation with heading `(x: Int, y: Int)` representing currently live cells.
///
/// # Returns
/// A new relation with heading `(x: Int, y: Int)` representing live cells in the next generation.
pub fn next_generation(live_cells: &Relation) -> Result<Relation, DatabaseError> {
    if live_cells.is_empty() {
        return Ok(live_cells.clone());
    }

    // 1. Create neighbor offsets relation
    let offset_heading = TupleType::new()
        .with_attribute("dx", ScalarType::Int)
        .with_attribute("dy", ScalarType::Int);
    let mut offsets = Relation::new(RelationType::new(offset_heading));

    // Insert 8 neighbors
    for dx in -1..=1 {
        for dy in -1..=1 {
            if dx != 0 || dy != 0 {
                offsets
                    .insert(tuple! { dx: dx as i64, dy: dy as i64 })
                    .unwrap();
            }
        }
    }

    // 2. Cross Join live cells with offsets.
    // Since they share no attributes, natural join acts as cross join.
    let cell_neighbors = live_cells.join(&offsets)?;

    // 3. Extend to compute neighbor coordinates: nx = x + dx, ny = y + dy
    let neighbor_coords = cell_neighbors
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

    // 4. Summarize to count live neighbors per coordinate (nx, ny)
    let neighbor_counts = neighbor_coords
        .summarize(&["nx", "ny"], &[Aggregation::count("live_neighbors")])
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Rename nx -> x, ny -> y for easier comparison
    let neighbor_counts_renamed = neighbor_counts.rename(&[("nx", "x"), ("ny", "y")]);

    // 5. Apply rules
    // Rule 1: Exactly 3 neighbors -> Born or survives
    let three_neighbors =
        neighbor_counts_renamed.restrict(|t| t.get_typed::<i64>("live_neighbors") == Some(3));

    // Project to just (x, y)
    let survivors_from_3 = three_neighbors.project(&["x", "y"]);

    // Rule 2: Exactly 2 neighbors AND already live -> Survives
    let two_neighbors =
        neighbor_counts_renamed.restrict(|t| t.get_typed::<i64>("live_neighbors") == Some(2));

    let two_neighbors_coords = two_neighbors.project(&["x", "y"]);

    // Intersect with live cells to ensure they were already alive
    let survivors_from_2 = two_neighbors_coords
        .intersect(live_cells)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 6. Union to get next generation
    let next_gen = survivors_from_3
        .union(&survivors_from_2)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    Ok(next_gen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blinker_oscillator() {
        let heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int);

        let mut initial = Relation::new(RelationType::new(heading));
        // Vertical blinker
        initial.insert(tuple! { x: 0i64, y: -1i64 }).unwrap();
        initial.insert(tuple! { x: 0i64, y: 0i64 }).unwrap();
        initial.insert(tuple! { x: 0i64, y: 1i64 }).unwrap();

        let next_gen = next_generation(&initial).unwrap();

        assert_eq!(next_gen.cardinality(), 3);

        // Should become horizontal blinker
        let has_left = next_gen
            .tuples()
            .any(|t| t.get_typed::<i64>("x") == Some(-1) && t.get_typed::<i64>("y") == Some(0));
        let has_center = next_gen
            .tuples()
            .any(|t| t.get_typed::<i64>("x") == Some(0) && t.get_typed::<i64>("y") == Some(0));
        let has_right = next_gen
            .tuples()
            .any(|t| t.get_typed::<i64>("x") == Some(1) && t.get_typed::<i64>("y") == Some(0));

        assert!(has_left);
        assert!(has_center);
        assert!(has_right);

        // Back to vertical
        let gen3 = next_generation(&next_gen).unwrap();
        assert_eq!(gen3.cardinality(), 3);

        let has_top = gen3
            .tuples()
            .any(|t| t.get_typed::<i64>("x") == Some(0) && t.get_typed::<i64>("y") == Some(-1));
        let has_bottom = gen3
            .tuples()
            .any(|t| t.get_typed::<i64>("x") == Some(0) && t.get_typed::<i64>("y") == Some(1));

        assert!(has_top);
        assert!(has_bottom);
    }
}
