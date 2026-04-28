use crate::algebra::{Aggregation, AggregationFn};
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, ScalarValue, Tuple};
use std::collections::HashMap;

/// Evaluates one generation of Conway's Game of Life purely using relational algebra.
///
/// The grid is infinite, represented sparsely by the coordinates of alive cells.
///
/// `alive_cells` must be a relation with exactly two attributes: `x` (Int) and `y` (Int).
///
/// # Examples
///
/// ```
/// use relvar_core::experimental::game_of_life::next_generation;
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::values::{Relation, Tuple};
/// use relvar_core::tuple;
///
/// // 1. Create a heading for our coordinate relation
/// let heading = TupleType::new()
///     .with_attribute("x", ScalarType::Int)
///     .with_attribute("y", ScalarType::Int);
///
/// // 2. Create the initial relation representing a horizontal "Blinker"
/// let mut blinker_h = Relation::new(RelationType::new(heading));
/// blinker_h.insert(tuple!{x: 0i64, y: 0i64}).unwrap();
/// blinker_h.insert(tuple!{x: 1i64, y: 0i64}).unwrap();
/// blinker_h.insert(tuple!{x: 2i64, y: 0i64}).unwrap();
///
/// // 3. Compute the next generation purely via relational algebra
/// let blinker_v = next_generation(&blinker_h).unwrap();
///
/// // The blinker has oscillated to a vertical position
/// assert_eq!(blinker_v.cardinality(), 3);
/// ```
pub fn next_generation(alive_cells: &Relation) -> Result<Relation, crate::error::DatabaseError> {
    let offsets = generate_offsets();
    let counts_renamed = calculate_neighbor_counts(alive_cells, &offsets)?;
    apply_rules(alive_cells, &counts_renamed)
}

fn generate_offsets() -> Relation {
    let off_heading = TupleType::new()
        .with_attribute("dx", ScalarType::Int)
        .with_attribute("dy", ScalarType::Int);
    let mut offsets = Relation::new(RelationType::new(off_heading.clone()));
    for dx in -1..=1 {
        for dy in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let mut map = HashMap::new();
            map.insert("dx".to_string(), ScalarValue::Int(dx as i64));
            map.insert("dy".to_string(), ScalarValue::Int(dy as i64));
            let tuple = Tuple::new(off_heading.clone(), map).unwrap();
            offsets.insert(tuple).unwrap();
        }
    }
    offsets
}

fn calculate_neighbor_counts(
    alive_cells: &Relation,
    offsets: &Relation,
) -> Result<Relation, crate::error::DatabaseError> {
    // 2. Cross Join to generate all neighbors
    let cross = alive_cells.join(offsets)?;

    // 3. Extend to compute actual neighbor coordinates (nx, ny)
    let ext1 = cross
        .extend("nx", ScalarType::Int, |t: &Tuple| {
            let x = t.get_typed::<i64>("x").unwrap();
            let dx = t.get_typed::<i64>("dx").unwrap();
            ScalarValue::Int(x + dx)
        })
        .map_err(|e| crate::error::DatabaseError::AlgebraError(e.to_string()))?;

    let ext2 = ext1
        .extend("ny", ScalarType::Int, |t: &Tuple| {
            let y = t.get_typed::<i64>("y").unwrap();
            let dy = t.get_typed::<i64>("dy").unwrap();
            ScalarValue::Int(y + dy)
        })
        .map_err(|e| crate::error::DatabaseError::AlgebraError(e.to_string()))?;

    // 4. Project to (nx, ny) to count properly
    let connections = ext2.project(&["x", "y", "nx", "ny"]);

    // 5. Summarize to count neighbors per cell
    let counts = connections
        .summarize(
            &["nx", "ny"],
            &[Aggregation {
                result_name: "n_count".to_string(),
                result_type: ScalarType::Int,
                function: AggregationFn::Count,
            }],
        )
        .map_err(|e| crate::error::DatabaseError::AlgebraError(e.to_string()))?;

    // 6. Rename back to (x, y)
    Ok(counts.rename(&[("nx", "x"), ("ny", "y")]))
}

fn apply_rules(
    alive_cells: &Relation,
    counts_renamed: &Relation,
) -> Result<Relation, crate::error::DatabaseError> {
    // 7. Rule 1: Stays Alive (alive cells with 2 or 3 neighbors)
    let alive_with_neighbors = alive_cells.join(counts_renamed)?;
    let stays_alive = alive_with_neighbors
        .restrict(|t: &Tuple| {
            let count = t.get_typed::<i64>("n_count").unwrap();
            count == 2 || count == 3
        })
        .project(&["x", "y"]);

    // 8. Rule 2: Born (dead cells with exactly 3 neighbors)
    let all_with_3_neighbors = counts_renamed
        .restrict(|t: &Tuple| t.get_typed::<i64>("n_count").unwrap() == 3)
        .project(&["x", "y"]);

    let born = all_with_3_neighbors
        .difference(alive_cells)
        .map_err(|e| crate::error::DatabaseError::AlgebraError(e.to_string()))?;

    // 9. Union for next generation
    stays_alive
        .union(&born)
        .map_err(|e| crate::error::DatabaseError::AlgebraError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;

    fn make_alive_relation(coords: &[(i64, i64)]) -> Relation {
        let heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int);
        let mut rel = Relation::new(RelationType::new(heading));
        for &(x, y) in coords {
            rel.insert(tuple! {x: x, y: y}).unwrap();
        }
        rel
    }

    #[test]
    fn test_blinker_oscillator() {
        // Blinker phase 1: Horizontal
        let blinker_h = make_alive_relation(&[(0, 0), (1, 0), (2, 0)]);

        let blinker_v = next_generation(&blinker_h).unwrap();

        assert_eq!(blinker_v.cardinality(), 3);

        let expected_v = make_alive_relation(&[(1, -1), (1, 0), (1, 1)]);

        // Assert they are identical
        assert_eq!(blinker_v.difference(&expected_v).unwrap().cardinality(), 0);
        assert_eq!(expected_v.difference(&blinker_v).unwrap().cardinality(), 0);

        let back_to_h = next_generation(&blinker_v).unwrap();
        assert_eq!(back_to_h.cardinality(), 3);
        assert_eq!(back_to_h.difference(&blinker_h).unwrap().cardinality(), 0);
        assert_eq!(blinker_h.difference(&back_to_h).unwrap().cardinality(), 0);
    }
}
