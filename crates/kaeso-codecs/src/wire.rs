use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt;

use indexmap::IndexMap;
use kaeso_kpp::{DialectId, Extras, HEADER_PREFIX, KppError, Origin, RawJson, Source};
use serde::de::{DeserializeOwned, Error as _, MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use serde_json::value::RawValue;

use crate::struct_fields::{write_into, written_names};

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Ext<T> {
    pub known: T,
    pub extra: IndexMap<String, RawJson>,
    pub verbatim: IndexMap<String, RawJson>,
}

impl<T> Ext<T> {
    pub(crate) fn new(known: T) -> Self {
        Ext {
            known,
            extra: IndexMap::new(),
            verbatim: IndexMap::new(),
        }
    }

    pub(crate) fn with(
        known: T,
        extra: IndexMap<String, RawJson>,
        verbatim: IndexMap<String, RawJson>,
    ) -> Self {
        Ext {
            known,
            extra,
            verbatim,
        }
    }
}

impl<T> std::ops::Deref for Ext<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.known
    }
}

impl<T> std::ops::DerefMut for Ext<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.known
    }
}

// Above this many keys a duplicate is found through a set rather than by comparing each new key
// with the ones before it.
const LINEAR_KEYS: usize = 16;

struct Key<'a>(Cow<'a, str>);

impl<'de: 'a, 'a> Deserialize<'de> for Key<'a> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Key<'de>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a string")
            }

            fn visit_borrowed_str<E: serde::de::Error>(
                self,
                v: &'de str,
            ) -> Result<Self::Value, E> {
                Ok(Key(Cow::Borrowed(v)))
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(Key(Cow::Owned(v.to_owned())))
            }

            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(Key(Cow::Owned(v)))
            }
        }
        deserializer.deserialize_str(V)
    }
}

// The keys of an object as written, borrowed where they need no unescaping; an object with a
// repeated key is refused.
struct Fields<'a>(Vec<(Cow<'a, str>, &'a RawValue)>);

impl<'de: 'a, 'a> Deserialize<'de> for Fields<'a> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Fields<'de>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a JSON object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut out: Vec<(Cow<'de, str>, &'de RawValue)> = Vec::new();
                let mut seen: Option<HashSet<String>> = None;
                while let Some((Key(key), value)) = map.next_entry::<Key<'de>, &'de RawValue>()? {
                    let repeated = match &mut seen {
                        Some(seen) => !seen.insert(key.as_ref().to_owned()),
                        None => out.iter().any(|(k, _)| *k == key),
                    };
                    if repeated {
                        return Err(A::Error::custom(format!("duplicate key `{key}`")));
                    }
                    out.push((key, value));
                    if seen.is_none() && out.len() > LINEAR_KEYS {
                        seen = Some(out.iter().map(|(k, _)| k.as_ref().to_owned()).collect());
                    }
                }
                Ok(Fields(out))
            }
        }
        deserializer.deserialize_map(V)
    }
}

// The value of `key` in an object, `None` where `entries_of` gives none or lacks the key, found
// without collecting the other keys as strings.
pub(crate) fn field_of<'a>(raw: &'a RawValue, key: &str) -> Option<&'a RawValue> {
    if !raw.get().starts_with('{') {
        return None;
    }
    let Fields(fields) = serde_json::from_str(raw.get()).ok()?;
    fields.into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

pub(crate) fn entries_of(raw: &RawValue) -> Option<Vec<(Cow<'_, str>, &RawValue)>> {
    if !raw.get().starts_with('{') {
        return None;
    }
    let Fields(fields) = serde_json::from_str(raw.get()).ok()?;
    Some(fields)
}

impl<T: DeserializeOwned + Serialize> Ext<T> {
    pub(crate) fn from_raw(raw: &RawValue) -> Result<Self, serde_json::Error> {
        Ext::from_text(raw.get())
    }

    pub(crate) fn from_text(text: &str) -> Result<Self, serde_json::Error> {
        match one_pass::read::<T>(text) {
            Some((known, extra)) => Ok(Ext {
                known,
                extra,
                verbatim: IndexMap::new(),
            }),
            None => Ext::two_passes(text),
        }
    }

