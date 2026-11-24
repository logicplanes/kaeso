#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use serde_json::json;

use super::*;
use crate::ingest::otlp;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/tests/server/fixtures/sources/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn json_logs(name: &str) -> Batch {
    logs(&otlp::logs_from_json(&fixture(name), usize::MAX).unwrap())
}

#[test]
fn claude_codes_api_requests_become_requests_with_its_own_token_counts() {
    let b = json_logs("claude-code-logs-1.json");
    assert_eq!(b.requests.len(), 1, "{b:#?}");
    assert_eq!(b.refused, 0);
    let r = &b.requests[0];
    assert_eq!(r.external_id, "req_01EXAMPLE0C4AA1B023B5AA98FFB4");
    assert_eq!(
        r.provider_request_id.as_deref(),
        Some("req_01EXAMPLE0C4AA1B023B5AA98FFB4")
    );
    assert_eq!(r.model, "claude-haiku-5-5");
    assert_eq!(r.provider.as_deref(), Some("anthropic"));
    assert_eq!(
        (
            r.tokens.input,
            r.tokens.output,
            r.tokens.cache_read,
            r.tokens.cache_write
        ),
        (2, 62, 11_873, 16_236)
    );
    assert_eq!(
        r.tokens.cache_write_1h, 16_236,
        "Claude Code caches for an hour"
    );
    assert_eq!(
        r.reported_cost,
        Some("0.0033971300000000004".parse().unwrap())
    );
    assert_eq!(r.duration, Duration::from_millis(899));
    assert_eq!(r.ttfb, Some(Duration::from_millis(749)));
    assert_eq!(r.client.as_deref(), Some("claude-cli/2.1.322"));
    assert_eq!(r.email.as_deref(), Some("ada@example.com"));
    assert_eq!(r.prompt, None, "a redacted prompt is no prompt");
    assert!(
        b.tool_calls.is_empty(),
        "Read is a built-in tool, not an MCP one"
    );
    let at = r.started_at.unwrap();
    assert_eq!(at.date().year(), 2026);
}

#[test]
fn the_same_export_as_protobuf_reads_the_same() {
    let pb = otlp::logs_from_protobuf(&fixture("claude-code-logs-2.pb"), usize::MAX).unwrap();
    let b = logs(&pb);
    assert_eq!(b.requests.len(), 1, "{b:#?}");
    assert_eq!(b.requests[0].model, "claude-haiku-5-5");
    assert!(b.requests[0].tokens.output > 0);
    for name in ["claude-code-logs-1.pb", "claude-code-logs-3.pb"] {
        assert!(
            otlp::logs_from_protobuf(&fixture(name), usize::MAX).is_ok(),
            "{name}"
        );
    }
}

#[test]
fn claude_codes_mcp_calls_keep_their_names_only_when_it_sends_them() {
    let named = json_logs("claude-code-mcp-named.json");
    assert_eq!(named.tool_calls.len(), 1, "{named:#?}");
    let c = &named.tool_calls[0];
    assert_eq!(c.server.as_deref(), Some("tracker"));
    assert_eq!(c.tool, "get_me");
    assert!(c.ok);
    assert_eq!(c.duration, Duration::from_millis(19));
    assert_eq!(c.external_id, "toolu_01EXAMPLEA79D9296683DBC17EF0E");
    let unnamed = json_logs("claude-code-mcp-unnamed.json");
    assert_eq!(unnamed.tool_calls.len(), 1, "{unnamed:#?}");
    assert_eq!(unnamed.tool_calls[0].server, None);
    assert_eq!(unnamed.tool_calls[0].tool, "mcp_tool");
    assert_eq!(unnamed.requests.len(), 2, "each API request of the turn");
}

#[test]
fn claude_code_spans_and_metrics_count_nothing_its_events_already_count() {
    let spans =
        spans(&otlp::spans_from_json(&fixture("claude-code-traces.json"), usize::MAX).unwrap());
    assert!(
        spans.requests.is_empty() && spans.tool_calls.is_empty(),
        "{spans:#?}"
    );
    assert!(otlp::metrics_from_json(&fixture("claude-code-metrics.json")).unwrap() > 0);
}

#[test]
fn codex_counts_a_request_once_its_usage_arrives_with_the_cache_taken_out() {
    let b = json_logs("codex-logs.json");
    assert_eq!(
        b.requests.len(),
        1,
        "the completion without usage isn't a request: {b:#?}"
    );
    let r = &b.requests[0];
    assert_eq!(r.model, "gpt-5.4-mini");
    assert_eq!(
        r.provider, None,
        "the export names no provider; the conversation's start would"
    );
    assert!(r.wants.iter().any(|w| w.what == Wanted::Provider));
    assert_eq!((r.tokens.input, r.tokens.output), (37, 11));
    assert_eq!(r.client.as_deref(), Some("codex_cli_rs/0.171.0"));
    assert_eq!(
        r.ttfb,
        Some(Duration::from_millis(34)),
        "the time to the response's headers"
    );
    assert!(r.duration >= Duration::from_millis(34));
}

fn record(attrs: serde_json::Value) -> serde_json::Value {
    let attributes: Vec<serde_json::Value> = attrs
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| {
            let value = match v {
                serde_json::Value::String(s) => json!({ "stringValue": s }),
                serde_json::Value::Bool(b) => json!({ "boolValue": b }),
                serde_json::Value::Number(n) if n.is_i64() => json!({ "intValue": n.to_string() }),
                serde_json::Value::Number(n) => json!({ "doubleValue": n }),
                _ => json!({}),
            };
            json!({ "key": k, "value": value })
        })
        .collect();
    json!({ "timeUnixNano": "1795388060384000000", "attributes": attributes })
}

