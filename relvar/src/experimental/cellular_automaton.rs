use relvar_core::algebra::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

/// Relational Cellular Automaton (Conway's Game of Life)
///
/// Implements Game of Life using purely relational algebra.
pub struct CellularAutomaton {
    /// The current state of the automaton, a relation of alive cells.
    /// Expected heading: `{x: Int, y: Int}`
    pub state: Relation,
}

impl CellularAutomaton {
    /// Creates a new Cellular Automaton from an initial state.
    pub fn new(initial_state: Relation) -> Self {
        Self {
            state: initial_state,
        }
    }

    /// Computes the next generation of the cellular automaton.
    pub fn step(&mut self) -> Result<(), DatabaseError> {
        let mut offsets = Relation::new(RelationType::new(
            TupleType::new()
                .with_attribute("dx", ScalarType::Int)
                .with_attribute("dy", ScalarType::Int),
        ));

        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                offsets
                    .insert(tuple! { dx: dx as i64, dy: dy as i64 })
                    .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
            }
        }

        let joined = self.state.join(&offsets)?;

        let extended_x = joined
            .extend("nx", ScalarType::Int, |t| {
                let x = t.get_typed::<i64>("x").unwrap();
                let dx = t.get_typed::<i64>("dx").unwrap();
                ScalarValue::Int(x + dx)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let neighbors = extended_x
            .extend("ny", ScalarType::Int, |t| {
                let y = t.get_typed::<i64>("y").unwrap();
                let dy = t.get_typed::<i64>("dy").unwrap();
                ScalarValue::Int(y + dy)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let neighbor_counts = neighbors
            .summarize(&["nx", "ny"], &[Aggregation::count("n_count")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let count_3 = neighbor_counts.restrict(|t| t.get_typed::<i64>("n_count") == Some(3));
        let count_3_projected = count_3.project(&["nx", "ny"]);

        let count_2 = neighbor_counts.restrict(|t| t.get_typed::<i64>("n_count") == Some(2));
        let count_2_projected = count_2.project(&["nx", "ny"]);

        let state_renamed = self.state.rename(&[("x", "nx"), ("y", "ny")]);

        let survivors_from_2 = state_renamed
            .intersect(&count_2_projected)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let next_state_renamed = survivors_from_2
            .union(&count_3_projected)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        self.state = next_state_renamed.rename(&[("nx", "x"), ("ny", "y")]);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blinker_oscillator() -> Result<(), DatabaseError> {
        // A blinker is a line of 3 cells.
        // It oscillates between horizontal and vertical.

        let heading = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int);
        let mut state = Relation::new(RelationType::new(heading.clone()));

        // Vertical blinker: (0, -1), (0, 0), (0, 1)
        state.insert(tuple! { x: 0i64, y: -1i64 })?;
        state.insert(tuple! { x: 0i64, y: 0i64 })?;
        state.insert(tuple! { x: 0i64, y: 1i64 })?;

        let mut automaton = CellularAutomaton::new(state);

        // First step -> should become horizontal
        automaton.step()?;

        assert_eq!(automaton.state.cardinality(), 3);

        let horizontal: Vec<_> = automaton
            .state
            .tuples()
            .map(|t| {
                (
                    t.get_typed::<i64>("x").unwrap(),
                    t.get_typed::<i64>("y").unwrap(),
                )
            })
            .collect();

        assert!(horizontal.contains(&(-1, 0)));
        assert!(horizontal.contains(&(0, 0)));
        assert!(horizontal.contains(&(1, 0)));

        // Second step -> should become vertical again
        automaton.step()?;

        assert_eq!(automaton.state.cardinality(), 3);

        let vertical: Vec<_> = automaton
            .state
            .tuples()
            .map(|t| {
                (
                    t.get_typed::<i64>("x").unwrap(),
                    t.get_typed::<i64>("y").unwrap(),
                )
            })
            .collect();

        assert!(vertical.contains(&(0, -1)));
        assert!(vertical.contains(&(0, 0)));
        assert!(vertical.contains(&(0, 1)));

        Ok(())
    }
}
