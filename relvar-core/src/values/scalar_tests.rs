#[cfg(test)]
mod tests {
    use crate::values::scalar::{ScalarValueError, ScalarValue};

    #[test]
    fn test_scalar_value_error_display() {
        let err = ScalarValueError;
        assert_eq!(err.to_string(), "Cannot extract observer from built-in type");
    }
}
