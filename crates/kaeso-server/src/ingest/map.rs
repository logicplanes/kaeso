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

use super::otlp::{Attrs, Group, LogRecord, Span, Value};
use crate::services::observed::{Batch, ObservedRequest, ObservedToolCall, Refusal};

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
                    }
                    .normalized(InputCounts::Uncached, OutputCounts::WithReasoning);
                    let end = ended(r);
                    batch.requests.push(ObservedRequest {
                        external_id,
                        started_at: started(end, duration),
                        duration,
                        ttfb: a.u64("ttft_ms").map(Duration::from_millis),
                        provider: Some(claude_provider(a).to_owned()),
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
                        prompt: a
                            .str("prompt.id")
                            .and_then(|p| content.prompts.get(p).cloned()),
                        reply: request_id.and_then(|id| content.replies.get(id).cloned()),
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
                    });
                }
                _ => {}
            }
        }
    }
}

// Codex -------------------------------------------------------------------------------------

fn codex(groups: &[Group<LogRecord>], batch: &mut Batch) {
    // When each conversation's latest request began: `codex.api_request` is logged once the
    // response's headers arrive, with the time that took; the usage comes on a later event.
    let mut began: HashMap<String, (OffsetDateTime, Duration)> = HashMap::new();
    for g in groups {
        for r in &g.items {
            let a = &r.attributes;
            let name = event_name(&g.scope, r);
            let version = a.str("app.version");
            let conversation = a.str("conversation.id").unwrap_or_default().to_owned();
            match name.as_str() {
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
                            provider: Some("openai".into()),
                            model: model.to_owned(),
                            status: status.filter(|s| *s >= 400).unwrap_or(502),
                            error_message: error.or(Some("The request failed.".into())),
                            client: Some(client("codex_cli_rs", version)),
                            email: a.str("user.email").map(str::to_owned),
                            dialect: Some(DialectId::OpenAiResponses),
                            ..ObservedRequest::default()
                        });
                    } else if let Some(at) = ended(r) {
                        began.insert(conversation, (at - headers, headers));
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
                    let start = began.remove(&conversation);
                    batch.requests.push(ObservedRequest {
                        external_id: hashed(&[
                            &name,
                            &conversation,
                            a.str("event.timestamp").unwrap_or_default(),
                            &r.observed_time_unix_nano.to_string(),
                        ]),
                        started_at: start.map(|(s, _)| s).or(end),
                        duration: match (start, end) {
                            (Some((s, _)), Some(e)) if e >= s => {
                                (e - s).try_into().unwrap_or_default()
                            }
                            _ => Duration::ZERO,
                        },
                        ttfb: start.map(|(_, h)| h),
                        provider: Some("openai".into()),
                        model: model.to_owned(),
                        status: 502,
                        error_message: text(a.str("error.message"))
                            .or(Some("The response failed.".into())),
                        stream: true,
                        client: Some(client("codex_cli_rs", version)),
                        email: a.str("user.email").map(str::to_owned),
                        dialect: Some(DialectId::OpenAiResponses),
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
                    }
                    .normalized(InputCounts::WithCache, OutputCounts::WithReasoning);
                    let end = ended(r);
                    let start = began.remove(&conversation);
                    let (started_at, duration, ttfb) = match (start, end) {
                        (Some((s, headers)), Some(e)) if e >= s => (
                            Some(s),
                            (e - s).try_into().unwrap_or_default(),
                            Some(headers),
                        ),
                        _ => (end, Duration::ZERO, None),
                    };
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
                        provider: Some("openai".into()),
                        model: model.to_owned(),
                        status: if failed.is_some() { 502 } else { 200 },
                        error_message: failed.map(str::to_owned),
                        tokens,
                        reported_cost: None,
                        stream: true,
                        client: Some(client("codex_cli_rs", version)),
                        email: a.str("user.email").map(str::to_owned),
                        tags: Vec::new(),
                        provider_request_id: None,
                        dialect: Some(DialectId::OpenAiResponses),
                        prompt: None,
                        reply: None,
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
                    });
                }
                _ => {}
            }
        }
    }
}

// Gemini CLI --------------------------------------------------------------------------------

