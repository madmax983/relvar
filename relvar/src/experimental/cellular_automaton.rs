//! Relational Conway's Game of Life (Experimental)
//!
//! This module implements Conway's Game of Life purely using relational algebra.
//! It demonstrates how complex, iterative spatial algorithms can be expressed
//! using set-based operations rather than traditional array manipulations.
//!
//! # The Relational Approach
//!
//! In a traditional array-based implementation, you iterate over every cell in a 2D grid,
//! count its neighbors, and apply rules. In the relational approach:
//!
//! 1. **State:** The board is just a relation `(x: Int, y: Int)` containing ONLY the live cells.
//! 2. **Deltas:** We define a constant relation of neighbor offsets `(dx: Int, dy: Int)`.
//! 3. **Pairing:** A Cartesian Product (Join with no common attributes) pairs every live cell with every offset.
//! 4. **Projection:** We compute `nx = x + dx` and `ny = y + dy` to get a relation of all "neighboring coordinates".
//! 5. **Grouping:** We Summarize (Group By `nx, ny`) and Count to find exactly how many live neighbors every coordinate has.
//! 6. **Rules:**
//!    - *Births:* Restrict the counts relation to `count == 3` and remove cells that are already alive (Difference).
//!    - *Survival:* Restrict the counts relation to `count == 2 OR count == 3`, and Join with the current live cells.
//! 7. **Next Generation:** Union the Births and Survivals.
//!
//! This approach naturally handles an infinite grid without arbitrary boundaries.

use relvar_core::{
    algebra::{Aggregation, AggregationFn},
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue, Tuple},
};
use std::collections::BTreeMap;
use std::sync::Arc;

/// A relational implementation of Conway's Game of Life.
pub struct GameOfLife {
    /// A relation with heading `(x: Int, y: Int)` representing live cells.
    pub state: Relation,
    /// Pre-computed relation of the 8 neighbor deltas `(dx: Int, dy: Int)`.
    deltas: Relation,
}

impl GameOfLife {
    /// Creates a new Game of Life simulator from an initial state relation.
    ///
    /// The input relation MUST have exactly the heading `(x: Int, y: Int)`.
    pub fn new(initial_state: Relation) -> Result<Self, Box<dyn std::error::Error>> {
        // Verify heading
        let tt = initial_state.relation_type().tuple_type();
        if tt.degree() != 2 || !tt.has_attribute("x") || !tt.has_attribute("y") {
            return Err("Initial state must have exact heading (x: Int, y: Int)".into());
        }

        // Pre-compute the 8 neighbor deltas (Cartesian product of [-1, 0, 1] minus (0,0))
        let deltas_type = RelationType::new(
            TupleType::new()
                .with_attribute("dx", ScalarType::Int)
                .with_attribute("dy", ScalarType::Int),
        );
        let mut deltas = Relation::new(deltas_type.clone());
        let arc_tt = Arc::new(deltas_type.tuple_type().clone());

        for dx in -1i64..=1 {
            for dy in -1i64..=1 {
                if dx == 0 && dy == 0 {
                    continue; // Skip self
                }

                let mut map = BTreeMap::new();
                map.insert("dx".to_string(), ScalarValue::Int(dx));
                map.insert("dy".to_string(), ScalarValue::Int(dy));

                let t = Tuple::new(arc_tt.clone(), map)?;
                deltas.insert(t)?;
            }
        }

        Ok(Self {
            state: initial_state,
            deltas,
        })
    }

