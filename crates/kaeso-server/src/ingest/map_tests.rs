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
    assert_eq!(r.provider.as_deref(), Some("openai"));
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
    assert_eq!(r.provider.as_deref(), Some("openai"));
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
    assert_eq!(
        (r.prompt.as_deref(), r.reply.as_deref()),
        (Some("Hi"), Some("Hello"))
    );
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