// Gemini CLI's `service.version` is Node's, not its own, so its requests name no version.
fn gemini_cli(groups: &[Group<LogRecord>], batch: &mut Batch) {
    let mut prompts: HashMap<String, String> = HashMap::new();
    for g in groups {
        for r in &g.items {
            let a = &r.attributes;
            if event_name(&g.scope, r) == "gemini_cli.api_request"
                && let (Some(id), Some(t)) = (a.str("prompt_id"), text(a.str("request_text")))
            {
                prompts.insert(id.to_owned(), t);
            }
        }
    }
    for g in groups {
        for r in &g.items {
            let a = &r.attributes;
            let name = event_name(&g.scope, r);
            match name.as_str() {
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
                    }
                    .normalized(InputCounts::WithCache, OutputCounts::WithoutReasoning);
                    let prompt_id = a.str("prompt_id").unwrap_or_default();
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
                        ttfb: None,
                        provider: Some("google".into()),
                        model: model.to_owned(),
                        status: match status_of(a.get("status_code")) {
                            Some(s) => s,
                            None if error => 502,
                            None => 200,
                        },
                        error_message: if error {
                            text(a.str("error.message")).or(Some("The request failed.".into()))
                        } else {
                            None
                        },
                        tokens,
                        reported_cost: None,
                        stream: true,
                        client: Some("GeminiCLI".into()),
                        email: a.str("user.email").map(str::to_owned),
                        tags: Vec::new(),
                        provider_request_id: None,
                        dialect: Some(DialectId::Gemini),
                        prompt: prompts.get(prompt_id).cloned(),
                        reply: text(a.str("response_text")),
                    });
                }
                "gemini_cli.tool_call" if a.str("tool_type") == Some("mcp") => {
                    let Some(tool) = a.str("function_name") else {
                        continue;
                    };
                    let duration = ms(a.u64("duration_ms"));
                    let ok = a.bool("success").unwrap_or(true);
                    batch.tool_calls.push(ObservedToolCall {
                        external_id: hashed(&[
                            &name,
                            a.str("session.id").unwrap_or_default(),
                            a.str("prompt_id").unwrap_or_default(),
                            &r.time_unix_nano.to_string(),
                            tool,
                        ]),
                        started_at: started(ended(r), duration),
                        duration,
                        tool: tool.to_owned(),
                        server: a.str("mcp_server_name").map(str::to_owned),
                        ok,
                        error_type: (!ok)
                            .then(|| a.str("error_type").map(str::to_owned))
                            .flatten(),
                        error_message: (!ok).then(|| text(a.str("error"))).flatten(),
                        client: Some("gemini-cli".into()),
                        email: a.str("user.email").map(str::to_owned),
                        arguments: text(a.str("function_args")),
                        result: None,
                    });
                }
                _ => {}
            }
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
    Some(
        Counted {
            input: input.unwrap_or(0),
            output: output.unwrap_or(0),
            cache_read: a.u64("gen_ai.usage.cache_read.input_tokens").unwrap_or(0),
            cache_write: a.u64("gen_ai.usage.cache_write.input_tokens").unwrap_or(0),
            reasoning: a.u64("gen_ai.usage.reasoning.output_tokens").unwrap_or(0),
        }
        .normalized(InputCounts::WithCache, OutputCounts::WithReasoning),
    )
}

/// The last text of a role in `gen_ai.input.messages`/`gen_ai.output.messages` (JSON), or the
/// value itself when it's plain text, as `gen_ai.prompt`/`gen_ai.completion` are.
fn semconv_text(v: Option<&str>, role: &str) -> Option<String> {
    let raw = text(v)?;
    let Ok(Json::Array(messages)) = serde_json::from_str::<Json>(&raw) else {
        return Some(raw);
    };
    messages
        .iter()
        .rev()
        .filter(|m| {
            m.get("role")
                .and_then(Json::as_str)
                .is_none_or(|r| r == role)
        })
        .find_map(|m| {
            let parts = m.get("parts").or(m.get("content"))?;
            match parts {
                Json::String(s) => Some(s.clone()),
                Json::Array(ps) => {
                    let t: Vec<&str> = ps
                        .iter()
                        .filter_map(|p| p.get("content").or(p.get("text")).and_then(Json::as_str))
                        .collect();
                    (!t.is_empty()).then(|| t.join("\n"))
                }
                _ => None,
            }
        })
}

/// Services whose spans and GenAI records repeat what their own log events already report.
const LEFT_OUT: &[&str] = &["claude-code", "gemini-cli"];

