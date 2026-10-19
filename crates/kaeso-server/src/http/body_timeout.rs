use std::time::Duration;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use tower_http::timeout::{TimeoutBody, TimeoutError};

pub async fn middleware(State(wait): State<Duration>, req: Request, next: Next) -> Response {
    next.run(req.map(|body| Body::new(TimeoutBody::new(wait, body))))
        .await
}

pub fn timed_out(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut cause = Some(error);
    while let Some(e) = cause {
        if e.is::<TimeoutError>() {
            return true;
        }
        cause = e.source();
    }
    false
}
