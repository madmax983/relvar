use relvar_core::constraints::ConstraintExpression;

#[test]
fn test_constraint_expression_recursion_limit() {
    // My custom limit is 32 (MAX_RECURSION_DEPTH).
    // Serde JSON default limit is 128.
    // If I use depth 64, serde_json would allow it, but my guard should block it.
    let depth = 64;
    let mut json = String::new();

    for _ in 0..depth {
        json.push_str("{\"Not\":");
    }

    // The innermost expression needs to be valid.
    json.push_str(r#"{"Cmp":{"left":"a","op":"Eq","right":{"Value":{"Int":1}}}}"#);

    for _ in 0..depth {
        json.push('}');
    }

    // Do NOT disable recursion limit (avoids API issues).
    // rely on default > 32.
    let result: Result<ConstraintExpression, _> = serde_json::from_str(&json);

    match result {
        Ok(_) => panic!(
            "Deserialization should have failed due to custom recursion limit (depth 64 > 32)"
        ),
        Err(e) => {
            let msg = e.to_string();
            println!("Deserialization failed as expected: {}", msg);
            // My error message is "Recursion limit exceeded"
            assert!(
                msg.contains("Recursion limit exceeded"),
                "Error message should mention recursion limit, got: {}",
                msg
            );
        }
    }
}
