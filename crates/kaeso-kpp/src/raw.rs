use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RawJson(Box<RawValue>);

impl RawJson {
    pub fn from_string(json: String) -> Result<Self, serde_json::Error> {
        RawValue::from_string(json).map(RawJson)
    }

    pub fn from_serialize<T: Serialize + ?Sized>(value: &T) -> Result<Self, serde_json::Error> {
        serde_json::value::to_raw_value(value).map(RawJson)
    }

    pub fn empty_object() -> Self {
        RawJson::from_string("{}".to_owned()).unwrap_or_else(|_| RawJson::null())
    }

    pub fn null() -> Self {
        RawJson(RawValue::NULL.to_owned())
    }

    pub fn get(&self) -> &str {
        self.0.get()
    }

    pub fn to_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(self.0.get())
    }

    pub fn parse<'a, T: Deserialize<'a>>(&'a self) -> Result<T, serde_json::Error> {
        serde_json::from_str(self.0.get())
    }

    pub fn is_object(&self) -> bool {
        self.0.get().trim_start().starts_with('{')
    }

    pub fn into_inner(self) -> Box<RawValue> {
        self.0
    }

    pub fn as_raw(&self) -> &RawValue {
        &self.0
    }
}

impl From<Box<RawValue>> for RawJson {
    fn from(raw: Box<RawValue>) -> Self {
        RawJson(raw)
    }
}

impl From<&RawValue> for RawJson {
    fn from(raw: &RawValue) -> Self {
        RawJson(raw.to_owned())
    }
}

impl TryFrom<&serde_json::Value> for RawJson {
    type Error = serde_json::Error;

    fn try_from(value: &serde_json::Value) -> Result<Self, Self::Error> {
        RawJson::from_serialize(value)
    }
}

impl PartialEq for RawJson {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl Eq for RawJson {}

impl fmt::Debug for RawJson {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RawJson({})", self.get())
    }
}

impl fmt::Display for RawJson {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.get())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_bytes_exactly() {
        let text = "{ \"b\": 1,  \"a\": [1.50, 2e3] }";
        let raw = RawJson::from_string(text.to_owned()).unwrap();
        assert_eq!(raw.get(), text);
        assert!(raw.is_object());
        let wrapped = serde_json::to_string(&vec![raw.clone()]).unwrap();
        assert_eq!(wrapped, format!("[{text}]"));
        let back: Vec<RawJson> = serde_json::from_str(&wrapped).unwrap();
        assert_eq!(back, vec![raw]);
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(RawJson::from_string("{\"a\":".to_owned()).is_err());
    }

    #[test]
    fn equality_is_by_bytes() {
        let a = RawJson::from_string("{\"a\":1}".to_owned()).unwrap();
        let b = RawJson::from_string("{\"a\": 1}".to_owned()).unwrap();
        assert_ne!(a, b);
        assert_eq!(a.to_value().unwrap(), b.to_value().unwrap());
        assert_eq!(RawJson::empty_object().get(), "{}");
        assert_eq!(RawJson::null().get(), "null");
    }
}