    fn two_passes(text: &str) -> Result<Self, serde_json::Error> {
        let declared = struct_fields::<T>();
        let Fields(entries) = serde_json::from_str(text)?;
        let known: T = serde_json::from_str(text)?;
        let written = written_names(&known)?;
        let mut extra = IndexMap::new();
        let mut verbatim = IndexMap::new();
        for (key, value) in entries {
            if !declared.contains(&key.as_ref()) {
                if !carriable(&key) {
                    return Err(serde::de::Error::custom(format!(
                        "the field `{}` has a name Kaeso cannot carry; remove it",
                        Short(&key, 64)
                    )));
                }
                extra.insert(key.into_owned(), RawJson::from(value));
            } else if !written.contains(&key.as_ref()) {
                verbatim.insert(key.into_owned(), RawJson::from(value));
            }
        }
        Ok(Ext {
            known,
            extra,
            verbatim,
        })
    }

    pub(crate) fn from_slice(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        // A body these parses read whole is valid JSON, so it skips the pass that only checks
        // that; any other gets the errors that pass gives.
        if let Some(ext) = std::str::from_utf8(bytes)
            .ok()
            .and_then(|text| Ext::from_text(text).ok())
        {
            return Ok(ext);
        }
        let raw: &RawValue = serde_json::from_slice(bytes)?;
        Ext::from_raw(raw)
    }
}

// Reads an object into `T` and its undeclared entries in one pass over it. Anything that pass
// can't settle the way `two_passes` would, such as a repeated key, a declared value `T` doesn't
// write back, a name that can't be carried or any error, gives `None`, and `two_passes` decides.
mod one_pass {
    use std::borrow::Cow;

    use indexmap::IndexMap;
    use kaeso_kpp::RawJson;
    use serde::de::value::CowStrDeserializer;
    use serde::de::{DeserializeOwned, DeserializeSeed, MapAccess, Visitor};
    use serde::{Deserializer, Serialize};
    use serde_json::value::RawValue;

    use super::{Key, carriable, struct_fields, written_names};

    type Seen<'de> = Vec<(Cow<'de, str>, Option<&'de RawValue>)>;

    pub(super) fn read<T: DeserializeOwned + Serialize>(
        text: &str,
    ) -> Option<(T, IndexMap<String, RawJson>)> {
        let declared = struct_fields::<T>();
        let mut seen = Seen::new();
        let mut de = serde_json::Deserializer::from_str(text);
        let known = T::deserialize(Spy {
            inner: &mut de,
            declared,
            seen: &mut seen,
        })
        .ok()?;
        de.end().ok()?;
        let written = written_names(&known).ok()?;
        let mut extra = IndexMap::new();
        for (key, raw) in seen {
            if declared.contains(&key.as_ref()) {
                if !written.contains(&key.as_ref()) {
                    return None;
                }
            } else if !carriable(&key) {
                return None;
            } else {
                extra.insert(key.into_owned(), RawJson::from(raw?));
            }
        }
        Some((known, extra))
    }

    #[derive(Debug)]
    struct Unsettled;

