// Simple test to isolate the issue

use date::{Database, ScalarType};
use date::types::{RelationType, TupleType};
use date::tuple;
use tempfile::TempDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Creating database...");
    let temp_dir = TempDir::new()?;
    let mut db = Database::open(temp_dir.path())?;

    println!("Creating relation...");
    let emp_type = RelationType::new(
        TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
    );
    db.create_relvar("TEST", emp_type)?;

    println!("Querying empty relation...");
    let result = db.query("TEST")?;
    println!("Empty relation cardinality: {}", result.cardinality());

    println!("Inserting tuple...");
    db.insert("TEST", tuple! { id: 1i64 })?;
    println!("Insert completed");

    println!("Querying after insert...");
    let result2 = db.query("TEST")?;
    println!("Cardinality: {}", result2.cardinality());

    println!("Tuples:");
    for tuple in result2.tuples() {
        println!("  {:?}", tuple);
    }

    println!("SUCCESS!");
    Ok(())
}
