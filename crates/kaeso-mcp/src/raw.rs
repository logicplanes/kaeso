use std::borrow::Cow;
use std::fmt;

use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, Visitor};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

pub type Member<'a> = (Cow<'a, str>, Cow<'a, RawValue>);

// A JSON object whose members keep their order and their bytes: a proxy must pass on what it
// doesn't understand exactly as it came, so values are never parsed into `serde_json::Value`.
#[derive(Debug, Clone, Default)]
pub struct RawObject<'a> {
    members: Vec<Member<'a>>,
}

impl<'a> RawObject<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    pub fn of(raw: &'a RawValue) -> Result<Self, serde_json::Error> {
        serde_json::from_str(raw.get())
    }

    pub fn get(&self, key: &str) -> Option<&RawValue> {
        self.members
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_ref())
    }

    pub fn borrowed(&self, key: &str) -> Option<&'a RawValue> {
        self.members
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| match v {
                Cow::Borrowed(v) => Some(*v),
                Cow::Owned(_) => None,
            })
    }

    pub fn string(&self, key: &str) -> Option<String> {
        self.get(key).and_then(string)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.members.iter().any(|(k, _)| k == key)
    }

    pub fn set(&mut self, key: &str, value: Box<RawValue>) {
        match self.members.iter_mut().find(|(k, _)| k == key) {
            Some((_, v)) => *v = Cow::Owned(value),
            None => self
                .members
                .push((Cow::Owned(key.to_owned()), Cow::Owned(value))),
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<Cow<'a, RawValue>> {
        let at = self.members.iter().position(|(k, _)| k == key)?;
        Some(self.members.remove(at).1)
    }

    pub fn members(&self) -> impl Iterator<Item = (&str, &RawValue)> {
        self.members.iter().map(|(k, v)| (k.as_ref(), v.as_ref()))
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn write(&self, out: &mut Vec<u8>) {
        out.push(b'{');
        for (i, (key, value)) in self.members.iter().enumerate() {
            if i > 0 {
                out.push(b',');
            }
            write_string(out, key);
            out.push(b':');
            out.extend_from_slice(value.get().as_bytes());
        }
        out.push(b'}');
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            self.members
                .iter()
                .map(|(k, v)| k.len() + v.get().len() + 4)
                .sum::<usize>()
                + 2,
        );
        self.write(&mut out);
        out
    }

    pub fn into_raw(self) -> Box<RawValue> {
        let bytes = self.to_vec();
        // `write` emits valid JSON from valid members, so this conversion can't fail; an empty
        // object is the harmless answer if it ever did.
        String::from_utf8(bytes)
            .ok()
            .and_then(|s| RawValue::from_string(s).ok())
            .unwrap_or_else(empty_object)
    }
}

pub fn empty_object() -> Box<RawValue> {
    raw_string("{}")
}

pub(crate) fn raw_string(json: &str) -> Box<RawValue> {
    RawValue::from_string(json.to_owned()).unwrap_or_else(|_| null())
}

pub fn null() -> Box<RawValue> {
    serde_json::value::to_raw_value(&()).unwrap_or_else(|_| Box::<RawValue>::default())
}

pub fn string(raw: &RawValue) -> Option<String> {
    serde_json::from_str::<String>(raw.get()).ok()
}

pub fn json_string(value: &str) -> Box<RawValue> {
    serde_json::value::to_raw_value(value).unwrap_or_else(|_| null())
}

pub(crate) fn write_string(out: &mut Vec<u8>, value: &str) {
    if serde_json::to_writer(&mut *out, value).is_err() {
        out.extend_from_slice(b"\"\"");
    }
}

impl Serialize for RawObject<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.members.len()))?;
        for (key, value) in &self.members {
            map.serialize_entry(key.as_ref(), value.as_ref())?;
        }
        map.end()
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for RawObject<'a> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(ObjectVisitor)
    }
}

struct ObjectVisitor;

impl<'de> Visitor<'de> for ObjectVisitor {
    type Value = RawObject<'de>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON object")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
        let mut members: Vec<Member<'de>> = Vec::with_capacity(map.size_hint().unwrap_or(4));
        while let Some(key) = map.next_key_seed(KeySeed)? {
            // Two members with one name would let a check read the first while the server that
            // gets the message reads the last.
            if members.iter().any(|(k, _)| *k == key) {
                return Err(de::Error::custom(format_args!("duplicate member `{key}`")));
            }
            let value: &'de RawValue = map.next_value()?;
            members.push((key, Cow::Borrowed(value)));
        }
        Ok(RawObject { members })
    }
}

