//! Scans over JSON text that serde_json has already read whole, such as a body after
//! [`crate::limits::request_shape`] or a `RawValue`: they find where strings and values end,
//! eight bytes at a time through strings, and leave every check of the JSON to serde_json.

use std::borrow::Cow;
use std::collections::HashSet;

// Just past the quote that closes a string whose content starts at `from`, or the end of the
// text for one that never closes; every byte in between counts, an escaped quote included.
pub(crate) fn string_end(text: &[u8], mut from: usize) -> usize {
    loop {
        let rest = text.get(from..).unwrap_or_default();
        from += quote_or_backslash(rest);
        match text.get(from) {
            Some(b'"') => return from + 1,
            Some(_) => from += 2,
            None => return text.len(),
        }
        if from >= text.len() {
            return text.len();
        }
    }
}

// The index of the first `"` or `\` in `bytes`, or its length, eight bytes at a time: strings,
// most of a request's bytes, hold neither for long stretches.
fn quote_or_backslash(bytes: &[u8]) -> usize {
    const ONES: u64 = 0x0101_0101_0101_0101;
    const HIGH: u64 = 0x8080_8080_8080_8080;
    let quotes = ONES * u64::from(b'"');
    let backslashes = ONES * u64::from(b'\\');
    let (words, rest) = bytes.as_chunks::<8>();
    for (i, eight) in words.iter().enumerate() {
        let word = u64::from_le_bytes(*eight);
        let (q, b) = (word ^ quotes, word ^ backslashes);
        let found = ((q.wrapping_sub(ONES) & !q) | (b.wrapping_sub(ONES) & !b)) & HIGH;
        if found != 0 {
            return i * 8 + (found.trailing_zeros() / 8) as usize;
        }
    }
    words.len() * 8
        + rest
            .iter()
            .position(|&c| c == b'"' || c == b'\\')
            .unwrap_or(rest.len())
}

fn skip_whitespace(text: &[u8], mut at: usize) -> usize {
    while text.get(at).is_some_and(u8::is_ascii_whitespace) {
        at += 1;
    }
    at
}

