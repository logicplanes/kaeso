use serde_json::Value;

use crate::headers::{PARAM_PREFIX, encode};

pub const ANNOTATION: &str = "x-mcp-header";
const SAFE_INTEGER: i64 = (1 << 53) - 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParamHeader {
    pub name: String,
    pub path: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParamError {
    Name(String),
    Duplicate(String),
    NotPrimitive(String),
    Unreachable(String),
}

impl std::fmt::Display for ParamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParamError::Name(n) => write!(f, "`{n}` is not a header name"),
            ParamError::Duplicate(n) => write!(f, "`{n}` names two parameters"),
            ParamError::NotPrimitive(n) => write!(
                f,
                "`{n}` is on a parameter that isn't a string, integer or boolean"
            ),
            ParamError::Unreachable(n) => write!(f, "`{n}` sits where only `properties` may lead"),
        }
    }
}

fn token(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+.^_`|~-".contains(&b))
}

// Every annotation must be reachable through `properties` keys alone (no `items`, composition,
// conditionals or `$ref`); a tool with any other placement is invalid and clients drop it.
fn collect(value: &Value, path: &mut Vec<String>, found: &mut Vec<(Vec<String>, Value, Value)>) {
    match value {
        Value::Object(map) => {
            if let Some(name) = map.get(ANNOTATION) {
                found.push((path.clone(), name.clone(), value.clone()));
            }
            for (k, v) in map {
                path.push(k.clone());
                collect(v, path, found);
                path.pop();
            }
        }
        Value::Array(items) => {
            for (i, v) in items.iter().enumerate() {
                path.push(i.to_string());
                collect(v, path, found);
                path.pop();
            }
        }
        _ => {}
    }
}

pub fn param_headers(input_schema: &Value) -> Result<Vec<ParamHeader>, ParamError> {
    let mut found = Vec::new();
    collect(input_schema, &mut Vec::new(), &mut found);
    let mut out: Vec<ParamHeader> = Vec::new();
    for (path, name, schema) in found {
        let name = name.as_str().unwrap_or_default().to_owned();
        if !token(&name) {
            return Err(ParamError::Name(name));
        }
        let reachable = !path.is_empty()
            && path.len() % 2 == 0
            && path
                .chunks(2)
                .all(|pair| pair.first().map(String::as_str) == Some("properties"));
        if !reachable {
            return Err(ParamError::Unreachable(name));
        }
        let primitive = matches!(
            schema.get("type").and_then(Value::as_str),
            Some("string" | "integer" | "boolean")
        );
        if !primitive {
            return Err(ParamError::NotPrimitive(name));
        }
        if out.iter().any(|h| h.name.eq_ignore_ascii_case(&name)) {
            return Err(ParamError::Duplicate(name));
        }
        out.push(ParamHeader {
            name,
            path: path
                .chunks(2)
                .filter_map(|pair| pair.get(1).cloned())
                .collect(),
        });
    }
    Ok(out)
}

pub fn values(headers: &[ParamHeader], arguments: &Value) -> Vec<(String, String)> {
    headers
        .iter()
        .filter_map(|h| {
            let mut at = arguments;
            for key in &h.path {
                at = at.get(key)?;
            }
            let text = match at {
                Value::String(s) => s.clone(),
                Value::Bool(b) => b.to_string(),
                Value::Number(n) => {
                    let i = n.as_i64().filter(|i| i.abs() <= SAFE_INTEGER)?;
                    i.to_string()
                }
                _ => return None,
            };
            Some((
                format!("{PARAM_PREFIX}{}", h.name),
                encode(&text).into_owned(),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn annotated_parameters_become_headers() {
        let schema = json!({
            "type": "object",
            "properties": {
                "region": { "type": "string", "x-mcp-header": "Region" },
                "query": { "type": "string" },
                "options": {
                    "type": "object",
                    "properties": { "dry": { "type": "boolean", "x-mcp-header": "Dry-Run" } }
                },
                "limit": { "type": "integer", "x-mcp-header": "Limit" }
            }
        });
        let headers = param_headers(&schema).unwrap();
        assert_eq!(headers.len(), 3);
        let args =
            json!({ "region": "Köln", "options": { "dry": true }, "limit": 42, "query": "x" });
        let mut got = values(&headers, &args);
        got.sort();
        assert_eq!(
            got,
            [
                ("mcp-param-Dry-Run".to_owned(), "true".to_owned()),
                ("mcp-param-Limit".to_owned(), "42".to_owned()),
                (
                    "mcp-param-Region".to_owned(),
                    "=?base64?S8O2bG4=?=".to_owned()
                ),
            ]
        );
        assert!(values(&headers, &json!({ "region": null })).is_empty());
        assert!(values(&headers, &json!({ "limit": 1e300 })).is_empty());
    }

    #[test]
    fn misplaced_annotations_make_the_tool_invalid() {
        for (schema, expected) in [
            (
                json!({ "properties": { "a": { "type": "array", "items": { "type": "string", "x-mcp-header": "A" } } } }),
                ParamError::Unreachable("A".into()),
            ),
            (
                json!({ "properties": { "a": { "oneOf": [ { "type": "string", "x-mcp-header": "A" } ] } } }),
                ParamError::Unreachable("A".into()),
            ),
            (
                json!({ "properties": { "a": { "type": "number", "x-mcp-header": "A" } } }),
                ParamError::NotPrimitive("A".into()),
            ),
            (
                json!({ "properties": { "a": { "type": "string", "x-mcp-header": "" } } }),
                ParamError::Name(String::new()),
            ),
            (
                json!({ "properties": { "a": { "type": "string", "x-mcp-header": "a b" } } }),
                ParamError::Name("a b".into()),
            ),
            (
                json!({ "properties": {
                "a": { "type": "string", "x-mcp-header": "Region" },
                "b": { "type": "string", "x-mcp-header": "region" } } }),
                ParamError::Duplicate("region".into()),
            ),
            (
                json!({ "type": "string", "x-mcp-header": "Root" }),
                ParamError::Unreachable("Root".into()),
            ),
        ] {
            assert_eq!(param_headers(&schema).unwrap_err(), expected, "{schema}");
        }
        assert!(
            param_headers(&json!({ "type": "object" }))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            ParamError::Duplicate("x".into()).to_string(),
            "`x` names two parameters"
        );
    }
}
