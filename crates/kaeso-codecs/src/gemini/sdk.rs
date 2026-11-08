//! The snake-case keys google-genai 2.32.0 writes in some objects of a request, which the API
//! reads as proto JSON reads every field's original name. Kaeso reads them by their camel-case
//! names and writes them back as the app spelled them when the request goes to Gemini.

use std::borrow::Cow;

use serde_json::{Map, Value};

/// The objects whose keys google-genai spells in snake case, by the key that holds them, and
/// those keys with their camel-case names.
const RENAMED: &[(&str, &[(&str, &str)])] = &[
    ("inlineData", &[("mime_type", "mimeType")]),
    (
        "fileData",
        &[("mime_type", "mimeType"), ("file_uri", "fileUri")],
    ),
    (
        "thinkingConfig",
        &[
            ("include_thoughts", "includeThoughts"),
            ("thinking_budget", "thinkingBudget"),
            ("thinking_level", "thinkingLevel"),
        ],
    ),
    (
        "functionDeclarations",
        &[
            ("parameters_json_schema", "parametersJsonSchema"),
            ("response_json_schema", "responseJsonSchema"),
        ],
    ),
];

/// The snake-case keys in the order of `RENAMED`, searched for before anything is parsed.
const NEEDLES: &[&str] = &[
    "\"mime_type\"",
    "\"file_uri\"",
    "\"include_thoughts\"",
    "\"thinking_budget\"",
    "\"thinking_level\"",
    "\"parameters_json_schema\"",
    "\"response_json_schema\"",
];

/// The body with every snake-case key of `RENAMED` under its camel-case name, and whether any
/// was renamed. A body without one, or that isn't JSON, comes back as it was.
pub(crate) fn to_camel(body: &[u8]) -> (Cow<'_, [u8]>, bool) {
    let Ok(text) = std::str::from_utf8(body) else {
        return (Cow::Borrowed(body), false);
    };
    if !NEEDLES.iter().any(|n| text.contains(n)) {
        return (Cow::Borrowed(body), false);
    }
    let Ok(mut value) = serde_json::from_str::<Value>(text) else {
        return (Cow::Borrowed(body), false);
    };
    let mut renamed = false;
    rename(&mut value, None, true, &mut renamed);
    match serde_json::to_vec(&value) {
        Ok(out) if renamed => (Cow::Owned(out), true),
        _ => (Cow::Borrowed(body), false),
    }
}

/// An encoded body with the keys of `RENAMED` in snake case again.
pub(crate) fn to_snake(body: Vec<u8>) -> Vec<u8> {
    let Ok(mut value) = serde_json::from_slice::<Value>(&body) else {
        return body;
    };
    let mut renamed = false;
    rename(&mut value, None, false, &mut renamed);
    if !renamed {
        return body;
    }
    serde_json::to_vec(&value).unwrap_or(body)
}

fn rename(value: &mut Value, holder: Option<&str>, camel: bool, renamed: &mut bool) {
    match value {
        Value::Object(map) => {
            if let Some((_, keys)) = holder.and_then(|h| RENAMED.iter().find(|(k, _)| *k == h)) {
                rename_keys(map, keys, camel, renamed);
            }
            for (k, v) in map.iter_mut() {
                rename(v, Some(k.as_str()), camel, renamed);
            }
        }
        Value::Array(items) => {
            for item in items {
                rename(item, holder, camel, renamed);
            }
        }
        _ => {}
    }
}

fn rename_keys(
    map: &mut Map<String, Value>,
    keys: &[(&str, &str)],
    camel: bool,
    renamed: &mut bool,
) {
    for (snake, camel_key) in keys {
        let (from, to) = if camel {
            (*snake, *camel_key)
        } else {
            (*camel_key, *snake)
        };
        if map.contains_key(to) {
            continue;
        }
        if let Some(v) = map.remove(from) {
            map.insert(to.to_owned(), v);
            *renamed = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_keys_become_camel_case_and_back_only_where_google_genai_writes_them() {
        let sdk = serde_json::json!({
            "contents": [{"role": "user", "parts": [
                {"inlineData": {"mime_type": "image/png", "data": "AA=="}},
                {"fileData": {"file_uri": "https://example.com/a.pdf", "mime_type": "application/pdf"}},
                {"text": "mime_type"}
            ]}],
            "tools": [{"functionDeclarations": [{"name": "f", "parameters_json_schema": {"type": "object", "properties": {"mime_type": {"type": "string"}}}}]}],
            "generationConfig": {"thinkingConfig": {"thinking_level": "LOW", "include_thoughts": true}}
        });
        let body = serde_json::to_vec(&sdk).unwrap();
        let (camel, renamed) = to_camel(&body);
        assert!(renamed);
        let camel: Value = serde_json::from_slice(&camel).unwrap();
        assert_eq!(
            camel["contents"][0]["parts"][0]["inlineData"]["mimeType"],
            "image/png"
        );
        assert_eq!(
            camel["contents"][0]["parts"][1]["fileData"]["fileUri"],
            "https://example.com/a.pdf"
        );
        assert_eq!(camel["contents"][0]["parts"][2]["text"], "mime_type");
        let schema = &camel["tools"][0]["functionDeclarations"][0]["parametersJsonSchema"];
        assert!(schema["properties"]["mime_type"].is_object(), "{camel}");
        assert_eq!(
            camel["generationConfig"]["thinkingConfig"],
            serde_json::json!({"thinkingLevel": "LOW", "includeThoughts": true})
        );
        let back: Value =
            serde_json::from_slice(&to_snake(serde_json::to_vec(&camel).unwrap())).unwrap();
        assert_eq!(back, sdk);

        let plain = br#"{"contents": [{"parts": [{"text": "hi"}]}]}"#;
        assert!(matches!(to_camel(plain), (Cow::Borrowed(_), false)));
        let both = br#"{"generationConfig": {"thinkingConfig": {"thinkingLevel": "LOW", "thinking_level": "HIGH"}}}"#;
        let (out, renamed) = to_camel(both);
        assert!(!renamed);
        assert_eq!(&*out, &both[..]);
    }
}
