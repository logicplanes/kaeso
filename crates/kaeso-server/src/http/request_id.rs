use axum::extract::Request;
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use kaeso_core::id::RequestId;
use tracing::Instrument;

pub const HEADER: &str = "kaeso-request-id";

tokio::task_local! {
    static CURRENT: RequestId;
}

pub fn current() -> Option<RequestId> {
    CURRENT.try_with(|id| *id).ok()
}

pub async fn middleware(mut req: Request, next: Next) -> Response {
    let id = RequestId::new();
    req.extensions_mut().insert(id);
    let span = tracing::info_span!(
        "request",
        request_id = %id,
        method = %req.method(),
        route = tracing::field::Empty,
        org = tracing::field::Empty,
        project = tracing::field::Empty,
        key = tracing::field::Empty,
        status = tracing::field::Empty,
    );
    let mut response = CURRENT
        .scope(id, next.run(req).instrument(span.clone()))
        .await;
    span.record("status", response.status().as_u16());
    if let Ok(value) = HeaderValue::from_str(&id.to_string()) {
        response.headers_mut().insert(HEADER, value);
    }
    response
}
