use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::Response;

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

pub async fn middleware(req: Request, next: Next) -> Response {
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
    let (provider, model) = (header("kaeso-provider"), header("kaeso-model"));
    metrics::counter!(crate::metrics::REQUESTS, "surface" => surface, "route" => route.clone(), "status" => status, "provider" => provider, "model" => model).increment(1);
    metrics::histogram!(crate::metrics::REQUEST_DURATION, "surface" => surface, "route" => route)
        .record(started.elapsed().as_secs_f64());
    resp
}