    impl std::fmt::Display for Unsettled {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("unsettled")
        }
    }

    impl std::error::Error for Unsettled {}

    impl serde::de::Error for Unsettled {
        fn custom<M: std::fmt::Display>(_: M) -> Self {
            Unsettled
        }
    }

    fn unsettled<E>(_: E) -> Unsettled {
        Unsettled
    }

    struct Spy<'a, 's, 'de> {
        inner: &'a mut serde_json::Deserializer<serde_json::de::StrRead<'de>>,
        declared: &'static [&'static str],
        seen: &'s mut Seen<'de>,
    }

    impl<'de> Deserializer<'de> for Spy<'_, '_, 'de> {
        type Error = Unsettled;

        fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Unsettled> {
            Err(Unsettled)
        }

        fn deserialize_struct<V: Visitor<'de>>(
            self,
            _: &'static str,
            _: &'static [&'static str],
            visitor: V,
        ) -> Result<V::Value, Unsettled> {
            self.deserialize_map(visitor)
        }

        fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Unsettled> {
            let Spy {
                inner,
                declared,
                seen,
            } = self;
            inner
                .deserialize_map(Wrap {
                    visitor,
                    declared,
                    seen,
                })
                .map_err(unsettled)
        }

        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes
            byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct enum
            identifier ignored_any
        }
    }

    struct Wrap<'s, 'de, V> {
        visitor: V,
        declared: &'static [&'static str],
        seen: &'s mut Seen<'de>,
    }

    impl<'de, V: Visitor<'de>> Visitor<'de> for Wrap<'_, 'de, V> {
        type Value = V::Value;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a JSON object")
        }

        fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
            self.visitor
                .visit_map(Entries {
                    map,
                    declared: self.declared,
                    seen: self.seen,
                })
                .map_err(serde::de::Error::custom)
        }
    }

    struct Entries<'s, 'de, A> {
        map: A,
        declared: &'static [&'static str],
        seen: &'s mut Seen<'de>,
    }

    impl<'de, A: MapAccess<'de>> MapAccess<'de> for Entries<'_, 'de, A> {
        type Error = Unsettled;

        fn next_key_seed<K: DeserializeSeed<'de>>(
            &mut self,
            seed: K,
        ) -> Result<Option<K::Value>, Unsettled> {
            let Some(Key(key)) = self.map.next_key::<Key<'de>>().map_err(unsettled)? else {
                return Ok(None);
            };
            if self.seen.iter().any(|(k, _)| *k == key) {
                return Err(Unsettled);
            }
            self.seen.push((key.clone(), None));
            seed.deserialize(CowStrDeserializer::<Unsettled>::new(key))
                .map(Some)
        }

        fn next_value_seed<S: DeserializeSeed<'de>>(
            &mut self,
            seed: S,
        ) -> Result<S::Value, Unsettled> {
            let Some((key, raw)) = self.seen.last_mut() else {
                return Err(Unsettled);
            };
            if self.declared.contains(&key.as_ref()) {
                return self.map.next_value_seed(seed).map_err(unsettled);
            }
            let value: &'de RawValue = self.map.next_value().map_err(unsettled)?;
            *raw = Some(value);
            seed.deserialize(value).map_err(unsettled)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[derive(Debug, Serialize, serde::Deserialize)]
        struct Block {
            #[serde(rename = "type")]
            ty: String,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            text: Option<String>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            raw: Option<Box<RawValue>>,
        }

        #[test]
        fn one_pass_reads_what_two_passes_read_or_leaves_it_to_them() {
            let (known, extra) =
                read::<Block>(r#"{"type":"text","z":{"a": 1},"text":"hi","raw":[1, 2],"y":null}"#)
                    .unwrap();
            assert_eq!(known.ty, "text");
            assert_eq!(known.raw.as_ref().unwrap().get(), "[1, 2]");
            assert_eq!(
                extra
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.get()))
                    .collect::<Vec<_>>(),
                [("z", r#"{"a": 1}"#), ("y", "null")]
            );
            for unsettled in [
                r#"{"type":"text","type":"text"}"#,
                r#"{"type":"text","z":1,"z":2}"#,
                r#"{"type":"text","text":null}"#,
                r#"{"type":"text","a.b":1}"#,
                r#"{"type":5}"#,
                r#"["text"]"#,
                r#"{"type":"text"} x"#,
            ] {
                assert!(read::<Block>(unsettled).is_none(), "{unsettled}");
            }
        }
    }
}

impl<'de, T: DeserializeOwned + Serialize> Deserialize<'de> for Ext<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = <Box<RawValue>>::deserialize(deserializer)?;
        Ext::from_raw(&raw).map_err(D::Error::custom)
    }
}