struct KeySeed;

impl<'de> DeserializeSeed<'de> for KeySeed {
    type Value = Cow<'de, str>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_str(KeyVisitor)
    }
}

struct KeyVisitor;

impl<'de> Visitor<'de> for KeyVisitor {
    type Value = Cow<'de, str>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a member name")
    }

    fn visit_borrowed_str<E: de::Error>(self, v: &'de str) -> Result<Self::Value, E> {
        Ok(Cow::Borrowed(v))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(Cow::Owned(v.to_owned()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
        Ok(Cow::Owned(v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn members_keep_their_order_and_bytes() {
        let body = br#"{"b": [1,  2], "a":{"x" :1.50},"c":"\u00e9"}"#;
        let o = RawObject::parse(body).unwrap();
        let keys: Vec<_> = o.members().map(|(k, _)| k).collect();
        assert_eq!(keys, ["b", "a", "c"]);
        assert_eq!(o.get("b").unwrap().get(), "[1,  2]");
        assert_eq!(o.get("a").unwrap().get(), r#"{"x" :1.50}"#);
        assert_eq!(o.get("c").unwrap().get(), r#""\u00e9""#);
        assert_eq!(
            String::from_utf8(o.to_vec()).unwrap(),
            r#"{"b":[1,  2],"a":{"x" :1.50},"c":"\u00e9"}"#
        );
    }

    #[test]
    fn values_are_borrowed_from_the_body() {
        let body = br#"{"arguments":{"text":"hello"}}"#;
        let o = RawObject::parse(body).unwrap();
        let (_, value) = o.members.first().unwrap();
        assert!(matches!(value, Cow::Borrowed(_)));
        let start = body.as_ptr() as usize;
        let at = value.get().as_ptr() as usize;
        assert!(at >= start && at < start + body.len());
    }

    #[test]
    fn escaped_names_are_read_and_written_as_json() {
        let o = RawObject::parse(br#"{"n\u0061me":"x","q\"":1}"#).unwrap();
        assert_eq!(o.string("name").as_deref(), Some("x"));
        assert!(o.contains("q\""));
        assert_eq!(
            String::from_utf8(o.to_vec()).unwrap(),
            r#"{"name":"x","q\"":1}"#
        );
    }

    #[test]
    fn duplicate_members_are_refused() {
        let err = RawObject::parse(br#"{"name":"a","name":"b"}"#).unwrap_err();
        assert!(err.to_string().contains("duplicate member `name`"));
    }

    #[test]
    fn only_objects_parse() {
        assert!(RawObject::parse(b"[1,2]").is_err());
        assert!(RawObject::parse(b"\"x\"").is_err());
        assert!(RawObject::parse(b"{").is_err());
        assert!(RawObject::parse(b"{}").unwrap().is_empty());
    }

    #[test]
    fn set_replaces_in_place_and_appends_new_members() {
        let mut o = RawObject::parse(br#"{"name":"x__a","arguments":{}}"#).unwrap();
        o.set("name", json_string("a"));
        o.set("extra", json_string("y"));
        assert_eq!(
            String::from_utf8(o.to_vec()).unwrap(),
            r#"{"name":"a","arguments":{},"extra":"y"}"#
        );
        assert_eq!(o.len(), 3);
        assert_eq!(o.remove("extra").unwrap().get(), r#""y""#);
        assert!(o.remove("extra").is_none());
    }

    #[test]
    fn serialize_matches_write() {
        let o = RawObject::parse(br#"{"a": {"b" : [ 1 ]}}"#).unwrap();
        assert_eq!(serde_json::to_vec(&o).unwrap(), o.to_vec());
        assert_eq!(o.clone().into_raw().get(), r#"{"a":{"b" : [ 1 ]}}"#);
    }

    #[test]
    fn helpers_build_json() {
        assert_eq!(null().get(), "null");
        assert_eq!(empty_object().get(), "{}");
        assert_eq!(json_string("a\"b").get(), r#""a\"b""#);
        assert_eq!(string(&json_string("é")).as_deref(), Some("é"));
        assert_eq!(string(&null()), None);
        let raw = RawValue::from_string(r#"{"k":1}"#.to_owned()).unwrap();
        assert_eq!(RawObject::of(&raw).unwrap().get("k").unwrap().get(), "1");
    }
}
