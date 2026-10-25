use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

use axum::http::Method;
use kaeso_core::rights::Right;
use serde_json::{Map, Value, json};

use crate::platform::PREFIX;

#[derive(Clone, Debug)]
pub struct Tool {
    pub name: String,
    pub title: String,
    pub description: String,
    pub method: Method,
    pub path: String,
    pub right: Right,
    pub path_params: Vec<String>,
    pub query_params: Vec<String>,
    pub body: bool,
    pub input_schema: Value,
}

impl Tool {
    pub fn listing(&self) -> Value {
        let read_only = self.method == Method::GET;
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": self.input_schema,
            "annotations": {
                "title": self.title,
                "readOnlyHint": read_only,
                "destructiveHint": self.method == Method::DELETE,
                "idempotentHint": read_only || self.method == Method::PUT || self.method == Method::DELETE,
                "openWorldHint": false,
            },
        })
    }
}

#[derive(Debug, Default)]
pub struct Tools {
    list: Vec<Tool>,
    by_name: HashMap<String, usize>,
}

impl Tools {
    pub fn all(&self) -> &[Tool] {
        &self.list
    }

    pub fn get(&self, name: &str) -> Option<&Tool> {
        self.by_name.get(name).and_then(|i| self.list.get(*i))
    }
}

pub fn catalogue() -> &'static Tools {
    static TOOLS: OnceLock<Tools> = OnceLock::new();
    TOOLS.get_or_init(|| {
        serde_json::to_value(crate::platform::openapi())
            .map(|doc| build(&doc))
            .unwrap_or_default()
    })
}

pub fn snake(operation_id: &str) -> String {
    let mut out = String::with_capacity(operation_id.len() + 8);
    for (i, c) in operation_id.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

// These check a right an agent can hold, but their handlers then refuse anything but a console
// session (minting platform keys, the Playground) or answer with an image, not JSON.
const NOT_TOOLS: &[&str] = &[
    "get_me_avatars_hash",
    "get_orgs_org_projects_project_playground_models",
    "post_orgs_org_platform_keys",
    "post_orgs_org_platform_keys_key_rotate",
    "post_orgs_org_projects_project_playground_responses",
];

fn agent_usable(op: &Value) -> bool {
    op["security"]
        .as_array()
        .is_some_and(|s| s.iter().any(|req| req.get("agent").is_some()))
}

fn rewrite_refs(v: &mut Value, found: &mut BTreeSet<String>) {
    match v {
        Value::Object(map) => {
            if let Some(Value::String(r)) = map.get_mut("$ref")
                && let Some(name) = r.strip_prefix("#/components/schemas/")
            {
                found.insert(name.to_owned());
                *r = format!("#/$defs/{name}");
            }
            for child in map.values_mut() {
                rewrite_refs(child, found);
            }
        }
        Value::Array(items) => {
            for child in items {
                rewrite_refs(child, found);
            }
        }
        _ => {}
    }
}

fn defs(doc: &Value, mut pending: BTreeSet<String>) -> Map<String, Value> {
    let mut out = Map::new();
    while let Some(name) = pending.pop_first() {
        if out.contains_key(&name) {
            continue;
        }
        let mut schema = doc
            .pointer(&format!("/components/schemas/{name}"))
            .cloned()
            .unwrap_or(Value::Null);
        let mut more = BTreeSet::new();
        rewrite_refs(&mut schema, &mut more);
        pending.extend(more.into_iter().filter(|m| !out.contains_key(m)));
        out.insert(name, schema);
    }
    out
}

fn input_schema(doc: &Value, path: &str, op: &Value) -> (Value, Vec<String>, Vec<String>, bool) {
    let mut properties = Map::new();
    let mut required = Vec::new();
    let mut path_params = Vec::new();
    let mut query_params = Vec::new();
    let mut refs = BTreeSet::new();
    for p in op["parameters"].as_array().into_iter().flatten() {
        let Some(name) = p["name"].as_str() else {
            continue;
        };
        let place = p["in"].as_str().unwrap_or_default();
        if place != "path" && place != "query" {
            continue;
        }
        let mut schema = p["schema"].clone();
        rewrite_refs(&mut schema, &mut refs);
        if let (Some(obj), Some(d)) = (schema.as_object_mut(), p["description"].as_str()) {
            obj.entry("description").or_insert_with(|| json!(d));
        }
        properties.insert(name.to_owned(), schema);
        if place == "path" {
            required.push(name.to_owned());
            path_params.push(name.to_owned());
        } else {
            if p["required"].as_bool() == Some(true) {
                required.push(name.to_owned());
            }
            query_params.push(name.to_owned());
        }
    }
    let body = op.pointer("/requestBody/content/application~1json/schema");
    let has_body = body.is_some();
    if let Some(body) = body {
        let mut schema = body.clone();
        rewrite_refs(&mut schema, &mut refs);
        properties.insert("body".to_owned(), schema);
        if op.pointer("/requestBody/required").and_then(Value::as_bool) != Some(false) {
            required.push("body".to_owned());
        }
    }
    path_params.sort_by_key(|p| path.find(&format!("{{{p}}}")));
    let mut schema = json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    });
    let defs = defs(doc, refs);
    if let (Some(obj), false) = (schema.as_object_mut(), defs.is_empty()) {
        obj.insert("$defs".to_owned(), Value::Object(defs));
    }
    (schema, path_params, query_params, has_body)
}

