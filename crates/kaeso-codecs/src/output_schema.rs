use kaeso_kpp::{DialectId, KppError, OutputSpec, RawJson, ToolDef};
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
const MERGEABLE: [&str; 6] = [
    "type",
    "properties",
    "required",
    "description",
    "title",
    "additionalProperties",
];

enum Unfit {
    NoKeys,
    OpenObject(String),
    ObjectBranches(String),
}

struct Rewrite {
    changed: bool,
    refs: Vec<String>,
}

// Where a schema sits: as the error names it (`properties.totals`) and as a `$ref` points to it.
struct Place {
    at: String,
    ptr: String,
}

impl Place {
    fn root() -> Place {
        Place {
            at: String::new(),
            ptr: String::new(),
        }
    }

    fn field(&self, key: &str) -> Place {
        Place {
            at: within(&self.at, key),
            ptr: format!("{}/{key}", self.ptr),
        }
    }

    fn member(&self, key: &str, name: &str) -> Place {
        let escaped = name.replace('~', "~0").replace('/', "~1");
        Place {
            at: within(&self.at, &format!("{key}.{name}")),
            ptr: format!("{}/{key}/{escaped}", self.ptr),
        }
    }

    fn item(&self, key: &str, i: usize) -> Place {
        Place {
            at: within(&self.at, &format!("{key}[{i}]")),
            ptr: format!("{}/{key}/{i}", self.ptr),
        }
    }
}

