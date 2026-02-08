use crate::Database;
use crate::database::DatabaseError;
use crate::storage_engine::StorageEngine;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// Registers system views (virtual relvars) that expose database metadata.
///
/// This function defines the following virtual relvars:
///
/// - `_RELVARS`: Lists all relvars in the database.
///   - `name` (String): The name of the relvar.
///   - `degree` (Int): The number of attributes in the relvar.
///
/// - `_ATTRIBUTES`: Lists all attributes of all relvars.
///   - `relvar_name` (String): The name of the relvar.
///   - `attribute_name` (String): The name of the attribute.
///   - `type` (String): The scalar type of the attribute.
///   - `is_pk` (Bool): Whether the attribute is part of the primary key.
pub fn register<E: StorageEngine + 'static>(db: &mut Database<E>) -> Result<(), DatabaseError> {
    // Define _RELVARS
    let relvars_type = RelationType::new(
        TupleType::new()
            .with_attribute("name", ScalarType::String)
            .with_attribute("degree", ScalarType::Int),
    );

    db.define_virtual_relvar("_RELVARS", relvars_type, relvars_evaluator::<E>)?;

    // Define _ATTRIBUTES
    let attributes_type = RelationType::new(
        TupleType::new()
            .with_attribute("relvar_name", ScalarType::String)
            .with_attribute("attribute_name", ScalarType::String)
            .with_attribute("type", ScalarType::String)
            .with_attribute("is_pk", ScalarType::Bool),
    );

    db.define_virtual_relvar("_ATTRIBUTES", attributes_type, attributes_evaluator::<E>)?;

    Ok(())
}

fn relvars_evaluator<E: StorageEngine>(db: &mut Database<E>) -> Result<Relation, DatabaseError> {
    let relvar_names = db.list_relvars();

    let heading = TupleType::new()
        .with_attribute("name", ScalarType::String)
        .with_attribute("degree", ScalarType::Int);

    // Create empty relation
    let mut relation = Relation::new(RelationType::new(heading));

    for name in relvar_names {
        // Skip if we can't get the type (shouldn't happen for valid relvars)
        if let Ok(rel_type) = db.get_relvar_type(&name) {
            let degree = rel_type.degree() as i64;
            relation.insert(tuple! {
                name: name,
                degree: degree
            })?;
        }
    }

    Ok(relation)
}

fn attributes_evaluator<E: StorageEngine>(db: &mut Database<E>) -> Result<Relation, DatabaseError> {
    let relvar_names = db.list_relvars();

    let heading = TupleType::new()
        .with_attribute("relvar_name", ScalarType::String)
        .with_attribute("attribute_name", ScalarType::String)
        .with_attribute("type", ScalarType::String)
        .with_attribute("is_pk", ScalarType::Bool);

    let mut relation = Relation::new(RelationType::new(heading));

    for name in relvar_names {
        if let Ok(rel_type) = db.get_relvar_type(&name) {
            // Check for PK
            let pk_attrs = if let Some(key_constraints) = db.get_key_constraints(&name) {
                if let Some(pk) = key_constraints.primary_key() {
                    pk.attributes().to_vec()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            for attr_name in rel_type.heading().attribute_names() {
                let attr_type = rel_type.heading().get_attribute_type(attr_name).unwrap();
                let type_str = format!("{:?}", attr_type);
                let is_pk = pk_attrs.contains(attr_name);

                relation.insert(tuple! {
                    relvar_name: name.clone(),
                    attribute_name: attr_name.clone(),
                    type: type_str,
                    is_pk: is_pk
                })?;
            }
        }
    }

    Ok(relation)
}