impl<T: Serialize> Serialize for Ext<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.extra.is_empty() && self.verbatim.is_empty() {
            return self.known.serialize(serializer);
        }
        let mut map = serializer.serialize_map(None)?;
        let mut written: HashSet<&str> = write_into(&self.known, &mut map)?.into_iter().collect();
        for (key, value) in self.verbatim.iter().chain(&self.extra) {
            if written.insert(key) {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}

fn carriable(key: &str) -> bool {
    !key.contains('.') && !key.starts_with(HEADER_PREFIX)
}

pub(crate) fn struct_fields<T: DeserializeOwned>() -> &'static [&'static str] {
    struct Grab<'a>(&'a mut Option<&'static [&'static str]>);

    // An error that carries no message: `T::deserialize` always fails here, once per decoded
    // object, and a message would be built and dropped each time.
    #[derive(Debug)]
    struct Stop;

    impl fmt::Display for Stop {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("stop")
        }
    }

    impl std::error::Error for Stop {}

    impl serde::de::Error for Stop {
        fn custom<M: fmt::Display>(_: M) -> Self {
            Stop
        }
    }

    impl<'de> Deserializer<'de> for Grab<'_> {
        type Error = Stop;

        fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Self::Error> {
            Err(Stop)
        }

        fn deserialize_struct<V: Visitor<'de>>(
            self,
            _name: &'static str,
            fields: &'static [&'static str],
            _visitor: V,
        ) -> Result<V::Value, Self::Error> {
            *self.0 = Some(fields);
            Err(Stop)
        }

        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
            bytes byte_buf option unit unit_struct newtype_struct seq tuple
            tuple_struct map enum identifier ignored_any
        }
    }

    let mut fields = None;
    let _ = T::deserialize(Grab(&mut fields));
    fields.unwrap_or(&[])
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Maybe<T> {
    #[default]
    Absent,
    Null,
    Value(T),
}

impl<T> Maybe<T> {
    pub(crate) fn is_absent(&self) -> bool {
        matches!(self, Maybe::Absent)
    }

    pub(crate) fn as_option(&self) -> Option<&T> {
        match self {
            Maybe::Value(v) => Some(v),
            Maybe::Absent | Maybe::Null => None,
        }
    }

    pub(crate) fn into_option(self) -> Option<T> {
        match self {
            Maybe::Value(v) => Some(v),
            Maybe::Absent | Maybe::Null => None,
        }
    }

    pub(crate) fn from_option(value: Option<T>, if_none: Maybe<T>) -> Self {
        match value {
            Some(v) => Maybe::Value(v),
            None => if_none,
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Maybe<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Option::<T>::deserialize(deserializer)? {
            Some(v) => Maybe::Value(v),
            None => Maybe::Null,
        })
    }
}

impl<T: Serialize> Serialize for Maybe<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Maybe::Value(v) => v.serialize(serializer),
            Maybe::Absent | Maybe::Null => serializer.serialize_none(),
        }
    }
}

pub(crate) const FORM_INTEGERS: &str = "integers";

pub(crate) fn note_integers(extras: &mut Extras, numbers: &[(&str, Option<&serde_json::Number>)]) {
    let keys: Vec<Value> = numbers
        .iter()
        .filter(|(_, n)| n.is_some_and(|n| !n.is_f64()))
        .map(|(key, _)| Value::from(*key))
        .collect();
    if !keys.is_empty() {
        extras
            .form
            .insert(FORM_INTEGERS.to_owned(), Value::Array(keys));
    }
}

pub(crate) fn spelled_number(
    value: Option<f64>,
    key: &str,
    extras: &Extras,
    dialect: DialectId,
) -> Option<serde_json::Number> {
    let value = value?;
    let integer = extras
        .form_value(dialect, FORM_INTEGERS)
        .and_then(Value::as_array)
        .is_some_and(|keys| keys.iter().any(|k| k == key));
    if integer
        && value.fract() == 0.0
        && let Ok(number) = value.to_string().parse::<serde_json::Number>()
    {
        return Some(number);
    }
    serde_json::Number::from_f64(value)
}

// The `type` of an object that writes it first and plainly, as every SDK does, read without
// parsing the rest; `None` for any other spelling, which `type_tag` then reads.
pub(crate) fn leading_type(raw: &RawValue) -> Option<&str> {
    let rest = raw.get().strip_prefix("{\"type\":\"")?;
    let ty = rest.get(..rest.find('"')?)?;
    (!ty.contains('\\')).then_some(ty)
}