impl Rewrite {
    fn node(&mut self, node: &mut Map<String, Value>, place: &Place) -> Result<(), Unfit> {
        self.flatten_all_of(node, place)?;
        self.merge_all_of(node, place)?;
        // Claude closes every object, so one that lists no keys could only ever be `{}`.
        if !place.at.is_empty()
            && !lists_keys(node)
            && node.get("additionalProperties") != Some(&Value::Bool(false))
        {
            return Err(Unfit::OpenObject(place.at.clone()));
        }
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
                for (name, schema) in schemas.iter_mut() {
                    self.value(schema, &place.member(key, name))?;
                }
            }
        }
        for key in SCHEMA_LISTS {
            if let Some(Value::Array(schemas)) = node.get_mut(key) {
                for (i, schema) in schemas.iter_mut().enumerate() {
                    self.value(schema, &place.item(key, i))?;
                }
            }
        }
        for key in SCHEMAS {
            if let Some(schema) = node.get_mut(key) {
                self.value(schema, &place.field(key))?;
            }
        }
        Ok(())
    }

    fn value(&mut self, value: &mut Value, place: &Place) -> Result<(), Unfit> {
        match value {
            Value::Object(node) => self.node(node, place),
            _ => Ok(()),
        }
    }

    fn points_into(&self, ptr: &str) -> bool {
        let target = format!("#{ptr}");
        self.refs.iter().any(|r| {
            r.strip_prefix(&target)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
        })
    }

    // Claude answers a nested `allOf` with 400 "nested allOf is not supported" (checked 2026-11-27).
    fn flatten_all_of(
        &mut self,
        node: &mut Map<String, Value>,
        place: &Place,
    ) -> Result<(), Unfit> {
        let nested = matches!(node.get("allOf"), Some(Value::Array(parts))
            if parts.iter().any(|p| matches!(p, Value::Object(o) if all_of_only(o))));
        if !nested {
            return Ok(());
        }
        if self.points_into(&place.field("allOf").ptr) {
            return Err(Unfit::ObjectBranches(within(&place.at, "allOf")));
        }
        if let Some(Value::Array(parts)) = node.remove("allOf") {
            let mut flat = Vec::with_capacity(parts.len());
            for part in parts {
                flat_parts(part, &mut flat);
            }
            node.insert("allOf".to_owned(), Value::Array(flat));
            self.changed = true;
        }
        Ok(())
    }

    // `allOf` applies every branch to one value, so closing each object branch on its own would
    // forbid the keys the others require; their keys join the node's own instead.
    fn merge_all_of(&mut self, node: &mut Map<String, Value>, place: &Place) -> Result<(), Unfit> {
        let Some(Value::Array(branches)) = node.get("allOf") else {
            return Ok(());
        };
        let objects: Vec<usize> = (0..branches.len())
            .filter(|&i| branches.get(i).is_some_and(object_like))
            .collect();
        let own = ["properties", "required"]
            .iter()
            .any(|k| node.contains_key(*k))
            || is_object(node);
        if objects.len() + usize::from(own) < 2 {
            return Ok(());
        }
        let refused = || Unfit::ObjectBranches(within(&place.at, "allOf"));
        let open = |n: &Map<String, Value>| {
            n.get("additionalProperties")
                .is_none_or(|a| *a == Value::Bool(true))
        };
        let mut merged = Vec::with_capacity(objects.len());
        let mut hoisted = Vec::new();
        for &i in &objects {
            let Some(Value::Object(b)) = branches.get(i) else {
                return Err(refused());
            };
            let mut b = b.clone();
            // TypeBox writes an intersection of intersections as an object branch with an `allOf`.
            let inner = place.item("allOf", i);
            self.flatten_all_of(&mut b, &inner)?;
            self.merge_all_of(&mut b, &inner)?;
            // What the inner join leaves, such as `minProperties`, applies to the same value.
            if let Some(Value::Array(rest)) = b.remove("allOf") {
                hoisted.extend(rest);
            }
            if !(mergeable(&b) && open(&b)) {
                return Err(refused());
            }
            merged.push(b);
        }
        let ty = merged
            .iter()
            .fold(types_of(node), |types, b| both(types, types_of(b)));
        // Without a written type the node stays untyped: `node` types it once its keys are known.
        let ty = match ty {
            None => None,
            Some(t) if t == ["object"] => Some(Value::from("object")),
            Some(t) if t.iter().any(|t| t == "object") => {
                Some(t.into_iter().map(Value::from).collect())
            }
            Some(_) => return Err(refused()),
        };
        let combined = changed_by_join(node, &merged)
            .iter()
            .any(|name| self.points_into(&place.member("properties", name).ptr));
        if !open(node) || self.points_into(&place.field("allOf").ptr) || combined {
            return Err(refused());
        }
        let Some(Value::Array(mut branches)) = node.remove("allOf") else {
            return Ok(());
        };
        for &i in objects.iter().rev() {
            branches.remove(i);
        }
        for branch in merged {
            join_object(node, branch);
        }
        if let Some(ty) = ty {
            node.insert("type".to_owned(), ty);
        }
        for part in hoisted {
            if !branches.contains(&part) {
                branches.push(part);
            }
        }
        node.remove("additionalProperties");
        if !branches.is_empty() {
            node.insert("allOf".to_owned(), Value::Array(branches));
        }
        self.changed = true;
        Ok(())
    }
}

fn types_of(node: &Map<String, Value>) -> Option<Vec<String>> {
    match node.get("type")? {
        Value::String(t) => Some(vec![t.clone()]),
        Value::Array(types) => Some(
            types
                .iter()
                .filter_map(|t| t.as_str().map(str::to_owned))
                .collect(),
        ),
        _ => None,
    }
}

fn both(a: Option<Vec<String>>, b: Option<Vec<String>>) -> Option<Vec<String>> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.into_iter().filter(|t| b.contains(t)).collect()),
        (a, b) => a.or(b),
    }
}

fn changed_by_join(node: &Map<String, Value>, branches: &[Map<String, Value>]) -> Vec<String> {
    let mut first: Vec<(&String, &Value)> = Vec::new();
    let mut changed = Vec::new();
    let properties = std::iter::once(node)
        .chain(branches)
        .filter_map(|n| n.get("properties").and_then(Value::as_object))
        .flatten();
    for (name, schema) in properties {
        match first.iter().find(|(n, _)| *n == name) {
            Some((_, own)) if *own != schema && !changed.contains(name) => {
                changed.push(name.clone());
            }
            Some(_) => {}
            None => first.push((name, schema)),
        }
    }
    changed
}

