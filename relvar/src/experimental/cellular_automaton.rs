//! Experimental Relational Cellular Automaton.
//!
//! This module demonstrates implementing Conway's Game of Life purely using
//! Relational Algebra (Cartesian Product, Extend, Summarize, Restrict, Union).
//!
//! # Philosophy
//!
//! Cellular automata are typically implemented with imperative arrays. However,
//! the Game of Life rules can be expressed purely as operations on sets (relations).
//! By representing the grid as a relation of live cells `(x: Int, y: Int)`, we can
//! generate all neighbors, count them using `Summarize`, and apply survival/birth
//! rules using `Restrict` and `Union`.
//!
//! This shows the expressive power of relational algebra for spatial and rule-based
//! simulations.

use relvar_core::algebra::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

/// Computes the next generation of a Conway's Game of Life grid.
///
/// The grid is represented as a relation containing only the coordinates
/// of currently *live* cells.
///
/// # Arguments
///
/// * `live_cells` - A relation with heading `(x: Int, y: Int)` representing live cells.
///
/// # Returns
///
/// A new relation with the same heading `(x: Int, y: Int)` representing the live cells
/// in the next generation.
///
/// # Errors
///
/// Returns `DatabaseError::AlgebraError` if internal relational operations fail.
pub fn next_generation(live_cells: &Relation) -> Result<Relation, DatabaseError> {
    // 1. Create a relation of neighbor deltas (dx, dy)
    let delta_heading = TupleType::new()
        .with_attribute("dx", ScalarType::Int)
        .with_attribute("dy", ScalarType::Int);
    let mut deltas = Relation::new(RelationType::new(delta_heading.clone()));

    // Insert the 8 neighbors
    let neighbor_coords = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];

    for (dx, dy) in neighbor_coords {
        deltas.insert(tuple! { dx: dx as i64, dy: dy as i64 })?;
    }

    // 2. Cross join live_cells with deltas to find all adjacent coordinates
    // Since headings are disjoint (x,y vs dx,dy), natural join acts as Cartesian product
    let all_adjacent = live_cells.join(&deltas)?;

    // 3. Compute absolute neighbor coordinates: nx = x + dx, ny = y + dy
    // Wait, `project` removes duplicates (relations are sets).
    // We must summarize BEFORE projecting away the source (x, y).
    // But summarize groups by the specified attributes.
    // If we extend and then group by nx, ny, it works perfectly.
    let neighbor_coords_rel = all_adjacent
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

    // 4. Summarize to count live neighbors for each (nx, ny)

    // Group by (nx, ny) and count occurrences of (x, y) - each unique (x, y) that generated this (nx, ny)
    let neighbor_counts = neighbor_coords_rel
        .summarize(&["nx", "ny"], &[Aggregation::count("live_neighbors")])
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 5. Apply Game of Life Rules

    // Rule 1: Birth - exactly 3 live neighbors
    let births = neighbor_counts.restrict(|t| t.get_typed::<i64>("live_neighbors") == Some(3));

    // Rule 2: Survival - 2 or 3 live neighbors AND currently alive
    let survivals_potential = neighbor_counts.restrict(|t| {
        let n = t.get_typed::<i64>("live_neighbors").unwrap_or(0);
        n == 2 || n == 3
    });

    // Rename original live cells to match (nx, ny) for join
    let original_renamed = live_cells.rename(&[("x", "nx"), ("y", "ny")]);

    // Natural join keeps only cells that were already alive
    let survivals = survivals_potential.join(&original_renamed)?;

    // 6. Union births and survivals
    let next_gen_nx_ny = births
        .union(&survivals)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 7. Project away the count and rename back to (x, y)
    let next_gen = next_gen_nx_ny
        .project(&["nx", "ny"])
        .rename(&[("nx", "x"), ("ny", "y")]);

    Ok(next_gen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_still_life() {
        // A block is a 2x2 square of live cells. It should not change.
        // (0,0), (1,0)
        // (0,1), (1,1)
        let heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int);
        let mut block = Relation::new(RelationType::new(heading));

        block.insert(tuple! { x: 0i64, y: 0i64 }).unwrap();
        block.insert(tuple! { x: 1i64, y: 0i64 }).unwrap();
        block.insert(tuple! { x: 0i64, y: 1i64 }).unwrap();
        block.insert(tuple! { x: 1i64, y: 1i64 }).unwrap();

        let next = next_generation(&block).unwrap();

        assert_eq!(next.cardinality(), 4);
        assert!(
            next.tuples()
                .any(|t| t.get_typed::<i64>("x") == Some(0) && t.get_typed::<i64>("y") == Some(0))
        );
        assert!(
            next.tuples()
                .any(|t| t.get_typed::<i64>("x") == Some(1) && t.get_typed::<i64>("y") == Some(0))
        );
        assert!(
            next.tuples()
                .any(|t| t.get_typed::<i64>("x") == Some(0) && t.get_typed::<i64>("y") == Some(1))
        );
        assert!(
            next.tuples()
                .any(|t| t.get_typed::<i64>("x") == Some(1) && t.get_typed::<i64>("y") == Some(1))
        );
    }

    #[test]
    fn test_blinker_oscillator() {
        // A blinker is a 1x3 line. It should oscillate between horizontal and vertical.
        // Gen 1: (0,-1), (0,0), (0,1)
        let heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int);
        let mut gen1 = Relation::new(RelationType::new(heading));

        gen1.insert(tuple! { x: 0i64, y: -1i64 }).unwrap();
        gen1.insert(tuple! { x: 0i64, y: 0i64 }).unwrap();
        gen1.insert(tuple! { x: 0i64, y: 1i64 }).unwrap();

        let gen2 = next_generation(&gen1).unwrap();

        // Gen 2 should be: (-1,0), (0,0), (1,0)
        assert_eq!(gen2.cardinality(), 3);
        assert!(
            gen2.tuples()
                .any(|t| t.get_typed::<i64>("x") == Some(-1) && t.get_typed::<i64>("y") == Some(0))
        );
        assert!(
            gen2.tuples()
                .any(|t| t.get_typed::<i64>("x") == Some(0) && t.get_typed::<i64>("y") == Some(0))
        );
        assert!(
            gen2.tuples()
                .any(|t| t.get_typed::<i64>("x") == Some(1) && t.get_typed::<i64>("y") == Some(0))
        );

        // Gen 3 should be back to Gen 1
        let gen3 = next_generation(&gen2).unwrap();

        assert_eq!(gen3.cardinality(), 3);
        assert!(
            gen3.tuples()
                .any(|t| t.get_typed::<i64>("x") == Some(0) && t.get_typed::<i64>("y") == Some(-1))
        );
        assert!(
            gen3.tuples()
                .any(|t| t.get_typed::<i64>("x") == Some(0) && t.get_typed::<i64>("y") == Some(0))
        );
        assert!(
            gen3.tuples()
                .any(|t| t.get_typed::<i64>("x") == Some(0) && t.get_typed::<i64>("y") == Some(1))
        );
    }
}
