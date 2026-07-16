use relvar::visualizer::SchemaVisualizer;
use relvar::{Database, InMemoryEngine, RelationType, ScalarType, TupleType};

#[test]
fn test_dot_injection_relvar_name() {
    let mut db = Database::new(InMemoryEngine::new());

    // Malicious relvar name attempting to inject a new edge
    let malicious_name = "Malicious\"; node_b -> node_c; \"";

    let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    db.create_relvar(malicious_name, rel_type).unwrap();

    let visualizer = SchemaVisualizer::new(&db);
    let dot = visualizer.to_dot();

    println!("{}", dot);

    // If vulnerable, the DOT string will contain the injected edge directly
    // Ideally, the name should be quoted/escaped so it's treated as a single identifier
}

#[test]
fn test_html_injection_attribute_name() {
    let mut db = Database::new(InMemoryEngine::new());

    let malicious_attr = "id</b></td></tr><tr><td bgcolor=\"red\">INJECTED";

    let rel_type =
        RelationType::new(TupleType::new().with_attribute(malicious_attr, ScalarType::Int));
    db.create_relvar("VulnerableTable", rel_type).unwrap();

    let visualizer = SchemaVisualizer::new(&db);
    let dot = visualizer.to_dot();

    println!("{}", dot);

    // If vulnerable, the DOT string will have broken HTML structure
}