    /// Computes the next generation of the simulation.
    pub fn tick(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // If the board is empty, it stays empty.
        if self.state.is_empty() {
            return Ok(());
        }

        // 1. Pair every live cell with every neighbor offset.
        // Because `state` (x,y) and `deltas` (dx,dy) have no common attributes,
        // this Natural Join acts as a Cartesian Product.
        let paired = self.state.join(&self.deltas)?;

        // 2. Compute the actual absolute coordinates of every neighbor.
        let neighbors_extended = paired
            .extend("nx", ScalarType::Int, |t| {
                let x = t.get_typed::<i64>("x").unwrap();
                let dx = t.get_typed::<i64>("dx").unwrap();
                ScalarValue::Int(x + dx)
            })?
            .extend("ny", ScalarType::Int, |t| {
                let y = t.get_typed::<i64>("y").unwrap();
                let dy = t.get_typed::<i64>("dy").unwrap();
                ScalarValue::Int(y + dy)
            })?;

        // 3. Count how many times each (nx, ny) coordinate was hit by a neighbor.
        let neighbor_counts = neighbors_extended.summarize(
            &["nx", "ny"],
            &[Aggregation {
                result_name: "count".to_string(),
                result_type: ScalarType::Int,
                function: AggregationFn::Count,
            }],
        )?;

        // Rename nx -> x, ny -> y so we can interact with the original state.
        let board_counts = neighbor_counts.rename(&[("nx", "x"), ("ny", "y")]);

        // 4. Apply the rules of Life.

        // BIRTH: Exactly 3 neighbors, and currently dead (not in state).
        let potential_births = board_counts
            .clone()
            .restrict_into(|t: &Tuple| t.get_typed::<i64>("count").unwrap() == 3);
        let births_coords = potential_births.project(&["x", "y"]);
        // Difference removes cells that are already alive.
        let actual_births = births_coords.difference(&self.state)?;

        // SURVIVAL: 2 or 3 neighbors, AND currently alive (must be in state).
        let potential_survivors = board_counts.restrict_into(|t: &Tuple| {
            let c = t.get_typed::<i64>("count").unwrap();
            c == 2 || c == 3
        });
        let survivors_coords = potential_survivors.project(&["x", "y"]);
        // Intersect filters to only cells that are currently alive.
        let actual_survivors = survivors_coords.intersect(&self.state)?;

        // 5. The new state is the Union of Births and Survivors.
        self.state = actual_survivors.union(&actual_births)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;

    fn make_state(coords: &[(i64, i64)]) -> Relation {
        let rt = RelationType::new(
            TupleType::new()
                .with_attribute("x", ScalarType::Int)
                .with_attribute("y", ScalarType::Int),
        );
        let mut r = Relation::new(rt);
        for &(x, y) in coords {
            r.insert(tuple! { x: x, y: y }).unwrap();
        }
        r
    }

    #[test]
    fn test_blinker() {
        // A blinker oscillates between vertical and horizontal.
        let mut life = GameOfLife::new(make_state(&[(0, -1), (0, 0), (0, 1)])).unwrap();

        assert_eq!(life.state.cardinality(), 3);

        // Tick 1 (Horizontal): (-1,0), (0,0), (1,0)
        life.tick().unwrap();
        assert_eq!(life.state.cardinality(), 3);
        let h_state = make_state(&[(-1, 0), (0, 0), (1, 0)]);
        assert!(life.state.difference(&h_state).unwrap().is_empty());
        assert!(h_state.difference(&life.state).unwrap().is_empty());

        // Tick 2 (Vertical again)
        life.tick().unwrap();
        assert_eq!(life.state.cardinality(), 3);
        let v_state = make_state(&[(0, -1), (0, 0), (0, 1)]);
        assert!(life.state.difference(&v_state).unwrap().is_empty());
        assert!(v_state.difference(&life.state).unwrap().is_empty());
    }

    #[test]
    fn test_block() {
        // A block is a 2x2 still life.
        let mut life = GameOfLife::new(make_state(&[(0, 0), (0, 1), (1, 0), (1, 1)])).unwrap();

        life.tick().unwrap();

        // Should be completely unchanged.
        assert_eq!(life.state.cardinality(), 4);
        let expected = make_state(&[(0, 0), (0, 1), (1, 0), (1, 1)]);
        assert!(life.state.difference(&expected).unwrap().is_empty());
    }

    #[test]
    fn test_empty_board() {
        let mut life = GameOfLife::new(make_state(&[])).unwrap();
        life.tick().unwrap();
        assert_eq!(life.state.cardinality(), 0);
    }
}