pub fn build(doc: &Value) -> Tools {
    let mut tools = BTreeMap::new();
    for (path, item) in doc["paths"].as_object().into_iter().flatten() {
        for (method, op) in item.as_object().into_iter().flatten() {
            let Ok(method) = method.to_uppercase().parse::<Method>() else {
                continue;
            };
            let Some(right) = op["x-kaeso-access"]
                .as_str()
                .and_then(|a| a.strip_prefix("right:"))
                .and_then(|r| r.parse::<Right>().ok())
            else {
                continue;
            };
            if !agent_usable(op) {
                continue;
            }
            let Some(id) = op["operationId"].as_str() else {
                continue;
            };
            if NOT_TOOLS.contains(&snake(id).as_str()) {
                continue;
            }
            let short = path.trim_start_matches(PREFIX).to_owned();
            let (input_schema, path_params, query_params, body) = input_schema(doc, &short, op);
            let summary = op["summary"]
                .as_str()
                .unwrap_or_default()
                .replace('\n', " ");
            let details = op["description"].as_str().unwrap_or_default();
            let tool = Tool {
                name: snake(id),
                title: summary.trim_end_matches('.').to_owned(),
                description: format!("{summary}\n\n{details}\n\nPlatform API: `{method} {path}`."),
                method,
                path: short,
                right,
                path_params,
                query_params,
                body,
                input_schema,
            };
            tools.insert(tool.name.clone(), tool);
        }
    }
    let list: Vec<Tool> = tools.into_values().collect();
    let by_name = list
        .iter()
        .enumerate()
        .map(|(i, t)| (t.name.clone(), i))
        .collect();
    Tools { list, by_name }
}

