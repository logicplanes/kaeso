use std::collections::HashSet;
use std::fmt;

use indexmap::IndexMap;
use kaeso_kpp::{DialectId, Extras, HEADER_PREFIX, KppError, Origin, RawJson};
use serde::de::{DeserializeOwned, Error as _, MapAccess, Visitor};
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

struct Entries<'a>(Vec<(String, &'a RawValue)>);

impl<'de: 'a, 'a> Deserialize<'de> for Entries<'a> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Entries<'de>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a JSON object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut seen = HashSet::new();
                let mut out = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, &'de RawValue>()? {
                    if !seen.insert(key.clone()) {
                        return Err(A::Error::custom(format!("duplicate key `{key}`")));
                    }
                    out.push((key, value));
                }
                Ok(Entries(out))
            }
        }
        deserializer.deserialize_map(V)
    }
}

pub(crate) fn entries_of(raw: &RawValue) -> Option<Vec<(String, &RawValue)>> {
    if !raw.get().starts_with('{') {
        return None;
    }
    serde_json::from_str::<Entries<'_>>(raw.get())
        .ok()
        .map(|e| e.0)
}

impl<T: DeserializeOwned + Serialize> Ext<T> {
    pub(crate) fn from_raw(raw: &RawValue) -> Result<Self, serde_json::Error> {
        let declared = struct_fields::<T>();
        let Entries(entries) = serde_json::from_str(raw.get())?;
        let known: T = serde_json::from_str(raw.get())?;
        let written = written_names(&known)?;
        let mut extra = IndexMap::new();
        let mut verbatim = IndexMap::new();
        for (key, value) in entries {
            if !declared.contains(&key.as_str()) {
                if !carriable(&key) {
                    return Err(serde::de::Error::custom(format!(
                        "the field `{}` has a name Kaeso cannot carry; remove it",
                        Short(&key, 64)
                    )));
                }
                extra.insert(key, RawJson::from(value));
            } else if !written.contains(&key.as_str()) {
                verbatim.insert(key, RawJson::from(value));
            }
        }
        Ok(Ext {
            known,
            extra,
            verbatim,
        })
    }

    pub(crate) fn from_slice(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        let raw: &RawValue = serde_json::from_slice(bytes)?;
        Ext::from_raw(raw)
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

    impl<'de> Deserializer<'de> for Grab<'_> {
        type Error = serde::de::value::Error;

        fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Self::Error> {
            Err(serde::de::Error::custom("not a struct"))
        }

        fn deserialize_struct<V: Visitor<'de>>(
            self,
            _name: &'static str,
            fields: &'static [&'static str],
            _visitor: V,
        ) -> Result<V::Value, Self::Error> {
            *self.0 = Some(fields);
            Err(serde::de::Error::custom("fields recorded"))
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

impl<'de, T: DeserializeOwned> Deserialize<'de> for StringOr<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = <Box<RawValue>>::deserialize(deserializer)?;
        let text = raw.get().trim_start();
        if text.starts_with('"') {
            serde_json::from_str(raw.get())
                .map(StringOr::String)
                .map_err(D::Error::custom)
        } else if text.starts_with('[') {
            serde_json::from_str(raw.get())
                .map(StringOr::List)
                .map_err(D::Error::custom)
        } else {
            Err(D::Error::custom("expected a string or an array"))
        }
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
