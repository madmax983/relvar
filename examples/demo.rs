// Demonstration of the Date RDBMS database API
// This is the verification example from the implementation plan

use date::tuple;
use date::types::{RelationType, TupleType};
use date::{Database, ScalarType};
use tempfile::TempDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Date RDBMS Demo ===\n");

    // Create database
    let temp_dir = TempDir::new()?;
    let mut db = Database::open(temp_dir.path())?;
    println!("✓ Created database");

    // Define relation type
    let emp_type = RelationType::new(
        TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String)
            .with_attribute("dept_id".to_string(), ScalarType::Int),
    );

    // Create base relvar
    db.create_relvar("EMP", emp_type)?;
    println!("✓ Created EMP relation");

    // Insert tuples
    db.insert(
        "EMP",
        tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 },
    )?;
    db.insert("EMP", tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })?;
    db.insert(
        "EMP",
        tuple! { emp_id: 3i64, name: "Charlie", dept_id: 10i64 },
    )?;
    println!("✓ Inserted 3 employees");

    // Check all employees were inserted
    let all_emps = db.query("EMP")?;
    println!("\nAll employees: {}", all_emps.cardinality());
    for tuple in all_emps.tuples() {
        let emp_id = tuple.get_typed::<i64>("emp_id").unwrap();
        let name = tuple.get_typed::<String>("name").unwrap();
        let dept_id = tuple.get_typed::<i64>("dept_id").unwrap();
        println!("  emp_id: {}, name: {}, dept_id: {}", emp_id, name, dept_id);
    }

    // Query using algebra
    let result = db
        .query("EMP")?
        .restrict(|t| t.get_typed::<i64>("dept_id").unwrap() == 10)
        .project(&["emp_id", "name"]);

    println!("\n=== Query Result (dept_id = 10) ===");
    println!("Cardinality: {}", result.cardinality());
    println!("Degree: {}", result.degree());

    for tuple in result.tuples() {
        let emp_id = tuple.get_typed::<i64>("emp_id").unwrap();
        let name = tuple.get_typed::<String>("name").unwrap();
        println!("  emp_id: {}, name: {}", emp_id, name);
    }

    // Verify
    assert_eq!(result.cardinality(), 2);
    assert_eq!(result.degree(), 2);
    println!("\n✓ Query verification passed");

    // Test delete
    let deleted = db.delete("EMP", |t| t.get_typed::<i64>("emp_id").unwrap() == 2)?;
    println!("\n✓ Deleted {} tuple(s)", deleted);

    let remaining = db.query("EMP")?;
    println!("✓ Remaining tuples: {}", remaining.cardinality());
    assert_eq!(remaining.cardinality(), 2);

    println!("\n=== All Tests Passed! ===");
    println!("\nPhase 12: Database API - COMPLETE");

    Ok(())
}
