use kaeso_kpp::{Block, DialectId, KppError, OutputSpec, RawJson, ToolDef};
use serde_json::{Map, Value};

use crate::adapt::Draft;
use crate::cross::{self, Setting};
use crate::openai_responses::namespaces;

const D: DialectId = DialectId::AnthropicMessages;

// Claude's structured outputs and strict tools take a subset of JSON Schema
// (https://platform.claude.com/docs/en/build-with-claude/structured-outputs#json-schema-limitations).
// These keywords are refused with a 400 wherever they sit; they go into the schema's description
// instead, as Anthropic's SDKs do it, so the model still reads them.
const REFUSED: [&str; 18] = [
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "multipleOf",
    "maxItems",
    "uniqueItems",
    "prefixItems",
    "contains",
    "minContains",
    "maxContains",
    "minProperties",
    "maxProperties",
    "patternProperties",
    "propertyNames",
    "dependentRequired",
    "dependentSchemas",
    "not",
];
const STRING_FORMATS: [&str; 10] = [
    "date-time",
    "time",
    "date",
    "duration",
    "email",
    "hostname",
    "uri",
    "ipv4",
    "ipv6",
    "uuid",
];
const SCHEMA_MAPS: [&str; 3] = ["properties", "$defs", "definitions"];
const SCHEMA_LISTS: [&str; 3] = ["anyOf", "allOf", "oneOf"];
const SCHEMAS: [&str; 4] = ["items", "if", "then", "else"];
struct NoKeys;

struct Rewrite {
    changed: bool,
}

impl Rewrite {
    fn node(&mut self, node: &mut Map<String, Value>) {
        let mut moved = Map::new();
        for key in REFUSED {
            if let Some(value) = node.remove(key) {
                moved.insert(key.to_owned(), value);
            }
        }
        if node
            .get("minItems")
            .and_then(Value::as_u64)
            .is_some_and(|n| n > 1)
            && let Some(value) = node.remove("minItems")
        {
            moved.insert("minItems".to_owned(), value);
        }
        if node
            .get("format")
            .and_then(Value::as_str)
            .is_some_and(|f| !STRING_FORMATS.contains(&f))
            && let Some(value) = node.remove("format")
        {
            moved.insert("format".to_owned(), value);
        }
        if node.contains_key("oneOf") {
            match node.remove("oneOf") {
                Some(variants) if !node.contains_key("anyOf") => {
                    node.insert("anyOf".to_owned(), variants);
                }
                Some(variants) => {
                    moved.insert("oneOf".to_owned(), variants);
                }
                None => {}
            }
            self.changed = true;
        }
        if node.contains_key("properties") && !node.contains_key("type") {
            node.insert("type".to_owned(), Value::from("object"));
            self.changed = true;
        }
        if is_object(node) && node.get("additionalProperties") != Some(&Value::Bool(false)) {
            node.insert("additionalProperties".to_owned(), Value::Bool(false));
            self.changed = true;
        }
        if !moved.is_empty() {
            describe(node, &moved);
            self.changed = true;
        }
        for key in SCHEMA_MAPS {
            if let Some(Value::Object(schemas)) = node.get_mut(key) {
                for schema in schemas.values_mut() {
                    self.value(schema);
                }
            }
        }
        for key in SCHEMA_LISTS {
            if let Some(Value::Array(schemas)) = node.get_mut(key) {
                for schema in schemas {
                    self.value(schema);
                }
            }
        }
        for key in SCHEMAS {
            if let Some(schema) = node.get_mut(key) {
                self.value(schema);
            }
        }
    }

    fn value(&mut self, value: &mut Value) {
        if let Value::Object(node) = value {
            self.node(node);
        }
    }
}

fn is_object(node: &Map<String, Value>) -> bool {
    match node.get("type") {
        Some(Value::String(t)) => t == "object",
        Some(Value::Array(types)) => types.iter().any(|t| t == "object"),
        _ => false,
    }
}

fn lists_keys(node: &Map<String, Value>) -> bool {
    let composed = ["anyOf", "allOf", "oneOf", "$ref"]
        .iter()
        .any(|k| node.contains_key(*k));
    let listed = node
        .get("properties")
        .and_then(Value::as_object)
        .is_some_and(|p| !p.is_empty());
    composed || listed || !is_object(node)
}

