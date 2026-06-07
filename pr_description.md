🎻 Bard: [documentation update]

📖 Chapter: The `Spreadsheet` Module
🔦 Insight: The experimental `Spreadsheet` engine had placeholder examples for the main struct and its methods (`new` and `evaluate`), making it unclear how to initialize the `values` and `formulas` relations and how to resolve the spreadsheet's formulas to a fixpoint. Added comprehensive executable `/// # Examples` doc-tests to `Spreadsheet` and its methods, clearly demonstrating how to construct the initial state and trigger the evaluation of formulas using relational algebra operations.
🧪 Example: Added 3 executable doctests showing initialization and evaluation.
🖼️ Preview:
```rust
let val_type = TupleType::new()
    .with_attribute("id", ScalarType::String)
    .with_attribute("val", ScalarType::Float);
let mut values = Relation::new(RelationType::new(val_type));
values.insert(tuple! { id: "A1".to_string(), val: 10.0f64 }).unwrap();

let spreadsheet = Spreadsheet { values, formulas };
```