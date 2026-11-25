//! What each client's records mean. Every reader takes what its client documents (see the docs'
//! setup guides for the sources) and turns it into observed requests and tool calls; a record it
//! doesn't know is left alone, and one it can't use is counted as refused with the reason.

use std::collections::HashMap;
use std::time::Duration;

use kaeso_core::sources::{Counted, InputCounts, OutputCounts};
use kaeso_kpp::DialectId;
use rust_decimal::Decimal;
use serde_json::Value as Json;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use super::content::{self, Exchange};
use super::otlp::{Attrs, Group, LogRecord, Span, Value};
use crate::services::observed::{
    Batch, Carried, ObservedRequest, ObservedToolCall, Refusal, UNNAMED_PROVIDER, Want, Wanted,
};

/// What clients write where they leave a value out because content logging is off.
const REDACTED: &[&str] = &["<REDACTED>", "[REDACTED]", "REDACTED"];

fn text(v: Option<&str>) -> Option<String> {
    v.map(str::trim)
        .filter(|s| !s.is_empty() && !REDACTED.contains(s))
        .map(str::to_owned)
}

// A client that reports cost as a double (Claude Code's `cost_usd`) gets it read back from its
// shortest decimal form, which is what the client printed; Kaeso never computes with the float.
fn decimal(v: Option<&Value>) -> Option<Decimal> {
    v.and_then(Value::as_decimal_text)
        .and_then(|t| t.parse::<Decimal>().ok())
        .filter(|d| !d.is_sign_negative())
}

fn ms(v: Option<u64>) -> Duration {
    Duration::from_millis(v.unwrap_or(0))
}

fn at_nanos(n: u64) -> Option<OffsetDateTime> {
    (n > 0)
        .then(|| OffsetDateTime::from_unix_timestamp_nanos(i128::from(n)).ok())
        .flatten()
}

fn at_rfc3339(s: Option<&str>) -> Option<OffsetDateTime> {
    s.and_then(|s| OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok())
}

/// When a record's request ended: the record's own time, its `event.timestamp`, or when the
/// exporter saw it.
fn ended(r: &LogRecord) -> Option<OffsetDateTime> {
    at_nanos(r.time_unix_nano)
        .or_else(|| at_rfc3339(r.attributes.str("event.timestamp")))
        .or_else(|| at_nanos(r.observed_time_unix_nano))
}

fn started(end: Option<OffsetDateTime>, duration: Duration) -> Option<OffsetDateTime> {
    end.map(|e| e - duration)
}

/// An id for a record that names none of its own: the same record hashes the same.
fn hashed(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p.as_bytes());
        h.update([0]);
    }
    let d = h.finalize();
    d.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

fn status_of(v: Option<&Value>) -> Option<u16> {
    v.and_then(Value::as_u64)
        .and_then(|s| u16::try_from(s).ok())
        .filter(|s| (100..=599).contains(s))
}

fn looks_like_a_name(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_')
}

/// The record's event name with its client's prefix, whichever field carried it: the
/// `event.name` attribute first, as every client here writes one; Codex's `eventName` field is the
/// source line that logged it, and Claude Code's attribute lacks the prefix its body has.
fn event_name(scope: &str, r: &LogRecord) -> String {
    let body = r.body.as_ref().and_then(Value::as_str).unwrap_or_default();
    let attr = r.attributes.str("event.name").unwrap_or_default();
    let candidates = [attr, body, r.event_name.as_str()];
    let name = candidates
        .into_iter()
        .find(|n| looks_like_a_name(n) && n.contains('.'))
        .or_else(|| candidates.into_iter().find(|n| looks_like_a_name(n)))
        .unwrap_or_default();
    if scope.starts_with("com.anthropic.claude_code") && !name.starts_with("claude_code.") {
        format!("claude_code.{name}")
    } else {
        name.to_owned()
    }
}

/// A client as a user agent, so the request log names it as it names the client's own calls.
fn client(name: &str, version: Option<&str>) -> String {
    match version {
        Some(v) if !v.is_empty() => format!("{name}/{v}"),
        _ => name.to_owned(),
    }
}

fn want(what: Wanted, key: String) -> Want {
    Want { what, key }
}

fn leave(batch: &mut Batch, what: Wanted, key: String, value: String) {
    batch.carried.push(Carried { what, key, value });
}

fn refuse(batch: &mut Batch, why: Refusal) {
    batch.refused += 1;
    batch.refusal = Some(why);
}

/// Splits an MCP tool name as Claude Code writes it: `mcp__<server>__<tool>`.
fn mcp_name(name: &str) -> Option<(String, String)> {
    let rest = name.strip_prefix("mcp__")?;
    let (server, tool) = rest.split_once("__")?;
    (!server.is_empty() && !tool.is_empty()).then(|| (server.to_owned(), tool.to_owned()))
}

// Claude Code ------------------------------------------------------------------------------

/// Claude Code's provider by its `anthropic.provider` attribute.
fn claude_provider(a: &Attrs) -> &'static str {
    match a.str("anthropic.provider") {
        Some("bedrock") => "amazon-bedrock",
        Some("vertex") => "google",
        Some("foundry") => "azure",
        _ => "anthropic",
    }
}

#[derive(Default)]
struct ClaudeContent {
    prompts: HashMap<String, String>,
    replies: HashMap<String, String>,
    /// Prompts a request of this batch already went with, or looked for.
    asked: std::collections::HashSet<String>,
}