fn within(at: &str, key: &str) -> String {
    if at.is_empty() {
        key.to_owned()
    } else {
        format!("{at}.{key}")
    }
}

fn object_like(schema: &Value) -> bool {
    let Value::Object(s) = schema else {
        return false;
    };
    is_object(s)
        || ["properties", "required", "$ref", "anyOf", "oneOf", "allOf"]
            .iter()
            .any(|k| s.contains_key(*k))
}

fn mergeable(branch: &Map<String, Value>) -> bool {
    branch.keys().all(|k| MERGEABLE.contains(&k.as_str()))
        && branch.get("type").is_none_or(|_| is_object(branch))
}

fn join_object(node: &mut Map<String, Value>, branch: Map<String, Value>) {
    for (key, value) in branch {
        match (key.as_str(), value) {
            ("properties", Value::Object(added)) => {
                let Value::Object(properties) = node
                    .entry("properties")
                    .or_insert_with(|| Value::Object(Map::new()))
                else {
                    continue;
                };
                for (name, schema) in added {
                    match properties.get_mut(&name) {
                        Some(own) if *own != schema => combine(own, schema),
                        Some(_) => {}
                        None => {
                            properties.insert(name, schema);
                        }
                    }
                }
            }
            ("required", Value::Array(added)) => {
                if let Value::Array(required) = node
                    .entry("required")
                    .or_insert_with(|| Value::Array(Vec::new()))
                {
                    for name in added {
                        if !required.contains(&name) {
                            required.push(name);
                        }
                    }
                }
            }
            ("description", Value::String(text)) => {
                let joined = match node.get("description").and_then(Value::as_str) {
                    Some(own) if !own.is_empty() => format!("{own}\n\n{text}"),
                    _ => text,
                };
                node.insert("description".to_owned(), Value::String(joined));
            }
            ("title", title) => {
                node.entry("title").or_insert(title);
            }
            _ => {}
        }
    }
}

// A property several branches define takes all of their schemas in one flat `allOf`.
fn combine(own: &mut Value, schema: Value) {
    let mut parts = Vec::new();
    flat_parts(std::mem::take(own), &mut parts);
    flat_parts(schema, &mut parts);
    *own = match parts.len() {
        1 => parts.pop().unwrap_or_default(),
        _ => serde_json::json!({ "allOf": parts }),
    };
}

fn flat_parts(schema: Value, out: &mut Vec<Value>) {
    match schema {
        Value::Object(mut s) if all_of_only(&s) => {
            if let Some(Value::Array(parts)) = s.remove("allOf") {
                for part in parts {
                    flat_parts(part, out);
                }
            }
        }
        other if !out.contains(&other) => out.push(other),
        _ => {}
    }
}

fn all_of_only(o: &Map<String, Value>) -> bool {
    o.len() == 1 && matches!(o.get("allOf"), Some(Value::Array(_)))
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

fn rewritten(raw: &RawJson, output: bool) -> Result<Option<RawJson>, Unfit> {
    let Ok(Value::Object(mut root)) = raw.to_value() else {
        return Ok(None);
    };
    if output && !lists_keys(&root) {
        return Err(Unfit::NoKeys);
    }
    let mut refs = Vec::new();
    refs_in(&Value::Object(root.clone()), &mut refs);
    let mut rewrite = Rewrite {
        changed: false,
        refs,
    };
    rewrite.node(&mut root, &Place::root())?;
    if !rewrite.changed {
        return Ok(None);
    }
    Ok(RawJson::from_serialize(&root).ok())
}

fn refs_in(value: &Value, refs: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, v) in map {
                match (key.as_str(), v) {
                    ("$ref", Value::String(r)) => refs.push(r.clone()),
                    _ => refs_in(v, refs),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|v| refs_in(v, refs)),
        _ => {}
    }
}