fn export(service: &str, records: Vec<serde_json::Value>) -> Batch {
    let body = json!({ "resourceLogs": [{
        "resource": { "attributes": [{ "key": "service.name", "value": { "stringValue": service } }] },
        "scopeLogs": [{ "scope": { "name": service }, "logRecords": records }]
    }]});
    logs(&otlp::logs_from_json(body.to_string().as_bytes(), usize::MAX).unwrap())
}

#[test]
fn codex_mcp_calls_and_failed_responses_are_kept() {
    let b = export(
        "codex_exec",
        vec![
            record(
                json!({ "event.name": "codex.tool_result", "tool_name": "list_issues", "call_id": "call_1",
                "duration_ms": "120", "success": "false", "mcp_server": "linear", "arguments": "{}", "output": "boom" }),
            ),
            record(
                json!({ "event.name": "codex.tool_result", "tool_name": "shell", "call_id": "call_2", "success": "true" }),
            ),
            record(
                json!({ "event.name": "codex.api_request", "duration_ms": "40", "attempt": "1",
                "http.response.status_code": "429", "error.message": "rate limited", "model": "gpt-5.4-mini",
                "conversation.id": "c1", "event.timestamp": "2026-11-23T00:37:05.023Z" }),
            ),
            record(
                json!({ "event.name": "codex.api_request", "duration_ms": "30",
                "http.response.status_code": "200", "model": "gpt-5.4-mini",
                "conversation.id": "c1", "event.timestamp": "2026-11-23T00:37:06.023Z" }),
            ),
            record(
                json!({ "event.name": "codex.sse_event", "event.kind": "response.failed",
                "error.message": "stream disconnected", "model": "gpt-5.4-mini", "conversation.id": "c1",
                "event.timestamp": "2026-11-23T00:37:07.023Z" }),
            ),
        ],
    );
    assert_eq!(b.tool_calls.len(), 1, "only MCP tools are tool calls");
    assert_eq!(b.tool_calls[0].server.as_deref(), Some("linear"));
    assert!(!b.tool_calls[0].ok);
    assert_eq!(b.requests.len(), 2, "{b:#?}");
    assert_eq!(b.requests[0].status, 429);
    assert_eq!(b.requests[0].error_message.as_deref(), Some("rate limited"));
    assert_eq!(b.requests[1].status, 502);
    assert_eq!(
        b.requests[1].error_message.as_deref(),
        Some("stream disconnected")
    );
    assert_ne!(b.requests[0].external_id, b.requests[1].external_id);
}

#[test]
fn gemini_cli_counts_thoughts_as_output_and_its_genai_copy_not_at_all() {
    let b = json_logs("gemini-cli-logs.json");
    assert_eq!(b.requests.len(), 1, "{b:#?}");
    let r = &b.requests[0];
    assert_eq!(r.model, "gemini-3.6-flash");
    assert_eq!(r.provider.as_deref(), Some("google"));
    assert_eq!((r.tokens.input, r.tokens.output), (13, 19));
    assert_eq!(r.duration, Duration::from_millis(41));
    assert_eq!(r.status, 200);
    let thinking = export(
        "gemini-cli",
        vec![record(
            json!({ "event.name": "gemini_cli.api_response", "model": "gemini-2.5-pro",
            "input_token_count": 1000, "cached_content_token_count": 600, "output_token_count": 50,
            "thoughts_token_count": 200, "duration_ms": 900, "prompt_id": "p1" }),
        )],
    );
    let t = thinking.requests[0].tokens;
    assert_eq!(
        (t.input, t.cache_read, t.output, t.reasoning),
        (400, 600, 250, 200)
    );
}

#[test]
fn claude_code_errors_carry_their_status_and_message() {
    let b = export(
        "claude-code",
        vec![record(
            json!({ "event.name": "claude_code.api_error", "model": "claude-sonnet-5",
            "error": "Overloaded", "status_code": 529, "duration_ms": 3000, "attempt": 2,
            "client_request_id": "8dc962da" }),
        )],
    );
    let r = &b.requests[0];
    assert_eq!(
        (r.status, r.error_message.as_deref()),
        (529, Some("Overloaded"))
    );
    assert_eq!(r.external_id, "8dc962da:2");
    let nameless = export(
        "claude-code",
        vec![record(
            json!({ "event.name": "claude_code.api_request", "input_tokens": 3 }),
        )],
    );
    assert_eq!((nameless.requests.len(), nameless.refused), (0, 1));
    assert_eq!(nameless.refusal, Some(Refusal::NoModel));
}

