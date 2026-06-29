//! Relational Material Requirements Planning (MRP)
//!
//! This module demonstrates how to implement a Supply Chain Bill of Materials (BOM)
//! explosion using purely relational algebra. It iteratively breaks down product
//! demand into sub-assemblies and raw materials, multiplying quantities and
//! aggregating requirements.
//!
//! # Concept
//!
//! - **BOM**: Relation `(parent: String, child: String, qty_per: Int)`.
//! - **Demand**: Relation `(part: String, qty_required: Int)`.
//! - **Inventory**: Relation `(part: String, stock: Int)`.
//!
//! The algorithm iteratively joins demand with the BOM, multiplying quantities,
//! separating leaf nodes (raw materials) from intermediate assemblies, and accumulating
//! total raw material requirements. Finally, it joins with inventory to find shortages.

use relvar_core::{
    algebra::{Aggregation, ExtendError, SummarizeError, UnionError},
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue, Tuple},
};

/// Computes the total raw materials required to fulfill a given demand.
pub fn explode_bom(bom: &Relation, demand: &Relation) -> Result<Relation, DatabaseError> {
    // We need to find which parts are parents (assemblies) and which are leaves (raw materials)
    // parents_rel = bom { parent }
    let parents_rel = bom.project(&["parent"]);

    // We will accumulate raw materials here
    // Schema: { part: String, qty_required: Int }
    let mut raw_materials = Relation::new(demand.relation_type().clone());
    let mut current_demand = demand.clone();

    loop {
        if current_demand.cardinality() == 0 {
            break;
        }

        // 1. Separate current demand into raw materials (leaves) and assemblies (parents)
        // assemblies_demand = current_demand JOIN (parents_rel RENAME parent -> part)
        let parents_as_part = parents_rel.rename(&[("parent", "part")]);

        let assemblies_demand = current_demand.semijoin(&parents_as_part);

        // leaves_demand = current_demand MINUS assemblies_demand
        let leaves_demand = current_demand.semidifference(&parents_as_part);

        // 2. Add leaves_demand to raw_materials accumulator
        raw_materials = raw_materials
            .union(&leaves_demand)
            .map_err(|e: UnionError| DatabaseError::AlgebraError(e.to_string()))?;

        // If no assemblies, we're done
        if assemblies_demand.cardinality() == 0 {
            break;
        }

        // 3. Explode assemblies_demand
        // Rename part -> parent to match BOM
        let assemblies_renamed = assemblies_demand.rename(&[("part", "parent")]);

        // Join with BOM: { parent, child, qty_per, qty_required }
        let joined = assemblies_renamed.join(bom)?;

        // Extend to calculate new_qty = qty_required * qty_per
        let extended = joined
            .extend("new_qty", ScalarType::Int, |t: &Tuple| {
                let req = t.get_typed::<i64>("qty_required").unwrap();
                let per = t.get_typed::<i64>("qty_per").unwrap();
                ScalarValue::Int(req * per)
            })
            .map_err(|e: ExtendError| DatabaseError::AlgebraError(e.to_string()))?;

        // Project to { child as part, new_qty as qty_required }
        let next_level_raw = extended.project(&["child", "new_qty"]);
        let next_level_renamed =
            next_level_raw.rename(&[("child", "part"), ("new_qty", "qty_required")]);

        // 4. Summarize by part to combine duplicate requirements
        // e.g. if two assemblies need the same child part
        current_demand = next_level_renamed
            .summarize(
                &["part"],
                &[Aggregation::sum("qty_required", "qty_required")],
            )
            .map_err(|e: SummarizeError| DatabaseError::AlgebraError(e.to_string()))?;
    }

    // Finally, we might have accumulated identical raw materials from different iterations
    // Summarize one last time to ensure uniqueness by part
    let final_raw_materials = raw_materials
        .summarize(
            &["part"],
            &[Aggregation::sum("qty_required", "qty_required")],
        )
        .map_err(|e: SummarizeError| DatabaseError::AlgebraError(e.to_string()))?;

    Ok(final_raw_materials)
}

