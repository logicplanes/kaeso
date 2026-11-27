//! What a Gemini app's function declarations hold besides what a function tool of another
//! format can carry.

use indexmap::IndexMap;
use kaeso_kpp::{DialectId, GenerateRequest, ToolDef};
use serde_json::Value;

use crate::adapt::Draft;

const D: DialectId = DialectId::Gemini;
const FORM_GROUP: &str = "group";
/// The keys of a `FunctionDeclaration` that describe what the app's function returns
/// (`response` as a `Schema`, `responseJsonSchema` as JSON Schema).
const RETURN_SCHEMAS: [&str; 2] = ["response", "responseJsonSchema"];

/// Where each of a Gemini app's function tools sits in its request,
/// `tools[<tool>].functionDeclarations[<declaration>]`, by its index in `r.tools`; `None` for
/// a tool from another format or one that isn't a function.
pub(crate) fn wire_paths(r: &GenerateRequest) -> Vec<Option<String>> {
    if !r.extras.is_from(D) {
        return Vec::new();
    }
    let mut seen: IndexMap<u64, usize> = IndexMap::new();
    r.tools
        .iter()
        .map(|t| {
            let ToolDef::Function(f) = t else {
                return None;
            };
            let group = f.extras.form_value(D, FORM_GROUP).and_then(Value::as_u64)?;
            let n = seen.entry(group).or_insert(0);
            let path = format!("tools[{group}].functionDeclarations[{n}]");
            *n += 1;
            Some(path)
        })
        .collect()
}

/// Leaves out what a Gemini app's function declarations say about the value its function
/// returns, for a request that goes to another format: the app reads that value, not the
/// model, which calls the function without it. Each one left out is named by its path.
pub(crate) fn for_foreign(d: &mut Draft<'_>) {
    let r = d.get();
    if !r.extras.is_from(D) {
        return;
    }
    let paths = wire_paths(r);
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
        d.leave_out(format!("{at}.{key}"));
    }
}