#[test]
fn genai_spans_become_requests_and_mcp_tool_spans_tool_calls() {
    let span = |attrs: serde_json::Value, name: &str| {
        let attributes: Vec<serde_json::Value> = attrs
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| match v {
                serde_json::Value::String(s) => json!({ "key": k, "value": { "stringValue": s } }),
                other => json!({ "key": k, "value": { "intValue": other.to_string() } }),
            })
            .collect();
        json!({ "traceId": "5b8efff798038103d269b633813fc60c", "spanId": "eee19b7ec3c1b174",
            "name": name, "startTimeUnixNano": "1795388060000000000", "endTimeUnixNano": "1795388061500000000",
            "attributes": attributes })
    };
    let body = json!({ "resourceSpans": [{
        "resource": { "attributes": [{ "key": "service.name", "value": { "stringValue": "openrouter" } }] },
        "scopeSpans": [{ "spans": [
            span(json!({ "gen_ai.operation.name": "chat", "gen_ai.provider.name": "openai",
                "gen_ai.request.model": "gpt-5.4-mini", "gen_ai.response.id": "gen-123",
                "gen_ai.usage.input_tokens": 1200, "gen_ai.usage.output_tokens": 80,
                "gen_ai.usage.cache_read.input_tokens": 1000, "user.id": "u1" }), "chat gpt-5.4-mini"),
            span(json!({ "gen_ai.operation.name": "execute_tool", "gen_ai.tool.name": "search",
                "mcp.method.name": "tools/call", "server.address": "docs.example.com",
                "gen_ai.tool.call.id": "call_9" }), "tools/call search"),
            span(json!({ "gen_ai.operation.name": "execute_tool", "gen_ai.tool.name": "local_fn" }), "execute_tool local_fn"),
            span(json!({ "gen_ai.operation.name": "chat", "gen_ai.usage.input_tokens": 5 }), "chat")
        ]}]
    }]});
    let b = spans(&otlp::spans_from_json(body.to_string().as_bytes(), usize::MAX).unwrap());
    assert_eq!(b.requests.len(), 1, "{b:#?}");
    let r = &b.requests[0];
    assert_eq!(r.external_id, "gen-123");
    assert_eq!(
        (r.tokens.input, r.tokens.cache_read, r.tokens.output),
        (200, 1000, 80)
    );
    assert_eq!(r.duration, Duration::from_millis(1500));
    assert_eq!(
        r.provider.as_deref(),
        Some("openrouter"),
        "OpenRouter made the request it reports, whichever provider it routed to"
    );
    assert_eq!(b.tool_calls.len(), 1, "only MCP tool calls: {b:#?}");
    assert_eq!(b.tool_calls[0].server.as_deref(), Some("docs.example.com"));
    assert_eq!((b.refused, b.refusal), (1, Some(Refusal::NoModel)));
}

#[test]
fn litellm_payloads_become_requests_and_mcp_calls() {
    let b = litellm(&[
        json!({
            "id": "chatcmpl-1", "call_type": "acompletion", "model": "anthropic/claude-sonnet-5",
            "status": "success", "prompt_tokens": 1500, "completion_tokens": 40,
            "startTime": 1795388060.0, "endTime": 1795388061.25, "completionStartTime": 1795388060.5,
            "response_cost": 0.0123,
            "metadata": { "user_api_key_user_email": "ada@example.com",
                "usage_object": { "cache_read_input_tokens": 1000, "cache_creation_input_tokens": 300 } },
            "messages": [{ "role": "user", "content": "Hi" }],
            "response": { "choices": [{ "message": { "content": "Hello" } }] },
            "request_tags": ["nightly"]
        }),
        json!({ "id": "chatcmpl-2", "model": "gpt-5.4", "status": "failure",
            "error_str": "Rate limited", "error_information": { "error_code": "429" } }),
        json!({ "id": "mcp-1", "status": "success",
            "metadata": { "mcp_tool_call_metadata": { "name": "list_issues", "mcp_server_name": "linear",
                "arguments": { "team": "ENG" } } } }),
        json!({ "model": "x" }),
    ]);
    assert_eq!(b.requests.len(), 2, "{b:#?}");
    let r = &b.requests[0];
    assert_eq!(r.provider.as_deref(), Some("anthropic"));
    assert_eq!(r.model, "claude-sonnet-5");
    assert_eq!(
        (r.tokens.input, r.tokens.cache_read, r.tokens.cache_write),
        (200, 1000, 300)
    );
    assert_eq!(r.duration, Duration::from_millis(1250));
    assert_eq!(r.ttfb, Some(Duration::from_millis(500)));
    assert_eq!(r.reported_cost, Some("0.0123".parse().unwrap()));
    assert_eq!(r.prompt.as_deref(), Some("Hi"));
    assert_eq!(r.reply_parts, vec![kaeso_kpp::Block::text("Hello")]);
    assert_eq!(b.requests[1].status, 429);
    assert_eq!(b.tool_calls[0].server.as_deref(), Some("linear"));
    assert_eq!((b.refused, b.refusal), (1, Some(Refusal::NoId)));
}

#[test]
fn antigravity_steps_become_requests_keyed_by_conversation_and_step() {
    let hook = json!({ "conversationId": "ec33ebf9", "modelName": "gemini-pro-agent", "lastUserInput": "Fix the test" });
    let b = antigravity(
        &hook,
        &[
            json!({ "step_index": 1, "type": "USER_INPUT", "created_at": "2026-11-22T22:45:10Z" }),
            json!({ "step_index": 2, "type": "PLANNER_RESPONSE", "created_at": "2026-11-22T22:45:12Z",
                "model": "gemini-4-neon-high", "input_tokens": 10359, "cache_read_tokens": 2000, "output_tokens": 241 }),
            json!({ "type": "PLANNER_RESPONSE" }),
        ],
    );
    assert_eq!(b.requests.len(), 1, "{b:#?}");
    let r = &b.requests[0];
    assert_eq!(r.external_id, "ec33ebf9:2");
    assert_eq!(
        (r.tokens.input, r.tokens.cache_read, r.tokens.output),
        (10_359, 2_000, 241)
    );
    assert_eq!(r.model, "gemini-4-neon-high");
    assert_eq!((b.refused, b.refusal), (1, Some(Refusal::NoId)));
}