pub(crate) fn type_tag(raw: &RawValue) -> Result<Option<String>, serde_json::Error> {
    #[derive(Deserialize)]
    struct Tag {
        #[serde(rename = "type", default)]
        ty: Option<String>,
    }
    if !raw.get().trim_start().starts_with('{') {
        return Ok(None);
    }
    Ok(serde_json::from_str::<Tag>(raw.get())?.ty)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum StringOr<T> {
    String(String),
    List(Vec<T>),
}

// Read in the one pass over the value, not captured and parsed again: message content, the
// bulk of a request, goes through here.
impl<'de, T: DeserializeOwned> Deserialize<'de> for StringOr<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V<T>(std::marker::PhantomData<T>);

        fn neither<E: serde::de::Error, X>() -> Result<X, E> {
            Err(E::custom("expected a string or an array"))
        }

        impl<'de, T: DeserializeOwned> Visitor<'de> for V<T> {
            type Value = StringOr<T>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a string or an array")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(StringOr::String(v.to_owned()))
            }

            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(StringOr::String(v))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut list = Vec::new();
                while let Some(item) = seq.next_element::<T>()? {
                    list.push(item);
                }
                Ok(StringOr::List(list))
            }

            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Self::Value, E> {
                neither()
            }

            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<Self::Value, E> {
                neither()
            }

            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<Self::Value, E> {
                neither()
            }

            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Self::Value, E> {
                neither()
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                neither()
            }

            fn visit_map<A: MapAccess<'de>>(self, _: A) -> Result<Self::Value, A::Error> {
                neither()
            }
        }

        deserializer.deserialize_any(V(std::marker::PhantomData))
    }
}

impl<T: Serialize> Serialize for StringOr<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            StringOr::String(s) => s.serialize(serializer),
            StringOr::List(l) => l.serialize(serializer),
        }
    }
}

pub(crate) const VERBATIM_FORM_KEY: &str = "verbatim";
pub(crate) const NO_ARGUMENTS: &str = "{}";

pub(crate) fn extras_from(
    origin: &Origin,
    extra: IndexMap<String, RawJson>,
    verbatim: IndexMap<String, RawJson>,
) -> Extras {
    let mut e = Extras::new(origin.clone());
    e.fields = extra;
    verbatim_into_form(&mut e, "", verbatim);
    e
}

pub(crate) fn add_nested_extra(
    extras: &mut Extras,
    prefix: &str,
    extra: IndexMap<String, RawJson>,
) {
    for (k, v) in extra {
        extras.fields.insert(format!("{prefix}.{k}"), v);
    }
}

pub(crate) fn verbatim_into_form(
    extras: &mut Extras,
    prefix: &str,
    verbatim: IndexMap<String, RawJson>,
) {
    if verbatim.is_empty() {
        return;
    }
    let object: serde_json::Map<String, Value> = verbatim
        .into_iter()
        .filter_map(|(k, v)| Some((k, v.to_value().ok()?)))
        .collect();
    extras
        .form
        .insert(verbatim_key(prefix), Value::Object(object));
}

fn verbatim_key(prefix: &str) -> String {
    if prefix.is_empty() {
        VERBATIM_FORM_KEY.to_owned()
    } else {
        format!("{VERBATIM_FORM_KEY}:{prefix}")
    }
}

pub(crate) fn verbatim_from_form(
    extras: &Extras,
    dialect: DialectId,
    prefix: &str,
) -> IndexMap<String, RawJson> {
    match extras.form_value(dialect, &verbatim_key(prefix)) {
        Some(Value::Object(map)) => map
            .iter()
            .filter_map(|(k, v)| Some((k.clone(), RawJson::from_serialize(v).ok()?)))
            .collect(),
        _ => IndexMap::new(),
    }
}

pub(crate) fn extra_for(extras: &Extras, dialect: DialectId) -> IndexMap<String, RawJson> {
    if !extras.is_from(dialect) {
        return IndexMap::new();
    }
    extras
        .body_fields()
        .filter(|(k, _)| !k.contains('.'))
        .map(|(k, v)| (k.to_owned(), v.clone()))
        .collect()
}

pub(crate) fn nested_extra_for(
    extras: &Extras,
    dialect: DialectId,
    prefix: &str,
) -> IndexMap<String, RawJson> {
    if !extras.is_from(dialect) {
        return IndexMap::new();
    }
    extras
        .nested_fields(prefix)
        .filter(|(k, _)| !k.contains('.'))
        .map(|(k, v)| (k.to_owned(), v.clone()))
        .collect()
}

