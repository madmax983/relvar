//! Relational Event Bus
//!
//! A publish/subscribe system where events and subscriptions are modeled
//! as relations. The routing of events to subscribers is performed natively
//! through relational algebra (specifically, Natural Joins).

use relvar_core::{
    error::DatabaseError,
    values::Relation,
};

/// A Relational Event Bus.
///
/// Subscriptions and events are stored as relations.
/// Dispatching is evaluated by joining the events against the subscriptions.
pub struct EventBus {
    /// Schema: `(subscriber_id: String, event_type: String)`
    pub subscriptions: Relation,
    /// Schema: `(event_id: String, event_type: String, payload: String)`
    pub events: Relation,
}

impl EventBus {
    /// Creates a new Relational Event Bus.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::event_bus::EventBus;
    /// // Note: This is a placeholder example
    /// ```
    pub fn new(subscriptions: Relation, events: Relation) -> Self {
        Self {
            subscriptions,
            events,
        }
    }

    /// Evaluates the dispatch of all pending events to subscribers.
    ///
    /// Returns a relation representing the messages to be delivered.
    /// Schema: `(subscriber_id: String, event_id: String, event_type: String, payload: String)`
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::event_bus::EventBus;
    /// // Note: This is a placeholder example
    /// ```
    pub fn dispatch(&self) -> Result<Relation, DatabaseError> {
        // Natural join on `event_type` automatically routes events to the correct subscribers.
        self.subscriptions.join(&self.events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_event_bus_dispatch() {
        let sub_heading = TupleType::new()
            .with_attribute("subscriber_id", ScalarType::String)
            .with_attribute("event_type", ScalarType::String);
        let mut subs = Relation::new(RelationType::new(sub_heading));

        subs.insert(tuple! { subscriber_id: "UserA", event_type: "LOGIN" }).unwrap();
        subs.insert(tuple! { subscriber_id: "UserB", event_type: "LOGIN" }).unwrap();
        subs.insert(tuple! { subscriber_id: "UserA", event_type: "LOGOUT" }).unwrap();

        let event_heading = TupleType::new()
            .with_attribute("event_id", ScalarType::String)
            .with_attribute("event_type", ScalarType::String)
            .with_attribute("payload", ScalarType::String);
        let mut events = Relation::new(RelationType::new(event_heading));

        events.insert(tuple! { event_id: "e1", event_type: "LOGIN", payload: "IP: 192.168.1.1" }).unwrap();
        events.insert(tuple! { event_id: "e2", event_type: "PURCHASE", payload: "Item: Apple" }).unwrap();

        let bus = EventBus::new(subs, events);
        let dispatches = bus.dispatch().unwrap();

        // Expect UserA and UserB to get e1 (LOGIN), and no one to get e2 (PURCHASE).
        assert_eq!(dispatches.cardinality(), 2);
    }
}