fn claude_code(groups: &[Group<LogRecord>], batch: &mut Batch) {
    let mut content = ClaudeContent::default();
    for g in groups {
        for r in &g.items {
            let a = &r.attributes;
            match event_name(&g.scope, r).as_str() {
                "claude_code.user_prompt" => {
                    if let (Some(id), Some(p)) = (
                        a.str("prompt.id"),
                        text(a.str("prompt_text").or(a.str("prompt"))),
                    ) {
                        content.prompts.insert(id.to_owned(), p);
                    }
                }
                "claude_code.assistant_response" => {
                    if let (Some(id), Some(t)) = (a.str("request_id"), text(a.str("response"))) {
                        content.replies.insert(id.to_owned(), t);
                    }
                }
                _ => {}
            }
        }
    }
    for g in groups {
        let version = g.resource.str("service.version");
        for r in &g.items {
            let a = &r.attributes;
            let name = event_name(&g.scope, r);
            match name.as_str() {
                "claude_code.api_request" | "claude_code.api_error" => {
                    let Some(model) = a.str("model") else {
                        refuse(batch, Refusal::NoModel);
                        continue;
                    };
                    let duration = ms(a.u64("duration_ms"));
                    let error = name == "claude_code.api_error";
                    let request_id = a.str("request_id");
                    let external_id = match (request_id, a.str("client_request_id")) {
                        (Some(id), _) if !error => id.to_owned(),
                        (_, Some(c)) if error => {
                            format!("{c}:{}", a.u64("attempt").unwrap_or(1))
                        }
                        (Some(id), _) => format!("{id}:error"),
                        _ => hashed(&[
                            &name,
                            a.str("session.id").unwrap_or_default(),
                            &a.u64("event.sequence").unwrap_or(0).to_string(),
                            &r.time_unix_nano.to_string(),
                        ]),
                    };
                    let tokens = Counted {
                        input: a.u64("input_tokens").unwrap_or(0),
                        output: a.u64("output_tokens").unwrap_or(0),
                        cache_read: a.u64("cache_read_tokens").unwrap_or(0),
                        cache_write: a.u64("cache_creation_tokens").unwrap_or(0),
                        reasoning: 0,
                        cache_write_1h: a.u64("cache_creation_1h_tokens").unwrap_or(0),
                    }
                    .normalized(InputCounts::Uncached, OutputCounts::WithReasoning);
                    let end = ended(r);
                    // A prompt goes with the first request it led to, not copied into each; the
                    // client may have exported it in the batch before.
                    let prompt_id = a.str("prompt.id");
                    let prompt = prompt_id.and_then(|p| content.prompts.remove(p));
                    let wants = match (prompt_id, &prompt) {
                        (Some(id), None) if !content.asked.contains(id) => {
                            vec![want(Wanted::Prompt, format!("claude_code:{id}"))]
                        }
                        _ => Vec::new(),
                    };
                    if let Some(id) = prompt_id {
                        content.asked.insert(id.to_owned());
                    }
                    batch.requests.push(ObservedRequest {
                        external_id,
                        started_at: started(end, duration),
                        duration,
                        ttfb: a.u64("ttft_ms").map(Duration::from_millis),
                        provider: Some(claude_provider(a).to_owned()),
                        provider_guessed: true,
                        reasoning_effort: a.str("effort").map(str::to_owned),
                        model: model.to_owned(),
                        status: if error {
                            status_of(a.get("status_code")).unwrap_or(502)
                        } else {
                            200
                        },
                        error_message: if error {
                            text(a.str("error")).or(Some("The request failed.".into()))
                        } else {
                            None
                        },
                        tokens,
                        reported_cost: decimal(a.get("cost_usd")),
                        stream: true,
                        client: Some(client("claude-cli", version)),
                        email: a.str("user.email").map(str::to_owned),
                        tags: Vec::new(),
                        provider_request_id: request_id.map(str::to_owned),
                        dialect: Some(DialectId::AnthropicMessages),
                        prompt,
                        reply: request_id.and_then(|id| content.replies.remove(id)),
                        wants,
                        ..ObservedRequest::default()
                    });
                }
                "claude_code.tool_result" => {
                    let details = a
                        .str("tool_parameters")
                        .and_then(|p| serde_json::from_str::<Json>(p).ok());
                    let named = details.as_ref().and_then(|d| {
                        Some((
                            d.get("mcp_server_name")?.as_str()?.to_owned(),
                            d.get("mcp_tool_name")?.as_str()?.to_owned(),
                        ))
                    });
                    // Without OTEL_LOG_TOOL_DETAILS Claude Code hides an MCP tool's names: it
                    // says `mcp_tool` and the server's scope, and the call is kept unnamed.
                    let unnamed = a.str("tool_name") == Some("mcp_tool")
                        || a.str("mcp_server_scope").is_some();
                    let Some((server, tool)) = named
                        .map(|(s, t)| (Some(s), t))
                        .or_else(|| {
                            a.str("tool_name")
                                .and_then(mcp_name)
                                .map(|(s, t)| (Some(s), t))
                        })
                        .or_else(|| unnamed.then(|| (None, "mcp_tool".to_owned())))
                    else {
                        continue;
                    };
                    let duration = ms(a.u64("duration_ms"));
                    let ok = a.bool("success").unwrap_or(true);
                    batch.tool_calls.push(ObservedToolCall {
                        external_id: a.str("tool_use_id").map_or_else(
                            || hashed(&[&name, &r.time_unix_nano.to_string(), &tool]),
                            str::to_owned,
                        ),
                        started_at: started(ended(r), duration),
                        duration,
                        tool,
                        server,
                        ok,
                        error_type: (!ok)
                            .then(|| a.str("error_type").map(str::to_owned))
                            .flatten(),
                        error_message: (!ok).then(|| text(a.str("error"))).flatten(),
                        client: Some("claude-code".into()),
                        email: a.str("user.email").map(str::to_owned),
                        arguments: text(a.str("tool_input")),
                        result: None,
                        aliases: Vec::new(),
                    });
                }
                _ => {}
            }
        }
    }
    for (id, prompt) in content.prompts {
        if !content.asked.contains(&id) {
            leave(batch, Wanted::Prompt, format!("claude_code:{id}"), prompt);
        }
    }
}

// Codex -------------------------------------------------------------------------------------

/// Kaeso's provider for the name Codex gives the provider it was configured with: its own
/// providers' display names, or the `name` a person gave a provider of their own, read as an id.
fn codex_provider(name: &str) -> Option<String> {
    let id = match name {
        "OpenAI" => "openai",
        "Amazon Bedrock" | "Amazon Bedrock Runtime" => "amazon-bedrock",
        "Google Cloud Vertex AI" => "gemini-agent-platform",
        // Codex names both Ollama and LM Studio `gpt-oss`: a local server Kaeso can't price.
        "gpt-oss" => UNNAMED_PROVIDER,
        "" => return None,
        other => {
            let id = other.trim().to_ascii_lowercase().replace([' ', '_'], "-");
            return kaeso_core::provider::provider(&id).map(|_| id);
        }
    };
    Some(id.to_owned())
}

/// Codex's request state, by conversation: what its latest prompt was, when its latest request
/// began, and which provider it talks to. Each may have come in an earlier export.
#[derive(Default)]
struct CodexTurns {
    prompts: HashMap<String, String>,
    began: HashMap<String, (OffsetDateTime, Duration)>,
    providers: HashMap<String, String>,
    /// Conversations whose current prompt this batch has (redacted or not), or whose request
    /// already looked for it in an earlier batch.
    prompted: std::collections::HashSet<String>,
}

/// What one Codex request takes from its conversation.
struct Turn {
    began: Option<(OffsetDateTime, Duration)>,
    prompt: Option<String>,
    provider: Option<String>,
    wants: Vec<Want>,
}

impl CodexTurns {
    /// What a request of the conversation takes: its start and prompt, from this batch or, failing
    /// that, from an earlier one, and the provider.
    fn take(&mut self, conversation: &str) -> Turn {
        let mut wants = Vec::new();
        let began = self.began.remove(conversation);
        if began.is_none() && !conversation.is_empty() {
            wants.push(want(Wanted::Began, format!("codex:{conversation}")));
        }
        let prompt = self.prompts.remove(conversation);
        if prompt.is_none() && !conversation.is_empty() && !self.prompted.contains(conversation) {
            wants.push(want(Wanted::Prompt, format!("codex:{conversation}")));
        }
        self.prompted.insert(conversation.to_owned());
        let provider = self.providers.get(conversation).cloned();
        if provider.is_none() && !conversation.is_empty() {
            wants.push(want(Wanted::Provider, format!("codex:{conversation}")));
        }
        Turn {
            began,
            prompt,
            provider,
            wants,
        }
    }
}

fn timed(
    start: Option<(OffsetDateTime, Duration)>,
    end: Option<OffsetDateTime>,
) -> (Option<OffsetDateTime>, Duration, Option<Duration>) {
    match (start, end) {
        (Some((s, headers)), Some(e)) if e >= s => (
            Some(s),
            (e - s).try_into().unwrap_or_default(),
            Some(headers),
        ),
        _ => (end, Duration::ZERO, None),
    }
}

