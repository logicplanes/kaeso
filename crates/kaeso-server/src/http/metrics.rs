use std::time::Instant;

use axum::extract::{MatchedPath, Request, State};
use axum::middleware::Next;
use axum::response::Response;

use crate::state::AppState;

const CUSTOM_MODEL: &str = "custom";

fn surface(path: &str) -> &'static str {
    if path.starts_with("/v1/") {
        "model_api"
    } else if path.starts_with("/platform/") {
        "platform_api"
    } else if path == "/healthz" || path == "/readyz" {
        "health"
    } else {
        "other"
    }
}

// An organization's own provider ids would make the series unbounded and show one
// tenant's names to whoever scrapes the metrics.
fn provider_label(provider: String) -> String {
    if provider.is_empty() || kaeso_core::provider::provider(&provider).is_some() {
        provider
    } else {
        kaeso_core::providers::METRIC_LABEL.to_owned()
    }
}

fn model_label(state: &AppState, provider: &str, model: String) -> String {
    let listed = state
        .kpmr
        .current()
        .snapshot
        .providers
        .get(provider)
        .is_some_and(|p| p.models.contains_key(&model));
    if model.is_empty() || listed {
        model
    } else {
        CUSTOM_MODEL.to_owned()
    }
}

pub async fn middleware(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let surface = surface(req.uri().path());
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| "unmatched".to_owned(), |m| m.as_str().to_owned());
    let started = Instant::now();
    let resp = next.run(req).await;
    let header = |name: &str| {
        resp.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned()
    };
    let status = resp.status().as_u16().to_string();
    let provider = provider_label(header("kaeso-provider"));
    let model = model_label(&state, &provider, header("kaeso-model"));
    metrics::counter!(crate::metrics::REQUESTS, "surface" => surface, "route" => route.clone(), "status" => status, "provider" => provider, "model" => model).increment(1);
    metrics::histogram!(crate::metrics::REQUEST_DURATION, "surface" => surface, "route" => route)
        .record(started.elapsed().as_secs_f64());
    resp
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_custom_providers_id_never_becomes_a_metrics_label() {
        assert_eq!(super::provider_label("anthropic".into()), "anthropic");
        assert_eq!(super::provider_label(String::new()), "");
        assert_eq!(super::provider_label("acme-vllm".into()), "custom");
    }
}
