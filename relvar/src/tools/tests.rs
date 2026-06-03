use relvar_core::values::{Relation, ScalarValue};
use relvar_core::types::{RelationType, TupleType, ScalarType};
use relvar_core::tuple;
use crate::tools::exporter::{to_ascii_table, to_csv, to_json};

#[test]
fn test_exporter_ascii_empty_relation() {
    let heading = TupleType::new();
    let relation = Relation::new(RelationType::new(heading));

    let table = to_ascii_table(&relation);
    assert_eq!(table, "(empty relation)");
}

#[test]
fn test_exporter_format_scalar_csv_relation() {
    let heading = TupleType::new()
        .with_attribute("rel", ScalarType::Relation(Box::new(RelationType::new(TupleType::new()))));

    let mut relation = Relation::new(RelationType::new(heading.clone()));
    let inner_rel = Relation::new(RelationType::new(TupleType::new()));
    relation.insert(tuple! { rel: ScalarValue::Relation(inner_rel) }).unwrap();

    let csv = to_csv(&relation, ',').unwrap();
    assert!(csv.contains("<Relation>"));
}

#[test]
fn test_exporter_format_scalar_json_relation() {
    let heading = TupleType::new()
        .with_attribute("rel", ScalarType::Relation(Box::new(RelationType::new(TupleType::new()))));

    let mut relation = Relation::new(RelationType::new(heading.clone()));
    let inner_rel = Relation::new(RelationType::new(TupleType::new()));
    relation.insert(tuple! { rel: ScalarValue::Relation(inner_rel) }).unwrap();

    let mut buf = Vec::new();
    to_json(&relation, &mut buf).unwrap();
    let json = String::from_utf8(buf).unwrap();
    assert!(json.contains("<Relation>"));
}

use crate::tools::importer::{from_csv, ImporterError};
use std::io::Cursor;

#[test]
fn test_importer_from_csv_empty() {
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
    );
    let csv_data = "id\n";
    let relation = from_csv(Cursor::new(csv_data), rel_type, ',').unwrap();
    assert_eq!(relation.cardinality(), 0);
}

#[test]
fn test_importer_from_csv_limit_exceeded() {
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
    );
    let csv_data = "id\n".to_string() + &"1\n".repeat(10_000_000 / 2);
    let result = from_csv(Cursor::new(csv_data), rel_type, ',');
    assert!(result.is_err());
    assert!(matches!(result, Err(ImporterError::LimitExceeded(_))));
}

#[test]
fn test_importer_from_csv_line_too_long() {
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("name", ScalarType::String)
    );
    let csv_data = "name\n".to_string() + &"A".repeat(1_048_576 + 10) + "\n";
    let result = from_csv(Cursor::new(csv_data), rel_type, ',');
    assert!(result.is_err());
    assert!(matches!(result, Err(ImporterError::LimitExceeded(_))));
}

#[test]
fn test_importer_from_csv_max_rows_exceeded() {
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
    );
    let csv_data = "id\n".to_string() + &"1\n".repeat(1_000_000 + 10);
    let result = from_csv(Cursor::new(csv_data), rel_type, ',');
    assert!(result.is_err());
    assert!(matches!(result, Err(ImporterError::LimitExceeded(_))));
}

use relvar_core::database::Database;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::constraints::{KeyConstraints, PrimaryKey};
use crate::tools::visualizer::SchemaVisualizer;

#[test]
fn test_visualizer_with_primary_key() {
    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
    );
    db.create_relvar("USERS", rel_type).unwrap();
    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    db.set_key_constraints("USERS", KeyConstraints::new().with_primary_key(pk)).unwrap();

    let viz = SchemaVisualizer::new(&db);
    let dot = viz.to_dot();
    assert!(dot.contains("<u>id</u>"));
}
