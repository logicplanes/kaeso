use kaeso_kpp::{
    Block, DialectId, GenerateRequest, KppError, RawItem, RawJson, ToolChoice, ToolDef,
};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::adapt::Draft;
use crate::cross::{HOSTED_SEARCH_TYPES, leaves_out_tool};
use crate::openai_responses::{ResponsesProfile, WebSearchTool};
use crate::wire::type_tag;

const RESPONSES: DialectId = DialectId::OpenAiResponses;
const ANTHROPIC: DialectId = DialectId::AnthropicMessages;
const CHAT: DialectId = DialectId::OpenAiChat;

const TOOL: &str = "web_search";
const SEARCH_CALL: &str = "web_search_call";
const KAESO_SEARCH_ID: &str = "ws_kaeso_";
const ANTHROPIC_TOOL: &str = "web_search_20250305";
const ANTHROPIC_NAME: &str = "web_search";
const ACCESS: &str = "external_web_access";
const CONTENT_TYPES: &str = "search_content_types";
const LOCATION_FIELDS: &[&str] = &["type", "city", "country", "region", "timezone"];
// xAI's filters take at most five domains each, and not both lists in one request.
// https://docs.x.ai/developers/tools/web-search
const XAI_MAX_DOMAINS: usize = 5;

fn refuse(feature: &str, path: &str, target: &str, hint: &str) -> KppError {
    KppError::unsupported(
        feature,
        format!(
            "`{feature}` comes from an {} request and has no equivalent in {target}. {hint}",
            RESPONSES.display_name(),
        ),
    )
    .with_param(path)
}

fn is_hosted_search(item: &RawItem) -> bool {
    item.origin.is(RESPONSES)
        && type_tag(item.raw.as_raw())
            .ok()
            .flatten()
            .is_some_and(|t| HOSTED_SEARCH_TYPES.contains(&t.as_str()))
}

fn fields_of(item: &RawItem, path: &str) -> Result<Map<String, Value>, KppError> {
    match item.raw.to_value() {
        Ok(Value::Object(fields)) => Ok(fields),
        _ => Err(KppError::invalid_request(format!(
            "`{path}` must be a `web_search` tool object."
        ))
        .with_param(path)),
    }
}

fn cached_only(fields: &Map<String, Value>, path: &str) -> Result<bool, KppError> {
    match fields.get(ACCESS) {
        None | Some(Value::Null | Value::Bool(true)) => Ok(false),
        Some(Value::Bool(false)) => Ok(true),
        Some(_) => Err(KppError::invalid_request(format!(
            "`{path}.{ACCESS}` must be `true` or `false`."
        ))
        .with_param(format!("{path}.{ACCESS}"))),
    }
}

fn asks_for_images(value: &Value, path: &str) -> Result<bool, KppError> {
    let types = match value {
        Value::Null => return Ok(false),
        Value::Array(types) => types,
        _ => {
            return Err(KppError::invalid_request(format!(
                "`{path}` must be a list of content types."
            ))
            .with_param(path));
        }
    };
    let mut images = false;
    for t in types {
        match t.as_str() {
            Some("text") => {}
            Some("image") => images = true,
            _ => {
                return Err(KppError::invalid_request(format!(
                    "`{path}` holds `text` and `image` only."
                ))
                .with_param(path));
            }
        }
    }
    Ok(images)
}

fn raw_tool(origin: DialectId, tool: Map<String, Value>) -> Result<ToolDef, KppError> {
    let raw = RawJson::from_serialize(&Value::Object(tool))
        .map_err(|e| KppError::invalid_request(format!("`web_search` cannot be sent: {e}.")))?;
    Ok(ToolDef::Unknown(RawItem::new(
        kaeso_kpp::Origin::new(origin),
        raw,
    )))
}

enum Mapped {
    Tool(ToolDef),
    LeftOut,
}

