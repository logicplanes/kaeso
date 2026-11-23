//! Kaeso's own JSON for sources that aren't a known client: a batch of requests and tool calls.
//! Every field is named in the docs' HTTP ingest guide; a record with a field Kaeso doesn't know
//! is refused rather than guessed at.

use std::time::Duration;

use kaeso_core::sources::{Counted, InputCounts, OutputCounts};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::Value as Json;
use time::OffsetDateTime;

use crate::services::observed::{Batch, ObservedRequest, ObservedToolCall, Refusal};

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Events {
    #[serde(default)]
    pub requests: Vec<Json>,
    #[serde(default)]
    pub tool_calls: Vec<Json>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tokens {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_tokens: u64,
    #[serde(default)]
    cache_write_tokens: u64,
    #[serde(default)]
    reasoning_tokens: u64,
    /// The part of `cache_write_tokens` cached for an hour, which Anthropic bills higher.
    #[serde(default)]
    cache_write_1h_tokens: u64,
    /// Whether `input_tokens` holds the cached tokens too, as OpenAI and Gemini count them.
    #[serde(default)]
    input_includes_cache: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestIn {
    id: String,
    #[serde(default, with = "time::serde::rfc3339::option")]
    started_at: Option<OffsetDateTime>,
    #[serde(default)]
    duration_ms: Option<u64>,
    #[serde(default)]
    ttft_ms: Option<u64>,
    #[serde(default)]
    provider: Option<String>,
    model: String,
    #[serde(default)]
    status: Option<u16>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    usage: Option<Tokens>,
    /// A decimal string or number in USD.
    #[serde(default)]
    cost_usd: Option<Json>,
    #[serde(default)]
    stream: bool,
    #[serde(default)]
    client: Option<String>,
    #[serde(default)]
    user_email: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    provider_request_id: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    reply: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolCallIn {
    id: String,
    #[serde(default, with = "time::serde::rfc3339::option")]
    started_at: Option<OffsetDateTime>,
    #[serde(default)]
    duration_ms: Option<u64>,
    #[serde(default)]
    server: Option<String>,
    tool: String,
    #[serde(default = "yes")]
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    client: Option<String>,
    #[serde(default)]
    user_email: Option<String>,
    #[serde(default)]
    arguments: Option<Json>,
    #[serde(default)]
    result: Option<Json>,
}

const fn yes() -> bool {
    true
}

/// The most tags a request keeps, as the gateway's `kaeso-tags` allows.
const MAX_TAGS: usize = 10;

fn cost(v: Option<&Json>) -> Option<Decimal> {
    match v? {
        Json::String(s) => s.trim().parse().ok(),
        Json::Number(n) => n.to_string().parse().ok(),
        _ => None,
    }
}

fn text_of(v: Option<Json>) -> Option<String> {
    match v? {
        Json::String(s) => Some(s),
        Json::Null => None,
        other => Some(other.to_string()),
    }
}

fn request(r: RequestIn) -> Result<ObservedRequest, Refusal> {
    if r.id.trim().is_empty() {
        return Err(Refusal::NoId);
    }
    let tokens = r.usage.map_or(Counted::default(), |u| {
        Counted {
            input: u.input_tokens,
            output: u.output_tokens,
            cache_read: u.cache_read_tokens,
            cache_write: u.cache_write_tokens,
            reasoning: u.reasoning_tokens,
            cache_write_1h: u.cache_write_1h_tokens,
        }
        .normalized(
            if u.input_includes_cache {
                InputCounts::WithCache
            } else {
                InputCounts::Uncached
            },
            OutputCounts::WithReasoning,
        )
    });
    let status = r
        .status
        .unwrap_or(if r.error.is_some() { 502 } else { 200 });
    let reported_cost = match &r.cost_usd {
        None => None,
        Some(v) => Some(cost(Some(v)).ok_or(Refusal::CostTooHigh)?),
    };
    Ok(ObservedRequest {
        external_id: r.id,
        started_at: r.started_at,
        duration: Duration::from_millis(r.duration_ms.unwrap_or(0)),
        ttfb: r.ttft_ms.map(Duration::from_millis),
        provider: r.provider,
        model: r.model,
        status,
        error_message: r
            .error
            .or_else(|| (status >= 400).then(|| "The request failed.".into())),
        tokens,
        reported_cost,
        stream: r.stream,
        client: r.client,
        email: r.user_email,
        tags: r.tags.into_iter().take(MAX_TAGS).collect(),
        provider_request_id: r.provider_request_id,
        dialect: None,
        prompt: r.prompt,
        reply: r.reply,
    })
}

fn tool_call(c: ToolCallIn) -> Result<ObservedToolCall, Refusal> {
    if c.id.trim().is_empty() {
        return Err(Refusal::NoId);
    }
    Ok(ObservedToolCall {
        external_id: c.id,
        started_at: c.started_at,
        duration: Duration::from_millis(c.duration_ms.unwrap_or(0)),
        tool: c.tool,
        server: c.server,
        ok: c.ok,
        error_type: None,
        error_message: c.error,
        client: c.client,
        email: c.user_email,
        arguments: text_of(c.arguments),
        result: text_of(c.result),
    })
}

/// The records of a batch; one Kaeso can't read is refused with the reason, the rest are kept.
pub fn batch(e: Events) -> Batch {
    let mut out = Batch::default();
    let mut refuse = |why: Refusal| {
        out.refused += 1;
        out.refusal = Some(why);
    };
    let mut requests = Vec::new();
    for r in e.requests {
        match serde_json::from_value::<RequestIn>(r)
            .map_err(|_| Refusal::Unreadable)
            .and_then(request)
        {
            Ok(r) => requests.push(r),
            Err(why) => refuse(why),
        }
    }
    let mut calls = Vec::new();
    for c in e.tool_calls {
        match serde_json::from_value::<ToolCallIn>(c)
            .map_err(|_| Refusal::Unreadable)
            .and_then(tool_call)
        {
            Ok(c) => calls.push(c),
            Err(why) => refuse(why),
        }
    }
    out.requests = requests;
    out.tool_calls = calls;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn events(v: Json) -> Batch {
        batch(serde_json::from_value(v).unwrap())
    }

    #[test]
    fn a_request_reads_with_its_tokens_counted_the_way_kaeso_counts_them() {
        let b = events(json!({ "requests": [{
            "id": "run-1",
            "started_at": "2026-11-23T10:00:00Z",
            "duration_ms": 1200,
            "provider": "openai",
            "model": "gpt-5.4-mini",
            "usage": { "input_tokens": 1000, "cache_read_tokens": 800, "output_tokens": 50, "input_includes_cache": true },
            "cost_usd": "0.0012",
            "user_email": "ada@example.com",
            "tags": ["nightly"]
        }]}));
        assert_eq!(b.refused, 0);
        let r = &b.requests[0];
        assert_eq!(r.tokens.input, 200);
        assert_eq!(r.tokens.cache_read, 800);
        assert_eq!(r.status, 200);
        assert_eq!(r.reported_cost, Some("0.0012".parse().unwrap()));
        assert_eq!(r.email.as_deref(), Some("ada@example.com"));
    }

    #[test]
    fn a_record_with_a_field_kaeso_does_not_know_is_refused_and_the_rest_kept() {
        let b = events(json!({
            "requests": [
                { "id": "a", "model": "m", "colour": "blue" },
                { "id": "b", "model": "m", "error": "upstream timed out" },
                { "id": "", "model": "m" },
                { "id": "c", "model": "m", "cost_usd": "lots" }
            ],
            "tool_calls": [
                { "id": "t1", "server": "linear", "tool": "list_issues", "arguments": { "team": "ENG" } },
                { "id": "t2" }
            ]
        }));
        assert_eq!(b.requests.len(), 1);
        assert_eq!(b.requests[0].status, 502);
        assert_eq!(
            b.requests[0].error_message.as_deref(),
            Some("upstream timed out")
        );
        assert_eq!(b.tool_calls.len(), 1);
        assert_eq!(
            b.tool_calls[0].arguments.as_deref(),
            Some("{\"team\":\"ENG\"}")
        );
        assert_eq!(b.refused, 4);
    }

    #[test]
    fn an_unknown_field_of_the_batch_itself_fails_the_batch() {
        assert!(serde_json::from_value::<Events>(json!({ "requests": [], "spans": [] })).is_err());
    }
}