#[test]
fn records_of_unknown_events_are_left_alone() {
    let b = export(
        "claude-code",
        vec![record(
            json!({ "event.name": "claude_code.plugin_loaded", "plugin.name": "x" }),
        )],
    );
    assert_eq!(b, Batch::default());
}

#[test]
fn genai_records_keep_their_own_ids_and_agent_spans_count_nothing() {
    let b = export(
        "my-agent",
        vec![
            record(
                json!({ "event.name": "gen_ai.tool.call", "gen_ai.operation.name": "execute_tool",
                "gen_ai.tool.name": "search", "mcp.method.name": "tools/call", "server.address": "docs.example.com" }),
            ),
            record(
                json!({ "event.name": "gen_ai.client.inference.operation.details", "gen_ai.operation.name": "chat",
                "gen_ai.request.model": "gpt-5.4-mini", "gen_ai.usage.input_tokens": 10, "gen_ai.usage.output_tokens": 2 }),
            ),
            record(
                json!({ "event.name": "gen_ai.client.inference.operation.details", "gen_ai.operation.name": "chat",
                "gen_ai.request.model": "claude-haiku-4-5", "gen_ai.usage.input_tokens": 30, "gen_ai.usage.output_tokens": 4 }),
            ),
            record(
                json!({ "event.name": "gen_ai.agent.run", "gen_ai.operation.name": "invoke_agent",
                "gen_ai.request.model": "gpt-5.4-mini", "gen_ai.usage.input_tokens": 40, "gen_ai.usage.output_tokens": 6 }),
            ),
        ],
    );
    assert_eq!(
        b.requests.len(),
        2,
        "the agent's own record sums its calls: {b:#?}"
    );
    assert_eq!(b.tool_calls.len(), 1);
    let ids: std::collections::HashSet<&str> = b
        .requests
        .iter()
        .map(|r| r.external_id.as_str())
        .chain(b.tool_calls.iter().map(|c| c.external_id.as_str()))
        .collect();
    assert_eq!(ids.len(), 3, "every record has an id of its own: {ids:?}");
    assert!(!ids.contains(""));
}

#[test]
fn a_genai_record_and_its_span_name_the_same_call() {
    let body = json!({ "resourceLogs": [{
        "resource": { "attributes": [{ "key": "service.name", "value": { "stringValue": "my-agent" } }] },
        "scopeLogs": [{ "logRecords": [{
            "timeUnixNano": "1795388060384000000",
            "traceId": "5b8efff798038103d269b633813fc60c", "spanId": "eee19b7ec3c1b174",
            "attributes": [
                { "key": "gen_ai.operation.name", "value": { "stringValue": "chat" } },
                { "key": "gen_ai.request.model", "value": { "stringValue": "gpt-5.4-mini" } },
                { "key": "gen_ai.usage.input_tokens", "value": { "intValue": "10" } }
            ]
        }]}]
    }]});
    let b = logs(&otlp::logs_from_json(body.to_string().as_bytes(), usize::MAX).unwrap());
    assert_eq!(
        b.requests[0].external_id,
        "5b8efff798038103d269b633813fc60ceee19b7ec3c1b174"
    );
}

#[test]
fn antigravity_without_a_conversation_is_refused_and_litellm_names_its_provider() {
    let b = antigravity(
        &json!({ "modelName": "gemini-pro-agent" }),
        &[json!({ "step_index": 2, "type": "PLANNER_RESPONSE", "model": "m", "input_tokens": 1 })],
    );
    assert_eq!(
        (b.requests.len(), b.refused, b.refusal),
        (0, 1, Some(Refusal::NoId))
    );
    let l = litellm(&[
        json!({ "id": "a", "model": "meta-llama/Llama-3.3-70B-Instruct-Turbo", "custom_llm_provider": "togetherai",
            "status": "success", "prompt_tokens": 5, "completion_tokens": 1 }),
        json!({ "id": "b", "model": "meta-llama/Llama-3.3-70B-Instruct-Turbo", "status": "success" }),
        json!({ "id": "c", "model": "gpt-5.4-mini", "hidden_params": { "custom_llm_provider": "openai" }, "status": "success" }),
    ]);
    assert_eq!(l.requests[0].provider.as_deref(), Some("togetherai"));
    assert_eq!(
        l.requests[0].model,
        "meta-llama/Llama-3.3-70B-Instruct-Turbo"
    );
    assert_eq!(
        l.requests[1].provider, None,
        "an organisation is no provider"
    );
    assert_eq!(
        l.requests[1].model,
        "meta-llama/Llama-3.3-70B-Instruct-Turbo"
    );
    assert_eq!(l.requests[2].provider.as_deref(), Some("openai"));
}

fn record_at(attrs: serde_json::Value, nanos: u64) -> serde_json::Value {
    let mut r = record(attrs);
    r["timeUnixNano"] = json!(nanos.to_string());
    r
}

fn wants(r: &ObservedRequest, what: Wanted) -> Option<&str> {
    r.wants
        .iter()
        .find(|w| w.what == what)
        .map(|w| w.key.as_str())
}

fn carried(b: &Batch, what: Wanted) -> Vec<(&str, &str)> {
    b.carried
        .iter()
        .filter(|c| c.what == what)
        .map(|c| (c.key.as_str(), c.value.as_str()))
        .collect()
}

