use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::raw::write_string;

// The parts of a tool definition that change what a client and its model are told; `_meta` and
// anything else the server adds are left out, so they never hold a tool for review.
pub const DEFINITION_FIELDS: [&str; 6] = [
    "name",
    "title",
    "description",
    "inputSchema",
    "outputSchema",
    "annotations",
];

pub fn write(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push(b'{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_string(out, key);
                out.push(b':');
                if let Some(v) = map.get(key) {
                    write(v, out);
                }
            }
            out.push(b'}');
        }
        Value::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write(item, out);
            }
            out.push(b']');
        }
        Value::String(s) => write_string(out, s),
        other => out.extend_from_slice(other.to_string().as_bytes()),
    }
}

pub fn to_vec(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    write(value, &mut out);
    out
}

pub fn definition(tool: &Value) -> Value {
    let mut out = serde_json::Map::new();
    for field in DEFINITION_FIELDS {
        if let Some(v) = tool.get(field).filter(|v| !v.is_null()) {
            out.insert(field.to_owned(), v.clone());
        }
    }
    Value::Object(out)
}

pub fn definition_hash(tool: &Value) -> [u8; 32] {
    Sha256::digest(to_vec(&definition(tool))).into()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn keys_are_sorted_at_every_level_without_whitespace() {
        let v: Value = serde_json::from_str(
            r#"{ "b": [ {"y": 1, "x": "é"} ], "a": { "d": null, "c": true } }"#,
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(to_vec(&v)).unwrap(),
            r#"{"a":{"c":true,"d":null},"b":[{"x":"é","y":1}]}"#
        );
    }

    #[test]
    fn reordering_keys_is_not_a_change() {
        let a = json!({
            "name": "create_post",
            "description": "Posts to X.",
            "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } } }
        });
        let b: Value = serde_json::from_str(
            r#"{"inputSchema":{"properties":{"text":{"type":"string"}},"type":"object"},"description":"Posts to X.","name":"create_post"}"#,
        )
        .unwrap();
        assert_eq!(definition_hash(&a), definition_hash(&b));
    }

    #[test]
    fn every_part_of_the_definition_counts() {
        let base = json!({
            "name": "create_post",
            "title": "Create post",
            "description": "Posts to X.",
            "inputSchema": { "type": "object" },
            "outputSchema": { "type": "object" },
            "annotations": { "destructiveHint": true }
        });
        let h = definition_hash(&base);
        for (field, value) in [
            ("name", json!("create_posts")),
            ("title", json!("Post")),
            ("description", json!("Posts to X, and also reads your DMs.")),
            (
                "inputSchema",
                json!({ "type": "object", "properties": { "x": {} } }),
            ),
            ("outputSchema", json!({ "type": "array" })),
            ("annotations", json!({ "destructiveHint": false })),
        ] {
            let mut changed = base.clone();
            changed[field] = value;
            assert_ne!(definition_hash(&changed), h, "{field}");
        }
    }

    #[test]
    fn meta_and_unknown_fields_do_not_count() {
        let a = json!({ "name": "t", "inputSchema": { "type": "object" } });
        let mut b = a.clone();
        b["_meta"] = json!({ "io.modelcontextprotocol/ui": { "resourceUri": "ui://t" } });
        b["icons"] = json!([]);
        b["title"] = Value::Null;
        assert_eq!(definition_hash(&a), definition_hash(&b));
        assert_eq!(definition(&b), a);
    }
}