// Codex logs each event as it happens, so the records are read in their order: a prompt goes with
// the next request of its conversation, and `codex.api_request`, logged once the response's
// headers arrive, says when that request began; the usage comes on a later event.
fn codex(groups: &[Group<LogRecord>], batch: &mut Batch) {
    let mut turns = CodexTurns::default();
    for g in groups {
        for r in &g.items {
            let a = &r.attributes;
            let name = event_name(&g.scope, r);
            let version = a.str("app.version");
            let conversation = a.str("conversation.id").unwrap_or_default().to_owned();
            match name.as_str() {
                "codex.conversation_starts" => {
                    if let Some(provider) = a.str("provider_name").and_then(codex_provider) {
                        turns
                            .providers
                            .insert(conversation.clone(), provider.clone());
                        if !conversation.is_empty() {
                            leave(
                                batch,
                                Wanted::Provider,
                                format!("codex:{conversation}"),
                                provider,
                            );
                        }
                    }
                }
                "codex.user_prompt" => {
                    turns.prompted.insert(conversation.clone());
                    match text(a.str("prompt")) {
                        Some(p) => {
                            turns.prompts.insert(conversation, p);
                        }
                        None => {
                            turns.prompts.remove(&conversation);
                        }
                    }
                }
                "codex.api_request" => {
                    let headers = ms(a.u64("duration_ms"));
                    let status = status_of(a.get("http.response.status_code"));
                    let error = text(a.str("error.message"));
                    if status.is_some_and(|s| s >= 400) || error.is_some() {
                        let Some(model) = a.str("model").or(a.str("slug")) else {
                            refuse(batch, Refusal::NoModel);
                            continue;
                        };
                        let end = ended(r);
                        let provider = turns.providers.get(&conversation).cloned();
                        batch.requests.push(ObservedRequest {
                            external_id: hashed(&[
                                &name,
                                &conversation,
                                a.str("event.timestamp").unwrap_or_default(),
                                &r.observed_time_unix_nano.to_string(),
                                &a.u64("attempt").unwrap_or(0).to_string(),
                            ]),
                            started_at: started(end, headers),
                            duration: headers,
                            unguessed: provider.as_deref() == Some(UNNAMED_PROVIDER),
                            provider: provider.filter(|p| p != UNNAMED_PROVIDER),
                            provider_guessed: true,
                            model: model.to_owned(),
                            status: status.filter(|s| *s >= 400).unwrap_or(502),
                            error_message: error.or(Some("The request failed.".into())),
                            client: Some(client("codex_cli_rs", version)),
                            email: a.str("user.email").map(str::to_owned),
                            dialect: Some(DialectId::OpenAiResponses),
                            ..ObservedRequest::default()
                        });
                    } else if let Some(at) = ended(r)
                        && a.str("endpoint").is_none_or(|e| e == "/responses")
                    {
                        // Codex's other calls, such as summarizing its memories, aren't the
                        // conversation's next request.
                        turns.began.insert(conversation, (at - headers, headers));
                    }
                }
                "codex.sse_event"
                    if a.str("event.kind") == Some("response.failed")
                        && a.get("input_token_count").is_none() =>
                {
                    let Some(model) = a.str("model").or(a.str("slug")) else {
                        refuse(batch, Refusal::NoModel);
                        continue;
                    };
                    let end = ended(r);
                    let Turn {
                        began: start,
                        prompt,
                        provider,
                        wants,
                    } = turns.take(&conversation);
                    let (started_at, duration, ttfb) = timed(start, end);
                    batch.requests.push(ObservedRequest {
                        external_id: hashed(&[
                            &name,
                            &conversation,
                            a.str("event.timestamp").unwrap_or_default(),
                            &r.observed_time_unix_nano.to_string(),
                        ]),
                        started_at,
                        duration,
                        ttfb,
                        unguessed: provider.as_deref() == Some(UNNAMED_PROVIDER),
                        provider: provider.filter(|p| p != UNNAMED_PROVIDER),
                        provider_guessed: true,
                        model: model.to_owned(),
                        status: 502,
                        error_message: text(a.str("error.message"))
                            .or(Some("The response failed.".into())),
                        stream: true,
                        client: Some(client("codex_cli_rs", version)),
                        email: a.str("user.email").map(str::to_owned),
                        dialect: Some(DialectId::OpenAiResponses),
                        prompt,
                        wants,
                        ..ObservedRequest::default()
                    });
                }
                "codex.sse_event"
                    if a.str("event.kind") == Some("response.completed")
                        && a.get("input_token_count").is_some() =>
                {
                    let Some(model) = a.str("model").or(a.str("slug")) else {
                        refuse(batch, Refusal::NoModel);
                        continue;
                    };
                    let failed = a.str("error.message");
                    let tokens = Counted {
                        input: a.u64("input_token_count").unwrap_or(0),
                        output: a.u64("output_token_count").unwrap_or(0),
                        cache_read: a.u64("cached_token_count").unwrap_or(0),
                        cache_write: a.u64("cache_write_token_count").unwrap_or(0),
                        reasoning: a.u64("reasoning_token_count").unwrap_or(0),
                        cache_write_1h: 0,
                    }
                    .normalized(InputCounts::WithCache, OutputCounts::WithReasoning);
                    let end = ended(r);
                    let Turn {
                        began: start,
                        prompt,
                        provider,
                        wants,
                    } = turns.take(&conversation);
                    let (started_at, duration, ttfb) = timed(start, end);
                    batch.requests.push(ObservedRequest {
                        external_id: hashed(&[
                            &name,
                            a.str("conversation.id").unwrap_or_default(),
                            a.str("event.timestamp").unwrap_or_default(),
                            &r.time_unix_nano.to_string(),
                            &tokens.input.to_string(),
                            &tokens.output.to_string(),
                        ]),
                        started_at,
                        duration,
                        ttfb,
                        unguessed: provider.as_deref() == Some(UNNAMED_PROVIDER),
                        provider: provider.filter(|p| p != UNNAMED_PROVIDER),
                        provider_guessed: true,
                        model: model.to_owned(),
                        status: if failed.is_some() { 502 } else { 200 },
                        error_message: failed.map(str::to_owned),
                        tokens,
                        stream: true,
                        client: Some(client("codex_cli_rs", version)),
                        email: a.str("user.email").map(str::to_owned),
                        dialect: Some(DialectId::OpenAiResponses),
                        prompt,
                        wants,
                        ..ObservedRequest::default()
                    });
                }
                "codex.tool_result" => {
                    let Some(server) = a.str("mcp_server") else {
                        continue;
                    };
                    let Some(tool) = a.str("tool_name") else {
                        continue;
                    };
                    let duration = ms(a.u64("duration_ms"));
                    let ok = a.bool("success").unwrap_or(true);
                    batch.tool_calls.push(ObservedToolCall {
                        external_id: a.str("call_id").map_or_else(
                            || hashed(&[&name, &r.time_unix_nano.to_string(), tool]),
                            str::to_owned,
                        ),
                        started_at: started(ended(r), duration),
                        duration,
                        tool: tool.to_owned(),
                        server: Some(server.to_owned()),
                        ok,
                        error_type: None,
                        error_message: None,
                        client: Some("codex".into()),
                        email: a.str("user.email").map(str::to_owned),
                        arguments: text(a.str("arguments")),
                        result: text(a.str("output")),
                        aliases: Vec::new(),
                    });
                }
                _ => {}
            }
        }
    }
    // What the batch's last turns began with goes to the next batch, where their usage arrives.
    for (conversation, (at, headers)) in turns.began {
        if !conversation.is_empty() {
            let at = at.unix_timestamp_nanos() / 1_000_000;
            leave(
                batch,
                Wanted::Began,
                format!("codex:{conversation}"),
                format!("{at}:{}", headers.as_millis()),
            );
        }
    }
    for (conversation, prompt) in turns.prompts {
        if !conversation.is_empty() {
            leave(
                batch,
                Wanted::Prompt,
                format!("codex:{conversation}"),
                prompt,
            );
        }
    }
}

// Gemini CLI --------------------------------------------------------------------------------

/// Kaeso's provider for Gemini CLI's `auth_type`: an API key is the Gemini API, Vertex AI and a
/// Google Cloud machine's credentials are Vertex AI; signing in with Google or through a gateway
/// names no provider Kaeso can price, so the model decides.
fn gemini_provider(auth: Option<&str>) -> Option<String> {
    match auth? {
        "gemini-api-key" => Some("google".into()),
        "vertex-ai" | "compute-default-credentials" => Some("gemini-agent-platform".into()),
        _ => None,
    }
}

/// An MCP tool's name without the `mcp_<server>_` Gemini CLI puts before it, the server's name
/// written as Gemini CLI writes it there: characters other than letters, digits, `_`, `-`, `.`
/// and `:` become `_`.
fn gemini_mcp_tool<'a>(name: &'a str, server: Option<&str>) -> &'a str {
    let Some(server) = server else {
        return name;
    };
    let written: String = server
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "_-.:".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    name.strip_prefix("mcp_")
        .and_then(|rest| rest.strip_prefix(written.as_str()))
        .and_then(|rest| rest.strip_prefix('_'))
        .filter(|tool| !tool.is_empty())
        .unwrap_or(name)
}

