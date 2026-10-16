use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::dialect::{DialectId, Origin};
use crate::raw::RawJson;

pub const HEADER_PREFIX: &str = "header:";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Extras {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub fields: IndexMap<String, RawJson>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub form: IndexMap<String, Value>,
}

impl PartialEq for Extras {
    fn eq(&self, other: &Self) -> bool {
        if self.is_empty() && other.is_empty() {
            return true;
        }
        self.origin == other.origin && self.fields == other.fields && self.form == other.form
    }
}

impl Extras {
    pub fn new(origin: impl Into<Origin>) -> Self {
        Extras {
            origin: Some(origin.into()),
            fields: IndexMap::new(),
            form: IndexMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty() && self.form.is_empty()
    }

    pub fn is_from(&self, dialect: DialectId) -> bool {
        self.origin.as_ref().is_some_and(|o| o.is(dialect))
    }

    pub fn fields_for(&self, dialect: DialectId) -> Option<&IndexMap<String, RawJson>> {
        self.is_from(dialect).then_some(&self.fields)
    }

    pub fn form_for(&self, dialect: DialectId) -> Option<&IndexMap<String, Value>> {
        self.is_from(dialect).then_some(&self.form)
    }

    pub fn form_value(&self, dialect: DialectId, key: &str) -> Option<&Value> {
        self.form_for(dialect).and_then(|f| f.get(key))
    }

    pub fn form_flag(&self, dialect: DialectId, key: &str) -> bool {
        matches!(self.form_value(dialect, key), Some(Value::Bool(true)))
    }

    pub fn form_str(&self, dialect: DialectId, key: &str) -> Option<&str> {
        self.form_value(dialect, key).and_then(Value::as_str)
    }

    pub fn with_field(mut self, key: impl Into<String>, value: RawJson) -> Self {
        self.fields.insert(key.into(), value);
        self
    }

    pub fn with_form(mut self, key: impl Into<String>, value: Value) -> Self {
        self.form.insert(key.into(), value);
        self
    }

    pub fn insert_header(&mut self, name: &str, value: &str) {
        let key = format!("{HEADER_PREFIX}{}", name.to_ascii_lowercase());
        let joined = match self.fields.get(&key).and_then(|v| v.parse::<String>().ok()) {
            Some(existing) => format!("{existing}, {value}"),
            None => value.to_owned(),
        };
        if let Ok(raw) = RawJson::from_serialize(&joined) {
            self.fields.insert(key, raw);
        }
    }

    pub fn headers(&self) -> impl Iterator<Item = (&str, String)> {
        self.fields.iter().filter_map(|(k, v)| {
            let name = k.strip_prefix(HEADER_PREFIX)?;
            Some((name, v.parse::<String>().ok()?))
        })
    }

    pub fn body_fields(&self) -> impl Iterator<Item = (&str, &RawJson)> {
        self.fields
            .iter()
            .filter(|(k, _)| !k.starts_with(HEADER_PREFIX))
            .map(|(k, v)| (k.as_str(), v))
    }

    pub fn nested_fields<'a>(
        &'a self,
        prefix: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a RawJson)> {
        self.body_fields().filter_map(move |(k, v)| {
            let rest = k.strip_prefix(prefix)?.strip_prefix('.')?;
            Some((rest, v))
        })
    }

    pub fn foreign_fields(&self, target: DialectId) -> impl Iterator<Item = &str> {
        let foreign = !self.is_from(target);
        self.fields
            .keys()
            .filter(move |_| foreign)
            .map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn raw(s: &str) -> RawJson {
        RawJson::from_string(s.to_owned()).unwrap()
    }

    #[test]
    fn empty_extras_serialize_to_empty_object_and_are_equal() {
        assert_eq!(serde_json::to_string(&Extras::default()).unwrap(), "{}");
        let back: Extras = serde_json::from_str("{}").unwrap();
        assert!(back.is_empty());
        assert_eq!(Extras::new(DialectId::OpenAiChat), Extras::default());
        assert_ne!(
            Extras::new(DialectId::OpenAiChat).with_field("a", raw("1")),
            Extras::new(DialectId::AnthropicMessages).with_field("a", raw("1"))
        );
    }

    #[test]
    fn fields_keep_bytes_and_are_bound_to_origin() {
        let e = Extras::new(DialectId::AnthropicMessages)
            .with_field("big", raw("123456789012345678901234567890"))
            .with_form("content", json!("string"));
        assert_eq!(
            serde_json::to_string(&e.fields).unwrap(),
            r#"{"big":123456789012345678901234567890}"#
        );
        assert!(e.fields_for(DialectId::AnthropicMessages).is_some());
        assert!(e.fields_for(DialectId::OpenAiChat).is_none());
        assert_eq!(
            e.form_str(DialectId::AnthropicMessages, "content"),
            Some("string")
        );
        assert_eq!(e.form_str(DialectId::OpenAiChat, "content"), None);
        assert_eq!(
            e.foreign_fields(DialectId::OpenAiChat).collect::<Vec<_>>(),
            vec!["big"]
        );
        assert_eq!(e.foreign_fields(DialectId::AnthropicMessages).count(), 0);
    }

    #[test]
    fn headers_are_joined_and_kept_apart_from_body_fields() {
        let mut e = Extras::new(DialectId::AnthropicMessages).with_field("x", raw("1"));
        e.insert_header("Anthropic-Beta", "a");
        e.insert_header("anthropic-beta", "b");
        assert_eq!(
            e.headers().collect::<Vec<_>>(),
            vec![("anthropic-beta", "a, b".to_owned())]
        );
        assert_eq!(
            e.body_fields().map(|(k, _)| k).collect::<Vec<_>>(),
            vec!["x"]
        );
    }

    #[test]
    fn nested_fields_follow_the_key_path_convention() {
        let e = Extras::new(DialectId::OpenAiChat)
            .with_field("function.x", raw("1"))
            .with_field("functional", raw("2"))
            .with_field("y", raw("3"));
        assert_eq!(
            e.nested_fields("function")
                .map(|(k, _)| k)
                .collect::<Vec<_>>(),
            vec!["x"]
        );
    }
}
