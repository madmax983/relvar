use relvar::{Database, InMemoryEngine};
use relvar::types::{TupleType, RelationType, ScalarType};
use relvar::visualizer::SchemaVisualizer;

#[test]
fn test_visualizer_injection() {
    let mut db = Database::new(InMemoryEngine::new());

    // Attempt to inject a new node definition using a malicious relvar name
    // The goal is to break out of the node ID string or the HTML label
    let malicious_name = "Malicious\"; node_injection [label=\"INJECTED\"]; \"";

    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
    );

    // We expect this to fail or result in a sanitized string, not a valid injection
    db.create_relvar(malicious_name, rel_type).unwrap();

    let visualizer = SchemaVisualizer::new(&db);
    let dot = visualizer.to_dot();

    println!("DOT Output:\n{}", dot);

    // Check if the injection worked
    // If successful, we would see `node_injection [label="INJECTED"]` as a separate statement
    // outside of quotes.
    // If secured, the whole thing should be inside quotes or escaped.

    assert!(!dot.contains("node_injection [label=\"INJECTED\"];"), "Injection successful!");

    // Also verify that the malicious name is properly quoted/escaped
    assert!(dot.contains(&format!("\"{}\"", malicious_name.replace("\"", "\\\""))));
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

    // If vulnerable, the DOT string will contain unescaped HTML tags that break the table structure
    // We check that the malicious string is HTML-escaped
    assert!(dot.contains("id&lt;/b&gt;&lt;/td&gt;&lt;/tr&gt;&lt;tr&gt;&lt;td bgcolor=&quot;red&quot;&gt;INJECTED"));
}