/// Joins raw material requirements with inventory to compute shortages.
pub fn compute_shortages(
    requirements: &Relation,
    inventory: &Relation,
) -> Result<Relation, DatabaseError> {
    // inventory: { part, stock }
    // requirements: { part, qty_required }

    // Some requirements might not exist in inventory at all.
    // To handle this properly, we need to do a Left Outer Join simulation or just standard Join.
    // Let's assume inventory has all parts, even with 0 stock. If not, we can union with missing parts having 0 stock.

    let joined = requirements.join(inventory)?;

    let extended = joined
        .extend("shortage", ScalarType::Int, |t: &Tuple| {
            let req = t.get_typed::<i64>("qty_required").unwrap();
            let stock = t.get_typed::<i64>("stock").unwrap();
            let diff = req - stock;
            ScalarValue::Int(if diff > 0 { diff } else { 0 })
        })
        .map_err(|e: ExtendError| DatabaseError::AlgebraError(e.to_string()))?;

    // Restrict to only actual shortages
    let filtered = extended.restrict(|t| t.get_typed::<i64>("shortage").unwrap() > 0);

    Ok(filtered.project(&["part", "shortage"]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_mrp_explosion() {
        // Schema
        let bom_type = RelationType::new(
            TupleType::new()
                .with_attribute("parent", ScalarType::String)
                .with_attribute("child", ScalarType::String)
                .with_attribute("qty_per", ScalarType::Int),
        );
        let demand_type = RelationType::new(
            TupleType::new()
                .with_attribute("part", ScalarType::String)
                .with_attribute("qty_required", ScalarType::Int),
        );
        let inventory_type = RelationType::new(
            TupleType::new()
                .with_attribute("part", ScalarType::String)
                .with_attribute("stock", ScalarType::Int),
        );

        let mut bom = Relation::new(bom_type);
        // Car consists of 1 Engine and 4 Wheels
        bom.insert(tuple! { parent: "Car", child: "Engine", qty_per: 1i64 })
            .unwrap();
        bom.insert(tuple! { parent: "Car", child: "Wheel", qty_per: 4i64 })
            .unwrap();

        // Engine consists of 4 Pistons and 1 Block
        bom.insert(tuple! { parent: "Engine", child: "Piston", qty_per: 4i64 })
            .unwrap();
        bom.insert(tuple! { parent: "Engine", child: "Block", qty_per: 1i64 })
            .unwrap();

        // Wheel consists of 1 Tire and 1 Rim
        bom.insert(tuple! { parent: "Wheel", child: "Tire", qty_per: 1i64 })
            .unwrap();
        bom.insert(tuple! { parent: "Wheel", child: "Rim", qty_per: 1i64 })
            .unwrap();

        let mut demand = Relation::new(demand_type);
        // We need 10 Cars
        demand
            .insert(tuple! { part: "Car", qty_required: 10i64 })
            .unwrap();
        // And an extra 5 Engines as spare parts
        demand
            .insert(tuple! { part: "Engine", qty_required: 5i64 })
            .unwrap();

        let raw_materials = explode_bom(&bom, &demand).unwrap();

        // 10 Cars -> 10 Engines, 40 Wheels
        // 5 Spare Engines -> 5 Engines
        // Total Engines = 15 -> 60 Pistons, 15 Blocks
        // Total Wheels = 40 -> 40 Tires, 40 Rims
        // Expected leaves: Piston: 60, Block: 15, Tire: 40, Rim: 40

        assert_eq!(raw_materials.cardinality(), 4);

        // Helper to extract qty
        let get_qty = |part_name: &str| -> i64 {
            let res =
                raw_materials.restrict(|t| t.get_typed::<String>("part").unwrap() == part_name);
            res.tuples()
                .next()
                .unwrap()
                .get_typed::<i64>("qty_required")
                .unwrap()
        };

        assert_eq!(get_qty("Piston"), 60);
        assert_eq!(get_qty("Block"), 15);
        assert_eq!(get_qty("Tire"), 40);
        assert_eq!(get_qty("Rim"), 40);

        // Now test shortages
        let mut inventory = Relation::new(inventory_type);
        inventory
            .insert(tuple! { part: "Piston", stock: 50i64 })
            .unwrap(); // Short 10
        inventory
            .insert(tuple! { part: "Block", stock: 20i64 })
            .unwrap(); // Ok
        inventory
            .insert(tuple! { part: "Tire", stock: 40i64 })
            .unwrap(); // Ok
        inventory
            .insert(tuple! { part: "Rim", stock: 10i64 })
            .unwrap(); // Short 30

        let shortages = compute_shortages(&raw_materials, &inventory).unwrap();

        assert_eq!(shortages.cardinality(), 2);

        let get_short = |part_name: &str| -> i64 {
            let res = shortages.restrict(|t| t.get_typed::<String>("part").unwrap() == part_name);
            res.tuples()
                .next()
                .unwrap()
                .get_typed::<i64>("shortage")
                .unwrap()
        };

        assert_eq!(get_short("Piston"), 10);
        assert_eq!(get_short("Rim"), 30);
    }
}
