use kaeso_kpp::{DialectId, KppError, RawJson, ToolDef};
use serde_json::{Map, Value};

use crate::adapt::Draft;
use crate::openai_chat::ChatProfile;
use crate::openai_responses::namespaces;

/// How a provider reads the `pattern` keywords in a function tool's parameter schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ToolPatterns {
    /// As the app wrote them.
    AsSent,
    /// The provider compiles every pattern and refuses ECMAScript escapes its engine lacks:
    /// `\0` becomes `\x00` and a `\b` in a character class `\x08`; a pattern with a named
    /// backreference (`\k<name>`) or an empty class (`[]`, `[^]`) is left out of the schema.
    Compiled,
}

const PATTERN: &str = "pattern";
const PATTERN_PROPERTIES: &str = "patternProperties";
const SCHEMA_MAPS: [&str; 5] = [
    "properties",
    PATTERN_PROPERTIES,
    "$defs",
    "definitions",
    "dependentSchemas",
];
const DATA: [&str; 4] = ["enum", "const", "default", "examples"];

enum Pattern {
    AsSent,
    Rewritten(String),
    LeftOut,
}

fn compiled(pattern: &str) -> Pattern {
    let mut out = String::with_capacity(pattern.len() + 4);
    let mut changed = false;
    let mut in_class = false;
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let Some(escaped) = chars.next() else {
                    out.push('\\');
                    break;
                };
                match escaped {
                    '0' if !chars.peek().is_some_and(char::is_ascii_digit) => {
                        out.push_str("\\x00");
                        changed = true;
                    }
                    'b' if in_class => {
                        out.push_str("\\x08");
                        changed = true;
                    }
                    'k' if chars.peek() == Some(&'<') => return Pattern::LeftOut,
                    other => {
                        out.push('\\');
                        out.push(other);
                    }
                }
            }
            '[' if !in_class => {
                in_class = true;
                out.push('[');
                if chars.peek() == Some(&'^') {
                    out.push('^');
                    chars.next();
                }
                // ECMAScript closes a class at the first `]`, so `[]` and `[^]` are classes
                // without members; a Rust-syntax engine reads that `]` as a member.
                if chars.peek() == Some(&']') {
                    return Pattern::LeftOut;
                }
            }
            ']' if in_class => {
                in_class = false;
                out.push(']');
            }
            other => out.push(other),
        }
    }
    if changed {
        Pattern::Rewritten(out)
    } else {
        Pattern::AsSent
    }
}

fn compile_schema(schema: &mut Value) -> bool {
    match schema {
        Value::Array(items) => items
            .iter_mut()
            .fold(false, |any, s| compile_schema(s) | any),
        Value::Object(object) => compile_object(object),
        _ => false,
    }
}

fn compile_map(map: &mut Value, patterns_as_keys: bool) -> bool {
    let Value::Object(entries) = map else {
        return false;
    };
    let mut changed = false;
    if patterns_as_keys {
        let keys: Vec<String> = entries.keys().cloned().collect();
        for key in keys {
            match compiled(&key) {
                Pattern::AsSent => {}
                Pattern::Rewritten(new) => {
                    if let Some(value) = entries.remove(&key) {
                        entries.insert(new, value);
                    }
                    changed = true;
                }
                Pattern::LeftOut => {
                    entries.remove(&key);
                    changed = true;
                }
            }
        }
    }
    for value in entries.values_mut() {
        changed |= compile_schema(value);
    }
    changed
}

fn compile_object(object: &mut Map<String, Value>) -> bool {
    let mut changed = false;
    let mut left_out = false;
    for (key, value) in object.iter_mut() {
        if DATA.contains(&key.as_str()) {
            continue;
        }
        if SCHEMA_MAPS.contains(&key.as_str()) {
            changed |= compile_map(value, key == PATTERN_PROPERTIES);
            continue;
        }
        if key == PATTERN {
            if let Value::String(pattern) = value {
                match compiled(pattern) {
                    Pattern::AsSent => {}
                    Pattern::Rewritten(new) => {
                        *pattern = new;
                        changed = true;
                    }
                    Pattern::LeftOut => left_out = true,
                }
            }
            continue;
        }
        changed |= compile_schema(value);
    }
    if left_out {
        object.remove(PATTERN);
        changed = true;
    }
    changed
}