/// Carried request contents are kept only while they are small; a longer conversation's later
/// requests still carry it whole in their own batch.
const MOST_CARRIED: usize = 128 * 1024;

// Gemini CLI's `service.version` is Node's, not its own, so its requests name no version. With
// prompt logging on, `api_request` holds the request's whole `contents` and `api_response` the
// reply as JSON, one response or the streamed chunks.
fn gemini_cli(groups: &[Group<LogRecord>], batch: &mut Batch) {
    let mut prompts: HashMap<String, String> = HashMap::new();
    let mut sent: HashMap<String, std::collections::VecDeque<String>> = HashMap::new();
    let mut asked = std::collections::HashSet::new();
    for g in groups {
        for r in &g.items {
            let a = &r.attributes;
            let name = event_name(&g.scope, r);
            match name.as_str() {
                "gemini_cli.user_prompt" => {
                    if let (Some(id), Some(p)) = (a.str("prompt_id"), text(a.str("prompt"))) {
                        prompts.insert(id.to_owned(), p);
                    }
                }
                "gemini_cli.api_request" => {
                    if let (Some(id), Some(t)) = (a.str("prompt_id"), text(a.str("request_text"))) {
                        sent.entry(id.to_owned()).or_default().push_back(t);
                    }
                }
                "gemini_cli.api_response" | "gemini_cli.api_error" => {
                    let error = name == "gemini_cli.api_error";
                    let Some(model) = a.str("model").or(a.str("model_name")) else {
                        refuse(batch, Refusal::NoModel);
                        continue;
                    };
                    let duration = ms(a.u64("duration_ms").or(a.u64("duration")));
                    // The prompt of a tool's answer (`tool_token_count`) is billed input that
                    // `input_token_count` leaves out.
                    let tokens = Counted {
                        input: a
                            .u64("input_token_count")
                            .unwrap_or(0)
                            .saturating_add(a.u64("tool_token_count").unwrap_or(0)),
                        output: a.u64("output_token_count").unwrap_or(0),
                        cache_read: a.u64("cached_content_token_count").unwrap_or(0),
                        cache_write: 0,
                        reasoning: a.u64("thoughts_token_count").unwrap_or(0),
                        cache_write_1h: 0,
                    }
                    .normalized(InputCounts::WithCache, OutputCounts::WithoutReasoning);
                    let prompt_id = a.str("prompt_id").unwrap_or_default();
                    let mut wants = Vec::new();
                    let exchange = match sent.get_mut(prompt_id).and_then(|q| q.pop_front()) {
                        Some(t) => serde_json::from_str::<Json>(&t)
                            .ok()
                            .and_then(|v| content::gemini_contents(&v)),
                        None => {
                            if !prompt_id.is_empty() {
                                wants.push(want(
                                    Wanted::GeminiContents,
                                    format!("gemini_cli:{prompt_id}"),
                                ));
                            }
                            None
                        }
                    };
                    // The person's prompt goes with the turn's first request.
                    let mut prompt = prompts.remove(prompt_id);
                    if prompt.is_none() && !prompt_id.is_empty() && !asked.contains(prompt_id) {
                        wants.push(want(Wanted::Prompt, format!("gemini_cli:{prompt_id}")));
                    }
                    asked.insert(prompt_id.to_owned());
                    let Exchange { system, messages } = exchange.unwrap_or_default();
                    if prompt.is_none() && !messages.is_empty() && wants.is_empty() {
                        prompt = Exchange {
                            system: None,
                            messages: messages.clone(),
                        }
                        .last_user_text();
                    }
                    let raw_reply = text(a.str("response_text"));
                    let reply_parts = raw_reply
                        .as_deref()
                        .and_then(|t| serde_json::from_str::<Json>(t).ok())
                        .and_then(|v| content::gemini_reply(&v))
                        .unwrap_or_default();
                    // An older Gemini CLI wrote the reply's text, not its JSON.
                    let reply = if reply_parts.is_empty() {
                        raw_reply
                    } else {
                        None
                    };
                    batch.requests.push(ObservedRequest {
                        external_id: hashed(&[
                            &name,
                            a.str("session.id").unwrap_or_default(),
                            prompt_id,
                            &r.time_unix_nano.to_string(),
                            a.str("event.timestamp").unwrap_or_default(),
                        ]),
                        started_at: started(ended(r), duration),
                        duration,
                        provider: gemini_provider(a.str("auth_type")),
                        provider_guessed: true,
                        model: model.to_owned(),
                        status: match status_of(a.get("status_code")) {
                            Some(s) => s,
                            None if error => 502,
                            None => 200,
                        },
                        error_message: if error {
                            text(a.str("error.message"))
                                .or(text(a.str("error")))
                                .or(Some("The request failed.".into()))
                        } else {
                            None
                        },
                        tokens,
                        stream: true,
                        client: Some("GeminiCLI".into()),
                        email: a.str("user.email").map(str::to_owned),
                        dialect: Some(DialectId::Gemini),
                        prompt,
                        reply,
                        system,
                        messages,
                        reply_parts,
                        wants,
                        ..ObservedRequest::default()
                    });
                }
                "gemini_cli.tool_call" if a.str("tool_type") == Some("mcp") => {
                    let Some(function) = a.str("function_name") else {
                        continue;
                    };
                    let server = a.str("mcp_server_name");
                    let tool = gemini_mcp_tool(function, server);
                    let duration = ms(a.u64("duration_ms"));
                    let ok = a.bool("success").unwrap_or(true);
                    batch.tool_calls.push(ObservedToolCall {
                        external_id: hashed(&[
                            &name,
                            a.str("session.id").unwrap_or_default(),
                            a.str("prompt_id").unwrap_or_default(),
                            &r.time_unix_nano.to_string(),
                            function,
                        ]),
                        started_at: started(ended(r), duration),
                        duration,
                        tool: tool.to_owned(),
                        server: server.map(str::to_owned),
                        ok,
                        error_type: (!ok)
                            .then(|| a.str("error_type").map(str::to_owned))
                            .flatten(),
                        error_message: (!ok).then(|| text(a.str("error"))).flatten(),
                        client: Some("gemini-cli".into()),
                        email: a.str("user.email").map(str::to_owned),
                        arguments: text(a.str("function_args")),
                        result: None,
                        aliases: Vec::new(),
                    });
                }
                _ => {}
            }
        }
    }
    for (id, prompt) in prompts {
        if !asked.contains(&id) {
            leave(batch, Wanted::Prompt, format!("gemini_cli:{id}"), prompt);
        }
    }
    for (id, mut queue) in sent {
        if let Some(contents) = queue.pop_back().filter(|c| c.len() <= MOST_CARRIED) {
            leave(
                batch,
                Wanted::GeminiContents,
                format!("gemini_cli:{id}"),
                contents,
            );
        }
    }
}

/// Grok Build's status for an `api_error` without one: a rate limit is a 429, the rest a 502.
fn grok_status(a: &Attrs) -> u16 {
    status_of(a.get("status_code")).unwrap_or(match a.str("error_category") {
        Some("rate_limit") => 429,
        _ => 502,
    })
}

