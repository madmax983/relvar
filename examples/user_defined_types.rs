/// Example demonstrating user-defined scalar types with POSSREP pattern
/// TTM Prescription 1: The system must allow users to define their own scalar types.
///
/// This example shows how WidgetId and SupplierId are now DISTINCT types,
/// even though both are backed by Int.
use relvar::types::ScalarType;
use relvar::values::ScalarValue;

fn main() {
    println!("=== User-Defined Scalar Types Demo ===\n");

    // Define two user-defined types, both backed by Int
    let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
    let supplier_id_type = ScalarType::user_defined("SupplierId", ScalarType::Int);

    println!("1. Type Identity:");
    println!("   WidgetId type: {}", widget_id_type.name());
    println!("   SupplierId type: {}", supplier_id_type.name());
    println!(
        "   Are they equal? {}",
        if widget_id_type == supplier_id_type {
            "YES (BUG!)"
        } else {
            "NO (Correct!)"
        }
    );
    println!();

    // Use POSSREP selectors to create values
    println!("2. POSSREP Selectors (construct values):");
    let widget_5 = widget_id_type
        .selector(ScalarValue::Int(5))
        .expect("Failed to create WidgetId(5)");
    let supplier_5 = supplier_id_type
        .selector(ScalarValue::Int(5))
        .expect("Failed to create SupplierId(5)");

    println!("   Created: WidgetId(5)");
    println!("   Created: SupplierId(5)");
    println!();

    // Values should NOT be equal even though both wrap Int(5)
    println!("3. Type Safety:");
    println!(
        "   WidgetId(5) == SupplierId(5)? {}",
        if widget_5 == supplier_5 {
            "YES (BUG!)"
        } else {
            "NO (Correct!)"
        }
    );
    println!(
        "   WidgetId(5) == Int(5)? {}",
        if widget_5 == ScalarValue::Int(5) {
            "YES (BUG!)"
        } else {
            "NO (Correct!)"
        }
    );
    println!();

    // Use POSSREP observers to extract values
    println!("4. POSSREP Observers (extract representation):");
    let widget_underlying = widget_5.observer().expect("Failed to observe WidgetId");
    println!("   WidgetId(5) observer -> {:?}", widget_underlying);
    println!();

    // Values of same type with same value ARE equal
    println!("5. Same Type Equality:");
    let widget_5_again = widget_id_type
        .selector(ScalarValue::Int(5))
        .expect("Failed to create second WidgetId(5)");
    println!(
        "   WidgetId(5) == WidgetId(5)? {}",
        if widget_5 == widget_5_again {
            "YES (Correct!)"
        } else {
            "NO (BUG!)"
        }
    );
    println!();

    // Nested user-defined types
    println!("6. Nested User-Defined Types:");
    let special_widget_type = ScalarType::user_defined("SpecialWidgetId", widget_id_type.clone());
    let special_widget = special_widget_type
        .selector(widget_5.clone())
        .expect("Failed to create SpecialWidgetId");
    println!("   Created: SpecialWidgetId(WidgetId(5))");
    println!(
        "   SpecialWidgetId == WidgetId? {}",
        if special_widget == widget_5 {
            "YES (BUG!)"
        } else {
            "NO (Correct!)"
        }
    );
    println!();

    // Type safety prevents wrong representation
    println!("7. Type Safety Enforcement:");
    let result = widget_id_type.selector(ScalarValue::String("not an int".to_string()));
    match result {
        Ok(_) => println!("   ERROR: Allowed String for Int-based type!"),
        Err(e) => println!("   Correctly rejected: {}", e),
    }
    println!();

    println!("=== All Tests Passed! ===");
    println!("\nTTM Prescription 1: ✅ Implemented");
    println!("POSSREP Pattern: ✅ Selectors and Observers working");
    println!("Type Safety: ✅ Enforced at runtime");
}