fn describe(node: &mut Map<String, Value>, moved: &Map<String, Value>) {
    let rules: Vec<String> = moved.iter().map(|(k, v)| format!("{k}: {v}")).collect();
    let note = format!("{{{}}}", rules.join(", "));
    let text = match node.get("description").and_then(Value::as_str) {
        Some(own) if !own.is_empty() => format!("{own}\n\n{note}"),
        _ => note,
    };
    node.insert("description".to_owned(), Value::String(text));
}

fn rewritten(raw: &RawJson, output: bool) -> Result<Option<RawJson>, NoKeys> {
    let Ok(Value::Object(mut root)) = raw.to_value() else {
        return Ok(None);
    };
    if output && !lists_keys(&root) {
        return Err(NoKeys);
    }
    let mut rewrite = Rewrite { changed: false };
    rewrite.node(&mut root);
    if !rewrite.changed {
        return Ok(None);
    }
    Ok(RawJson::from_serialize(&root).ok())
}

fn refusal(param: &str) -> KppError {
    KppError::unsupported_by(
        "output schema without keys",
        D,
        "Claude's structured outputs return only the keys a schema lists, and this one lists none; list them in `properties`.",
    )
    .with_param(param)
}

// Schemas from other formats are rewritten into the subset Claude takes, as Anthropic's SDKs
// do: objects are closed, `oneOf` becomes `anyOf`, and constraints Claude can't enforce move
// into descriptions. Claude then enforces a little more than the app asked, which never gives
// it output it can't use. An Anthropic app's own schema goes as it wrote it. Claude always
// enforces the schema, so OpenAI's `strict: false` is served strictly.
pub(crate) fn for_claude(d: &mut Draft<'_>) -> Result<(), KppError> {
    if d.get().extras.is_from(D) {
        return Ok(());
    }
    output(d, D, true)?;
    strict_tools(d)
}

// Anthropic Messages and Bedrock Converse have no JSON mode, Claude's structured outputs can't
// express an open object (every object needs `additionalProperties: false`), and Claude 5 models
// refuse an assistant prefill, so the system prompt asks for the object instead.
const JSON_MODE: &str =
    "Answer with one JSON object and nothing else: no code fence, no text before or after it.";

pub(crate) fn json_mode_by_instruction(d: &mut Draft<'_>, target: DialectId) {
    if !matches!(d.get().output, OutputSpec::JsonObject) {
        return;
    }
    let param = cross::app_field(d.get(), Setting::OutputFormat, target);
    let r = d.edit();
    r.output = OutputSpec::Text;
    r.system.push(Block::text(JSON_MODE));
    d.adapt(param.to_owned());
}

// Gemini always enforces a `responseJsonSchema` too, and takes every keyword OpenAI's does.
pub(crate) fn for_gemini(d: &mut Draft<'_>) -> Result<(), KppError> {
    if d.get().extras.is_from(DialectId::Gemini) {
        return Ok(());
    }
    output(d, DialectId::Gemini, false)
}

fn output(d: &mut Draft<'_>, target: DialectId, rewrite: bool) -> Result<(), KppError> {
    let OutputSpec::JsonSchema(spec) = &d.get().output else {
        return Ok(());
    };
    let param = cross::app_field(d.get(), Setting::OutputFormat, target);
    let schema = if rewrite {
        rewritten(&spec.schema, true).map_err(|_| refusal(param))?
    } else {
        None
    };
    let strict_off = spec.strict == Some(false);
    let schema = match &spec.description {
        Some(text) => Some(with_description(
            schema.as_ref().unwrap_or(&spec.schema),
            text,
        )),
        None => schema,
    };
    if schema.is_none() && !strict_off {
        return Ok(());
    }
    let OutputSpec::JsonSchema(spec) = &mut d.edit().output else {
        return Ok(());
    };
    if let Some(schema) = schema {
        spec.schema = schema;
    }
    spec.description = None;
    spec.strict = None;
    d.adapt(param.to_owned());
    Ok(())
}

// OpenAI's `json_schema.description` tells the model what the output is for; Claude and Gemini
// take a schema alone, so it leads the schema's own description.
fn with_description(schema: &RawJson, text: &str) -> RawJson {
    let Ok(Value::Object(mut root)) = schema.to_value() else {
        return schema.clone();
    };
    let combined = match root.get("description").and_then(Value::as_str) {
        Some(own) if !own.is_empty() => format!("{text}\n\n{own}"),
        _ => text.to_owned(),
    };
    root.insert("description".to_owned(), Value::String(combined));
    RawJson::from_serialize(&root).unwrap_or_else(|_| schema.clone())
}