// Grok Build's `input_tokens` is the whole prompt, its cache reads and writes included (its
// `TokenUsage` in xai-org/grok-build), and its `output_tokens` is xAI's `completion_tokens`, which
// leaves the reasoning out (docs.x.ai's Responses reference: total = prompt + completion +
// reasoning). A turn's reply comes once, after the turn's last request, which it goes with.
fn grok(groups: &[Group<LogRecord>], batch: &mut Batch) {
    let mut prompts: HashMap<String, String> = HashMap::new();
    let mut asked = std::collections::HashSet::new();
    let mut last_of_turn: HashMap<String, usize> = HashMap::new();
    for g in groups {
        let version = g
            .resource
            .str("service.version")
            .or(g.resource.str("client.version"));
        for r in &g.items {
            let a = &r.attributes;
            let name = event_name(&g.scope, r);
            let prompt_id = a.str("prompt.id").unwrap_or_default();
            match name.as_str() {
                "grok_code.user_prompt" => {
                    if let Some(p) = text(a.str("prompt")) {
                        prompts.insert(prompt_id.to_owned(), p);
                    }
                }
                "grok_code.assistant_response" => {
                    let Some(reply) = text(a.str("response")) else {
                        continue;
                    };
                    if let Some(r) = last_of_turn
                        .get(prompt_id)
                        .and_then(|i| batch.requests.get_mut(*i))
                    {
                        r.reply = Some(reply);
                    }
                }
                "grok_code.api_request" | "grok_code.api_error" => {
                    let Some(model) = a.str("model") else {
                        refuse(batch, Refusal::NoModel);
                        continue;
                    };
                    let error = name == "grok_code.api_error";
                    let duration = ms(a.u64("duration_ms"));
                    let tokens = Counted {
                        input: a.u64("input_tokens").unwrap_or(0),
                        output: a.u64("output_tokens").unwrap_or(0),
                        cache_read: a.u64("cache_read_tokens").unwrap_or(0),
                        cache_write: a.u64("cache_creation_tokens").unwrap_or(0),
                        reasoning: a.u64("reasoning_tokens").unwrap_or(0),
                        cache_write_1h: 0,
                    }
                    .normalized(InputCounts::WithCache, OutputCounts::WithoutReasoning);
                    let prompt = prompts.remove(prompt_id);
                    let mut wants = Vec::new();
                    if prompt.is_none() && !prompt_id.is_empty() && !asked.contains(prompt_id) {
                        wants.push(want(Wanted::Prompt, format!("grok:{prompt_id}")));
                    }
                    asked.insert(prompt_id.to_owned());
                    last_of_turn.insert(prompt_id.to_owned(), batch.requests.len());
                    batch.requests.push(ObservedRequest {
                        external_id: hashed(&[
                            &name,
                            a.str("session.id").unwrap_or_default(),
                            &a.u64("event.sequence").unwrap_or(0).to_string(),
                            prompt_id,
                            &r.time_unix_nano.to_string(),
                        ]),
                        started_at: started(ended(r), duration),
                        duration,
                        provider: Some("xai".into()),
                        provider_guessed: true,
                        model: model.to_owned(),
                        status: if error { grok_status(a) } else { 200 },
                        error_message: error.then(|| {
                            a.str("error_category").map_or_else(
                                || "The request failed.".to_owned(),
                                |c| format!("The request failed: {c}."),
                            )
                        }),
                        tokens,
                        // xAI's cost in ticks of 1e-10 USD, which Grok Build divides by 10,000
                        // (`map_api_request` in its telemetry schema).
                        reported_cost: a
                            .u64("cost_usd_micros")
                            .filter(|c| *c > 0)
                            .map(|c| Decimal::from(c) / Decimal::from(1_000_000u32)),
                        stream: true,
                        client: Some(client("grok-cli", version)),
                        email: a.str("user.email").map(str::to_owned),
                        dialect: Some(DialectId::OpenAiChat),
                        prompt,
                        wants,
                        ..ObservedRequest::default()
                    });
                }
                "grok_code.tool_result" => {
                    // Only MCP tools name a server; without tool details Grok Build writes
                    // `mcp_server` and `mcp_tool` in place of their names.
                    let Some(server) = a.str("mcp_server.name") else {
                        continue;
                    };
                    let (server, tool) = match (server, a.str("mcp_tool.name")) {
                        ("mcp_server", _) | (_, None | Some("mcp_tool")) => (None, "mcp_tool"),
                        (s, Some(t)) => (Some(s.to_owned()), t),
                    };
                    let duration = ms(a.u64("duration_ms"));
                    let ok = a.bool("success").unwrap_or(true);
                    batch.tool_calls.push(ObservedToolCall {
                        external_id: a.str("tool_use_id").map_or_else(
                            || {
                                hashed(&[
                                    &name,
                                    a.str("session.id").unwrap_or_default(),
                                    &a.u64("event.sequence").unwrap_or(0).to_string(),
                                    &r.time_unix_nano.to_string(),
                                ])
                            },
                            str::to_owned,
                        ),
                        started_at: started(ended(r), duration),
                        duration,
                        tool: tool.to_owned(),
                        server,
                        ok,
                        error_type: (!ok).then(|| a.str("outcome").map(str::to_owned)).flatten(),
                        error_message: (!ok).then(|| text(a.str("error_message"))).flatten(),
                        client: Some("grok-cli".into()),
                        email: a.str("user.email").map(str::to_owned),
                        arguments: text(a.str("tool_input")),
                        result: text(a.str("tool_output")),
                        aliases: Vec::new(),
                    });
                }
                _ => {}
            }
        }
    }
    for (id, prompt) in prompts {
        if !id.is_empty() && !asked.contains(&id) {
            leave(batch, Wanted::Prompt, format!("grok:{id}"), prompt);
        }
    }
}

// The OpenTelemetry GenAI semantic conventions ------------------------------------------------

/// Kaeso's provider id for a `gen_ai.provider.name` (or the older `gen_ai.system`).
fn semconv_provider(name: &str) -> String {
    match name {
        "gcp.gemini" | "gcp.vertex_ai" | "gcp.gen_ai" | "gemini" | "vertex_ai" => "google",
        "aws.bedrock" => "amazon-bedrock",
        "azure.ai.openai" | "azure.ai.inference" | "az.ai.openai" | "az.ai.inference" => "azure",
        "mistral_ai" => "mistral",
        "x_ai" => "xai",
        "ibm.watsonx.ai" => "ibm",
        other => other,
    }
    .to_owned()
}

/// The usage a span or record carries under the GenAI conventions, with the older names
/// (`prompt_tokens`, `completion_tokens`) some instrumentations still write.
fn semconv_tokens(a: &Attrs) -> Option<Counted> {
    let input = a
        .u64("gen_ai.usage.input_tokens")
        .or(a.u64("gen_ai.usage.prompt_tokens"));
    let output = a
        .u64("gen_ai.usage.output_tokens")
        .or(a.u64("gen_ai.usage.completion_tokens"));
    if input.is_none() && output.is_none() {
        return None;
    }
    // OpenRouter's Broadcast also writes the cache and reasoning counts under the older
    // `…input_tokens.cached` names, and the one-hour cache writes on their own.
    let first = |names: &[&str]| names.iter().find_map(|n| a.u64(n)).unwrap_or(0);
    Some(
        Counted {
            input: input.unwrap_or(0),
            output: output.unwrap_or(0),
            cache_read: first(&[
                "gen_ai.usage.cache_read.input_tokens",
                "gen_ai.usage.input_tokens.cached",
            ]),
            cache_write: first(&[
                "gen_ai.usage.cache_write.input_tokens",
                "gen_ai.usage.input_tokens.cache_write",
            ]),
            reasoning: first(&[
                "gen_ai.usage.reasoning.output_tokens",
                "gen_ai.usage.output_tokens.reasoning",
            ]),
            cache_write_1h: first(&["gen_ai.usage.input_tokens.cache_write_1h"]),
        }
        .normalized(InputCounts::WithCache, OutputCounts::WithReasoning),
    )
}

/// What a span or record says was asked and answered: `gen_ai.input.messages` and
/// `gen_ai.output.messages` as the conventions write them, with `gen_ai.system_instructions`, or
/// the plain text of the older `gen_ai.prompt` and `gen_ai.completion` (which may also hold a
/// list of messages).
fn semconv_content(a: &Attrs, r: &mut ObservedRequest) {
    let input = text(a.str("gen_ai.input.messages").or(a.str("gen_ai.prompt")));
    if let Some(input) = input {
        match content::semconv_messages(&input, a.str("gen_ai.system_instructions")) {
            Some(e) => {
                r.prompt = e.last_user_text();
                r.system = e.system;
                r.messages = e.messages;
            }
            None => r.prompt = Some(input),
        }
    }
    let output = text(
        a.str("gen_ai.output.messages")
            .or(a.str("gen_ai.completion")),
    );
    if let Some(output) = output {
        match content::semconv_reply(&output) {
            Some(parts) => r.reply_parts = parts,
            None => r.reply = Some(output),
        }
    }
}

