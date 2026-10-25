use kaeso_kpp::{
    Block, DialectId, GenerateRequest, KppError, RawItem, RawJson, ToolChoice, ToolDef,
};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::wire::type_tag;

const RESPONSES: DialectId = DialectId::OpenAiResponses;
const ANTHROPIC: DialectId = DialectId::AnthropicMessages;

const SEARCH_CALL: &str = "web_search_call";
const KAESO_SEARCH_ID: &str = "ws_kaeso_";
const ANTHROPIC_TOOL: &str = "web_search_20250305";
const ANTHROPIC_NAME: &str = "web_search";
const ACCESS: &str = "external_web_access";
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

fn has_hosted_search(r: &GenerateRequest) -> bool {
    r.tools
        .iter()
        .any(|t| matches!(t, ToolDef::Unknown(item) if is_hosted_search(item)))
}

fn has_search_calls(r: &GenerateRequest) -> bool {
    r.messages
        .iter()
        .any(|m| m.blocks.iter().any(|b| search_call_id(b).is_some()))
}

pub(crate) fn for_anthropic(r: &GenerateRequest) -> Result<Option<GenerateRequest>, KppError> {
    if !has_hosted_search(r) && !has_search_calls(r) {
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

fn without_access(item: &RawItem, path: &str) -> Result<Option<ToolDef>, KppError> {
    let Ok(Value::Object(mut fields)) = item.raw.to_value() else {
        return Ok(None);
    };
    match fields.get(ACCESS) {
        None => return Ok(None),
        Some(Value::Bool(true) | Value::Null) => {}
        Some(_) => {
            return Err(KppError::unsupported(
                "web_search.external_web_access",
                format!(
                    "`external_web_access: false` asks for cached pages only, and this model's web search always reads live results. {CODEX_HINT}"
                ),
            )
            .with_param(format!("{path}.{ACCESS}")));
        }
    }
    fields.remove(ACCESS);
    let raw = RawJson::from_serialize(&Value::Object(fields))
        .map_err(|e| KppError::invalid_request(format!("`web_search` cannot be sent: {e}.")))?;
    Ok(Some(ToolDef::Unknown(RawItem::new(
        item.origin.clone(),
        raw,
    ))))
}

pub(crate) fn for_responses(
    r: &GenerateRequest,
    takes_access: bool,
) -> Result<Option<GenerateRequest>, KppError> {
    let ours = |id: Option<&str>| id.is_some_and(|id| id.starts_with(KAESO_SEARCH_ID));
    let mut out = (!takes_access && has_hosted_search(r)).then(|| r.clone());
    for (i, t) in out.iter_mut().flat_map(|o| o.tools.iter_mut()).enumerate() {
        if let ToolDef::Unknown(item) = t
            && is_hosted_search(item)
            && let Some(tool) = without_access(item, &format!("tools[{i}]"))?
        {
            *t = tool;
        }
    }
    let any = r.messages.iter().any(|m| {
        m.blocks
            .iter()
            .any(|b| search_call_id(b).is_some_and(|id| ours(id.as_deref())))
    });
    if any {
        without_search_calls(out.get_or_insert_with(|| r.clone()), ours);
    }
    Ok(out)
}

pub(crate) fn for_chat(r: &GenerateRequest) -> Result<Option<GenerateRequest>, KppError> {
    if !has_hosted_search(r) && !has_search_calls(r) {
        return Ok(None);
    }
    let mut out = r.clone();
    out.tools
        .retain(|t| !matches!(t, ToolDef::Unknown(item) if is_hosted_search(item)));
    if out.tools.is_empty() && !r.tools.is_empty() {
        match out.tool_choice {
            Some(ToolChoice::Required) => {
                return Err(KppError::unsupported_by(
                    "tool_choice `required`",
                    DialectId::OpenAiChat,
                    "This model has no hosted web search, and `web_search` is the only tool. Remove `tool_choice`.",
                )
                .with_param("tool_choice"));
            }
            Some(ToolChoice::Auto | ToolChoice::None) => out.tool_choice = None,
            _ => {}
        }
        out.parallel_tool_calls = None;
    }
    without_search_calls(&mut out, |_| true);
    Ok(Some(out))
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