fn strict_tools(d: &mut Draft<'_>) -> Result<(), KppError> {
    let mut changed = Vec::new();
    for (i, tool) in d.get().tools.iter().enumerate() {
        let ToolDef::Function(f) = tool else {
            continue;
        };
        if f.strict != Some(true) {
            continue;
        }
        let Some(raw) = f.parameters.as_ref() else {
            continue;
        };
        let path = namespaces::app_path(d.original(), &f.name).unwrap_or(format!("tools[{i}]"));
        if let Ok(Some(schema)) = rewritten(raw, false) {
            changed.push((i, path, schema));
        }
    }
    if changed.is_empty() {
        return Ok(());
    }
    let r = d.edit();
    let mut paths = Vec::with_capacity(changed.len());
    for (i, path, schema) in changed {
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
    use kaeso_kpp::{DialectId, KppError, Request, RequestKind};
    use serde_json::{Value, json};

    use crate::codec::{EgressCtx, HeaderList};
    use crate::dispatch;
    use crate::test_support::fixtures;

    const A: DialectId = DialectId::AnthropicMessages;
    const C: DialectId = DialectId::OpenAiChat;
    const G: DialectId = DialectId::Gemini;
    const R: DialectId = DialectId::OpenAiResponses;

    fn decoded(dialect: DialectId, body: &Value) -> Request {
        dispatch::decode_request(
            dialect,
            RequestKind::Generate,
            body.to_string().as_bytes(),
            &HeaderList::new(),
        )
        .unwrap()
    }

    fn fixture(name: &str) -> Request {
        let (_, body) = fixtures("openai_chat", "requests", "json")
            .into_iter()
            .find(|(n, _)| n == name)
            .unwrap();
        let mut body: Value = serde_json::from_slice(&body).unwrap();
        body["max_completion_tokens"] = json!(200);
        decoded(C, &body)
    }

    fn with_schema(spec: Value) -> Request {
        decoded(
            C,
            &json!({
                "model": "m", "max_completion_tokens": 200,
                "messages": [{"role": "user", "content": "Tokyo"}],
                "response_format": {"type": "json_schema", "json_schema": spec}
            }),
        )
    }

    fn sent(target: DialectId, req: &Request) -> Result<(Value, Vec<String>), KppError> {
        let mut ctx = EgressCtx::new(req);
        dispatch::check(target, req, &ctx)?;
        let out = dispatch::encode_request(target, req, &mut ctx)?;
        Ok((serde_json::from_slice(&out.body).unwrap(), ctx.adapted))
    }

    fn claude_schema(req: &Request) -> (Value, Vec<String>) {
        let (body, adapted) = sent(A, req).unwrap();
        (body["output_config"]["format"]["schema"].clone(), adapted)
    }

    #[test]
    fn llms_open_schema_reaches_claude_closed() {
        let (schema, adapted) = claude_schema(&fixture("llm-schema"));
        assert_eq!(
            schema,
            json!({
                "type": "object",
                "properties": {"name": {"type": "string"}, "country": {"type": "string"}},
                "required": ["name", "country"],
                "additionalProperties": false
            })
        );
        assert_eq!(adapted, ["response_format"]);
    }

    #[test]
    fn pydantic_constraints_move_into_descriptions_and_strict_tools_get_the_same() {
        let (schema, adapted) = claude_schema(&fixture("openai-python-parse-constraints"));
        let p = &schema["properties"];
        assert_eq!(
            p["population"],
            json!({"title": "Population", "type": "integer", "description": "{minimum: 0}"})
        );
        assert_eq!(
            p["rating"]["description"], "{maximum: 5, minimum: 1}",
            "{schema}"
        );
        assert!(p["rating"].get("minimum").is_none());
        assert_eq!(p["tags"]["description"], "{maxItems: 3}");
        assert_eq!(p["country"]["description"], "ISO country name");
        assert_eq!(adapted, ["response_format"]);

        let req = decoded(
            C,
            &json!({
                "model": "m", "max_completion_tokens": 200,
                "messages": [{"role": "user", "content": "Pick"}],
                "tools": [
                    {"type": "function", "function": {"name": "pick", "strict": true, "parameters": {
                        "type": "object", "additionalProperties": false, "required": ["n"],
                        "properties": {"n": {"type": "integer", "minimum": 1, "description": "A number"}}
                    }}},
                    {"type": "function", "function": {"name": "loose", "parameters": {
                        "type": "object", "properties": {"n": {"type": "integer", "minimum": 1}}
                    }}}
                ]
            }),
        );
        let (body, adapted) = sent(A, &req).unwrap();
        assert_eq!(
            body["tools"][0]["input_schema"]["properties"]["n"],
            json!({"type": "integer", "description": "A number\n\n{minimum: 1}"})
        );
        assert_eq!(
            body["tools"][1]["input_schema"]["properties"]["n"]["minimum"], 1,
            "a tool that isn't strict takes any schema"
        );
        assert_eq!(adapted, ["tools[0]"]);
    }

    #[test]
    fn every_nested_object_is_closed_and_unions_become_any_of() {
        let req = with_schema(json!({"name": "x", "strict": false, "schema": {
            "type": "object",
            "properties": {
                "items": {"type": "array", "minItems": 1, "items": {"properties": {"a": {"type": "string", "minLength": 2, "format": "email"}}}},
                "pair": {"type": "array", "minItems": 2, "items": {"type": "string", "format": "uri-reference"}},
                "either": {"oneOf": [{"$ref": "#/$defs/X"}, {"type": "null"}]},
                "map": {"type": "object", "properties": {"k": {"type": "string"}}, "additionalProperties": {"type": "string"}},
                "flag": {"type": ["object", "null"], "properties": {"on": {"type": "boolean"}}}
            },
            "$defs": {"X": {"type": "object", "properties": {"b": {"type": "integer", "not": {"const": 3}}}}}
        }}));
        let (schema, adapted) = claude_schema(&req);
        let p = &schema["properties"];
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            p["items"],
            json!({"type": "array", "minItems": 1, "items": {
                "properties": {"a": {"type": "string", "minLength": 2, "format": "email"}},
                "type": "object", "additionalProperties": false
            }})
        );
        assert_eq!(
            p["pair"],
            json!({"type": "array", "description": "{minItems: 2}",
                "items": {"type": "string", "description": "{format: \"uri-reference\"}"}})
        );
        assert_eq!(
            p["either"],
            json!({"anyOf": [{"$ref": "#/$defs/X"}, {"type": "null"}]})
        );
        assert_eq!(p["map"]["additionalProperties"], false);
        assert_eq!(p["flag"]["additionalProperties"], false);
        assert_eq!(
            schema["$defs"]["X"],
            json!({"type": "object", "additionalProperties": false,
                "properties": {"b": {"type": "integer", "description": "{not: {\"const\":3}}"}}})
        );
        assert_eq!(adapted, ["response_format"]);
    }

    #[test]
    fn a_closed_schema_claude_takes_goes_unchanged() {
        let spec = json!({"name": "x", "strict": true, "schema": {
            "type": "object", "properties": {"a": {"type": "string", "pattern": "^[a-z]+$"}},
            "required": ["a"], "additionalProperties": false
        }});
        let (schema, adapted) = claude_schema(&with_schema(spec.clone()));
        assert_eq!(schema, spec["schema"]);
        assert!(adapted.is_empty(), "{adapted:?}");
    }

    #[test]
    fn the_description_of_a_schema_leads_the_schemas_own() {
        let req = with_schema(
            json!({"name": "city", "description": "A city the user asked about.",
            "schema": {"type": "object", "description": "One city.", "properties": {"n": {"type": "string"}},
                "required": ["n"], "additionalProperties": false}}),
        );
        let (schema, adapted) = claude_schema(&req);
        assert_eq!(
            schema["description"],
            "A city the user asked about.\n\nOne city."
        );
        assert_eq!(adapted, ["response_format"]);
        let (body, adapted) = sent(G, &req).unwrap();
        assert_eq!(
            body["generationConfig"]["responseJsonSchema"]["description"],
            "A city the user asked about.\n\nOne city."
        );
        assert_eq!(adapted, ["response_format"]);
    }

    #[test]
    fn gemini_serves_strict_false_and_keeps_every_keyword() {
        let req = with_schema(json!({"name": "x", "strict": false, "schema": {
            "type": "object", "properties": {"n": {"type": "integer", "minimum": 1}}
        }}));
        let (body, adapted) = sent(G, &req).unwrap();
        assert_eq!(
            body["generationConfig"]["responseJsonSchema"],
            json!({"type": "object", "properties": {"n": {"type": "integer", "minimum": 1}}})
        );
        assert_eq!(adapted, ["response_format"]);
    }

    #[test]
    fn a_schema_that_lists_no_keys_is_refused_for_claude() {
        let req = with_schema(json!({"name": "x", "schema": {"type": "object"}}));
        let e = sent(A, &req).unwrap_err();
        assert_eq!(e.param.as_deref(), Some("response_format"));
        assert!(e.message.contains("lists none"), "{}", e.message);
        let empty =
            with_schema(json!({"name": "x", "schema": {"type": "object", "properties": {}}}));
        assert_eq!(
            sent(A, &empty).unwrap_err().param.as_deref(),
            Some("response_format")
        );
    }

    #[test]
    fn other_apps_formats_are_named_by_their_own_fields() {
        let responses = decoded(
            R,
            &json!({"model": "m", "input": "Tokyo", "max_output_tokens": 100, "text": {"format": {
                "type": "json_schema", "name": "x",
                "schema": {"type": "object", "properties": {"n": {"type": "string"}}}}}}),
        );
        assert_eq!(claude_schema(&responses).1, ["text.format"]);
        let gemini = decoded(
            G,
            &json!({"contents": [{"role": "user", "parts": [{"text": "Tokyo"}]}],
                "generationConfig": {"maxOutputTokens": 100, "responseMimeType": "application/json",
                    "responseJsonSchema": {"type": "object", "properties": {"n": {"type": "string"}}}}}),
        );
        assert_eq!(
            claude_schema(&gemini).1,
            ["generationConfig.responseJsonSchema"]
        );
    }

    #[test]
    fn an_anthropic_apps_own_schema_goes_as_it_wrote_it() {
        let req = decoded(
            A,
            &json!({"model": "m", "max_tokens": 100, "messages": [{"role": "user", "content": "Tokyo"}],
                "output_config": {"format": {"type": "json_schema",
                    "schema": {"type": "object", "properties": {"n": {"type": "integer", "minimum": 1}}}}}}),
        );
        let (schema, adapted) = claude_schema(&req);
        assert_eq!(schema["properties"]["n"]["minimum"], 1);
        assert!(schema.get("additionalProperties").is_none());
        assert!(adapted.is_empty());
    }

    fn system_texts(body: &Value) -> Vec<String> {
        body["system"]
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|b| b["text"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn json_mode_reaches_claude_and_bedrock_as_an_instruction_after_the_apps_own() {
        let req = fixture("instructor-json-mode");
        for target in [A, DialectId::BedrockConverse] {
            let (body, adapted) = sent(target, &req).unwrap_or_else(|e| panic!("{target}: {e}"));
            assert_eq!(adapted, ["response_format"], "{target}");
            let system = system_texts(&body);
            assert_eq!(system.len(), 2, "{target}: {body}");
            assert!(
                system[0].contains("json_schema"),
                "the app's prompt first: {body}"
            );
            assert_eq!(system[1], super::JSON_MODE, "{target}");
            assert!(body.get("output_config").is_none(), "{body}");
        }
        let (body, _) = sent(A, &fixture("json-object")).unwrap();
        assert_eq!(
            system_texts(&body).last().map(String::as_str),
            Some(super::JSON_MODE)
        );
    }

    #[test]
    fn json_mode_is_named_by_each_apps_field_and_counted_with_its_instruction() {
        let responses = decoded(
            R,
            &json!({"model": "m", "input": "Tokyo", "max_output_tokens": 100,
                "text": {"format": {"type": "json_object"}}}),
        );
        assert_eq!(sent(A, &responses).unwrap().1, ["text.format"]);
        let gemini = decoded(
            G,
            &json!({"contents": [{"role": "user", "parts": [{"text": "Tokyo"}]}],
                "generationConfig": {"maxOutputTokens": 100, "responseMimeType": "application/json"}}),
        );
        assert_eq!(
            sent(A, &gemini).unwrap().1,
            ["generationConfig.responseMimeType"]
        );
        let Request::Generate(g) = fixture("json-object") else {
            panic!("a generation request");
        };
        let count = Request::CountTokens(g);
        let (body, _) = sent(A, &count).unwrap();
        assert_eq!(
            system_texts(&body).last().map(String::as_str),
            Some(super::JSON_MODE),
            "a count holds the instruction the generation sends"
        );
    }

    #[test]
    fn formats_with_json_mode_get_it_natively() {
        let req = fixture("instructor-json-mode");
        let (body, adapted) = sent(C, &req).unwrap();
        assert_eq!(body["response_format"], json!({"type": "json_object"}));
        assert!(adapted.is_empty());
        let (body, adapted) = sent(R, &req).unwrap();
        assert_eq!(body["text"]["format"], json!({"type": "json_object"}));
        assert!(adapted.is_empty());
        let (body, adapted) = sent(G, &req).unwrap();
        assert_eq!(
            body["generationConfig"]["responseMimeType"],
            "application/json"
        );
        assert!(adapted.is_empty());
    }
}