/// Services whose spans and GenAI records repeat what their own log events already report.
const LEFT_OUT: &[&str] = &["claude-code", "gemini-cli", "grok-cli"];

/// Instrumentation scopes of those clients, for when a person renamed the service.
const SCOPES_LEFT_OUT: &[&str] = &[
    "gemini-cli",
    "com.anthropic.claude_code",
    "ai.xai.grok_code",
];

/// The prefixes of the events the clients' own readers read.
const CLIENT_EVENTS: &[&str] = &["claude_code.", "codex.", "gemini_cli.", "grok_code."];

fn left_out(service: &str, scope: &str) -> bool {
    LEFT_OUT.contains(&service)
        || service.starts_with("codex")
        || scope.starts_with("codex")
        || SCOPES_LEFT_OUT.iter().any(|s| scope.starts_with(s))
}

/// The operations that are one call to a model; an agent's own spans (`invoke_agent`, …) sum the
/// usage of the calls inside them and would count it twice.
const INFERENCE: &[&str] = &["chat", "text_completion", "generate_content", "embeddings"];

fn spans_of(groups: &[Group<Span>], batch: &mut Batch) {
    for g in groups {
        let service = g.resource.str("service.name").unwrap_or_default();
        if left_out(service, &g.scope) {
            continue;
        }
        for s in &g.items {
            let id = format!("{}{}", hex(&s.trace_id), hex(&s.span_id));
            span(service, &g.resource, s, &id, batch);
        }
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}

/// A span or GenAI record; `fallback_id` names it when it carries no id of its own.
fn span(service: &str, resource: &Attrs, s: &Span, fallback_id: &str, batch: &mut Batch) {
    let a = &s.attributes;
    let start = at_nanos(s.start_time_unix_nano);
    let end = at_nanos(s.end_time_unix_nano);
    let duration = match (start, end) {
        (Some(st), Some(en)) if en > st => (en - st).try_into().unwrap_or_default(),
        _ => Duration::ZERO,
    };
    let operation = a.str("gen_ai.operation.name").unwrap_or_default();
    let email = a
        .str("user.email")
        .or(a.str("enduser.id").filter(|u| u.contains('@')))
        .or(a.str("user.id").filter(|u| u.contains('@')))
        .or(resource.str("user.email"))
        .map(str::to_owned);
    let failed = s.status_code == 2;
    let tool_call = operation == "execute_tool" || a.str("mcp.method.name") == Some("tools/call");
    if tool_call {
        let Some(tool) = a.str("gen_ai.tool.name") else {
            return;
        };
        let server = a
            .str("mcp.server.name")
            .or(a.str("server.address"))
            .map(str::to_owned);
        if server.is_none() && a.str("mcp.method.name").is_none() {
            return;
        }
        let call_id = a.str("gen_ai.tool.call.id");
        batch.tool_calls.push(ObservedToolCall {
            external_id: call_id.unwrap_or(fallback_id).to_owned(),
            aliases: match call_id {
                Some(id) if id != fallback_id => vec![fallback_id.to_owned()],
                _ => Vec::new(),
            },
            started_at: start,
            duration,
            tool: tool.to_owned(),
            server,
            ok: !failed,
            error_type: a.str("error.type").filter(|_| failed).map(str::to_owned),
            error_message: (failed && !s.status_message.is_empty())
                .then(|| s.status_message.clone()),
            client: Some(service).filter(|s| !s.is_empty()).map(str::to_owned),
            email,
            arguments: text(a.str("gen_ai.tool.call.arguments")),
            result: text(a.str("gen_ai.tool.call.result")),
        });
        return;
    }
    let model = a
        .str("gen_ai.response.model")
        .or(a.str("gen_ai.request.model"));
    // A span without an operation is one call to a model when it names its model and usage, as
    // OpenRouter's generation spans do; an agent's own spans always name their operation.
    let tokens = semconv_tokens(a);
    let inference = if operation.is_empty() {
        a.str("gen_ai.response.id").is_some() || (model.is_some() && tokens.is_some())
    } else {
        INFERENCE.contains(&operation)
    };
    if !inference {
        return;
    }
    let Some(tokens) = tokens else {
        return;
    };
    let Some(model) = model else {
        refuse(batch, Refusal::NoModel);
        return;
    };
    // OpenRouter names the provider it routed to; the request was OpenRouter's, at its price.
    let provider = if service == "openrouter" {
        Some("openrouter".to_owned())
    } else {
        a.str("gen_ai.provider.name")
            .or(a.str("gen_ai.system"))
            .filter(|p| *p != "unknown")
            .map(semconv_provider)
    };
    let status =
        status_of(a.get("http.response.status_code")).unwrap_or(if failed { 502 } else { 200 });
    // A call can be exported as a span and as a log event written in it, and only one of them may
    // name the response's id: the call goes by both, so it counts once either way.
    let response_id = a.str("gen_ai.response.id");
    let aliases = match response_id {
        Some(id) if id != fallback_id => vec![fallback_id.to_owned()],
        _ => Vec::new(),
    };
    let mut request = ObservedRequest {
        external_id: response_id.unwrap_or(fallback_id).to_owned(),
        aliases,
        started_at: start,
        duration,
        ttfb: a
            .get("gen_ai.response.time_to_first_chunk")
            .and_then(|v| match v {
                Value::Double(d) if d.is_finite() && *d >= 0.0 => Some(Duration::from_secs_f64(*d)),
                _ => None,
            }),
        provider,
        model: model.to_owned(),
        status,
        error_message: (status >= 400).then(|| {
            text(Some(s.status_message.as_str()))
                .or(a.str("error.type").map(str::to_owned))
                .unwrap_or_else(|| "The request failed.".into())
        }),
        tokens,
        reported_cost: decimal(
            a.get("gen_ai.usage.cost")
                .or(a.get("gen_ai.usage.total_cost"))
                .or(a.get("gen_ai.cost")),
        ),
        stream: a.bool("gen_ai.request.stream").unwrap_or(false),
        client: Some(service).filter(|s| !s.is_empty()).map(str::to_owned),
        email,
        provider_request_id: response_id.map(str::to_owned),
        reasoning_effort: a.str("gen_ai.request.reasoning.level").map(str::to_owned),
        ..ObservedRequest::default()
    };
    semconv_content(a, &mut request);
    batch.requests.push(request);
}

/// GenAI log records, such as `gen_ai.client.inference.operation.details`, read like spans.
fn semconv_logs(groups: &[Group<LogRecord>], batch: &mut Batch) {
    for g in groups {
        let service = g.resource.str("service.name").unwrap_or_default();
        if left_out(service, &g.scope) {
            continue;
        }
        for r in &g.items {
            let name = event_name(&g.scope, r);
            if CLIENT_EVENTS.iter().any(|p| name.starts_with(p)) {
                continue;
            }
            if r.attributes.str("gen_ai.operation.name").is_none() {
                continue;
            }
            let end = at_nanos(r.time_unix_nano).or_else(|| at_nanos(r.observed_time_unix_nano));
            let nanos = |t: Option<OffsetDateTime>| {
                t.map_or(0, |t| u64::try_from(t.unix_timestamp_nanos()).unwrap_or(0))
            };
            // A record logged inside a span names it, so the record and an exported span of
            // the same call share an id and count once.
            let fallback = if r.trace_id.is_empty() && r.span_id.is_empty() {
                hashed(&[
                    &name,
                    &r.time_unix_nano.to_string(),
                    &r.observed_time_unix_nano.to_string(),
                    r.attributes.str("gen_ai.request.model").unwrap_or_default(),
                    r.attributes.str("gen_ai.tool.name").unwrap_or_default(),
                ])
            } else {
                format!("{}{}", hex(&r.trace_id), hex(&r.span_id))
            };
            let s = Span {
                trace_id: r.trace_id.clone(),
                span_id: r.span_id.clone(),
                name: name.clone(),
                start_time_unix_nano: nanos(end),
                end_time_unix_nano: nanos(end),
                attributes: r.attributes.clone(),
                status_code: 0,
                status_message: String::new(),
            };
            span(service, &g.resource, &s, &fallback, batch);
        }
    }
}

/// Every request and tool call in an OTLP logs export, whichever client sent it.
pub fn logs(groups: &[Group<LogRecord>]) -> Batch {
    let mut batch = Batch::default();
    claude_code(groups, &mut batch);
    codex(groups, &mut batch);
    gemini_cli(groups, &mut batch);
    grok(groups, &mut batch);
    semconv_logs(groups, &mut batch);
    let known = |g: &Group<LogRecord>, r: &LogRecord| {
        let service = g.resource.str("service.name").unwrap_or_default();
        left_out(service, &g.scope)
            || CLIENT_EVENTS
                .iter()
                .any(|p| event_name(&g.scope, r).starts_with(p))
            || genai(&r.attributes)
    };
    unrecognized(&mut batch, groups, known);
    batch
}

/// Every request and tool call in an OTLP traces export.
pub fn spans(groups: &[Group<Span>]) -> Batch {
    let mut batch = Batch::default();
    spans_of(groups, &mut batch);
    let known = |g: &Group<Span>, s: &Span| {
        let service = g.resource.str("service.name").unwrap_or_default();
        left_out(service, &g.scope) || service == "openrouter" || genai(&s.attributes)
    };
    unrecognized(&mut batch, groups, known);
    batch
}

fn genai(a: &Attrs) -> bool {
    a.0.iter()
        .any(|(k, _)| k.starts_with("gen_ai.") || k.starts_with("mcp."))
}

/// A batch none of whose records a reader knows is refused whole, so a source that has recorded
/// nothing yet says why instead of looking fine (`observed::record` lets such a batch pass once the
/// source has recorded something); a batch with a record Kaeso read leaves the rest alone.
fn unrecognized<T>(batch: &mut Batch, groups: &[Group<T>], known: impl Fn(&Group<T>, &T) -> bool) {
    let records: usize = groups.iter().map(|g| g.items.len()).sum();
    if records == 0 || groups.iter().any(|g| g.items.iter().any(|r| known(g, r))) {
        return;
    }
    batch.refused = batch
        .refused
        .saturating_add(u32::try_from(records).unwrap_or(u32::MAX));
    batch.refusal = Some(Refusal::Unrecognized);
}

// LiteLLM -----------------------------------------------------------------------------------

fn seconds(v: Option<&Json>) -> Option<OffsetDateTime> {
    let s = v?.as_f64()?;
    if !s.is_finite() || s <= 0.0 {
        return None;
    }
    // LiteLLM's times are float seconds; microseconds are as precise as they get.
    let micros = (s * 1e6).round() as i128;
    OffsetDateTime::from_unix_timestamp_nanos(micros * 1_000).ok()
}

fn json_u64(v: Option<&Json>) -> Option<u64> {
    match v? {
        Json::Number(n) => n.as_u64(),
        Json::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn json_str<'a>(v: &'a Json, path: &[&str]) -> Option<&'a str> {
    let mut at = v;
    for p in path {
        at = at.get(p)?;
    }
    at.as_str().filter(|s| !s.is_empty())
}

/// The batches LiteLLM's `generic_api` logger sends: a JSON array (or NDJSON, or one object) of
/// its `StandardLoggingPayload`.
pub fn litellm(items: &[Json]) -> Batch {
    let mut batch = Batch::default();
    for p in items {
        let Some(id) = json_str(p, &["id"]) else {
            refuse(&mut batch, Refusal::NoId);
            continue;
        };
        let email = json_str(p, &["metadata", "user_api_key_user_email"]).map(str::to_owned);
        let start = seconds(p.get("startTime"));
        let end = seconds(p.get("endTime"));
        let duration = match (start, end) {
            (Some(s), Some(e)) if e > s => (e - s).try_into().unwrap_or_default(),
            _ => Duration::ZERO,
        };
        let failed = json_str(p, &["status"]) == Some("failure");
        if let Some(mcp) = p
            .get("metadata")
            .and_then(|m| m.get("mcp_tool_call_metadata"))
            .filter(|m| m.is_object())
        {
            let Some(tool) = json_str(mcp, &["name"]) else {
                refuse(&mut batch, Refusal::NoTool);
                continue;
            };
            batch.tool_calls.push(ObservedToolCall {
                external_id: id.to_owned(),
                started_at: start,
                duration,
                tool: tool.to_owned(),
                server: json_str(mcp, &["mcp_server_name"]).map(str::to_owned),
                ok: !failed,
                error_type: None,
                error_message: failed
                    .then(|| json_str(p, &["error_str"]).map(str::to_owned))
                    .flatten(),
                client: Some("litellm".into()),
                email,
                arguments: mcp.get("arguments").map(Json::to_string),
                result: mcp.get("result").map(Json::to_string),
                aliases: Vec::new(),
            });
            continue;
        }
        let Some(model) = json_str(p, &["model"]) else {
            refuse(&mut batch, Refusal::NoModel);
            continue;
        };
        let usage = p.get("metadata").and_then(|m| m.get("usage_object"));
        let cache_read = usage
            .and_then(|u| {
                json_u64(u.get("cache_read_input_tokens")).or(json_u64(
                    u.get("prompt_tokens_details")
                        .and_then(|d| d.get("cached_tokens")),
                ))
            })
            .unwrap_or(0);
        let cache_write = usage
            .and_then(|u| json_u64(u.get("cache_creation_input_tokens")))
            .unwrap_or(0);
        // Anthropic's usage splits the writes by how long they stay cached.
        let cache_write_1h = usage
            .and_then(|u| json_u64(u.get("cache_creation")?.get("ephemeral_1h_input_tokens")))
            .unwrap_or(0);
        let reasoning = usage
            .and_then(|u| {
                json_u64(
                    u.get("completion_tokens_details")
                        .and_then(|d| d.get("reasoning_tokens")),
                )
            })
            .unwrap_or(0);
        let tokens = Counted {
            input: json_u64(p.get("prompt_tokens")).unwrap_or(0),
            output: json_u64(p.get("completion_tokens")).unwrap_or(0),
            cache_read,
            cache_write,
            reasoning,
            cache_write_1h,
        }
        .normalized(InputCounts::WithCache, OutputCounts::WithReasoning);
        // LiteLLM names the provider it called; a `provider/model` name says it only when the
        // prefix is a provider, not an organisation as in `meta-llama/Llama-3.3-70B`.
        let named = json_str(p, &["custom_llm_provider"])
            .or(json_str(p, &["hidden_params", "custom_llm_provider"]))
            .map(semconv_provider);
        let (provider, model) = match (named, model.split_once('/')) {
            (Some(named), Some((prefix, m))) if semconv_provider(prefix) == named => {
                (Some(named), m.to_owned())
            }
            (Some(named), _) => (Some(named), model.to_owned()),
            (None, Some((prefix, m)))
                if kaeso_core::provider::provider(&semconv_provider(prefix)).is_some() =>
            {
                (Some(semconv_provider(prefix)), m.to_owned())
            }
            (None, _) => (None, model.to_owned()),
        };
        let first = seconds(p.get("completionStartTime"));
        let exchange = content::openai_messages(p.get("messages"));
        let reply_parts = p
            .get("response")
            .and_then(|r| r.get("choices")?.get(0)?.get("message"))
            .map(content::openai_reply)
            .unwrap_or_default();
        // LiteLLM logs a cost of 0 when it couldn't price the call, and says why beside it.
        let unpriced = p
            .get("response_cost_failure_debug_info")
            .is_some_and(Json::is_object);
        batch.requests.push(ObservedRequest {
            external_id: id.to_owned(),
            started_at: start,
            duration,
            ttfb: match (start, first) {
                (Some(s), Some(f)) if f > s => (f - s).try_into().ok(),
                _ => None,
            },
            provider,
            model,
            status: if failed {
                json_str(p, &["error_information", "error_code"])
                    .and_then(|c| c.parse().ok())
                    .filter(|s| (400..=599).contains(s))
                    .unwrap_or(502)
            } else {
                200
            },
            error_message: failed.then(|| {
                json_str(p, &["error_str"])
                    .unwrap_or("The request failed.")
                    .to_owned()
            }),
            tokens,
            reported_cost: p
                .get("response_cost")
                .and_then(|c| match c {
                    Json::Number(n) => Some(n.to_string()),
                    Json::String(s) => Some(s.clone()),
                    _ => None,
                })
                .and_then(|t| t.parse::<Decimal>().ok())
                .filter(|d| !d.is_sign_negative() && !(unpriced && d.is_zero())),
            stream: false,
            client: Some("litellm".into()),
            email,
            tags: p
                .get("request_tags")
                .and_then(Json::as_array)
                .map(|t| {
                    t.iter()
                        .filter_map(Json::as_str)
                        .take(10)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            provider_request_id: Some(id.to_owned()),
            dialect: Some(DialectId::OpenAiChat),
            prompt: exchange.last_user_text(),
            system: exchange.system,
            messages: exchange.messages,
            reply_parts,
            ..ObservedRequest::default()
        });
    }
    batch
}

// Antigravity -------------------------------------------------------------------------------

/// An MCP tool call in Antigravity's transcript: `mcp_<server>_<tool>` for a server's tools it
/// loaded up front, `call_mcp_tool` for the others, whose arguments name the server and the tool
/// when content is sent. A server's name is read up to the first `_`.
fn antigravity_mcp(call: &Json) -> Option<(Option<String>, String)> {
    let name = json_str(call, &["name"])?;
    if name == "call_mcp_tool" {
        let args = call.get("args");
        let pick = |keys: &[&str]| {
            keys.iter()
                .find_map(|k| args.and_then(|a| a.get(*k)).and_then(Json::as_str))
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        let server = pick(&["ServerName", "serverName", "server_name", "server"]);
        let tool = pick(&["ToolName", "toolName", "tool_name", "tool"]);
        return Some(match tool {
            Some(t) => (server, t),
            None => (None, "mcp_tool".to_owned()),
        });
    }
    let (server, tool) = name.strip_prefix("mcp_")?.split_once('_')?;
    (!server.is_empty() && !tool.is_empty()).then(|| (Some(server.to_owned()), tool.to_owned()))
}

/// What Kaeso's Antigravity hook sends: the hook's input and the transcript records added since
/// it last ran, each a line of `transcript_full.jsonl`. A model call's tool calls run as the
/// steps right after it, in their order, which is how a call's result is found.
pub fn antigravity(hook: &Json, steps: &[Json]) -> Batch {
    let mut batch = Batch::default();
    let Some(conversation) = json_str(hook, &["conversationId"]) else {
        let planned = steps
            .iter()
            .filter(|s| json_str(s, &["type"]) == Some("PLANNER_RESPONSE"))
            .count();
        batch.refused = u32::try_from(planned).unwrap_or(u32::MAX);
        batch.refusal = (planned > 0).then_some(Refusal::NoId);
        return batch;
    };
    let fallback_model = json_str(hook, &["modelName"]);
    let by_index: HashMap<u64, &Json> = steps
        .iter()
        .filter_map(|s| Some((json_u64(s.get("step_index"))?, s)))
        .collect();
    let version = client(
        "Antigravity",
        json_str(hook, &["cliVersion"]).or(json_str(hook, &["version"])),
    );
    let email = json_str(hook, &["userEmail"]).map(str::to_owned);
    let mut prompt = json_str(hook, &["lastUserInput"]).map(str::to_owned);
    for s in steps {
        if json_str(s, &["type"]) != Some("PLANNER_RESPONSE") {
            continue;
        }
        let Some(step) = json_u64(s.get("step_index")) else {
            refuse(&mut batch, Refusal::NoId);
            continue;
        };
        let Some(model) = json_str(s, &["model"]).or(fallback_model) else {
            refuse(&mut batch, Refusal::NoModel);
            continue;
        };
        let started_at = at_rfc3339(json_str(s, &["created_at"]));
        let calls: &[Json] = s
            .get("tool_calls")
            .and_then(Json::as_array)
            .map_or(&[], Vec::as_slice);
        for (i, call) in calls.iter().enumerate() {
            let Some((server, tool)) = antigravity_mcp(call) else {
                continue;
            };
            let done = by_index.get(&(step + 1 + i as u64)).copied();
            let ended_at = done.and_then(|d| at_rfc3339(json_str(d, &["created_at"])));
            let ok = done.and_then(|d| json_str(d, &["status"])) != Some("ERROR");
            batch.tool_calls.push(ObservedToolCall {
                external_id: format!("{conversation}:{step}:{i}"),
                started_at,
                duration: match (started_at, ended_at) {
                    (Some(a), Some(b)) if b > a => (b - a).try_into().unwrap_or_default(),
                    _ => Duration::ZERO,
                },
                tool,
                server,
                ok,
                error_type: None,
                error_message: (!ok)
                    .then(|| {
                        done.and_then(|d| json_str(d, &["content"]))
                            .map(str::to_owned)
                    })
                    .flatten(),
                client: Some("antigravity".into()),
                email: email.clone(),
                arguments: call
                    .get("args")
                    .filter(|a| !a.is_null())
                    .map(Json::to_string),
                result: done
                    .and_then(|d| json_str(d, &["content"]))
                    .map(str::to_owned),
                ..ObservedToolCall::default()
            });
        }
        let tokens = Counted {
            input: json_u64(s.get("input_tokens")).unwrap_or(0),
            output: json_u64(s.get("output_tokens")).unwrap_or(0),
            cache_read: json_u64(s.get("cache_read_tokens")).unwrap_or(0),
            cache_write: json_u64(s.get("cache_write_tokens"))
                .or(json_u64(s.get("cache_creation_input_tokens")))
                .unwrap_or(0),
            reasoning: 0,
            cache_write_1h: 0,
        }
        .normalized(InputCounts::Uncached, OutputCounts::WithReasoning);
        let mut reply_parts: Vec<kaeso_kpp::Block> = Vec::new();
        if let Some(thinking) = json_str(s, &["thinking"]) {
            let mut r = kaeso_kpp::ReasoningBlock::default();
            r.text = vec![thinking.to_owned()];
            reply_parts.push(kaeso_kpp::Block::Reasoning(r));
        }
        if let Some(text) = json_str(s, &["content"]) {
            reply_parts.push(kaeso_kpp::Block::text(text));
        }
        for (i, call) in calls.iter().enumerate() {
            let (Some(name), Ok(args)) = (
                json_str(call, &["name"]),
                kaeso_kpp::RawJson::try_from(call.get("args").unwrap_or(&Json::Null)),
            ) else {
                continue;
            };
            reply_parts.push(kaeso_kpp::Block::tool_call(
                format!("{step}:{i}"),
                name,
                args,
            ));
        }
        batch.requests.push(ObservedRequest {
            external_id: format!("{conversation}:{step}"),
            started_at,
            provider: None,
            model: model.to_owned(),
            status: 200,
            tokens,
            stream: true,
            client: Some(version.clone()),
            email: email.clone(),
            dialect: Some(DialectId::Gemini),
            // The person's last prompt goes with the invocation's first model call.
            prompt: prompt.take(),
            reply_parts,
            ..ObservedRequest::default()
        });
    }
    batch
}

#[cfg(test)]
#[path = "map_tests.rs"]
mod tests;