// Just past the value that starts at `at`, in valid JSON.
fn value_end(text: &[u8], at: usize) -> usize {
    match text.get(at) {
        Some(b'"') => string_end(text, at + 1),
        Some(b'{' | b'[') => {
            let mut depth = 0usize;
            let mut i = at;
            while let Some(&byte) = text.get(i) {
                match byte {
                    b'"' => {
                        i = string_end(text, i + 1);
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            return i + 1;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            text.len()
        }
        _ => {
            let mut i = at;
            while text
                .get(i)
                .is_some_and(|&b| !matches!(b, b',' | b'}' | b']') && !b.is_ascii_whitespace())
            {
                i += 1;
            }
            i
        }
    }
}

/// The `type` member of the JSON object `json`, when a look at its keys settles it: `Some(None)`
/// when it has none or it is `null`, `Some(Some(tag))` when it is a string without escapes.
/// `None` leaves the answer to serde_json: a key or a tag with escapes, a repeated `type`, a tag
/// of another type, or text that is no object.
pub(crate) fn object_type(json: &str) -> Option<Option<&str>> {
    let text = json.as_bytes();
    let mut at = skip_whitespace(text, 0);
    if text.get(at) != Some(&b'{') {
        return None;
    }
    at += 1;
    let mut found: Option<Option<&str>> = None;
    loop {
        at = skip_whitespace(text, at);
        match text.get(at) {
            Some(b'}') => return Some(found.unwrap_or(None)),
            Some(b',') => {
                at += 1;
                continue;
            }
            Some(b'"') => {}
            _ => return None,
        }
        let key_end = string_end(text, at + 1);
        let key = text.get(at + 1..key_end.checked_sub(1)?)?;
        if key.contains(&b'\\') {
            return None;
        }
        at = skip_whitespace(text, key_end);
        if text.get(at) != Some(&b':') {
            return None;
        }
        let start = skip_whitespace(text, at + 1);
        let end = value_end(text, start);
        if key == b"type" {
            if found.is_some() {
                return None;
            }
            found = Some(match text.get(start..end)? {
                b"null" => None,
                [b'"', inner @ .., b'"'] if !inner.contains(&b'\\') => {
                    Some(json.get(start + 1..end - 1)?)
                }
                _ => return None,
            });
        }
        at = end;
    }
}

/// The members of the JSON object `json`, as serde_json reads them into
/// `(key, &RawValue)` pairs, without checking the values again: the key, borrowed unless it has
/// escapes, and the value's text. `None` for text that is no object or repeats a key.
pub(crate) fn members(json: &str) -> Option<Vec<(Cow<'_, str>, &str)>> {
    let text = json.as_bytes();
    let mut at = skip_whitespace(text, 0);
    if text.get(at) != Some(&b'{') {
        return None;
    }
    at += 1;
    let mut out: Vec<(Cow<'_, str>, &str)> = Vec::new();
    let mut seen: Option<HashSet<Cow<'_, str>>> = None;
    loop {
        at = skip_whitespace(text, at);
        match text.get(at) {
            Some(b'}') => return Some(out),
            Some(b',') => {
                at += 1;
                continue;
            }
            Some(b'"') => {}
            _ => return None,
        }
        let key_end = string_end(text, at + 1);
        let quoted = json.get(at..key_end)?;
        let key = match quoted.get(1..quoted.len().checked_sub(1)?) {
            Some(k) if !k.contains('\\') => Cow::Borrowed(k),
            _ => Cow::Owned(serde_json::from_str::<String>(quoted).ok()?),
        };
        at = skip_whitespace(text, key_end);
        if text.get(at) != Some(&b':') {
            return None;
        }
        let start = skip_whitespace(text, at + 1);
        let end = value_end(text, start);
        let repeated = match &mut seen {
            Some(seen) => !seen.insert(key.clone()),
            None => out.iter().any(|(k, _)| *k == key),
        };
        if repeated {
            return None;
        }
        out.push((key, json.get(start..end)?));
        if seen.is_none() && out.len() > 16 {
            seen = Some(out.iter().map(|(k, _)| k.clone()).collect());
        }
        at = end;
    }
}

/// The elements of the JSON array `json`, as serde_json reads them into `Vec<&RawValue>`,
/// without checking them again. `None` for text that is no array.
pub(crate) fn elements(json: &str) -> Option<Vec<&str>> {
    let text = json.as_bytes();
    let mut at = skip_whitespace(text, 0);
    if text.get(at) != Some(&b'[') {
        return None;
    }
    at += 1;
    let mut out = Vec::new();
    loop {
        at = skip_whitespace(text, at);
        match text.get(at) {
            Some(b']') => return Some(out),
            Some(b',') => {
                at += 1;
                continue;
            }
            Some(_) => {}
            None => return None,
        }
        let end = value_end(text, at);
        out.push(json.get(at..end)?);
        at = end;
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use serde::Deserialize;
    use serde_json::value::RawValue;

    use super::*;

    fn serde_type(raw: &RawValue) -> Result<Option<String>, String> {
        #[derive(Deserialize)]
        struct Tag {
            #[serde(rename = "type", default)]
            ty: Option<String>,
        }
        serde_json::from_str::<Tag>(raw.get())
            .map(|t| t.ty)
            .map_err(|e| e.to_string())
    }

    fn key() -> impl Strategy<Value = String> {
        prop_oneof![
            Just(r#""type""#.to_owned()),
            Just(r#""\u0074ype""#.to_owned()),
            Just(r#""typ""#.to_owned()),
            Just(r#""types""#.to_owned()),
            Just(r#""t\"ype""#.to_owned()),
            Just(r#""text""#.to_owned()),
            "[a-z]{0,6}".prop_map(|k| format!("\"{k}\"")),
        ]
    }

    fn scalar() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("null".to_owned()),
            Just("true".to_owned()),
            Just("-12.5e3".to_owned()),
            Just("0".to_owned()),
            Just(r#""a \"quoted\" \\ text""#.to_owned()),
            Just(r#""\u00e9t\u00e9""#.to_owned()),
            Just(r#""""#.to_owned()),
            Just(r#""}],{[""#.to_owned()),
            "[a-z_ ]{0,24}".prop_map(|s| format!("\"{s}\"")),
        ]
    }

    fn value() -> impl Strategy<Value = String> {
        scalar().prop_recursive(3, 24, 4, |inner| {
            prop_oneof![
                proptest::collection::vec(inner.clone(), 0..4)
                    .prop_map(|v| format!("[{}]", v.join(","))),
                proptest::collection::vec((key(), inner), 0..4).prop_map(|members| {
                    let members: Vec<String> = members
                        .into_iter()
                        .map(|(k, v)| format!("{k}:{v}"))
                        .collect();
                    format!("{{{}}}", members.join(","))
                }),
            ]
        })
    }

    fn space() -> impl Strategy<Value = &'static str> {
        prop_oneof![Just(""), Just(" "), Just("\n  "), Just("\t")]
    }

    fn object() -> impl Strategy<Value = String> {
        proptest::collection::vec((key(), value(), space(), space()), 0..6).prop_map(|members| {
            let members: Vec<String> = members
                .into_iter()
                .map(|(k, v, a, b)| format!("{a}{k}{b}:{a}{v}{b}"))
                .collect();
            format!("{{{}}}", members.join(","))
        })
    }

    fn serde_members(raw: &RawValue) -> Option<Vec<(String, String)>> {
        crate::wire::entries_of(raw).map(|e| {
            e.into_iter()
                .map(|(k, v)| (k.into_owned(), v.get().to_owned()))
                .collect()
        })
    }

    proptest! {
        #[test]
        fn members_split_an_object_as_serde_json_does(json in object()) {
            let Ok(raw) = serde_json::from_str::<Box<RawValue>>(&json) else {
                return Ok(());
            };
            let split = members(raw.get()).map(|m| {
                m.into_iter().map(|(k, v)| (k.into_owned(), v.to_owned())).collect::<Vec<_>>()
            });
            prop_assert_eq!(split, serde_members(&raw), "{}", json);
        }

        #[test]
        fn elements_split_an_array_as_serde_json_does(json in value()) {
            let Ok(raw) = serde_json::from_str::<Box<RawValue>>(&json) else {
                return Ok(());
            };
            let theirs = serde_json::from_str::<Vec<&RawValue>>(raw.get())
                .ok()
                .map(|v| v.into_iter().map(RawValue::get).collect::<Vec<_>>());
            prop_assert_eq!(elements(raw.get()), theirs, "{}", json);
        }

        #[test]
        fn object_type_answers_as_serde_json_does_whenever_it_answers(json in object()) {
            let Ok(raw) = serde_json::from_str::<Box<RawValue>>(&json) else {
                return Ok(());
            };
            if let Some(found) = object_type(raw.get()) {
                prop_assert_eq!(Ok(found.map(str::to_owned)), serde_type(&raw), "{}", json);
            }
        }
    }

    #[test]
    fn object_type_settles_the_usual_objects_itself() {
        for (json, ty) in [
            (r#"{"type":"text","text":"a \"b\" c"}"#, Some("text")),
            (r#"{"name":"x","input_schema":{"type":"object"}}"#, None),
            (
                r#"{ "a" : [1, {"type": "x"}], "type" : "tool_use" }"#,
                Some("tool_use"),
            ),
            (r#"{"type":null}"#, None),
            ("{}", None),
        ] {
            assert_eq!(object_type(json), Some(ty), "{json}");
        }
    }

    #[test]
    fn object_type_leaves_escapes_repeats_and_other_values_to_serde_json() {
        for json in [
            r#"{"type":"a","type":"b"}"#,
            r#"{"\u0074ype":"a"}"#,
            r#"{"type":"t\u0065xt"}"#,
            r#"{"type":5}"#,
            "[1]",
        ] {
            assert_eq!(object_type(json), None, "{json}");
        }
    }
}