#[test]
fn codex_takes_its_prompt_start_and_provider_from_the_export_before_when_they_came_there() {
    let first = export(
        "codex_exec",
        vec![
            record_at(
                json!({ "event.name": "codex.conversation_starts", "provider_name": "OpenAI", "conversation.id": "c1" }),
                1_795_388_060_000_000_000,
            ),
            record_at(
                json!({ "event.name": "codex.user_prompt", "prompt": "Fix the build", "conversation.id": "c1" }),
                1_795_388_060_100_000_000,
            ),
            record_at(
                json!({ "event.name": "codex.api_request", "duration_ms": "300", "http.response.status_code": "200",
                "model": "gpt-5.4-mini", "conversation.id": "c1" }),
                1_795_388_060_500_000_000,
            ),
        ],
    );
    assert!(first.requests.is_empty(), "{first:#?}");
    assert_eq!(carried(&first, Wanted::Provider), [("codex:c1", "openai")]);
    assert_eq!(
        carried(&first, Wanted::Prompt),
        [("codex:c1", "Fix the build")]
    );
    assert_eq!(
        carried(&first, Wanted::Began),
        [("codex:c1", "1795388060200:300")],
        "the request began when its headers' wait did"
    );

    let second = export(
        "codex_exec",
        vec![record_at(
            json!({ "event.name": "codex.sse_event", "event.kind": "response.completed", "model": "gpt-5.4-mini",
            "conversation.id": "c1", "input_token_count": "40", "output_token_count": "8" }),
            1_795_388_064_200_000_000,
        )],
    );
    let r = &second.requests[0];
    assert_eq!(r.duration, Duration::ZERO, "until the start arrives");
    assert_eq!(wants(r, Wanted::Began), Some("codex:c1"));
    assert_eq!(wants(r, Wanted::Prompt), Some("codex:c1"));
    assert_eq!(wants(r, Wanted::Provider), Some("codex:c1"));

    let whole = export(
        "codex_exec",
        vec![
            record_at(
                json!({ "event.name": "codex.conversation_starts", "provider_name": "OpenRouter", "conversation.id": "c2" }),
                1_795_388_060_000_000_000,
            ),
            record_at(
                json!({ "event.name": "codex.user_prompt", "prompt": "[REDACTED]", "conversation.id": "c2" }),
                1_795_388_060_100_000_000,
            ),
            record_at(
                json!({ "event.name": "codex.api_request", "duration_ms": "300", "http.response.status_code": "200",
                "model": "gpt-5.4-mini", "conversation.id": "c2" }),
                1_795_388_060_500_000_000,
            ),
            record_at(
                json!({ "event.name": "codex.sse_event", "event.kind": "response.completed", "model": "gpt-5.4-mini",
                "conversation.id": "c2", "input_token_count": "40", "output_token_count": "8" }),
                1_795_388_062_200_000_000,
            ),
        ],
    );
    let r = &whole.requests[0];
    assert_eq!(
        r.provider.as_deref(),
        Some("openrouter"),
        "a provider of a person's own, named by its id"
    );
    assert_eq!(r.duration, Duration::from_millis(2_000));
    assert_eq!(r.ttfb, Some(Duration::from_millis(300)));
    assert_eq!(r.prompt, None, "a redacted prompt is no prompt");
    assert!(r.wants.is_empty(), "the batch had all of it: {:?}", r.wants);
    assert!(whole.carried.iter().all(|c| c.what == Wanted::Provider));
    assert_eq!(
        codex_provider("gpt-oss").as_deref(),
        Some(UNNAMED_PROVIDER),
        "Ollama and LM Studio alike: a local server, priced as nothing"
    );
    assert_eq!(
        codex_provider("Amazon Bedrock").as_deref(),
        Some("amazon-bedrock")
    );
    assert_eq!(codex_provider("My proxy"), None);
}

#[test]
fn codex_times_a_request_from_its_own_call_not_from_a_memory_summary() {
    let b = export(
        "codex_exec",
        vec![
            record_at(
                json!({ "event.name": "codex.api_request", "duration_ms": "100", "http.response.status_code": "200",
                "endpoint": "/responses", "model": "gpt-5.4-mini", "conversation.id": "c1" }),
                1_795_388_060_100_000_000,
            ),
            record_at(
                json!({ "event.name": "codex.api_request", "duration_ms": "50", "http.response.status_code": "200",
                "endpoint": "/memories/trace_summarize", "model": "gpt-5.4-mini", "conversation.id": "c1" }),
                1_795_388_061_000_000_000,
            ),
            record_at(
                json!({ "event.name": "codex.sse_event", "event.kind": "response.completed", "model": "gpt-5.4-mini",
                "conversation.id": "c1", "input_token_count": "40", "output_token_count": "8" }),
                1_795_388_062_000_000_000,
            ),
        ],
    );
    assert_eq!(b.requests[0].duration, Duration::from_millis(2_000));
    assert_eq!(b.requests[0].ttfb, Some(Duration::from_millis(100)));
}