pub(crate) fn for_chat(
    d: &mut Draft<'_>,
    target: DialectId,
    profile: &ChatProfile,
) -> Result<(), KppError> {
    if target != DialectId::OpenAiChat || profile.request.tool_patterns == ToolPatterns::AsSent {
        return Ok(());
    }
    let mut compiled = Vec::new();
    for (i, tool) in d.get().tools.iter().enumerate() {
        let ToolDef::Function(f) = tool else {
            continue;
        };
        let Some(raw) = &f.parameters else {
            continue;
        };
        if !raw.get().contains(PATTERN) {
            continue;
        }
        let Ok(mut schema) = raw.to_value() else {
            continue;
        };
        if !compile_schema(&mut schema) {
            continue;
        }
        let path = namespaces::app_path(d.original(), &f.name).unwrap_or(format!("tools[{i}]"));
        let schema = RawJson::from_serialize(&schema)
            .map_err(|e| KppError::invalid_request(e.to_string()).with_param(path.clone()))?;
        compiled.push((i, path, schema));
    }
    if compiled.is_empty() {
        return Ok(());
    }
    let r = d.edit();
    let mut paths = Vec::with_capacity(compiled.len());
    for (i, path, schema) in compiled {
        if let Some(ToolDef::Function(f)) = r.tools.get_mut(i) {
            f.parameters = Some(schema);
            paths.push(path);
        }
    }
    for path in paths {
        d.adapt(path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rewritten(pattern: &str) -> Option<String> {
        match compiled(pattern) {
            Pattern::AsSent => Some(pattern.to_owned()),
            Pattern::Rewritten(new) => Some(new),
            Pattern::LeftOut => None,
        }
    }

    #[test]
    fn nul_and_backspace_escapes_become_hex_escapes() {
        assert_eq!(rewritten(r"^[^\0]*$").as_deref(), Some(r"^[^\x00]*$"));
        assert_eq!(rewritten(r"\0").as_deref(), Some(r"\x00"));
        assert_eq!(rewritten(r"^[\b\0]+$").as_deref(), Some(r"^[\x08\x00]+$"));
        assert_eq!(rewritten(r"^[^\]\b]$").as_deref(), Some(r"^[^\]\x08]$"));
    }

    #[test]
    fn escapes_the_engine_takes_stay_as_sent() {
        for pattern in [
            r"^\d+$",
            r"^a\b$",
            r"^[\x00-\x1f]$",
            r"^(a)\1$",
            r"^\07$",
            r"^(?=.*a).*$",
            r"^\\0$",
            r"^[a-z]$",
            r"trailing\",
            r"^[\\]b$",
        ] {
            assert_eq!(rewritten(pattern).as_deref(), Some(pattern), "{pattern}");
        }
    }

    #[test]
    fn named_backreferences_and_empty_classes_are_left_out() {
        for pattern in [r"^(?<x>a)\k<x>$", "^[]$", "^[^]$", r"a[]"] {
            assert!(rewritten(pattern).is_none(), "{pattern}");
        }
    }

    #[test]
    fn only_schema_keywords_are_rewritten() {
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "pattern": r"\0"},
                "items": {"type": "array", "items": {"pattern": r"\0"}},
                "choice": {"anyOf": [{"pattern": r"\0"}, {"enum": [r"\0"]}]},
                "data": {"const": {"pattern": r"\0"}, "default": r"\0", "examples": [r"\0"]},
                "named": {"$ref": "#/$defs/x"}
            },
            "patternProperties": {r"^\0": {"pattern": r"\k<x>"}},
            "$defs": {"x": {"pattern": r"[\b]"}}
        });
        assert!(compile_schema(&mut schema));
        assert_eq!(
            schema,
            serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "pattern": r"\x00"},
                    "items": {"type": "array", "items": {"pattern": r"\x00"}},
                    "choice": {"anyOf": [{"pattern": r"\x00"}, {"enum": [r"\0"]}]},
                    "data": {"const": {"pattern": r"\0"}, "default": r"\0", "examples": [r"\0"]},
                    "named": {"$ref": "#/$defs/x"}
                },
                "patternProperties": {r"^\x00": {}},
                "$defs": {"x": {"pattern": r"[\x08]"}}
            })
        );
        let mut unchanged = serde_json::json!({"pattern": r"^\d+$", "properties": {"pattern": {}}});
        assert!(!compile_schema(&mut unchanged));
    }
}