fn anthropic_tool(
    fields: &Map<String, Value>,
    path: &str,
    left_out: &mut Vec<String>,
) -> Result<ToolDef, KppError> {
    let target = ANTHROPIC.display_name();
    let mut tool = Map::new();
    tool.insert("type".into(), ANTHROPIC_TOOL.into());
    tool.insert("name".into(), ANTHROPIC_NAME.into());
    for (key, value) in fields {
        let at = format!("{path}.{key}");
        match (key.as_str(), value) {
            ("type" | ACCESS, _) | (_, Value::Null) => {}
            ("search_context_size", Value::String(s)) if s == "medium" => {}
            (CONTENT_TYPES, v) => {
                if asks_for_images(v, &at)? {
                    left_out.push(at);
                }
            }
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
                                target,
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
                            target,
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
                return Err(refuse(
                    &format!("web_search.{key}"),
                    &at,
                    target,
                    "Remove it.",
                ));
            }
        }
    }
    raw_tool(ANTHROPIC, tool)
}

fn xai_domains(value: &Value, at: &str, provider: &str) -> Result<Option<Value>, KppError> {
    match value {
        Value::Null => Ok(None),
        Value::Array(domains) if domains.is_empty() => Ok(None),
        Value::Array(domains) if domains.len() <= XAI_MAX_DOMAINS => Ok(Some(value.clone())),
        _ => Err(KppError::unsupported(
            "web_search.filters",
            format!(
                "{provider}'s web search takes a list of at most {XAI_MAX_DOMAINS} domains in `{at}`; send fewer."
            ),
        )
        .with_param(at)),
    }
}

