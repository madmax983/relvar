🌟 Nova: Relational Event Bus

💡 **The Spark:** "I noticed we can use natural joins to route data between entities."

🚀 **The Feature:** "Implemented `EventBus` where subscriptions and events are pure relations. Dispatching is evaluated natively via a relational Natural Join on `event_type`."

🔮 **The Potential:** "Could be used as the backbone for an entirely relational messaging queue or pub/sub broker architecture."

⚠️ **Risk:** "Low. Isolated in `src/experimental/event_bus.rs`."
