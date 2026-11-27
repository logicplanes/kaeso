use indexmap::IndexMap;
use kaeso_kpp::{DialectId, GenerateRequest, ToolDef};
use serde_json::Value;

use crate::adapt::Draft;

const D: DialectId = DialectId::Gemini;
const FORM_GROUP: &str = "group";
const RETURN_SCHEMAS: [&str; 2] = ["response", "responseJsonSchema"];

// KPP's tools are one flat list; an app wrote each function in a declaration of a tool.
pub(crate) fn wire_paths(r: &GenerateRequest) -> Vec<Option<String>> {
    if !r.extras.is_from(D) {
        return Vec::new();
    }
    let wrapper = if r.extras.form_str(D, super::request::FORM_COUNT) == Some("wrapped") {
        "generateContentRequest."
    } else {
        ""
    };
    let mut seen: IndexMap<u64, usize> = IndexMap::new();
    r.tools
        .iter()
        .map(|t| {
            let ToolDef::Function(f) = t else {
                return None;
            };
            let group = f.extras.form_value(D, FORM_GROUP).and_then(Value::as_u64)?;
            let n = seen.entry(group).or_insert(0);
            let path = format!("{wrapper}tools[{group}].functionDeclarations[{n}]");
            *n += 1;
            Some(path)
        })
        .collect()
}

// The app reads what its function returns, not the model, which calls the function without it.
pub(crate) fn for_foreign(d: &mut Draft<'_>) {
    let r = d.get();
    if !r.extras.is_from(D) {
        return;
    }
    let paths = wire_paths(r);
    // google-genai writes `response_json_schema`, which the decoder read as its camel-case name.
    let snake = r.extras.form_flag(D, super::request::FORM_SDK_NAMES);
    let found: Vec<(usize, &'static str)> = r
        .tools
        .iter()
        .enumerate()
        .flat_map(|(i, t)| {
            let keys: Vec<&'static str> = match t {
                ToolDef::Function(f) if f.extras.is_from(D) => RETURN_SCHEMAS
                    .into_iter()
                    .filter(|k| f.extras.fields.contains_key(*k))
                    .collect(),
                _ => Vec::new(),
            };
            keys.into_iter().map(move |k| (i, k))
        })
        .collect();
    if found.is_empty() {
        return;
    }
    let tools = &mut d.edit().tools;
    for (i, key) in &found {
        if let Some(ToolDef::Function(f)) = tools.get_mut(*i) {
            f.extras.fields.shift_remove(*key);
        }
    }
    for (i, key) in found {
        let at = paths
            .get(i)
            .cloned()
            .flatten()
            .unwrap_or_else(|| format!("tools[{i}]"));
        let key = match key {
            "responseJsonSchema" if snake => "response_json_schema",
            other => other,
        };
        d.leave_out(format!("{at}.{key}"));
    }
}
