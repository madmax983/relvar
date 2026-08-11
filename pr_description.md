# 🪒 Razor: [Remove MockRelation builder]

Replaced the over-engineered `MockRelation` builder pattern in `relvar/src/experimental/mock.rs` with a simple `generate_mock_relation` function that takes the 3 parameters directly. This simplifies the code and reduces cognitive load by avoiding a verbose pattern for a simple use case.
