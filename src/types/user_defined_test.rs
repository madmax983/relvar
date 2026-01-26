/// Tests for user-defined scalar types (Issue #4)
/// TTM Prescription 1: The system must allow users to define their own scalar types.
///
/// This test file demonstrates the critical type safety violation:
/// Currently, WidgetId(5) and SupplierId(5) would both be ScalarValue::Int(5)
/// and thus equal, violating semantic type distinction.
use crate::types::ScalarType;
use crate::values::ScalarValue;

#[cfg(test)]
mod tests {
    use super::*;

    /// RED: This test demonstrates the critical problem.
    /// WidgetId and SupplierId should be DISTINCT types,
    /// even though they're both backed by Int.
    #[test]
    fn test_user_defined_types_are_distinct_from_builtin_types() {
        // Define two user-defined types, both backed by Int
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let supplier_id_type = ScalarType::user_defined("SupplierId", ScalarType::Int);

        // These types should NOT be equal even though they have the same representation
        assert_ne!(widget_id_type, supplier_id_type);

        // They should also not equal the built-in Int type
        assert_ne!(widget_id_type, ScalarType::Int);
        assert_ne!(supplier_id_type, ScalarType::Int);
    }

    /// RED: Values of different user-defined types should not be equal,
    /// even if they have the same underlying representation value.
    #[test]
    fn test_user_defined_values_are_type_safe() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let supplier_id_type = ScalarType::user_defined("SupplierId", ScalarType::Int);

        // Create values with the same underlying Int value (5)
        let widget_5 = ScalarValue::user_defined(widget_id_type.clone(), ScalarValue::Int(5));
        let supplier_5 = ScalarValue::user_defined(supplier_id_type.clone(), ScalarValue::Int(5));

        // These should NOT be equal - different types!
        assert_ne!(widget_5, supplier_5);

        // Values should also not equal raw Int(5)
        assert_ne!(widget_5, ScalarValue::Int(5));
        assert_ne!(supplier_5, ScalarValue::Int(5));
    }

    /// RED: Values of the same user-defined type with same value SHOULD be equal
    #[test]
    fn test_user_defined_values_of_same_type_are_equal() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);

        let widget_5_a = ScalarValue::user_defined(widget_id_type.clone(), ScalarValue::Int(5));
        let widget_5_b = ScalarValue::user_defined(widget_id_type.clone(), ScalarValue::Int(5));
        let widget_7 = ScalarValue::user_defined(widget_id_type.clone(), ScalarValue::Int(7));

        assert_eq!(widget_5_a, widget_5_b);
        assert_ne!(widget_5_a, widget_7);
    }

    /// RED: User-defined types should have names
    #[test]
    fn test_user_defined_types_have_names() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);

        assert_eq!(widget_id_type.name(), "WidgetId");
    }

    /// RED: Values should know their type
    #[test]
    fn test_user_defined_values_carry_their_type() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let widget_5 = ScalarValue::user_defined(widget_id_type.clone(), ScalarValue::Int(5));

        assert_eq!(widget_5.scalar_type(), widget_id_type);
        assert_ne!(widget_5.scalar_type(), ScalarType::Int);
    }

    /// RED: POSSREP - selectors should construct values
    #[test]
    fn test_possrep_selector_constructs_value() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);

        // Selector: construct a WidgetId from an Int
        let widget = widget_id_type.selector(ScalarValue::Int(42)).unwrap();

        assert_eq!(widget.scalar_type(), widget_id_type);
    }

    /// RED: POSSREP - observers should extract representation
    #[test]
    fn test_possrep_observer_extracts_representation() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let widget = widget_id_type.selector(ScalarValue::Int(42)).unwrap();

        // Observer: extract the underlying Int value
        let underlying = widget.observer().unwrap();

        assert_eq!(underlying, ScalarValue::Int(42));
    }

    /// RED: User-defined types can be based on other user-defined types
    #[test]
    fn test_nested_user_defined_types() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let special_widget_type =
            ScalarType::user_defined("SpecialWidgetId", widget_id_type.clone());

        let widget = widget_id_type.selector(ScalarValue::Int(42)).unwrap();
        let special_widget = special_widget_type.selector(widget.clone()).unwrap();

        assert_ne!(special_widget.scalar_type(), widget_id_type);
        assert_eq!(special_widget.scalar_type(), special_widget_type);
    }

    /// RED: Cannot create user-defined value with wrong representation type
    #[test]
    fn test_type_safety_prevents_wrong_representation() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);

        // Should fail: trying to construct WidgetId from String
        let result = widget_id_type.selector(ScalarValue::String("not an int".to_string()));

        assert!(result.is_err());
    }

    /// RED: User-defined types can be serialized and deserialized
    #[test]
    fn test_user_defined_types_serialize() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let widget = widget_id_type.selector(ScalarValue::Int(42)).unwrap();

        let serialized = serde_json::to_string(&widget).unwrap();
        let deserialized: ScalarValue = serde_json::from_str(&serialized).unwrap();

        assert_eq!(widget, deserialized);
        assert_eq!(deserialized.scalar_type(), widget_id_type);
    }

    /// RED: User-defined types can be hashed (for use in HashSet, HashMap)
    #[test]
    fn test_user_defined_values_can_be_hashed() {
        use std::collections::HashSet;

        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let supplier_id_type = ScalarType::user_defined("SupplierId", ScalarType::Int);

        let mut set = HashSet::new();
        set.insert(widget_id_type.selector(ScalarValue::Int(5)).unwrap());
        set.insert(widget_id_type.selector(ScalarValue::Int(5)).unwrap()); // Duplicate
        set.insert(supplier_id_type.selector(ScalarValue::Int(5)).unwrap()); // Different type
        set.insert(ScalarValue::Int(5)); // Raw Int

        // Should have 3 distinct values:
        // - WidgetId(5)
        // - SupplierId(5)
        // - Int(5)
        assert_eq!(set.len(), 3);
    }

    /// RED: Types with the same name but different representations are distinct
    #[test]
    fn test_user_defined_type_names_must_be_unique() {
        let type1 = ScalarType::user_defined("MyType", ScalarType::Int);
        let type2 = ScalarType::user_defined("MyType", ScalarType::String);

        // This test documents that types with the same name but different
        // representations are distinct, as `PartialEq` is structural.
        assert_ne!(type1, type2);
        assert_eq!(type1.name(), type2.name());
    }
}