#[test]
fn claude_codes_prompt_goes_with_its_request_when_it_came_one_export_earlier() {
    let first = export(
        "claude-code",
        vec![record(
            json!({ "event.name": "claude_code.user_prompt", "prompt.id": "p1", "prompt": "Explain main.rs" }),
        )],
    );
    assert_eq!(
        carried(&first, Wanted::Prompt),
        [("claude_code:p1", "Explain main.rs")]
    );
    let second = export(
        "claude-code",
        vec![
            record(
                json!({ "event.name": "claude_code.api_request", "model": "claude-haiku-5-5", "prompt.id": "p1",
                "request_id": "req_1", "input_tokens": 3, "output_tokens": 2 }),
            ),
            record(
                json!({ "event.name": "claude_code.api_request", "model": "claude-haiku-5-5", "prompt.id": "p1",
                "request_id": "req_2", "input_tokens": 3, "output_tokens": 2 }),
            ),
        ],
    );
    assert_eq!(
        wants(&second.requests[0], Wanted::Prompt),
        Some("claude_code:p1")
    );
    assert_eq!(
        wants(&second.requests[1], Wanted::Prompt),
        None,
        "only the first request of a prompt goes with it"
    );
}

#[test]
fn gemini_cli_reads_the_whole_exchange_its_provider_and_mcp_tool_names() {
    let contents = json!([
        { "role": "user", "parts": [{ "text": "List the files" }] },
        { "role": "model", "parts": [{ "functionCall": { "name": "mcp_files_list", "args": { "path": "." } } }] },
        { "role": "user", "parts": [{ "functionResponse": { "name": "mcp_files_list", "response": { "output": "a.rs" } } }] }
    ]);
    let reply = json!([
        { "candidates": [{ "content": { "role": "model", "parts": [{ "text": "There is " }] } }] },
        { "candidates": [{ "content": { "role": "model", "parts": [{ "text": "a.rs." }] } }] }
    ]);
    let b = export(
        "gemini-cli",
        vec![
            record(
                json!({ "event.name": "gemini_cli.user_prompt", "prompt_id": "p1", "prompt": "List the files",
                "auth_type": "vertex-ai" }),
            ),
            record(
                json!({ "event.name": "gemini_cli.api_request", "prompt_id": "p1", "model": "gemini-3-flash",
                "request_text": contents.to_string() }),
            ),
            record(
                json!({ "event.name": "gemini_cli.api_response", "prompt_id": "p1", "model": "gemini-3-flash",
                "auth_type": "vertex-ai", "input_token_count": 20, "output_token_count": 5,
                "response_text": reply.to_string() }),
            ),
            record(
                json!({ "event.name": "gemini_cli.tool_call", "tool_type": "mcp", "function_name": "mcp_my_files_list",
                "mcp_server_name": "my files", "prompt_id": "p1", "success": true }),
            ),
        ],
    );
    let r = &b.requests[0];
    assert_eq!(r.provider.as_deref(), Some("gemini-agent-platform"));
    assert_eq!(r.prompt.as_deref(), Some("List the files"));
    assert_eq!(r.messages.len(), 3, "the whole conversation, not its JSON");
    assert_eq!(r.reply, None);
    assert_eq!(
        r.reply_parts,
        vec![kaeso_kpp::Block::text("There is a.rs.")]
    );
    assert!(r.wants.is_empty(), "{:?}", r.wants);
    assert_eq!(
        b.tool_calls[0].tool, "list",
        "without the `mcp_<server>_` before it"
    );
    assert_eq!(b.tool_calls[0].server.as_deref(), Some("my files"));
    assert_eq!(
        gemini_provider(Some("gemini-api-key")).as_deref(),
        Some("google")
    );
    assert_eq!(gemini_provider(Some("oauth-personal")), None);

    let split = export(
        "gemini-cli",
        vec![record(
            json!({ "event.name": "gemini_cli.api_request", "prompt_id": "p2", "model": "gemini-3-flash",
            "request_text": contents.to_string() }),
        )],
    );
    assert_eq!(
        carried(&split, Wanted::GeminiContents),
        [("gemini_cli:p2", contents.to_string().as_str())]
    );
}

#[test]
fn grok_builds_requests_count_their_cache_price_and_turn() {
    let b = export(
        "grok-cli",
        vec![
            record(
                json!({ "event.name": "grok_code.user_prompt", "prompt.id": "p1", "prompt": "Add a test" }),
            ),
            record(
                json!({ "event.name": "grok_code.api_request", "prompt.id": "p1", "session.id": "s1", "event.sequence": 4,
                "model": "grok-code-fast-2", "duration_ms": 1800, "input_tokens": 12000, "cache_read_tokens": 10000,
                "output_tokens": 300, "reasoning_tokens": 120, "cost_usd_micros": 2150 }),
            ),
            record(
                json!({ "event.name": "grok_code.api_request", "prompt.id": "p1", "session.id": "s1", "event.sequence": 9,
                "model": "grok-code-fast-2", "duration_ms": 900, "input_tokens": 12400, "cache_read_tokens": 12000,
                "output_tokens": 40 }),
            ),
            record(
                json!({ "event.name": "grok_code.assistant_response", "prompt.id": "p1", "response": "Added." }),
            ),
            record(
                json!({ "event.name": "grok_code.api_error", "prompt.id": "p1", "model": "grok-code-fast-2",
                "error_category": "rate_limit" }),
            ),
            record(
                json!({ "event.name": "grok_code.tool_result", "tool_name": "linear__list_issues", "tool_use_id": "call_1",
                "mcp_server.name": "linear", "mcp_tool.name": "list_issues", "success": true, "duration_ms": 80 }),
            ),
            record(
                json!({ "event.name": "grok_code.tool_result", "tool_name": "mcp_tool", "tool_use_id": "call_2",
                "mcp_server.name": "mcp_server", "mcp_tool.name": "mcp_tool", "success": false, "outcome": "error" }),
            ),
            record(
                json!({ "event.name": "grok_code.tool_result", "tool_name": "read_file", "tool_use_id": "call_3", "success": true }),
            ),
        ],
    );
    assert_eq!(b.requests.len(), 3, "{b:#?}");
    let r = &b.requests[0];
    assert_eq!(r.provider.as_deref(), Some("xai"));
    assert_eq!(
        (
            r.tokens.input,
            r.tokens.cache_read,
            r.tokens.output,
            r.tokens.reasoning
        ),
        (2_000, 10_000, 420, 120)
    );
    assert_eq!(r.reported_cost, Some("0.00215".parse().unwrap()));
    assert_eq!(r.prompt.as_deref(), Some("Add a test"));
    assert_eq!(r.reply, None);
    assert_eq!(b.requests[1].prompt, None);
    assert_eq!(
        b.requests[1].reply.as_deref(),
        Some("Added."),
        "the turn's reply follows its last request"
    );
    assert_eq!(b.requests[2].status, 429);
    assert_ne!(b.requests[0].external_id, b.requests[1].external_id);
    assert_eq!(b.tool_calls.len(), 2, "only MCP tools: {b:#?}");
    assert_eq!(
        (
            b.tool_calls[0].server.as_deref(),
            b.tool_calls[0].tool.as_str()
        ),
        (Some("linear"), "list_issues")
    );
    assert_eq!(
        (
            b.tool_calls[1].server.as_deref(),
            b.tool_calls[1].tool.as_str(),
            b.tool_calls[1].ok
        ),
        (None, "mcp_tool", false)
    );
    assert_eq!(b.refused, 0);
}

