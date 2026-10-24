use kaeso_kpp::{Block, DialectId, GenerateRequest, KppError, RawItem, RawJson, ToolDef};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::wire::type_tag;

const RESPONSES: DialectId = DialectId::OpenAiResponses;
const ANTHROPIC: DialectId = DialectId::AnthropicMessages;

const SEARCH_CALL: &str = "web_search_call";
const KAESO_SEARCH_ID: &str = "ws_kaeso_";
const ANTHROPIC_TOOL: &str = "web_search_20250305";
const ANTHROPIC_NAME: &str = "web_search";
const LOCATION_FIELDS: &[&str] = &["type", "city", "country", "region", "timezone"];
const CODEX_HINT: &str = "From Codex, set `web_search = \"live\"` or turn web search off.";

fn refuse(feature: &str, path: &str, hint: &str) -> KppError {
    KppError::unsupported(
        feature,
        format!(
            "`{feature}` comes from an {} request and has no equivalent in {}. {hint}",
            RESPONSES.display_name(),
            ANTHROPIC.display_name()
        ),
    )
    .with_param(path)
}

fn is_hosted_search(item: &RawItem) -> bool {
    item.origin.is(RESPONSES)
        && type_tag(item.raw.as_raw()).ok().flatten().as_deref() == Some("web_search")
}

fn anthropic_tool(item: &RawItem, path: &str) -> Result<ToolDef, KppError> {
    let Ok(Value::Object(fields)) = item.raw.to_value() else {
        return Err(refuse("web_search", path, "Send it as an object."));
    };
    let mut tool = Map::new();
    tool.insert("type".into(), ANTHROPIC_TOOL.into());
    tool.insert("name".into(), ANTHROPIC_NAME.into());
    for (key, value) in &fields {
        let at = format!("{path}.{key}");
        match (key.as_str(), value) {
            ("type", _) | (_, Value::Null) => {}
            ("external_web_access", Value::Bool(true)) => {}
            ("external_web_access", _) => {
                return Err(refuse(
                    "web_search.external_web_access",
                    &at,
                    &format!("Claude's web search always reads live results. {CODEX_HINT}"),
                ));
            }
            ("search_context_size", Value::String(s)) if s == "medium" => {}
            ("filters", Value::Object(filters)) => {
                for (k, v) in filters {
                    match (k.as_str(), v) {
                        (_, Value::Null) => {}
                        ("allowed_domains", Value::Array(domains)) => {
                            if !domains.is_empty() {
                                tool.insert("allowed_domains".into(), v.clone());
                            }
                        }
                        _ => {
                            return Err(refuse(
                                &format!("web_search.filters.{k}"),
                                &format!("{at}.{k}"),
                                "Remove it.",
                            ));
                        }
                    }
                }
            }
            ("user_location", Value::Object(location)) => {
                let mut out = Map::new();
                for (k, v) in location {
                    if v.is_null() {
                        continue;
                    }
                    let approximate = k != "type" || v.as_str() == Some("approximate");
                    if !LOCATION_FIELDS.contains(&k.as_str()) || !approximate {
                        return Err(refuse(
                            &format!("web_search.user_location.{k}"),
                            &format!("{at}.{k}"),
                            "Remove it.",
                        ));
                    }
                    out.insert(k.clone(), v.clone());
                }
                if !out.is_empty() {
                    out.insert("type".into(), "approximate".into());
                    tool.insert("user_location".into(), Value::Object(out));
                }
            }
            _ => {
                return Err(refuse(&format!("web_search.{key}"), &at, "Remove it."));
            }
        }
    }
    let raw = RawJson::from_serialize(&Value::Object(tool))
        .map_err(|e| KppError::invalid_request(format!("`web_search` cannot be sent: {e}.")))?;
    Ok(ToolDef::Unknown(RawItem::new(
        kaeso_kpp::Origin::new(ANTHROPIC),
        raw,
    )))
}