fn encode_segment(v: &str) -> String {
    url::form_urlencoded::byte_serialize(v.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Call {
    pub uri: String,
    pub body: Option<Value>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ArgumentError {
    #[error("`{0}` is required")]
    Missing(String),
    #[error("`{0}` must be a string, number or boolean")]
    NotScalar(String),
    #[error("`{0}` is not an argument of this tool")]
    Unknown(String),
}

pub fn call(tool: &Tool, args: &Map<String, Value>) -> Result<Call, ArgumentError> {
    for key in args.keys() {
        let known = tool.path_params.contains(key)
            || tool.query_params.contains(key)
            || (tool.body && key == "body");
        if !known {
            return Err(ArgumentError::Unknown(key.clone()));
        }
    }
    let mut path = tool.path.clone();
    for p in &tool.path_params {
        let value = args
            .get(p)
            .ok_or_else(|| ArgumentError::Missing(p.clone()))
            .and_then(|v| scalar(v).ok_or_else(|| ArgumentError::NotScalar(p.clone())))?;
        path = path.replace(&format!("{{{p}}}"), &encode_segment(&value));
    }
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    let mut any = false;
    for q in &tool.query_params {
        match args.get(q) {
            None | Some(Value::Null) => {}
            Some(Value::Array(items)) => {
                for item in items {
                    let v = scalar(item).ok_or_else(|| ArgumentError::NotScalar(q.clone()))?;
                    query.append_pair(q, &v);
                    any = true;
                }
            }
            Some(v) => {
                let v = scalar(v).ok_or_else(|| ArgumentError::NotScalar(q.clone()))?;
                query.append_pair(q, &v);
                any = true;
            }
        }
    }
    let mut uri = format!("{PREFIX}{path}");
    if any {
        uri.push('?');
        uri.push_str(&query.finish());
    }
    let body = if tool.body {
        Some(args.get("body").cloned().unwrap_or_else(|| json!({})))
    } else {
        None
    };
    Ok(Call { uri, body })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str) -> &'static Tool {
        catalogue()
            .get(name)
            .unwrap_or_else(|| panic!("no tool {name}"))
    }

    #[test]
    fn every_agent_route_is_a_tool_and_session_routes_are_not() {
        let tools = catalogue();
        assert!(tools.all().len() > 100, "{}", tools.all().len());
        assert!(tools.get("get_orgs_org_usage").is_some());
        assert!(tools.get("get_instance_system").is_some());
        assert!(tools.get("get_me").is_some());
        for session_only in [
            "get_me_agents",
            "post_me_password",
            "get_me_sessions",
            "put_instance_settings_admin_sso",
            "put_orgs_org_security_agents",
        ]
        .into_iter()
        .chain(NOT_TOOLS.iter().copied())
        {
            assert!(tools.get(session_only).is_none(), "{session_only}");
        }
        for t in tools.all() {
            assert!(
                t.name.len() <= 128
                    && t.name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_'),
                "{}",
                t.name
            );
            assert_eq!(t.input_schema["type"], "object", "{}", t.name);
            assert!(!t.title.is_empty(), "{}", t.name);
        }
    }

    #[test]
    fn schemas_carry_their_definitions() {
        let t = tool("patch_orgs_org_projects_project_limits_limit");
        let schema = &t.input_schema;
        assert!(schema["properties"]["body"].is_object());
        let text = schema.to_string();
        assert!(!text.contains("#/components/"), "{text}");
        for r in text.match_indices("#/$defs/") {
            let name: String = text[r.0 + 8..].chars().take_while(|c| *c != '"').collect();
            assert!(schema["$defs"][&name].is_object(), "missing {name}");
        }
    }

    #[test]
    fn calls_fill_the_path_and_query_and_refuse_strangers() {
        let t = tool("get_orgs_org_audit_log");
        let args = json!({ "org": "org_1", "action": "project.", "limit": 5 });
        let c = call(t, args.as_object().unwrap()).unwrap();
        assert!(
            c.uri.starts_with("/platform/v1/orgs/org_1/audit-log?"),
            "{}",
            c.uri
        );
        assert!(c.uri.contains("action=project."), "{}", c.uri);
        assert!(c.uri.contains("limit=5"), "{}", c.uri);
        assert_eq!(c.body, None);
        let err = call(t, json!({ "org": "org_1", "x": 1 }).as_object().unwrap()).unwrap_err();
        assert_eq!(err, ArgumentError::Unknown("x".into()));
        let err = call(t, json!({}).as_object().unwrap()).unwrap_err();
        assert_eq!(err, ArgumentError::Missing("org".into()));
        let err = call(t, json!({ "org": {} }).as_object().unwrap()).unwrap_err();
        assert_eq!(err, ArgumentError::NotScalar("org".into()));
        let c = call(t, json!({ "org": "a/b c" }).as_object().unwrap()).unwrap();
        assert_eq!(c.uri, "/platform/v1/orgs/a%2Fb%20c/audit-log");
    }

    #[test]
    fn names_are_snake_case_operation_ids() {
        assert_eq!(snake("getOrgsOrgProjects"), "get_orgs_org_projects");
        assert_eq!(snake("postMeTotpConfirm"), "post_me_totp_confirm");
    }
}
