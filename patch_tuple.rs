<<<<<<< SEARCH
impl<'a> TryFrom<&'a ScalarValue> for String {
    type Error = ();
    fn try_from(value: &'a ScalarValue) -> Result<Self, Self::Error> {
        match value {
            ScalarValue::String(v) => Ok(v.clone()),
            _ => Err(()),
        }
    }
}
=======
impl<'a> TryFrom<&'a ScalarValue> for String {
    type Error = ();
    fn try_from(value: &'a ScalarValue) -> Result<Self, Self::Error> {
        match value {
            ScalarValue::String(v) => Ok(v.clone()),
            _ => Err(()),
        }
    }
}

impl<'a> TryFrom<&'a ScalarValue> for &'a str {
    type Error = ();
    fn try_from(value: &'a ScalarValue) -> Result<Self, Self::Error> {
        match value {
            ScalarValue::String(v) => Ok(v.as_str()),
            _ => Err(()),
        }
    }
}
>>>>>>> REPLACE