fn search_call_id(block: &Block) -> Option<Option<String>> {
    #[derive(Deserialize)]
    struct Item {
        #[serde(rename = "type")]
        ty: String,
        #[serde(default)]
        id: Option<String>,
    }
    let Block::Unknown(item) = block else {
        return None;
    };
    if !item.origin.is(RESPONSES) {
        return None;
    }
    let parsed: Item = item.raw.parse().ok()?;
    (parsed.ty == SEARCH_CALL).then_some(parsed.id)
}

fn without_search_calls(r: &mut GenerateRequest, drop: impl Fn(Option<&str>) -> bool) -> bool {
    let mut dropped = false;
    r.messages.retain_mut(|m| {
        let held = m.blocks.len();
        m.blocks
            .retain(|b| search_call_id(b).is_none_or(|id| !drop(id.as_deref())));
        dropped |= m.blocks.len() != held;
        m.blocks.len() == held || !m.blocks.is_empty()
    });
    dropped
}

pub(crate) fn for_anthropic(r: &GenerateRequest) -> Result<Option<GenerateRequest>, KppError> {
    let searches = r
        .tools
        .iter()
        .any(|t| matches!(t, ToolDef::Unknown(item) if is_hosted_search(item)));
    let calls = r
        .messages
        .iter()
        .any(|m| m.blocks.iter().any(|b| search_call_id(b).is_some()));
    if !searches && !calls {
        return Ok(None);
    }
    let mut out = r.clone();
    for (i, t) in out.tools.iter_mut().enumerate() {
        if let ToolDef::Unknown(item) = t
            && is_hosted_search(item)
        {
            *t = anthropic_tool(item, &format!("tools[{i}]"))?;
        }
    }
    without_search_calls(&mut out, |_| true);
    Ok(Some(out))
}

pub(crate) fn for_responses(r: &GenerateRequest) -> Option<GenerateRequest> {
    let ours = |id: Option<&str>| id.is_some_and(|id| id.starts_with(KAESO_SEARCH_ID));
    let any = r.messages.iter().any(|m| {
        m.blocks
            .iter()
            .any(|b| search_call_id(b).is_some_and(|id| ours(id.as_deref())))
    });
    if !any {
        return None;
    }
    let mut out = r.clone();
    without_search_calls(&mut out, ours).then_some(out)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AnthropicSearch {
    pub(crate) tool_use_id: String,
    pub(crate) query: Option<String>,
}

pub(crate) fn anthropic_search(item: &RawItem) -> Option<AnthropicSearch> {
    #[derive(Deserialize)]
    struct ServerToolUse {
        #[serde(rename = "type")]
        ty: String,
        id: String,
        name: String,
        #[serde(default)]
        input: Option<Value>,
    }
    if !item.origin.is(ANTHROPIC) {
        return None;
    }
    let use_: ServerToolUse = item.raw.parse().ok()?;
    (use_.ty == "server_tool_use" && use_.name == ANTHROPIC_NAME).then(|| AnthropicSearch {
        tool_use_id: use_.id,
        query: use_.input.as_ref().and_then(query_of),
    })
}

pub(crate) fn query_of(input: &Value) -> Option<String> {
    input.get("query")?.as_str().map(str::to_owned)
}

pub(crate) fn anthropic_search_result(item: &RawItem) -> Option<(String, bool)> {
    #[derive(Deserialize)]
    struct Result {
        #[serde(rename = "type")]
        ty: String,
        tool_use_id: String,
        content: Value,
    }
    if !item.origin.is(ANTHROPIC) {
        return None;
    }
    let result: Result = item.raw.parse().ok()?;
    (result.ty == "web_search_tool_result").then(|| (result.tool_use_id, result.content.is_array()))
}

pub(crate) fn anthropic_input_delta(item: &RawItem) -> Option<String> {
    #[derive(Deserialize)]
    struct InputJsonDelta {
        #[serde(rename = "type")]
        ty: String,
        partial_json: String,
    }
    if !item.origin.is(ANTHROPIC) {
        return None;
    }
    let delta: InputJsonDelta = item.raw.parse().ok()?;
    (delta.ty == "input_json_delta").then_some(delta.partial_json)
}
