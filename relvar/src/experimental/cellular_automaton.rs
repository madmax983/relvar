use relvar_core::algebra::Aggregation;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

/// Computes the next generation of Conway's Game of Life using pure relational algebra.
pub fn next_generation(alive_cells: &Relation) -> Result<Relation, String> {
    let expected_type = TupleType::new()
        .with_attribute("x", ScalarType::Int)
        .with_attribute("y", ScalarType::Int);

    if alive_cells.relation_type().heading() != &expected_type {
        return Err("Input relation must have heading (x: Int, y: Int)".to_string());
    }

    let offset_type = TupleType::new()
        .with_attribute("dx", ScalarType::Int)
        .with_attribute("dy", ScalarType::Int);
    let mut offsets = Relation::new(RelationType::new(offset_type.clone()));

    for dx in -1..=1 {
        for dy in -1..=1 {
            if dx != 0 || dy != 0 {
                offsets
                    .insert(tuple! { dx: dx as i64, dy: dy as i64 })
                    .unwrap();
            }
        }
    }

    let joined = alive_cells.join(&offsets).map_err(|e| e.to_string())?;

    let with_nx = joined
        .extend("nx", ScalarType::Int, |t| {
            let x = t.get_typed::<i64>("x").unwrap();
            let dx = t.get_typed::<i64>("dx").unwrap();
            ScalarValue::Int(x + dx)
        })
        .map_err(|e| e.to_string())?;

    let with_ny = with_nx
        .extend("ny", ScalarType::Int, |t| {
            let y = t.get_typed::<i64>("y").unwrap();
            let dy = t.get_typed::<i64>("dy").unwrap();
            ScalarValue::Int(y + dy)
        })
        .map_err(|e| e.to_string())?;

    let projected = with_ny.project(&["nx", "ny", "x", "y"]);

    let neighbor_counts = projected
        .summarize(&["nx", "ny"], &[Aggregation::count("neighbors")])
        .map_err(|e| e.to_string())?;

    let renamed = neighbor_counts.rename(&[("nx", "x"), ("ny", "y")]);

    // Rule 1: Survival. Cell is alive, has 2 or 3 neighbors.
    let alive_with_counts = alive_cells.join(&renamed).map_err(|e| e.to_string())?;

    let survivors = alive_with_counts.restrict(|t| {
        let neighbors = t.get_typed::<i64>("neighbors").unwrap();
        neighbors == 2 || neighbors == 3
    });

    let new_survivors = survivors.project(&["x", "y"]);

    // Rule 2: Birth. Cell is dead (not in alive_cells), has exactly 3 neighbors.
    let three_neighbors = renamed.restrict(|t| {
        let neighbors = t.get_typed::<i64>("neighbors").unwrap();
        neighbors == 3
    });

    let candidates_for_birth = three_neighbors.project(&["x", "y"]);

    // We want candidates not in alive_cells
    let births = candidates_for_birth
        .difference(alive_cells)
        .map_err(|e| e.to_string())?;

    let next_gen = new_survivors.union(&births).map_err(|e| e.to_string())?;

    Ok(next_gen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blinker() {
        let expected_type = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int);

        let mut alive_cells = Relation::new(RelationType::new(expected_type.clone()));
        // horizontal blinker
        alive_cells.insert(tuple! { x: 0i64, y: 0i64 }).unwrap();
        alive_cells.insert(tuple! { x: 1i64, y: 0i64 }).unwrap();
        alive_cells.insert(tuple! { x: 2i64, y: 0i64 }).unwrap();

        let gen1 = next_generation(&alive_cells).unwrap();
        assert_eq!(gen1.cardinality(), 3);

        // vertical blinker expected
        assert!(gen1.contains(&tuple! { x: 1i64, y: -1i64 }));
        assert!(gen1.contains(&tuple! { x: 1i64, y: 0i64 }));
        assert!(gen1.contains(&tuple! { x: 1i64, y: 1i64 }));
    }
}