#[test]
fn antigravity_reports_mcp_tool_calls_with_their_results_and_names_no_provider() {
    let hook = json!({ "conversationId": "ec33ebf9", "lastUserInput": "Close the issue" });
    let b = antigravity(
        &hook,
        &[
            json!({ "step_index": 4, "type": "PLANNER_RESPONSE", "created_at": "2026-11-22T22:45:12Z",
                "model": "gemini-3.8-flash-low", "input_tokens": 900, "output_tokens": 40, "thinking": "Use Linear.",
                "tool_calls": [
                    { "name": "view_file", "args": { "Path": "a.rs" } },
                    { "name": "mcp_linear_close_issue", "args": { "id": "ENG-1" } },
                    { "name": "call_mcp_tool", "args": { "ServerName": "github", "ToolName": "merge" } }
                ] }),
            json!({ "step_index": 5, "type": "VIEW_FILE", "status": "DONE", "created_at": "2026-11-22T22:45:13Z" }),
            json!({ "step_index": 6, "type": "MCP_TOOL", "status": "DONE", "created_at": "2026-11-22T22:45:15Z" }),
            json!({ "step_index": 7, "type": "MCP_TOOL", "status": "ERROR", "created_at": "2026-11-22T22:45:16Z",
                "content": "Not allowed" }),
            json!({ "step_index": 8, "type": "PLANNER_RESPONSE", "created_at": "2026-11-22T22:45:18Z",
                "model": "claude-sonnet-5-5", "input_tokens": 950, "output_tokens": 12, "content": "Done." }),
        ],
    );
    assert_eq!(b.requests.len(), 2);
    assert!(
        b.requests.iter().all(|r| r.provider.is_none()),
        "the model decides, Gemini or Claude"
    );
    assert_eq!(b.requests[0].prompt.as_deref(), Some("Close the issue"));
    assert_eq!(b.requests[1].prompt, None);
    let kinds: Vec<&str> = b.requests[0]
        .reply_parts
        .iter()
        .map(kaeso_kpp::Block::kind)
        .collect();
    assert_eq!(kinds, ["reasoning", "tool_call", "tool_call", "tool_call"]);
    assert_eq!(b.tool_calls.len(), 2, "MCP calls only: {b:#?}");
    let linear = &b.tool_calls[0];
    assert_eq!(
        (linear.server.as_deref(), linear.tool.as_str()),
        (Some("linear"), "close_issue")
    );
    assert!(linear.ok);
    assert_eq!(linear.duration, Duration::from_secs(3));
    let github = &b.tool_calls[1];
    assert_eq!(
        (github.server.as_deref(), github.tool.as_str(), github.ok),
        (Some("github"), "merge", false)
    );
    assert_eq!(github.error_message.as_deref(), Some("Not allowed"));
}

fn trace_export(service: &str, spans: Vec<serde_json::Value>) -> Batch {
    let body = json!({ "resourceSpans": [{
        "resource": { "attributes": [{ "key": "service.name", "value": { "stringValue": service } }] },
        "scopeSpans": [{ "spans": spans }]
    }]});
    super::spans(&otlp::spans_from_json(body.to_string().as_bytes(), usize::MAX).unwrap())
}

fn span_with(attrs: serde_json::Value) -> serde_json::Value {
    let mut s = record(attrs);
    s["traceId"] = json!("5b8efff798038103d269b633813fc60c");
    s["spanId"] = json!("eee19b7ec3c1b174");
    s["name"] = json!("LLM Generation");
    s["startTimeUnixNano"] = json!("1795388060000000000");
    s["endTimeUnixNano"] = json!("1795388061000000000");
    s
}

