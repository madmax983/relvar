//! Relational Text Adventure Engine
//!
//! Models a Text Adventure / MUD game using purely relational algebra.
//!
//! # Concept
//!
//! - **Rooms**: `(room_id: String, name: String, description: String)`
//! - **Exits**: `(from_room: String, direction: String, to_room: String)`
//! - **Items**: `(item_id: String, room_id: String, name: String)`
//! - **Inventory**: `(item_id: String)`
//! - **PlayerState**: `(current_room: String)`

use relvar_core::{
    error::DatabaseError,
    values::{Relation, ScalarValue},
};

/// A Relational Text Adventure Engine.
pub struct TextAdventure {
    /// Rooms. Schema: `(room_id: String, name: String, description: String)`
    pub rooms: Relation,
    /// Exits. Schema: `(from_room: String, direction: String, to_room: String)`
    pub exits: Relation,
    /// Items in rooms. Schema: `(item_id: String, room_id: String, name: String)`
    pub items: Relation,
    /// Player Inventory. Schema: `(item_id: String)`
    pub inventory: Relation,
    /// Player State. Schema: `(current_room: String)`
    pub player_state: Relation,
}

impl TextAdventure {
    /// Creates a new Text Adventure instance.
    pub fn new(
        rooms: Relation,
        exits: Relation,
        items: Relation,
        inventory: Relation,
        player_state: Relation,
    ) -> Self {
        Self {
            rooms,
            exits,
            items,
            inventory,
            player_state,
        }
    }

    /// Looks around the current room, returning its details, exits, and items.
    pub fn look(&self) -> Result<(Relation, Relation, Relation), DatabaseError> {
        let current_room_info = self
            .player_state
            .rename(&[("current_room", "room_id")])
            .join(&self.rooms)?;

        let state_renamed = self.player_state.rename(&[("current_room", "from_room")]);
        let available_exits = state_renamed.join(&self.exits)?;

        let state_renamed_for_items = self.player_state.rename(&[("current_room", "room_id")]);
        let visible_items = state_renamed_for_items.join(&self.items)?;

        Ok((current_room_info, available_exits, visible_items))
    }

    /// Moves the player in a given direction.
    /// Returns true if successful, false if no exit exists.
    pub fn walk(&mut self, direction: &str) -> Result<bool, DatabaseError> {
        let state_renamed = self.player_state.rename(&[("current_room", "from_room")]);
        let available_exits = state_renamed.join(&self.exits)?;

        let dir_val = direction.to_string();
        let matched_exit = available_exits.restrict(move |t| {
            if let Some(ScalarValue::String(d)) = t.get("direction") {
                d == &dir_val
            } else {
                false
            }
        });

        if matched_exit.cardinality() == 0 {
            return Ok(false);
        }

        let new_state = matched_exit
            .project(&["to_room"])
            .rename(&[("to_room", "current_room")]);

        self.player_state = new_state;
        Ok(true)
    }

    /// Takes an item from the current room.
    pub fn take(&mut self, item_name: &str) -> Result<bool, DatabaseError> {
        let state_renamed = self.player_state.rename(&[("current_room", "room_id")]);
        let visible_items = state_renamed.join(&self.items)?;

        let item_name_val = item_name.to_string();
        let matched_item = visible_items.restrict(move |t| {
            if let Some(ScalarValue::String(n)) = t.get("name") {
                n == &item_name_val
            } else {
                false
            }
        });

        if matched_item.cardinality() == 0 {
            return Ok(false);
        }

        let item_to_remove = matched_item.project(&["item_id", "room_id", "name"]);
        self.items = self
            .items
            .difference(&item_to_remove)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let item_to_add = matched_item.project(&["item_id"]);
        self.inventory = self
            .inventory
            .union(&item_to_add)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::{
        tuple,
        types::{RelationType, ScalarType, TupleType},
    };

    #[test]
    fn test_text_adventure() {
        let room_heading = TupleType::new()
            .with_attribute("room_id", ScalarType::String)
            .with_attribute("name", ScalarType::String)
            .with_attribute("description", ScalarType::String);
        let mut rooms = Relation::new(RelationType::new(room_heading));
        rooms
            .insert(tuple! { room_id: "r1", name: "Dungeon", description: "A dark dungeon." })
            .unwrap();
        rooms
            .insert(tuple! { room_id: "r2", name: "Hallway", description: "A long hallway." })
            .unwrap();

        let exit_heading = TupleType::new()
            .with_attribute("from_room", ScalarType::String)
            .with_attribute("direction", ScalarType::String)
            .with_attribute("to_room", ScalarType::String);
        let mut exits = Relation::new(RelationType::new(exit_heading));
        exits
            .insert(tuple! { from_room: "r1", direction: "north", to_room: "r2" })
            .unwrap();
        exits
            .insert(tuple! { from_room: "r2", direction: "south", to_room: "r1" })
            .unwrap();

        let items_heading = TupleType::new()
            .with_attribute("item_id", ScalarType::String)
            .with_attribute("room_id", ScalarType::String)
            .with_attribute("name", ScalarType::String);
        let mut items = Relation::new(RelationType::new(items_heading));
        items
            .insert(tuple! { item_id: "i1", room_id: "r1", name: "key" })
            .unwrap();

        let inv_heading = TupleType::new().with_attribute("item_id", ScalarType::String);
        let inventory = Relation::new(RelationType::new(inv_heading));

        let state_heading = TupleType::new().with_attribute("current_room", ScalarType::String);
        let mut player_state = Relation::new(RelationType::new(state_heading));
        player_state.insert(tuple! { current_room: "r1" }).unwrap();

        let mut game = TextAdventure::new(rooms, exits, items, inventory, player_state);

        let (info, avail_exits, vis_items) = game.look().unwrap();
        assert_eq!(info.cardinality(), 1);
        assert_eq!(avail_exits.cardinality(), 1);
        assert_eq!(vis_items.cardinality(), 1);

        assert!(game.take("key").unwrap());
        assert_eq!(game.inventory.cardinality(), 1);
        assert_eq!(game.items.cardinality(), 0);

        assert!(!game.walk("east").unwrap());
        assert!(game.walk("north").unwrap());

        let (info2, _, _) = game.look().unwrap();
        let current_room_name = info2
            .tuples()
            .next()
            .unwrap()
            .get_typed::<String>("name")
            .unwrap();
        assert_eq!(current_room_name, "Hallway");
    }
}
