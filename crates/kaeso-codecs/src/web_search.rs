use std::collections::HashMap;

use kaeso_kpp::{
    Block, BlockDelta, BlockHeader, BlockStart, BlockStop, Delta, DialectId, GenerateRequest,
    KppError, RawItem, RawJson, StreamEvent, ToolChoice, ToolDef,
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
const ANTHROPIC_TOOLS: &[&str] = &[
    "web_search_20250305",
    "web_search_20260209",
    "web_search_20260318",
];
const ANTHROPIC_NAME: &str = "web_search";
const ACCESS: &str = "external_web_access";
const CONTENT_TYPES: &str = "search_content_types";
const LOCATION_FIELDS: &[&str] = &["type", "city", "country", "region", "timezone"];
// xAI's filters take at most five domains each, and not both lists in one request.
// https://docs.x.ai/developers/tools/web-search
const XAI_MAX_DOMAINS: usize = 5;

fn refuse(feature: &str, path: &str, target: &str, hint: &str) -> KppError {
    refuse_from(RESPONSES, feature, path, target, hint)
}

fn refuse_from(origin: DialectId, feature: &str, path: &str, target: &str, hint: &str) -> KppError {
    KppError::unsupported(
        feature,
        format!(
            "`{feature}` comes from an {} request and has no equivalent in {target}. {hint}",
            origin.display_name(),
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
            ("enable_image_understanding" | "enable_image_search", Value::Bool(on)) => {
                if *on {
                    left_out.push(at);
                }
            }
            ("filters", Value::Object(filters)) => {
                for (k, v) in filters {
                    // Anthropic's names; it takes one of the two lists.
                    // https://platform.claude.com/docs/en/agents-and-tools/tool-use/web-search-tool
                    let name = match k.as_str() {
                        "allowed_domains" => "allowed_domains",
                        "excluded_domains" => "blocked_domains",
                        _ if v.is_null() => continue,
                        _ => {
                            return Err(refuse(
                                &format!("web_search.filters.{k}"),
                                &format!("{at}.{k}"),
                                target,
                                "Remove it.",
                            ));
                        }
                    };
                    match v {
                        Value::Null => {}
                        Value::Array(domains) if domains.is_empty() => {}
                        Value::Array(_) => {
                            tool.insert(name.into(), v.clone());
                        }
                        _ => {
                            return Err(KppError::invalid_request(format!(
                                "`{at}.{k}` must be a list of domains."
                            ))
                            .with_param(format!("{at}.{k}")));
                        }
                    }
                }
                if tool.contains_key("allowed_domains") && tool.contains_key("blocked_domains") {
                    return Err(KppError::unsupported_by(
                        "web_search.filters",
                        ANTHROPIC,
                        "Claude's web search takes allowed or excluded domains, not both; send one list.",
                    )
                    .with_param(at));
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

fn is_anthropic_search(item: &RawItem) -> bool {
    item.origin.is(ANTHROPIC)
        && type_tag(item.raw.as_raw())
            .ok()
            .flatten()
            .is_some_and(|t| ANTHROPIC_TOOLS.contains(&t.as_str()))
}

fn hosted_searches(r: &GenerateRequest, origin: DialectId) -> Vec<usize> {
    let found = |item: &RawItem| match origin {
        RESPONSES => is_hosted_search(item),
        _ => is_anthropic_search(item),
    };
    r.tools
        .iter()
        .enumerate()
        .filter(|(_, t)| matches!(t, ToolDef::Unknown(item) if found(item)))
        .map(|(i, _)| i)
        .collect()
}

fn map_searches(
    d: &mut Draft<'_>,
    origin: DialectId,
    target: DialectId,
    mut map: impl FnMut(&Map<String, Value>, &str, &mut Vec<String>) -> Result<Mapped, KppError>,
) -> Result<(), KppError> {
    let found = hosted_searches(d.get(), origin);
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
        let (tool, cached) = match origin {
            RESPONSES => (TOOL, cached_only(&fields, &path)?),
            _ => (
                fields
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                false,
            ),
        };
        let out = if leaves_out_tool(origin, tool, target, cached) {
            left_out.push(path);
            Mapped::LeftOut
        } else {
            map(&fields, &path, &mut left_out)?
        };
        mapped.push((i, out));
    }
    let any_left_out = mapped.iter().any(|(_, m)| matches!(m, Mapped::LeftOut));
    if any_left_out && d.get().tool_choice.as_ref().is_some_and(forces_search) {
        return Err(KppError::unsupported_by(
            "tool_choice",
            target,
            "It asks for `web_search`, which is left out for this model. Choose `auto` or another tool.",
        )
        .with_param("tool_choice"));
    }
    let lost_all = mapped.iter().all(|(_, m)| matches!(m, Mapped::LeftOut))
        && mapped.len() == d.get().tools.len();
    if lost_all {
        match &d.get().tool_choice {
            Some(ToolChoice::Required | ToolChoice::Tool(_)) => {
                return Err(KppError::unsupported_by(
                    "tool_choice",
                    target,
                    "It asks for a tool, and the only tool, `web_search`, is left out for this model. Remove `tool_choice`.",
                )
                .with_param("tool_choice"));
            }
            Some(_) => left_out.push("tool_choice".to_owned()),
            None => {}
        }
        if d.get().parallel_tool_calls.is_some() {
            left_out.push("parallel_tool_calls".to_owned());
        }
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
    r.tools = kept;
    if lost_all {
        r.tool_choice = None;
        r.parallel_tool_calls = None;
    }
    Ok(())
}

fn forces_search(choice: &ToolChoice) -> bool {
    let ToolChoice::Unknown(item) = choice else {
        return false;
    };
    let Ok(choice) = item.raw.to_value() else {
        return false;
    };
    let is_search = |v: &Value| {
        v.get("type")
            .and_then(Value::as_str)
            .is_some_and(|t| HOSTED_SEARCH_TYPES.contains(&t))
    };
    is_search(&choice)
        || (choice.get("type").and_then(Value::as_str) == Some("allowed_tools")
            && choice
                .get("tools")
                .and_then(Value::as_array)
                .is_some_and(|tools| tools.iter().any(is_search)))
}

pub(crate) fn for_anthropic(d: &mut Draft<'_>) -> Result<(), KppError> {
    map_searches(d, RESPONSES, ANTHROPIC, |fields, path, left_out| {
        anthropic_tool(fields, path, left_out).map(Mapped::Tool)
    })?;
    without_search_calls(d, |_| true);
    Ok(())
}

pub(crate) fn for_chat(d: &mut Draft<'_>) -> Result<(), KppError> {
    map_searches(d, RESPONSES, CHAT, |_, _, _| Ok(Mapped::LeftOut))?;
    without_search_calls(d, |_| true);
    anthropic_searches(d, CHAT, |_, _| Ok(Mapped::LeftOut))?;
    without_claudes_searches(d, true);
    Ok(())
}

pub(crate) fn for_responses(d: &mut Draft<'_>, profile: ResponsesProfile) -> Result<(), KppError> {
    if profile.web_search == WebSearchTool::Xai {
        map_searches(d, RESPONSES, RESPONSES, |fields, path, _| {
            xai_tool(fields, path, profile.provider).map(Mapped::Tool)
        })?;
    }
    without_search_calls(d, |id| id.is_some_and(|id| id.starts_with(KAESO_SEARCH_ID)));
    let mut limit = None;
    anthropic_searches(d, RESPONSES, |fields, path| {
        let (fields, max_uses) = responses_fields(fields, path, profile)?;
        limit = limit.or(max_uses);
        match profile.web_search {
            WebSearchTool::Xai => xai_tool(&fields, path, profile.provider),
            _ => raw_tool(RESPONSES, fields),
        }
        .map(Mapped::Tool)
    })?;
    if let Some(n) = limit {
        let key = match profile.web_search {
            WebSearchTool::Xai => "max_turns",
            _ => "max_tool_calls",
        };
        d.set_search_limit(key, n);
    }
    without_claudes_searches(d, true);
    Ok(())
}

pub(crate) fn for_claude(d: &mut Draft<'_>) {
    without_claudes_searches(d, false);
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

const KAESO_SERVER_ID: &str = "srvtoolu_kaeso_";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResponsesSearch {
    id: String,
    completed: bool,
    query: Option<String>,
    sources: Vec<String>,
}

#[derive(Deserialize)]
struct SearchCall {
    #[serde(rename = "type")]
    ty: String,
    id: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    action: Option<SearchAction>,
}

#[derive(Deserialize)]
struct SearchAction {
    #[serde(rename = "type")]
    ty: String,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    queries: Option<Vec<String>>,
    #[serde(default)]
    sources: Option<Vec<SearchSource>>,
}

#[derive(Deserialize)]
struct SearchSource {
    #[serde(default)]
    url: Option<String>,
}

fn search_call(item: &RawItem) -> Option<SearchCall> {
    if !item.origin.is(RESPONSES) {
        return None;
    }
    let call: SearchCall = item.raw.parse().ok()?;
    (call.ty == SEARCH_CALL).then_some(call)
}

pub(crate) fn is_search_call(item: &RawItem) -> bool {
    search_call(item).is_some()
}

pub(crate) fn responses_search(item: &RawItem) -> Option<ResponsesSearch> {
    let call = search_call(item)?;
    let action = call.action.filter(|a| a.ty == "search")?;
    let completed = match call.status.as_deref() {
        Some("completed") => true,
        Some("failed") => false,
        _ => return None,
    };
    let query = action
        .query
        .or_else(|| action.queries.and_then(|q| q.into_iter().next()));
    Some(ResponsesSearch {
        id: call.id,
        completed,
        query,
        sources: action
            .sources
            .unwrap_or_default()
            .into_iter()
            .filter_map(|s| s.url)
            .collect(),
    })
}

fn anthropic_item(value: Value) -> Option<RawItem> {
    RawJson::from_serialize(&value)
        .ok()
        .map(|raw| RawItem::new(kaeso_kpp::Origin::new(ANTHROPIC), raw))
}

fn server_tool_id(search: &ResponsesSearch) -> String {
    let id: String = search
        .id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{KAESO_SERVER_ID}{id}")
}

pub(crate) fn server_tool_use(search: &ResponsesSearch, input: Value) -> Option<RawItem> {
    anthropic_item(serde_json::json!({
        "type": "server_tool_use",
        "id": server_tool_id(search),
        "name": ANTHROPIC_NAME,
        "input": input,
    }))
}

pub(crate) fn search_input(search: &ResponsesSearch) -> Value {
    match &search.query {
        Some(q) => serde_json::json!({ "query": q }),
        None => serde_json::json!({}),
    }
}

pub(crate) fn search_result(search: &ResponsesSearch) -> Option<RawItem> {
    let content = if search.completed {
        Value::Array(
            search
                .sources
                .iter()
                .map(|url| {
                    serde_json::json!({
                        "type": "web_search_result",
                        "url": url,
                        "title": url,
                        "encrypted_content": "",
                        "page_age": null,
                    })
                })
                .collect(),
        )
    } else {
        serde_json::json!({
            "type": "web_search_tool_result_error",
            "error_code": "unavailable",
        })
    };
    anthropic_item(serde_json::json!({
        "type": "web_search_tool_result",
        "tool_use_id": server_tool_id(search),
        "content": content,
    }))
}

pub(crate) fn completed_searches(blocks: &[Block]) -> u64 {
    let completed = |b: &Block| match b {
        Block::Unknown(item) => responses_search(item).is_some_and(|s| s.completed),
        _ => false,
    };
    blocks.iter().filter(|b| completed(b)).count() as u64
}

pub(crate) fn for_anthropic_reply(blocks: &[Block]) -> Option<Vec<Block>> {
    let found = |b: &Block| matches!(b, Block::Unknown(item) if responses_search(item).is_some());
    if !blocks.iter().any(found) {
        return None;
    }
    let mut out = Vec::with_capacity(blocks.len() + 1);
    for b in blocks {
        let search = match b {
            Block::Unknown(item) => responses_search(item),
            _ => None,
        };
        let Some(search) = search else {
            out.push(b.clone());
            continue;
        };
        out.extend(server_tool_use(&search, search_input(&search)).map(Block::Unknown));
        out.extend(search_result(&search).map(Block::Unknown));
    }
    Some(out)
}

#[derive(Default)]
pub(crate) struct ReplySearches {
    index: HashMap<u32, u32>,
    next: u32,
    held: HashMap<u32, RawItem>,
    completed: u64,
}

impl ReplySearches {
    fn index(&mut self, upstream: u32) -> u32 {
        if let Some(i) = self.index.get(&upstream) {
            return *i;
        }
        let i = self.fresh();
        self.index.insert(upstream, i);
        i
    }

    fn fresh(&mut self) -> u32 {
        let i = self.next;
        self.next = self.next.saturating_add(1);
        i
    }

    pub(crate) fn completed(&self) -> u64 {
        self.completed
    }

    fn found(&mut self, item: &RawItem) -> Vec<StreamEvent> {
        let Some(search) = responses_search(item) else {
            return Vec::new();
        };
        if search.completed {
            self.completed += 1;
        }
        let mut out = Vec::new();
        if let Some(start) = server_tool_use(&search, Value::Object(Map::new())) {
            let at = self.fresh();
            out.push(StreamEvent::BlockStart(BlockStart::new(
                at,
                BlockHeader::Unknown(start),
            )));
            let input = search_input(&search);
            if let Some(delta) = (input != Value::Object(Map::new()))
                .then(|| {
                    anthropic_item(serde_json::json!({
                        "type": "input_json_delta",
                        "partial_json": input.to_string(),
                    }))
                })
                .flatten()
            {
                out.push(StreamEvent::BlockDelta(BlockDelta::new(
                    at,
                    Delta::Unknown(delta),
                )));
            }
            out.push(StreamEvent::BlockStop(BlockStop::new(at)));
        }
        if let Some(result) = search_result(&search) {
            let at = self.fresh();
            out.push(StreamEvent::BlockStart(BlockStart::new(
                at,
                BlockHeader::Unknown(result),
            )));
            out.push(StreamEvent::BlockStop(BlockStop::new(at)));
        }
        out
    }

    pub(crate) fn map(&mut self, event: &StreamEvent) -> Option<Vec<StreamEvent>> {
        let upstream = match event {
            StreamEvent::BlockStart(s) => {
                if let BlockHeader::Unknown(item) = &s.block
                    && is_search_call(item)
                {
                    self.held.insert(s.index, item.clone());
                    return Some(Vec::new());
                }
                s.index
            }
            StreamEvent::BlockDelta(d) => {
                if let Some(held) = self.held.get_mut(&d.index) {
                    if let Delta::Unknown(item) = &d.delta
                        && is_search_call(item)
                    {
                        *held = item.clone();
                    }
                    return Some(Vec::new());
                }
                d.index
            }
            StreamEvent::BlockStop(s) => {
                if let Some(item) = self.held.remove(&s.index) {
                    return Some(self.found(&item));
                }
                s.index
            }
            _ => return None,
        };
        let at = self.index(upstream);
        if at == upstream {
            return None;
        }
        Some(vec![match event.clone() {
            StreamEvent::BlockStart(mut s) => {
                s.index = at;
                StreamEvent::BlockStart(s)
            }
            StreamEvent::BlockDelta(mut d) => {
                d.index = at;
                StreamEvent::BlockDelta(d)
            }
            StreamEvent::BlockStop(mut s) => {
                s.index = at;
                StreamEvent::BlockStop(s)
            }
            other => other,
        }])
    }
}

fn anthropic_searches(
    d: &mut Draft<'_>,
    target: DialectId,
    mut map: impl FnMut(&Map<String, Value>, &str) -> Result<Mapped, KppError>,
) -> Result<(), KppError> {
    let r = d.get();
    if hosted_searches(r, ANTHROPIC).is_empty() {
        return Ok(());
    }
    let named_tool = r
        .tools
        .iter()
        .any(|t| matches!(t, ToolDef::Function(f) if f.name == ANTHROPIC_NAME));
    if !named_tool && matches!(&r.tool_choice, Some(ToolChoice::Tool(n)) if n == ANTHROPIC_NAME) {
        return Err(refuse_from(
            ANTHROPIC,
            "tool_choice `web_search`",
            "tool_choice",
            target.display_name(),
            "Use `auto` or `any` instead, so the model decides when to search.",
        ));
    }
    map_searches(d, ANTHROPIC, target, |fields, path, _| map(fields, path))
}

fn default_callers(version: &str) -> &'static str {
    match version {
        ANTHROPIC_TOOL => "direct",
        _ => "code_execution_20260120",
    }
}

fn responses_fields(
    fields: &Map<String, Value>,
    path: &str,
    profile: ResponsesProfile,
) -> Result<(Map<String, Value>, Option<u64>), KppError> {
    let target = RESPONSES.display_name();
    let provider = profile.provider;
    let xai = profile.web_search == WebSearchTool::Xai;
    let version = fields
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let set = |key: &str| fields.get(key).is_some_and(|v| !v.is_null());
    if set("allowed_domains") && set("blocked_domains") {
        let at = path.replace('[', ".").replace(']', "");
        return Err(KppError::invalid_request(format!(
            "{at}.{version}: Only one of `allowed_domains` and `blocked_domains` can be provided."
        ))
        .with_param(path));
    }
    let mut tool = Map::new();
    tool.insert("type".into(), TOOL.into());
    let mut filters = Map::new();
    let mut limit = None;
    for (key, value) in fields {
        let at = format!("{path}.{key}");
        match (key.as_str(), value) {
            ("type" | "cache_control", _) | (_, Value::Null) => {}
            ("name", Value::String(n)) if n == ANTHROPIC_NAME => {}
            ("allowed_callers", Value::Array(callers)) if matches!(callers.as_slice(), [Value::String(c)] if c == "direct" || c == default_callers(version)) =>
                {}
            ("response_inclusion", Value::String(r))
                if version == "web_search_20260318" && (r == "full" || r == "excluded") => {}
            ("allowed_domains" | "blocked_domains", Value::Array(domains)) => {
                if xai && domains.len() > XAI_MAX_DOMAINS {
                    return Err(KppError::unsupported(
                        format!("web_search.{key}"),
                        format!(
                            "{provider}'s web search takes a list of at most {XAI_MAX_DOMAINS} domains in `{at}`; send fewer."
                        ),
                    )
                    .with_param(at));
                }
                let name = match (key.as_str(), xai) {
                    ("blocked_domains", true) => "excluded_domains",
                    _ => key.as_str(),
                };
                filters.insert(name.into(), value.clone());
            }
            ("max_uses", Value::Number(n)) if n.as_u64().is_some_and(|n| n > 0) => {
                limit = n.as_u64();
            }
            ("user_location", Value::Object(_)) if xai => {
                return Err(KppError::unsupported(
                    "web_search.user_location",
                    format!("{provider}'s web search has no `user_location`. Remove it."),
                )
                .with_param(at));
            }
            ("user_location", Value::Object(location)) => {
                let mut out = Map::new();
                for (k, v) in location {
                    if v.is_null() {
                        continue;
                    }
                    let approximate = k != "type" || v.as_str() == Some("approximate");
                    if !LOCATION_FIELDS.contains(&k.as_str()) || !approximate {
                        return Err(refuse_from(
                            ANTHROPIC,
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
                return Err(refuse_from(
                    ANTHROPIC,
                    &format!("web_search.{key}"),
                    &at,
                    target,
                    "Remove it, or send the request to a model served through its original format.",
                ));
            }
        }
    }
    filters.retain(|_, v| v.as_array().is_some_and(|a| !a.is_empty()));
    if !filters.is_empty() {
        tool.insert("filters".into(), Value::Object(filters));
    }
    Ok((tool, limit))
}

pub(crate) const SOURCES: &str = "web_search_call.action.sources";

pub(crate) fn without_search_limit(tools: &mut [ToolDef]) {
    for t in tools {
        if let ToolDef::Unknown(item) = t
            && is_anthropic_search(item)
            && let Ok(Value::Object(mut fields)) = item.raw.to_value()
            && fields.remove("max_uses").is_some()
            && let Ok(raw) = RawJson::from_serialize(&Value::Object(fields))
        {
            item.raw = raw;
        }
    }
}

pub(crate) fn asks_for_sources(r: &GenerateRequest) -> bool {
    !r.extras.is_from(RESPONSES) && !hosted_searches(r, RESPONSES).is_empty()
}

fn claudes_search(block: &Block) -> Option<(String, String)> {
    #[derive(Deserialize)]
    struct Ids {
        #[serde(rename = "type")]
        ty: String,
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        tool_use_id: Option<String>,
    }
    let Block::Unknown(item) = block else {
        return None;
    };
    if !item.origin.is(ANTHROPIC) {
        return None;
    }
    let ids = item.raw.parse::<Ids>().ok()?;
    let id = match ids.ty.as_str() {
        "server_tool_use" if ids.name.as_deref() == Some(ANTHROPIC_NAME) => ids.id,
        "web_search_tool_result" => ids.tool_use_id,
        _ => None,
    }?;
    Some((ids.ty, id))
}

fn cites_search(c: &kaeso_kpp::Citation) -> bool {
    c.origin.is(ANTHROPIC)
        && type_tag(c.raw.as_raw()).ok().flatten().as_deref() == Some("web_search_result_location")
}

fn without_claudes_searches(d: &mut Draft<'_>, all: bool) {
    let dropped =
        |b: &Block| claudes_search(b).is_some_and(|(_, id)| all || id.starts_with(KAESO_SERVER_ID));
    let cited =
        |b: &Block| all && matches!(b, Block::Text(t) if t.citations.iter().any(cites_search));
    let mut left_out = Vec::new();
    for (mi, m) in d.get().messages.iter().enumerate() {
        for (bi, b) in m.blocks.iter().enumerate() {
            if let Some((ty, id)) = claudes_search(b).filter(|_| dropped(b)) {
                left_out.push(format!("{ty} {id}"));
            } else if cited(b) {
                left_out.push(format!("messages[{mi}].blocks[{bi}].citations"));
            }
        }
    }
    if left_out.is_empty() {
        return;
    }
    for item in left_out {
        d.leave_out(item);
    }
    d.edit().messages.retain_mut(|m| {
        let held = m.blocks.len();
        m.blocks.retain(|b| !dropped(b));
        for b in &mut m.blocks {
            if let Block::Text(t) = b
                && all
            {
                t.citations.retain(|c| !cites_search(c));
            }
        }
        m.blocks.len() == held || !m.blocks.is_empty()
    });
}
