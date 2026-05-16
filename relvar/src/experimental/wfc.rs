//! Relational Wave Function Collapse (WFC).
//!
//! This module demonstrates how the Wave Function Collapse algorithm
//! can be implemented using purely relational algebra.
//!
//! # Concept
//!
//! - **Domain**: `(x: Int, y: Int, tile: String)` represents all possible tiles for each cell.
//! - **Rules**: `(tile: String, dx: Int, dy: Int, neighbor_tile: String)` defines allowed adjacencies.
//!
//! The constraint propagation iteratively removes tiles from the domain
//! that have no valid neighbors in a given direction, using set differences and joins.

use relvar_core::error::DatabaseError;
use relvar_core::types::ScalarType;
use relvar_core::values::{Relation, ScalarValue, Tuple};

/// A Relational Wave Function Collapse simulator.
pub struct WaveFunctionCollapse {
    /// Domain relation: (x: Int, y: Int, tile: String)
    pub domain: Relation,
    /// Rules relation: (tile: String, dx: Int, dy: Int, neighbor_tile: String)
    pub rules: Relation,
    /// Width of the grid.
    pub width: i64,
    /// Height of the grid.
    pub height: i64,
}

impl WaveFunctionCollapse {
    /// Creates a new WFC solver.
    pub fn new(domain: Relation, rules: Relation, width: i64, height: i64) -> Self {
        Self {
            domain,
            rules,
            width,
            height,
        }
    }

    /// Propagates constraints once. Returns `true` if the domain was modified.
    pub fn propagate(&mut self) -> Result<bool, DatabaseError> {
        let directions = self.rules.project(&["dx", "dy"]);

        let domain_dirs = self.domain.join(&directions)?;

        let with_n = domain_dirs
            .extend("nx", ScalarType::Int, |t: &Tuple| {
                let x = t.get_typed::<i64>("x").unwrap();
                let dx = t.get_typed::<i64>("dx").unwrap();
                ScalarValue::Int(x + dx)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let with_n = with_n
            .extend("ny", ScalarType::Int, |t: &Tuple| {
                let y = t.get_typed::<i64>("y").unwrap();
                let dy = t.get_typed::<i64>("dy").unwrap();
                ScalarValue::Int(y + dy)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let w = self.width;
        let h = self.height;

        let active_checks = with_n.restrict(|t: &Tuple| {
            let nx = t.get_typed::<i64>("nx").unwrap();
            let ny = t.get_typed::<i64>("ny").unwrap();
            nx >= 0 && nx < w && ny >= 0 && ny < h
        });

        let possible_support = active_checks.join(&self.rules)?;

        let neighbor_domain =
            self.domain
                .rename(&[("x", "nx"), ("y", "ny"), ("tile", "neighbor_tile")]);

        let actual_support = possible_support.join(&neighbor_domain)?;

        let supported_checks = actual_support.project(&["x", "y", "tile", "dx", "dy"]);

        let required_checks = active_checks.project(&["x", "y", "tile", "dx", "dy"]);

        let unsupported_checks = required_checks
            .difference(&supported_checks)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let to_remove = unsupported_checks.project(&["x", "y", "tile"]);

        let old_cardinality = self.domain.cardinality();
        self.domain = self
            .domain
            .difference(&to_remove)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(self.domain.cardinality() < old_cardinality)
    }

    /// Propagates until a fixpoint is reached.
    pub fn propagate_all(&mut self) -> Result<(), DatabaseError> {
        while self.propagate()? {}
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_wfc_propagation() -> Result<(), DatabaseError> {
        let domain_type = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int)
            .with_attribute("tile", ScalarType::String);

        let rules_type = TupleType::new()
            .with_attribute("tile", ScalarType::String)
            .with_attribute("dx", ScalarType::Int)
            .with_attribute("dy", ScalarType::Int)
            .with_attribute("neighbor_tile", ScalarType::String);

        let mut domain = Relation::new(RelationType::new(domain_type));
        let mut rules = Relation::new(RelationType::new(rules_type));

        domain.insert(tuple! { x: 0i64, y: 0i64, tile: "A" })?;
        domain.insert(tuple! { x: 0i64, y: 0i64, tile: "B" })?;
        domain.insert(tuple! { x: 1i64, y: 0i64, tile: "B" })?;
        domain.insert(tuple! { x: 1i64, y: 0i64, tile: "C" })?;

        rules.insert(tuple! { tile: "A", dx: 1i64, dy: 0i64, neighbor_tile: "B" })?;
        rules.insert(tuple! { tile: "B", dx: 1i64, dy: 0i64, neighbor_tile: "C" })?;
        rules.insert(tuple! { tile: "B", dx: -1i64, dy: 0i64, neighbor_tile: "A" })?;
        rules.insert(tuple! { tile: "C", dx: -1i64, dy: 0i64, neighbor_tile: "B" })?;

        let mut wfc = WaveFunctionCollapse::new(domain, rules, 2, 1);

        wfc.propagate_all()?;

        let remaining = wfc.domain.cardinality();
        assert_eq!(remaining, 4);

        let to_remove = Relation::new(wfc.domain.relation_type().clone());
        let mut to_remove = to_remove;
        to_remove.insert(tuple! { x: 1i64, y: 0i64, tile: "C" })?;

        wfc.domain = wfc
            .domain
            .difference(&to_remove)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        wfc.propagate_all()?;

        let count_a = wfc
            .domain
            .restrict(|t: &Tuple| t.get_typed::<String>("tile").unwrap() == "A")
            .cardinality();
        assert_eq!(count_a, 1);

        let count_b0 = wfc
            .domain
            .restrict(|t: &Tuple| {
                t.get_typed::<i64>("x").unwrap() == 0
                    && t.get_typed::<String>("tile").unwrap() == "B"
            })
            .cardinality();
        assert_eq!(count_b0, 0);

        Ok(())
    }
}
