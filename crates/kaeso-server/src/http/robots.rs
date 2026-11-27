use axum::Router;
use axum::extract::Request;
use axum::http::HeaderValue;
use axum::http::header::{self, HeaderName};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::state::AppState;

pub const HEADER: &str = "x-robots-tag";
pub const NOINDEX: &str = "noindex, nofollow";
pub const ROBOTS_TXT: &str = "User-agent: *\nDisallow: /\n";

// Whatever `search_engines` says: the gateway answers apps and the console, never a page a visitor
// starts from. The console's server serves the pages that setting opens to crawlers.
pub fn router() -> Router<AppState> {
    Router::new().route("/robots.txt", get(robots_txt))
}

async fn robots_txt() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        ROBOTS_TXT,
    )
}

pub async fn middleware(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    response.headers_mut().insert(
        HeaderName::from_static(HEADER),
        HeaderValue::from_static(NOINDEX),
    );
    response
}
