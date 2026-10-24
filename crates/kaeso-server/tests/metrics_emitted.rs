mod common;

use common::v1::{call, chat, messages};

#[tokio::test]
async fn model_requests_are_measured() {
    let handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .unwrap();
    kaeso_server::metrics::describe();
    let env = common::v1::env(None, Some(&[("KAESO_KEY_FAILURES_PER_ADDRESS", "1")])).await;
    call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "Hi")),
    )
    .await;
    call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("claude-haiku-4-5", "[mock:error=overloaded]")),
    )
    .await;
    call(
        &env,
        "/v1/messages",
        Some("sk-kso-wrong"),
        true,
        Some(messages("claude-haiku-4-5", "Hi")),
    )
    .await;
    for n in 0..2 {
        call(
            &env,
            "/v1/messages",
            Some(&format!("sk-kso-{n:0>48}")),
            true,
            Some(messages("claude-haiku-4-5", "Hi")),
        )
        .await;
    }
    let text = handle.render();
    for needle in [
        "kaeso_auth_failures_total{method=\"api_key\",reason=\"throttled\"} 1",
        "kaeso_key_failure_blocks_total 1",
        "kaeso_requests_total{surface=\"model_api\",route=\"/v1/messages\",status=\"200\",provider=\"anthropic\",model=\"claude-haiku-4-5\"}",
        "kaeso_request_duration_seconds",
        "kaeso_time_to_first_byte_seconds",
        "kaeso_gateway_overhead_seconds{provider=\"anthropic\",quantile=",
        "kaeso_tokens_total{provider=\"anthropic\",kind=\"output\"} 18",
        "kaeso_cost_microusd_total{provider=\"anthropic\"} 100",
        "kaeso_upstream_errors_total{kind=\"overloaded\",provider=\"anthropic\"} 1",
        "kaeso_auth_failures_total{method=\"api_key\",reason=\"refused\"} 2",
    ] {
        assert!(text.contains(needle), "missing `{needle}` in:\n{text}");
    }
    let mut cut = chat("claude-haiku-4-5", "[mock:cut]");
    cut["stream"] = serde_json::json!(true);
    call(
        &env,
        "/v1/chat/completions",
        Some(&env.key),
        false,
        Some(cut),
    )
    .await;
    let needle =
        "kaeso_stream_failures_total{kind=\"stream_interrupted\",provider=\"anthropic\"} 1";
    let text = handle.render();
    assert!(text.contains(needle), "missing `{needle}` in:\n{text}");

    let created = env
        .app
        .post(
            &format!("/orgs/{}/projects/{}/models", env.owner.org_id, env.project),
            serde_json::json!({
                "provider": "anthropic", "model_id": "house-model-7", "custom": true,
                "overrides": {
                    "input_modalities": ["text"], "output_modalities": ["text"],
                    "context_limit": 32000, "output_limit": 4000,
                    "input_price": "0.5", "output_price": "1.5",
                    "egress_dialect": "anthropic_messages"
                }
            }),
            Some(&env.owner.cookie),
        )
        .await;
    assert_eq!(created.status.as_u16(), 201, "{:?}", created.body);
    call(
        &env,
        "/v1/messages",
        Some(&env.key),
        true,
        Some(messages("house-model-7", "Hi")),
    )
    .await;
    let text = handle.render();
    assert!(
        text.contains("provider=\"anthropic\",model=\"custom\"}"),
        "custom models are labelled `custom`:\n{text}"
    );
    assert!(!text.contains("house-model-7"), "{text}");
}
