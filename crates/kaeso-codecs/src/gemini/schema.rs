//! Gemini's `Schema`, the OpenAPI 3.0 subset of `functionDeclarations[].parameters` and
//! `generationConfig.responseSchema`, written as the JSON Schema other formats take. The
//! mapping follows the discovery document's `Schema` (revision `20261125`), OpenAPI 3.0.3 and
//! google-genai's `Schema.json_schema`.

use std::borrow::Cow;

use indexmap::IndexMap;
use kaeso_kpp::{DialectId, OutputSpec, RawJson, ToolDef};
use serde_json::value::RawValue;

use crate::adapt::Draft;

const D: DialectId = DialectId::Gemini;
const FORM_SCHEMA: &str = "schema";
// serde_json reads a body at most 128 levels deep, and each schema level is at least one.
const MAX_DEPTH: usize = 128;
const NULL_BRANCH: &str = r#"{"type":"null"}"#;

/// Writes the Gemini `Schema`s of a request from a Gemini app as JSON Schema, for a request
/// that goes to another format.
pub(crate) fn for_foreign(d: &mut Draft<'_>) {
    let r = d.get();
    if !r.extras.is_from(D) {
        return;
    }
    let tools: Vec<usize> = r
        .tools
        .iter()
        .enumerate()
        .filter_map(|(i, t)| match t {
            ToolDef::Function(f)
                if f.parameters.is_some()
                    && f.extras.form_str(D, FORM_SCHEMA) == Some("parameters") =>
            {
                Some(i)
            }
            _ => None,
        })
        .collect();
    let output = matches!(r.output, OutputSpec::JsonSchema(_))
        && r.extras.form_str(D, FORM_SCHEMA) == Some("responseSchema");
    if tools.is_empty() && !output {
        return;
    }
    let r = d.edit();
    for i in tools {
        if let Some(ToolDef::Function(f)) = r.tools.get_mut(i)
            && let Some(p) = f.parameters.as_mut()
        {
            *p = to_json_schema(p);
        }
    }
    if output && let OutputSpec::JsonSchema(spec) = &mut r.output {
        spec.schema = to_json_schema(&spec.schema);
    }
}

/// `schema` as JSON Schema: lower-case types, `nullable` as a `null` type, branch or enum
/// value, `propertyOrdering` as the order of `properties`, `example` as `examples`, int64
/// counts written as strings as numbers, and google-genai's snake-case keys in camel case.
/// What isn't a Gemini `Schema` key is kept as written, in its place.
pub(crate) fn to_json_schema(schema: &RawJson) -> RawJson {
    match node(schema.as_raw(), 0) {
        Cow::Borrowed(_) => schema.clone(),
        Cow::Owned(text) => RawJson::from_string(text).unwrap_or_else(|_| schema.clone()),
    }
}

fn camel(key: &str) -> &str {
    match key {
        "any_of" => "anyOf",
        "property_ordering" => "propertyOrdering",
        "additional_properties" => "additionalProperties",
        "min_items" => "minItems",
        "max_items" => "maxItems",
        "min_length" => "minLength",
        "max_length" => "maxLength",
        "min_properties" => "minProperties",
        "max_properties" => "maxProperties",
        other => other,
    }
}

enum Type {
    Unspecified,
    Named(&'static str),
    AsWritten,
}

fn json_type(value: &RawValue) -> Type {
    let Ok(name) = serde_json::from_str::<&str>(value.get()) else {
        return Type::AsWritten;
    };
    const NAMES: [&str; 7] = [
        "string", "number", "integer", "boolean", "array", "object", "null",
    ];
    if name.eq_ignore_ascii_case("TYPE_UNSPECIFIED") {
        return Type::Unspecified;
    }
    NAMES
        .into_iter()
        .find(|n| n.eq_ignore_ascii_case(name))
        .map_or(Type::AsWritten, Type::Named)
}

// The discovery document types `minItems` and the other counts as int64, which proto JSON
// writes as a string; JSON Schema wants a number.
fn count(value: &RawValue) -> Cow<'_, str> {
    match serde_json::from_str::<&str>(value.get()) {
        Ok(text) if text.parse::<u64>().is_ok() => Cow::Owned(text.to_owned()),
        _ => Cow::Borrowed(value.get()),
    }
}