// Fields from https://docs.x.ai/developers/tools/web-search: `filters.allowed_domains`,
// `filters.excluded_domains`, `enable_image_understanding` and `enable_image_search`.
fn xai_tool(fields: &Map<String, Value>, path: &str, provider: &str) -> Result<ToolDef, KppError> {
    let unknown = |key: &str, at: &str| {
        KppError::unsupported(
            format!("web_search.{key}"),
            format!("{provider}'s web search has no `{key}`. Remove it."),
        )
        .with_param(at)
    };
    let mut tool = Map::new();
    tool.insert("type".into(), TOOL.into());
    let mut images = false;
    for (key, value) in fields {
        let at = format!("{path}.{key}");
        match (key.as_str(), value) {
            ("type" | ACCESS, _) | (_, Value::Null) => {}
            ("search_context_size", Value::String(s)) if s == "medium" => {}
            (CONTENT_TYPES, v) => images = asks_for_images(v, &at)?,
            ("enable_image_understanding" | "enable_image_search", Value::Bool(_)) => {
                tool.insert(key.clone(), value.clone());
            }
            ("filters", Value::Object(filters)) => {
                let mut out = Map::new();
                for (k, v) in filters {
                    let at = format!("{at}.{k}");
                    match k.as_str() {
                        "allowed_domains" | "excluded_domains" => {
                            if let Some(domains) = xai_domains(v, &at, provider)? {
                                out.insert(k.clone(), domains);
                            }
                        }
                        _ if v.is_null() => {}
                        _ => return Err(unknown(&format!("filters.{k}"), &at)),
                    }
                }
                if out.len() > 1 {
                    return Err(KppError::unsupported(
                        "web_search.filters",
                        format!(
                            "{provider}'s web search takes `allowed_domains` or `excluded_domains`, not both; send one."
                        ),
                    )
                    .with_param(at));
                }
                if !out.is_empty() {
                    tool.insert("filters".into(), Value::Object(out));
                }
            }
            _ => return Err(unknown(key, &at)),
        }
    }
    if images && !tool.contains_key("enable_image_search") {
        tool.insert("enable_image_search".into(), true.into());
    }
    raw_tool(RESPONSES, tool)
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

fn has_search_calls(r: &GenerateRequest, drop: &impl Fn(Option<&str>) -> bool) -> bool {
    r.messages.iter().any(|m| {
        m.blocks
            .iter()
            .any(|b| search_call_id(b).is_some_and(|id| drop(id.as_deref())))
    })
}

fn without_search_calls(d: &mut Draft<'_>, drop: impl Fn(Option<&str>) -> bool) {
    if !has_search_calls(d.get(), &drop) {
        return;
    }
    let dropped: Vec<String> = d
        .get()
        .messages
        .iter()
        .flat_map(|m| &m.blocks)
        .filter_map(search_call_id)
        .filter(|id| drop(id.as_deref()))
        .map(|id| match id {
            Some(id) => format!("{SEARCH_CALL} {id}"),
            None => SEARCH_CALL.to_owned(),
        })
        .collect();
    for item in dropped {
        d.leave_out(item);
    }
    d.edit().messages.retain_mut(|m| {
        let held = m.blocks.len();
        m.blocks
            .retain(|b| search_call_id(b).is_none_or(|id| !drop(id.as_deref())));
        m.blocks.len() == held || !m.blocks.is_empty()
    });
}

fn hosted_searches(r: &GenerateRequest) -> Vec<usize> {
    r.tools
        .iter()
        .enumerate()
        .filter(|(_, t)| matches!(t, ToolDef::Unknown(item) if is_hosted_search(item)))
        .map(|(i, _)| i)
        .collect()
}

fn map_searches(
    d: &mut Draft<'_>,
    target: DialectId,
    mut map: impl FnMut(&Map<String, Value>, &str, &mut Vec<String>) -> Result<Mapped, KppError>,
) -> Result<(), KppError> {
    let found = hosted_searches(d.get());
    if found.is_empty() {
        return Ok(());
    }
    let mut mapped = Vec::with_capacity(found.len());
    let mut left_out = Vec::new();
    for &i in &found {
        let path = format!("tools[{i}]");
        let Some(ToolDef::Unknown(item)) = d.get().tools.get(i) else {
            continue;
        };
        let fields = fields_of(item, &path)?;
        let cached = cached_only(&fields, &path)?;
        let out = if leaves_out_tool(RESPONSES, TOOL, target, cached) {
            left_out.push(path);
            Mapped::LeftOut
        } else {
            map(&fields, &path, &mut left_out)?
        };
        mapped.push((i, out));
    }
    for path in left_out {
        d.leave_out(path);
    }
    let r = d.edit();
    let mut kept = Vec::with_capacity(r.tools.len());
    for (i, tool) in std::mem::take(&mut r.tools).into_iter().enumerate() {
        match mapped.iter().position(|(at, _)| *at == i) {
            None => kept.push(tool),
            Some(k) => {
                if let (_, Mapped::Tool(t)) = mapped.swap_remove(k) {
                    kept.push(t);
                }
            }
        }
    }
    let lost_all = kept.is_empty();
    r.tools = kept;
    if lost_all {
        match r.tool_choice {
            Some(ToolChoice::Required) => {
                return Err(KppError::unsupported_by(
                    "tool_choice `required`",
                    target,
                    "The only tool, `web_search`, is left out for this model. Remove `tool_choice`.",
                )
                .with_param("tool_choice"));
            }
            Some(ToolChoice::Auto | ToolChoice::None) => r.tool_choice = None,
            _ => {}
        }
        r.parallel_tool_calls = None;
    }
    Ok(())
}

pub(crate) fn for_anthropic(d: &mut Draft<'_>) -> Result<(), KppError> {
    map_searches(d, ANTHROPIC, |fields, path, left_out| {
        anthropic_tool(fields, path, left_out).map(Mapped::Tool)
    })?;
    without_search_calls(d, |_| true);
    Ok(())
}

pub(crate) fn for_chat(d: &mut Draft<'_>) -> Result<(), KppError> {
    map_searches(d, CHAT, |_, _, _| Ok(Mapped::LeftOut))?;
    without_search_calls(d, |_| true);
    Ok(())
}

pub(crate) fn for_responses(d: &mut Draft<'_>, profile: ResponsesProfile) -> Result<(), KppError> {
    if profile.web_search == WebSearchTool::Xai {
        map_searches(d, RESPONSES, |fields, path, _| {
            xai_tool(fields, path, profile.provider).map(Mapped::Tool)
        })?;
    }
    without_search_calls(d, |id| id.is_some_and(|id| id.starts_with(KAESO_SEARCH_ID)));
    Ok(())
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