#[test]
fn openrouter_generations_count_without_an_operation_with_their_cost_cache_and_user() {
    let b = trace_export(
        "openrouter",
        vec![span_with(json!({
            "gen_ai.system": "anthropic", "gen_ai.request.model": "anthropic/claude-sonnet-5",
            "gen_ai.usage.input_tokens": 1200, "gen_ai.usage.input_tokens.cached": 1000,
            "gen_ai.usage.output_tokens": 50, "gen_ai.usage.output_tokens.reasoning": 10,
            "gen_ai.usage.total_cost": 0.0042, "user.id": "ada@example.com",
            "gen_ai.prompt": "Hi", "gen_ai.completion": "Hello"
        }))],
    );
    assert_eq!(b.requests.len(), 1, "{b:#?}");
    let r = &b.requests[0];
    assert_eq!(r.provider.as_deref(), Some("openrouter"));
    assert_eq!(r.model, "anthropic/claude-sonnet-5");
    assert_eq!(
        (r.tokens.input, r.tokens.cache_read, r.tokens.reasoning),
        (200, 1_000, 10)
    );
    assert_eq!(r.reported_cost, Some("0.0042".parse().unwrap()));
    assert_eq!(r.email.as_deref(), Some("ada@example.com"));
    assert_eq!(
        (r.prompt.as_deref(), r.reply.as_deref()),
        (Some("Hi"), Some("Hello"))
    );
}

#[test]
fn a_genai_span_and_the_log_event_written_in_it_share_an_id_whichever_names_the_response() {
    let span = trace_export(
        "my-agent",
        vec![span_with(json!({
            "gen_ai.operation.name": "chat", "gen_ai.request.model": "gpt-5.4-mini", "gen_ai.response.id": "resp_1",
            "gen_ai.usage.input_tokens": 10, "gen_ai.usage.output_tokens": 2
        }))],
    );
    let body = json!({ "resourceLogs": [{
        "resource": { "attributes": [{ "key": "service.name", "value": { "stringValue": "my-agent" } }] },
        "scopeLogs": [{ "logRecords": [{
            "timeUnixNano": "1795388061000000000",
            "traceId": "5b8efff798038103d269b633813fc60c", "spanId": "eee19b7ec3c1b174",
            "attributes": [
                { "key": "gen_ai.operation.name", "value": { "stringValue": "chat" } },
                { "key": "gen_ai.request.model", "value": { "stringValue": "gpt-5.4-mini" } },
                { "key": "gen_ai.usage.input_tokens", "value": { "intValue": "10" } }
            ]
        }]}]
    }]});
    let event = logs(&otlp::logs_from_json(body.to_string().as_bytes(), usize::MAX).unwrap());
    let ids = |r: &ObservedRequest| {
        std::iter::once(r.external_id.clone())
            .chain(r.aliases.iter().cloned())
            .collect::<std::collections::HashSet<_>>()
    };
    let (s, e) = (ids(&span.requests[0]), ids(&event.requests[0]));
    assert!(!s.is_disjoint(&e), "{s:?} and {e:?} name the same call");
}

#[test]
fn a_batch_of_records_no_reader_knows_is_refused_whole() {
    let b = export(
        "some-cli",
        vec![
            record(json!({ "event.name": "some_cli.api_call", "model": "m", "tokens": 5 })),
            record(json!({ "event.name": "some_cli.session_start" })),
        ],
    );
    assert_eq!((b.refused, b.refusal), (2, Some(Refusal::Unrecognized)));
    let spans = trace_export(
        "web",
        vec![span_with(
            json!({ "http.request.method": "GET", "url.path": "/" }),
        )],
    );
    assert_eq!(
        (spans.refused, spans.refusal),
        (1, Some(Refusal::Unrecognized))
    );
    let known = export(
        "claude-code",
        vec![record(json!({ "event.name": "claude_code.plugin_loaded" }))],
    );
    assert_eq!(
        known,
        Batch::default(),
        "a known client's other events are left alone"
    );
}

#[test]
fn litellm_keeps_the_whole_exchange_and_takes_a_cost_it_couldnt_work_out_as_unknown() {
    let b = litellm(&[
        json!({
            "id": "chatcmpl-9", "model": "gpt-5.4", "status": "success", "prompt_tokens": 30, "completion_tokens": 9,
            "response_cost": 0.0,
            "response_cost_failure_debug_info": { "error_str": "model not mapped", "traceback_str": "" },
            "messages": [
                { "role": "system", "content": "Be brief." },
                { "role": "user", "content": "Weather?" },
                { "role": "assistant", "content": null, "tool_calls": [{ "id": "c1", "type": "function",
                    "function": { "name": "weather", "arguments": "{}" } }] },
                { "role": "tool", "tool_call_id": "c1", "content": "Sunny" }
            ],
            "response": { "choices": [{ "message": { "content": "Sunny.", "reasoning_content": "From the tool." } }] }
        }),
        json!({
            "id": "chatcmpl-10", "model": "llama3", "status": "success", "prompt_tokens": 3, "completion_tokens": 1,
            "response_cost": 0.0
        }),
    ]);
    let r = &b.requests[0];
    assert_eq!(r.reported_cost, None);
    assert_eq!(r.system.as_deref(), Some("Be brief."));
    assert_eq!(r.messages.len(), 3);
    let kinds: Vec<&str> = r.reply_parts.iter().map(kaeso_kpp::Block::kind).collect();
    assert_eq!(kinds, ["reasoning", "text"]);
    assert_eq!(
        b.requests[1].reported_cost,
        Some(Decimal::ZERO),
        "a cost LiteLLM worked out to 0 is $0"
    );
}