fn refusal(unfit: Unfit, param: &str) -> KppError {
    let (feature, hint) = match unfit {
        Unfit::NoKeys => (
            "output schema without keys".to_owned(),
            "Claude's structured outputs return only the keys a schema lists, and this one lists none; list them in `properties`.",
        ),
        Unfit::OpenObject(at) => (
            format!("`{at}`, an object without keys,"),
            "Claude returns only the keys a schema lists; list them in `properties`, or send key and value pairs as a list of objects.",
        ),
        Unfit::ObjectBranches(at) => (
            format!("`{at}`, which Kaeso can't join into one object,"),
            "Claude closes every object, so list all of their keys in one object.",
        ),
    };
    KppError::unsupported_by(feature, D, hint).with_param(param)
}

// Schemas from other formats are rewritten into the subset Claude takes, as Anthropic's SDKs
// do: objects are closed, `oneOf` becomes `anyOf`, and constraints Claude can't enforce move
// into descriptions. Claude then enforces a little more than the app asked, which never gives
// it output it can't use. An Anthropic app's own schema goes as it wrote it. Claude always
// enforces the schema, so OpenAI's `strict: false` is served strictly. A token count leaves out
// an output schema it can't rewrite, as it does other output settings generation would refuse.
pub(crate) fn for_claude(d: &mut Draft<'_>, count: bool) -> Result<(), KppError> {
    if d.get().extras.is_from(D) {
        return Ok(());
    }
    match output(d, D, true) {
        Err(_) if count => {
            let param = cross::app_field(d.get(), Setting::OutputFormat, D);
            d.edit().output = OutputSpec::Text;
            d.leave_out(param.to_owned());
        }
        other => other?,
    }
    strict_tools(d, count)
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
        rewritten(&spec.schema, true).map_err(|unfit| refusal(unfit, param))?
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

fn strict_tools(d: &mut Draft<'_>, count: bool) -> Result<(), KppError> {
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
        match rewritten(raw, false) {
            Ok(Some(schema)) => changed.push((i, path, schema)),
            Err(unfit) if !count => return Err(refusal(unfit, &path)),
            _ => {}
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
    fn typeboxs_intersection_reaches_claude_as_one_closed_object() {
        let (schema, adapted) = claude_schema(&fixture("typebox-intersect"));
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
    fn typeboxs_nested_intersection_reaches_claude_as_one_closed_object() {
        let (schema, _) = claude_schema(&fixture("typebox-intersect-nested"));
        assert_eq!(
            schema,
            json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "country": {"type": "string"},
                    "population": {"type": "integer"}
                },
                "required": ["name", "country", "population"],
                "additionalProperties": false
            })
        );
    }

    #[test]
    fn a_nested_join_keeps_null_and_lifts_what_isnt_an_object() {
        let req = with_schema(json!({"name": "x", "schema": {
            "type": "object", "required": ["maybe"],
            "properties": {"maybe": {"type": ["object", "null"], "allOf": [
                {"properties": {"a": {"type": "string"}}, "allOf": [
                    {"properties": {"c": {"type": "string"}}}, {"minProperties": 1}
                ]},
                {"properties": {"b": {"type": "string"}}}
            ]}}
        }}));
        let (schema, _) = claude_schema(&req);
        let maybe = &schema["properties"]["maybe"];
        assert_eq!(maybe["type"], json!(["object", "null"]), "{schema}");
        assert_eq!(
            maybe["properties"],
            json!({"a": {"type": "string"}, "c": {"type": "string"}, "b": {"type": "string"}})
        );
        assert_eq!(
            maybe["allOf"],
            json!([{"description": "{minProperties: 1}"}])
        );
    }

    #[test]
    fn a_pydantic_dictionary_is_refused_for_claude_with_its_path() {
        let e = sent(A, &fixture("openai-python-parse-dict")).unwrap_err();
        assert_eq!(e.param.as_deref(), Some("response_format"));
        assert!(
            e.message
                .starts_with("`properties.totals`, an object without keys,"),
            "{}",
            e.message
        );
    }

    #[test]
    fn all_of_branches_join_into_one_closed_object() {
        let req = with_schema(json!({"name": "x", "schema": {
            "type": "object", "description": "A city.",
            "properties": {"name": {"type": "string"}},
            "allOf": [
                {"properties": {"name": {"minLength": 1}}, "required": ["name"]},
                {"properties": {"country": {"type": "string"}}, "required": ["country"],
                    "description": "Where it is."}
            ]
        }}));
        let (schema, adapted) = claude_schema(&req);
        assert_eq!(
            schema,
            json!({
                "type": "object", "description": "A city.\n\nWhere it is.",
                "properties": {
                    "name": {"allOf": [{"type": "string"}, {"minLength": 1}]},
                    "country": {"type": "string"}
                },
                "required": ["name", "country"],
                "additionalProperties": false
            }),
            "both keys stay allowed: {schema}"
        );
        assert_eq!(adapted, ["response_format"]);

        let referenced = with_schema(json!({"name": "x", "schema": {
            "$defs": {"Base": {"type": "object", "properties": {"id": {"type": "string"}}}},
            "allOf": [{"$ref": "#/$defs/Base"}, {"properties": {"n": {"type": "string"}}}]
        }}));
        let e = sent(A, &referenced).unwrap_err();
        assert_eq!(e.param.as_deref(), Some("response_format"));
        assert!(
            e.message.starts_with("`allOf`, which Kaeso can't join"),
            "{}",
            e.message
        );
    }

    #[test]
    fn a_property_several_branches_define_takes_all_their_schemas_in_one_list() {
        let address = |n: u32| {
            json!({"properties": {"address": {"type": "object", "required": ["zip"],
                "properties": {"zip": {"type": "string", "maxLength": n}}}}})
        };
        let req = with_schema(json!({"name": "x", "schema": {
            "type": "object", "required": ["address"],
            "allOf": [address(8), address(9), address(10)]
        }}));
        let (schema, _) = claude_schema(&req);
        let a = &schema["properties"]["address"];
        assert_eq!(a["type"], "object", "{schema}");
        assert_eq!(a["additionalProperties"], false);
        assert_eq!(a["required"], json!(["zip"]));
        assert_eq!(
            a["properties"]["zip"]["allOf"],
            json!([
                {"type": "string", "maxLength": 8},
                {"type": "string", "maxLength": 9},
                {"type": "string", "maxLength": 10}
            ])
        );
    }

    #[test]
    fn a_join_keeps_null_where_every_part_allows_it_and_keeps_references_whole() {
        let req = with_schema(json!({"name": "x", "schema": {
            "type": "object", "required": ["maybe"],
            "properties": {"maybe": {
                "type": ["object", "null"], "properties": {"a": {"type": "string"}},
                "allOf": [{"type": ["object", "null"], "properties": {"b": {"type": "string"}}}]
            }}
        }}));
        let (schema, _) = claude_schema(&req);
        assert_eq!(
            schema["properties"]["maybe"]["type"],
            json!(["object", "null"])
        );

        let pointing = with_schema(json!({"name": "x", "schema": {
            "type": "object",
            "properties": {
                "source": {"allOf": [
                    {"properties": {"id": {"type": "string"}}},
                    {"properties": {"n": {"type": "integer"}}}
                ]},
                "copy": {"$ref": "#/properties/source/allOf/0/properties/id"}
            }
        }}));
        let e = sent(A, &pointing).unwrap_err();
        assert!(
            e.message.starts_with("`properties.source.allOf`, which"),
            "{}",
            e.message
        );

        let unchanged = with_schema(json!({"name": "x", "schema": {
            "type": "object",
            "properties": {"id": {"type": "string"}, "copy": {"$ref": "#/properties/id"}},
            "allOf": [{"properties": {"id": {"type": "string"}, "region": {"type": "string"}}}]
        }}));
        let (schema, _) = claude_schema(&unchanged);
        assert_eq!(
            schema["properties"]["id"],
            json!({"type": "string"}),
            "{schema}"
        );
        assert_eq!(schema["properties"]["region"], json!({"type": "string"}));
    }

    #[test]
    fn a_join_flattens_lists_in_either_order() {
        let list = json!({"allOf": [{"maxLength": 8}, {"maxLength": 9}]});
        let one = json!({"maxLength": 10});
        let joined = |a: &Value, b: &Value| {
            let req = with_schema(json!({"name": "x", "schema": {
                "type": "object", "allOf": [
                    {"properties": {"zip": a}}, {"properties": {"zip": b}}
                ]
            }}));
            claude_schema(&req).0["properties"]["zip"]["allOf"]
                .as_array()
                .map(Vec::len)
        };
        assert_eq!(joined(&list, &one), Some(3));
        assert_eq!(joined(&one, &list), Some(3));

        let nested = with_schema(json!({"name": "x", "schema": {
            "type": "object", "properties": {"zip": {"allOf": [
                {"allOf": [{"type": "string"}, {"maxLength": 12}]}, {"maxLength": 10}
            ]}}
        }}));
        let (schema, _) = claude_schema(&nested);
        assert_eq!(
            schema["properties"]["zip"],
            json!({"allOf": [{"type": "string"}, {"maxLength": 12}, {"maxLength": 10}]}),
            "Claude refuses a nested allOf"
        );
    }

    #[test]
    fn an_open_object_without_keys_is_refused_wherever_it_sits() {
        let dictionary = with_schema(json!({"name": "x", "schema": {
            "type": "object", "required": ["totals"],
            "properties": {"totals": {"type": "object", "additionalProperties": {"type": "integer"}}}
        }}));
        let e = sent(A, &dictionary).unwrap_err();
        assert_eq!(e.param.as_deref(), Some("response_format"));
        assert!(
            e.message
                .starts_with("`properties.totals`, an object without keys,"),
            "{}",
            e.message
        );
        let closed = with_schema(json!({"name": "x", "schema": {
            "type": "object", "properties": {"none": {"type": "object", "additionalProperties": false}}
        }}));
        assert!(
            sent(A, &closed).is_ok(),
            "an object closed by the app stays"
        );

        let tool = decoded(
            C,
            &json!({
                "model": "m", "max_completion_tokens": 200,
                "messages": [{"role": "user", "content": "Count"}],
                "tools": [{"type": "function", "function": {"name": "count", "strict": true, "parameters": {
                    "type": "object", "required": ["by"],
                    "properties": {"by": {"type": "object", "additionalProperties": {"type": "integer"}}}
                }}}]
            }),
        );
        let e = sent(A, &tool).unwrap_err();
        assert_eq!(e.param.as_deref(), Some("tools[0]"));
        assert!(e.message.contains("`properties.by`"), "{}", e.message);
    }

    #[test]
    fn a_token_count_leaves_out_a_schema_claude_couldnt_take() {
        let req = dispatch::decode_request(
            R,
            RequestKind::CountTokens,
            json!({"model": "m", "input": "Tokyo", "text": {"format": {
                "type": "json_schema", "name": "x", "strict": false, "schema": {"type": "object"}}}})
            .to_string()
            .as_bytes(),
            &HeaderList::new(),
        )
        .unwrap();
        let mut ctx = EgressCtx::new(&req);
        dispatch::check(A, &req, &ctx).unwrap();
        let out = dispatch::encode_request(A, &req, &mut ctx).unwrap();
        let body: Value = serde_json::from_slice(&out.body).unwrap();
        assert!(body.get("output_config").is_none(), "{body}");
        assert_eq!(ctx.left_out, ["text.format"]);
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
}