pub(crate) fn parse_provider<T: DeserializeOwned + Serialize>(
    body: &[u8],
    dialect: DialectId,
) -> Result<Ext<T>, KppError> {
    Ext::from_slice(body).map_err(|e| {
        KppError::provider(format!(
            "The provider sent a response that is not a valid {} response: {}.",
            dialect.display_name(),
            without_position(&e)
        ))
    })
}

pub(crate) fn without_position(e: &impl fmt::Display) -> String {
    let text = e.to_string();
    match text.rfind(" at line ") {
        Some(i) => text.get(..i).unwrap_or(&text).to_owned(),
        None => text,
    }
}

pub(crate) fn to_body<T: Serialize>(value: &T) -> Result<Vec<u8>, KppError> {
    serde_json::to_vec(value).map_err(|e| {
        KppError::provider(format!(
            "Kaeso could not serialize a message ({e}). This is a bug in Kaeso; please report it."
        ))
    })
}

pub(crate) struct Short<'a>(pub &'a str, pub usize);

impl fmt::Display for Short<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.len() <= self.1 {
            return f.write_str(self.0);
        }
        let mut end = self.1;
        while !self.0.is_char_boundary(end) {
            end -= 1;
        }
        write!(f, "{}…", self.0.get(..end).unwrap_or_default())
    }
}

/// A wire field of base64 media: the text as an app or provider wrote it when decoding, or
/// inline bytes written as base64 straight into the output when encoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum B64 {
    Text(String),
    Bytes(bytes::Bytes),
}

impl Default for B64 {
    fn default() -> Self {
        B64::Text(String::new())
    }
}

impl B64 {
    /// The media these bytes are, decoded; text that isn't base64 is refused naming `param`.
    pub(crate) fn into_source(
        self,
        media_type: impl Into<String>,
        param: &str,
    ) -> Result<Source, KppError> {
        match self {
            B64::Text(t) => crate::media::from_base64(media_type, t, param),
            B64::Bytes(b) => Ok(Source::bytes(media_type, b)),
        }
    }
}

impl serde::Serialize for B64 {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            B64::Text(t) => s.serialize_str(t),
            B64::Bytes(b) => kaeso_kpp::Base64Text::new(b).serialize(s),
        }
    }
}

impl<'de> serde::Deserialize<'de> for B64 {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(B64::Text)
    }
}

pub(crate) fn is_data_url(url: &str) -> bool {
    url.get(..5)
        .is_some_and(|p| p.eq_ignore_ascii_case("data:"))
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DataUrl<'a> {
    pub(crate) media_type: &'a str,
    pub(crate) data: &'a str,
    pub(crate) header: Option<&'a str>,
}

pub(crate) fn parse_data_url(url: &str) -> Option<DataUrl<'_>> {
    if !is_data_url(url) {
        return None;
    }
    let (header, data) = url.split_once(',')?;
    let params = header.get(5..)?;
    let (rest, last) = params.rsplit_once(';')?;
    if !last.eq_ignore_ascii_case("base64") {
        return None;
    }
    let media_type = rest.split(';').next().unwrap_or_default();
    if media_type.is_empty() {
        return None;
    }
    let canonical = header.len() == "data:".len() + media_type.len() + ";base64".len()
        && header.starts_with("data:")
        && header.ends_with(";base64");
    Some(DataUrl {
        media_type,
        data,
        header: (!canonical).then_some(header),
    })
}