/// Instrumentation scopes of those clients, for when a person renamed the service.
const SCOPES_LEFT_OUT: &[&str] = &["gemini-cli", "com.anthropic.claude_code"];

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
        batch.tool_calls.push(ObservedToolCall {
            external_id: a
                .str("gen_ai.tool.call.id")
                .unwrap_or(fallback_id)
                .to_owned(),
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
    let inference = if operation.is_empty() {
        a.str("gen_ai.response.id").is_some()
    } else {
        INFERENCE.contains(&operation)
    };
    if !inference {
        return;
    }
    let Some(tokens) = semconv_tokens(a) else {
        return;
    };
    let Some(model) = a
        .str("gen_ai.response.model")
        .or(a.str("gen_ai.request.model"))
    else {
        refuse(batch, Refusal::NoModel);
        return;
    };
    let provider = a
        .str("gen_ai.provider.name")
        .or(a.str("gen_ai.system"))
        .map(semconv_provider);
    let status =
        status_of(a.get("http.response.status_code")).unwrap_or(if failed { 502 } else { 200 });
    batch.requests.push(ObservedRequest {
        external_id: a
            .str("gen_ai.response.id")
            .unwrap_or(fallback_id)
            .to_owned(),
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
        reported_cost: decimal(a.get("gen_ai.usage.cost").or(a.get("gen_ai.cost"))),
        stream: a.bool("gen_ai.request.stream").unwrap_or(false),
        client: Some(service).filter(|s| !s.is_empty()).map(str::to_owned),
        email,
        tags: Vec::new(),
        provider_request_id: a.str("gen_ai.response.id").map(str::to_owned),
        dialect: None,
        prompt: semconv_text(
            a.str("gen_ai.input.messages").or(a.str("gen_ai.prompt")),
            "user",
        ),
        reply: semconv_text(
            a.str("gen_ai.output.messages")
                .or(a.str("gen_ai.completion")),
            "assistant",
        ),
    });
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
            if name.starts_with("claude_code.")
                || name.starts_with("codex.")
                || name.starts_with("gemini_cli.")
            {
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
    semconv_logs(groups, &mut batch);
    batch
}

/// Every request and tool call in an OTLP traces export.
pub fn spans(groups: &[Group<Span>]) -> Batch {
    let mut batch = Batch::default();
    spans_of(groups, &mut batch);
    batch
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

/// Text of the last user message of an OpenAI-style message list. Content a source reports is
/// shown as text for people to read, never decoded as a request Kaeso would send, so it isn't
/// read through KPP.
fn last_user_text(messages: Option<&Json>) -> Option<String> {
    let list = messages?.as_array()?;
    list.iter()
        .rev()
        .find(|m| m.get("role").and_then(Json::as_str) == Some("user"))
        .and_then(|m| match m.get("content")? {
            Json::String(s) => Some(s.clone()),
            Json::Array(parts) => Some(
                parts
                    .iter()
                    .filter_map(|p| p.get("text").and_then(Json::as_str))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            _ => None,
        })
        .filter(|s| !s.is_empty())
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
                .filter(|d| !d.is_sign_negative()),
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
            prompt: last_user_text(p.get("messages")),
            reply: p
                .get("response")
                .and_then(|r| {
                    r.get("choices")?
                        .get(0)?
                        .get("message")?
                        .get("content")?
                        .as_str()
                })
                .map(str::to_owned),
        });
    }
    batch
}

// Antigravity -------------------------------------------------------------------------------

/// What Kaeso's Antigravity hook sends: the hook's input and the transcript records the model
/// call added, each a line of `transcript_full.jsonl`.
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
        let tokens = Counted {
            input: json_u64(s.get("input_tokens")).unwrap_or(0),
            output: json_u64(s.get("output_tokens")).unwrap_or(0),
            cache_read: json_u64(s.get("cache_read_tokens")).unwrap_or(0),
            cache_write: json_u64(s.get("cache_write_tokens"))
                .or(json_u64(s.get("cache_creation_input_tokens")))
                .unwrap_or(0),
            reasoning: 0,
        }
        .normalized(InputCounts::Uncached, OutputCounts::WithReasoning);
        batch.requests.push(ObservedRequest {
            external_id: format!("{conversation}:{step}"),
            started_at: at_rfc3339(json_str(s, &["created_at"])),
            duration: Duration::ZERO,
            ttfb: None,
            provider: Some("google".into()),
            model: model.to_owned(),
            status: 200,
            error_message: None,
            tokens,
            reported_cost: None,
            stream: true,
            client: Some(client(
                "Antigravity",
                json_str(hook, &["cliVersion"]).or(json_str(hook, &["version"])),
            )),
            email: json_str(hook, &["userEmail"]).map(str::to_owned),
            tags: Vec::new(),
            provider_request_id: None,
            dialect: Some(DialectId::Gemini),
            prompt: json_str(hook, &["lastUserInput"]).map(str::to_owned),
            reply: json_str(s, &["content"]).map(str::to_owned),
        });
    }
    batch
}

#[cfg(test)]
#[path = "map_tests.rs"]
mod tests;
