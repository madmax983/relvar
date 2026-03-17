use relvar_core::{
    algebra::Aggregation,
    tuple,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue, Tuple},
};

fn main() {
    // 1. Setup AliveCells relation
    let mut alive_cells = Relation::new(RelationType::new(
        TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int),
    ));

    // Glider pattern:
    // . O .
    // . . O
    // O O O
    alive_cells.insert(tuple! { x: 1i64, y: 0i64 }).unwrap();
    alive_cells.insert(tuple! { x: 2i64, y: 1i64 }).unwrap();
    alive_cells.insert(tuple! { x: 0i64, y: 2i64 }).unwrap();
    alive_cells.insert(tuple! { x: 1i64, y: 2i64 }).unwrap();
    alive_cells.insert(tuple! { x: 2i64, y: 2i64 }).unwrap();

    // 2. Setup NeighborOffsets relation
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
                .unwrap();
        }
    }

    // Cartesian Product: AliveCells x NeighborOffsets
    let crossed = alive_cells.join(&offsets).unwrap();

    // Extend to calculate nx = x + dx, ny = y + dy
    let extended = crossed
        .extend("nx", ScalarType::Int, |t| {
            let x = t.get_typed::<i64>("x").unwrap();
            let dx = t.get_typed::<i64>("dx").unwrap();
            ScalarValue::Int(x + dx)
        })
        .unwrap()
        .extend("ny", ScalarType::Int, |t| {
            let y = t.get_typed::<i64>("y").unwrap();
            let dy = t.get_typed::<i64>("dy").unwrap();
            ScalarValue::Int(y + dy)
        })
        .unwrap();

    // Summarize BEFORE projecting out x and y (source identifying attributes)
    // Group by nx, ny and count
    let counts = extended
        .summarize(&["nx", "ny"], &[Aggregation::count("n_count")])
        .unwrap();

    // Now we have counts(nx, ny, n_count)
    // Rename nx->x, ny->y to match AliveCells
    let counts = counts.rename(&[("nx", "x"), ("ny", "y")]);

    println!("Counts:\n{:?}", counts);

    // Rule 1: Survivors (alive and count in {2, 3})
    let survivors = alive_cells
        .join(&counts)
        .unwrap()
        .restrict(|t: &Tuple| {
            let c = t.get_typed::<i64>("n_count").unwrap();
            c == 2 || c == 3
        })
        .project(&["x", "y"]);

    // Rule 2: Births (dead and count == 3)
    let alive_with_count = alive_cells.join(&counts).unwrap();
    let dead_with_count = counts.difference(&alive_with_count).unwrap();
    let births = dead_with_count
        .restrict(|t: &Tuple| {
            let c = t.get_typed::<i64>("n_count").unwrap();
            c == 3
        })
        .project(&["x", "y"]);

    // Next generation
    let next_gen = survivors.union(&births).unwrap();

    println!("Next Gen:\n{:?}", next_gen);
}