pub(crate) fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, b| {
        (hash ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

pub(crate) fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

pub(crate) fn rewrite_name(
    original: &str,
    valid: fn(&str) -> bool,
    max_len: usize,
    sent_to_original: &mut IndexMap<String, String>,
) -> String {
    if valid(original) {
        return original.to_owned();
    }
    let suffix = format!("_{:016x}", fnv1a(original.as_bytes()));
    let keep = max_len.saturating_sub(suffix.len());
    let mut sent: String = original
        .chars()
        .map(|c| if is_name_char(c) { c } else { '_' })
        .take(keep)
        .collect();
    sent.push_str(&suffix);
    let mut candidate = sent.clone();
    let mut n = 0u32;
    while sent_to_original
        .get(&candidate)
        .is_some_and(|o| o != original)
    {
        n = n.saturating_add(1);
        candidate = format!(
            "{}{n}",
            sent.get(..sent.len().saturating_sub(4)).unwrap_or(&sent)
        );
    }
    sent_to_original.insert(candidate.clone(), original.to_owned());
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    struct Block {
        #[serde(rename = "type")]
        ty: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input: Option<Box<RawValue>>,
        #[serde(default, skip_serializing_if = "Maybe::is_absent")]
        refusal: Maybe<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nested: Option<Ext<Inner>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        opt: Option<u32>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        flag: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        list: Vec<u32>,
    }

    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Inner {
        a: u32,
    }

    fn round_trip(text: &str) -> (Ext<Block>, String) {
        let parsed: Ext<Block> = serde_json::from_str(text).unwrap();
        let out = serde_json::to_string(&parsed).unwrap();
        let a: Value = serde_json::from_str(text).unwrap();
        let b: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(a, b, "{out}");
        (parsed, out)
    }

    #[test]
    fn field_lookup_agrees_with_the_entries_of_an_object() {
        let wide: String = (0..20).map(|i| format!("\"k{i}\":{i},")).collect();
        for text in [
            r#"{"a":1,"b\u0022":"x","c":{"d":2}}"#.to_owned(),
            r#"{"a":1,"a":2}"#.to_owned(),
            format!("{{{wide}\"k3\":0}}"),
            format!("{{{wide}\"last\":true}}"),
            r#"[{"a":1}]"#.to_owned(),
        ] {
            let raw: Box<RawValue> = serde_json::from_str(&text).unwrap();
            let entries = entries_of(&raw);
            for key in ["a", "b\"", "c", "k3", "last", "missing"] {
                let via_entries = entries
                    .as_ref()
                    .and_then(|e| e.iter().find(|(k, _)| k == key).map(|(_, v)| v.get()));
                assert_eq!(
                    field_of(&raw, key).map(RawValue::get),
                    via_entries,
                    "{text} {key}"
                );
            }
        }
        let repeated: Box<RawValue> = serde_json::from_str(&format!("{{{wide}\"k3\":0}}")).unwrap();
        assert!(
            entries_of(&repeated).is_none(),
            "a repeat past the linear check is found"
        );
    }

    #[test]
    fn struct_fields_lists_declared_names_after_renames() {
        assert_eq!(
            struct_fields::<Block>(),
            ["type", "input", "refusal", "nested", "opt", "flag", "list"]
        );
        assert_eq!(struct_fields::<Inner>(), ["a"]);
    }

    #[test]
    fn ext_keeps_unknown_keys_raw_bytes_and_nulls() {
        let text = r#"{"type":"tool_use","zeta":1,"input":{"b": 2, "a": 1},"refusal":null,"nested":{"a":3,"x":[true]},"alpha":123456789012345678901234567890}"#;
        let (parsed, out) = round_trip(text);
        assert_eq!(parsed.ty, "tool_use");
        assert_eq!(parsed.input.as_ref().unwrap().get(), r#"{"b": 2, "a": 1}"#);
        assert_eq!(parsed.refusal, Maybe::Null);
        assert_eq!(parsed.extra.keys().collect::<Vec<_>>(), ["zeta", "alpha"]);
        let nested = parsed.nested.as_ref().unwrap();
        assert_eq!(nested.a, 3);
        assert_eq!(nested.extra.get("x").unwrap().get(), "[true]");
        assert!(out.contains(r#"{"b": 2, "a": 1}"#), "raw bytes kept: {out}");
        assert!(out.contains("123456789012345678901234567890"), "{out}");
    }

    #[test]
    fn ext_keeps_spellings_the_struct_cannot_reproduce() {
        let (parsed, _) = round_trip(r#"{"type":"x","opt":null,"flag":false,"list":[]}"#);
        assert_eq!(
            parsed.verbatim.keys().collect::<Vec<_>>(),
            ["opt", "flag", "list"]
        );
        assert!(parsed.extra.is_empty());

        let mut changed = parsed.clone();
        changed.opt = Some(7);
        let out = serde_json::to_string(&changed).unwrap();
        assert!(out.contains(r#""opt":7"#), "typed value wins: {out}");
        assert_eq!(out.matches("opt").count(), 1);
    }

    #[test]
    fn verbatim_survives_a_trip_through_kpp_extras() {
        let parsed: Ext<Block> = serde_json::from_str(r#"{"type":"x","opt":null,"z":1}"#).unwrap();
        let origin = Origin::new(DialectId::OpenAiChat);
        let extras = extras_from(&origin, parsed.extra.clone(), parsed.verbatim.clone());
        assert_eq!(extra_for(&extras, DialectId::OpenAiChat), parsed.extra);
        assert_eq!(
            verbatim_from_form(&extras, DialectId::OpenAiChat, ""),
            parsed.verbatim
        );
        assert!(extra_for(&extras, DialectId::AnthropicMessages).is_empty());
        assert!(verbatim_from_form(&extras, DialectId::AnthropicMessages, "").is_empty());
    }

    #[test]
    fn nested_extras_use_key_paths() {
        let origin = Origin::new(DialectId::OpenAiChat);
        let mut extras = extras_from(&origin, IndexMap::new(), IndexMap::new());
        let mut inner = IndexMap::new();
        inner.insert("detail".to_owned(), RawJson::from_serialize("low").unwrap());
        add_nested_extra(&mut extras, "image_url", inner.clone());
        assert_eq!(
            nested_extra_for(&extras, DialectId::OpenAiChat, "image_url"),
            inner
        );
        assert!(extra_for(&extras, DialectId::OpenAiChat).is_empty());
    }

    #[test]
    fn duplicate_keys_and_non_objects_are_rejected() {
        assert!(serde_json::from_str::<Ext<Block>>(r#"{"type":"a","z":1,"z":2}"#).is_err());
        assert!(serde_json::from_str::<Ext<Block>>(r#"{"type":"a","type":"b"}"#).is_err());
        assert!(serde_json::from_str::<Ext<Block>>("[1]").is_err());
    }

    #[test]
    fn maybe_distinguishes_absent_from_null() {
        let absent: Ext<Block> = serde_json::from_str(r#"{"type":"x"}"#).unwrap();
        assert_eq!(absent.refusal, Maybe::Absent);
        assert_eq!(serde_json::to_string(&absent).unwrap(), r#"{"type":"x"}"#);
        let value: Ext<Block> = serde_json::from_str(r#"{"type":"x","refusal":"no"}"#).unwrap();
        assert_eq!(value.refusal.as_option().map(String::as_str), Some("no"));
    }

    #[test]
    fn string_or_list_keeps_the_spelling() {
        let s: StringOr<Value> = serde_json::from_str(r#""hi""#).unwrap();
        assert_eq!(s, StringOr::String("hi".into()));
        let l: StringOr<Value> = serde_json::from_str(r#"[{"type":"text"}]"#).unwrap();
        assert!(matches!(l, StringOr::List(ref v) if v.len() == 1));
        assert_eq!(serde_json::to_string(&l).unwrap(), r#"[{"type":"text"}]"#);
        assert!(serde_json::from_str::<StringOr<Value>>("3").is_err());
    }

    #[test]
    fn type_tag_reads_only_the_tag() {
        let raw: Box<RawValue> =
            serde_json::from_str(r#"{"text":"x","type":"future_block","z":{}}"#).unwrap();
        assert_eq!(type_tag(&raw).unwrap().as_deref(), Some("future_block"));
        let raw: Box<RawValue> = serde_json::from_str("[1]").unwrap();
        assert_eq!(type_tag(&raw).unwrap(), None);
    }

    #[test]
    fn parse_errors_have_no_parser_positions() {
        let e = parse_provider::<Inner>(br#"{"a":"x"}"#, DialectId::OpenAiChat).unwrap_err();
        assert!(!e.message.contains("line"), "{}", e.message);
        assert!(e.message.contains("invalid type"), "{}", e.message);
    }

    #[test]
    fn short_truncates_on_char_boundaries() {
        assert_eq!(Short("héllo", 2).to_string(), "h…");
        assert_eq!(Short("hi", 5).to_string(), "hi");
    }
}