fn node(raw: &RawValue, depth: usize) -> Cow<'_, str> {
    let text = raw.get();
    if depth >= MAX_DEPTH || !text.starts_with('{') {
        return Cow::Borrowed(text);
    }
    let Ok(map) = serde_json::from_str::<IndexMap<String, &RawValue>>(text) else {
        return Cow::Borrowed(text);
    };
    let mut out: IndexMap<String, Cow<'_, str>> = IndexMap::with_capacity(map.len());
    let mut nullable = false;
    let mut ordering: Option<Vec<String>> = None;
    let mut changed = false;
    for (wire, value) in map {
        let key = camel(&wire);
        changed |= key != wire;
        let written = match key {
            "type" => match json_type(value) {
                Type::Unspecified => {
                    changed = true;
                    continue;
                }
                Type::Named(name) => Cow::Owned(format!("\"{name}\"")),
                Type::AsWritten => Cow::Borrowed(value.get()),
            },
            "nullable" => {
                nullable = value.get() == "true";
                changed = true;
                continue;
            }
            "format" if value.get() == "\"enum\"" => {
                changed = true;
                continue;
            }
            "propertyOrdering" => match serde_json::from_str(value.get()) {
                Ok(names) => {
                    ordering = Some(names);
                    changed = true;
                    continue;
                }
                Err(_) => Cow::Borrowed(value.get()),
            },
            "properties" => properties(value, depth),
            "items" | "additionalProperties" => node(value, depth + 1),
            "anyOf" => branches(value, depth),
            "minItems" | "maxItems" | "minLength" | "maxLength" | "minProperties"
            | "maxProperties" => count(value),
            "example" => {
                out.insert(
                    "examples".to_owned(),
                    Cow::Owned(format!("[{}]", value.get())),
                );
                changed = true;
                continue;
            }
            _ => Cow::Borrowed(value.get()),
        };
        changed |= written != value.get();
        out.insert(key.to_owned(), written);
    }
    changed |= numeric_enum(&mut out);
    if nullable {
        allow_null(&mut out);
    }
    if let Some(names) = ordering {
        order_properties(&mut out, &names);
    }
    if !changed {
        return Cow::Borrowed(text);
    }
    Cow::Owned(write(&out))
}

fn properties(value: &RawValue, depth: usize) -> Cow<'_, str> {
    let Ok(map) = serde_json::from_str::<IndexMap<String, &RawValue>>(value.get()) else {
        return Cow::Borrowed(value.get());
    };
    let mut changed = false;
    let out: IndexMap<String, Cow<'_, str>> = map
        .into_iter()
        .map(|(name, schema)| {
            let written = node(schema, depth + 1);
            changed |= matches!(written, Cow::Owned(_));
            (name, written)
        })
        .collect();
    if changed {
        Cow::Owned(write(&out))
    } else {
        Cow::Borrowed(value.get())
    }
}

fn branches(value: &RawValue, depth: usize) -> Cow<'_, str> {
    let Ok(items) = serde_json::from_str::<Vec<&RawValue>>(value.get()) else {
        return Cow::Borrowed(value.get());
    };
    let written: Vec<Cow<'_, str>> = items.into_iter().map(|b| node(b, depth + 1)).collect();
    if written.iter().all(|w| matches!(w, Cow::Borrowed(_))) {
        return Cow::Borrowed(value.get());
    }
    Cow::Owned(format!("[{}]", written.join(",")))
}

// Gemini writes the values of an `INTEGER` or `NUMBER` enum as strings (the `Schema.enum`
// example `{type:INTEGER, format:enum, enum:["101", "201", "301"]}`); JSON Schema compares
// them with the instance, so they are written as numbers.
fn numeric_enum(out: &mut IndexMap<String, Cow<'_, str>>) -> bool {
    let numeric = matches!(
        out.get("type").map(|t| t.as_ref()),
        Some("\"integer\"" | "\"number\"")
    );
    let Some(values) = out.get("enum").filter(|_| numeric).map(|v| v.to_string()) else {
        return false;
    };
    let Ok(items) = serde_json::from_str::<Vec<&RawValue>>(&values) else {
        return false;
    };
    let numbers: Option<Vec<String>> = items
        .iter()
        .map(|v| match serde_json::from_str::<&str>(v.get()) {
            Ok(text) => serde_json::from_str::<serde_json::Number>(text)
                .ok()
                .map(|n| n.to_string()),
            Err(_) => Some(v.get().to_owned()),
        })
        .collect();
    let Some(numbers) = numbers else {
        return false;
    };
    let written = format!("[{}]", numbers.join(","));
    if written == values {
        return false;
    }
    out.insert("enum".to_owned(), Cow::Owned(written));
    true
}

// `nullable` lets the value be null. An enum lists null among its values and keeps no
// `type`: Anthropic refuses an enum whose values don't all have every type of a type list.
// Otherwise a type becomes a list with `null`, and a union without a type gets a null branch.
fn allow_null(out: &mut IndexMap<String, Cow<'_, str>>) {
    if let Some(values) = out.get("enum").map(|v| v.to_string()) {
        let Ok(parsed) = serde_json::from_str::<Vec<&RawValue>>(&values) else {
            return;
        };
        let mut items: Vec<&str> = parsed.iter().map(|v| v.get()).collect();
        if !items.contains(&"null") {
            items.push("null");
        }
        out.insert(
            "enum".to_owned(),
            Cow::Owned(format!("[{}]", items.join(","))),
        );
        out.shift_remove("type");
        return;
    }
    if let Some(t) = out.get("type").map(|t| t.to_string()) {
        if let Ok(name) = serde_json::from_str::<&str>(&t)
            && name != "null"
        {
            out.insert(
                "type".to_owned(),
                Cow::Owned(format!(
                    "[{},\"null\"]",
                    serde_json::to_string(name).unwrap_or_default()
                )),
            );
        }
        return;
    }
    if let Some(any) = out.get("anyOf").map(|a| a.to_string())
        && let Ok(items) = serde_json::from_str::<Vec<&RawValue>>(&any)
    {
        let mut written: Vec<&str> = items.iter().map(|v| v.get()).collect();
        written.push(NULL_BRANCH);
        out.insert(
            "anyOf".to_owned(),
            Cow::Owned(format!("[{}]", written.join(","))),
        );
    }
}

fn order_properties(out: &mut IndexMap<String, Cow<'_, str>>, names: &[String]) {
    let Some(props) = out.get("properties").map(|p| p.to_string()) else {
        return;
    };
    let Ok(mut map) = serde_json::from_str::<IndexMap<String, &RawValue>>(&props) else {
        return;
    };
    let mut ordered: IndexMap<String, Cow<'_, str>> = IndexMap::with_capacity(map.len());
    for name in names {
        if let Some((key, value)) = map.shift_remove_entry(name) {
            ordered.insert(key, Cow::Borrowed(value.get()));
        }
    }
    ordered.extend(map.into_iter().map(|(k, v)| (k, Cow::Borrowed(v.get()))));
    let text = write(&ordered);
    out.insert("properties".to_owned(), Cow::Owned(text));
}

fn write(map: &IndexMap<String, Cow<'_, str>>) -> String {
    let mut text = String::from("{");
    for (i, (key, value)) in map.iter().enumerate() {
        if i > 0 {
            text.push(',');
        }
        text.push_str(&serde_json::to_string(key).unwrap_or_else(|_| "\"\"".to_owned()));
        text.push(':');
        text.push_str(value);
    }
    text.push('}');
    text
}
