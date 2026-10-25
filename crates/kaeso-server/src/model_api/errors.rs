use std::time::Duration;

use axum::body::Body;
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::Response;
use kaeso_codecs::dispatch;
use kaeso_kpp::{DialectId, ErrorKind, KppError};

use crate::http::request_id;

pub const MODEL_HEADER: &str = "kaeso-model";
pub const PROVIDER_HEADER: &str = "kaeso-provider";
pub const LEFT_OUT_HEADER: &str = "kaeso-left-out";
pub const TIMING_HEADER: &str = "server-timing";

#[derive(Clone, Debug, Default)]
pub struct Served {
    pub model: Option<String>,
    pub provider: Option<String>,
    pub left_out: Vec<String>,
}

pub fn stamp(resp: &mut Response, served: &Served) {
    if let Some(m) = &served.model
        && let Ok(v) = HeaderValue::from_str(m)
    {
        resp.headers_mut().insert(MODEL_HEADER, v);
    }
    if let Some(p) = &served.provider
        && let Ok(v) = HeaderValue::from_str(p)
    {
        resp.headers_mut().insert(PROVIDER_HEADER, v);
    }
    if !served.left_out.is_empty()
        && let Ok(v) = HeaderValue::from_str(&served.left_out.join(", "))
    {
        resp.headers_mut().insert(LEFT_OUT_HEADER, v);
    }
}

pub fn server_timing(kaeso: Duration, upstream: Option<Duration>) -> HeaderValue {
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let value = match upstream {
        Some(u) => format!("kaeso;dur={:.3}, upstream;dur={:.3}", ms(kaeso), ms(u)),
        None => format!("kaeso;dur={:.3}", ms(kaeso)),
    };
    HeaderValue::from_str(&value).unwrap_or_else(|_| HeaderValue::from_static("kaeso"))
}

pub fn respond(dialect: DialectId, err: &KppError, served: &Served) -> Response {
    let encoded = match request_id::current() {
        Some(id) => dispatch::encode_error_for_request(dialect, err, &id.to_string()),
        None => dispatch::encode_error(dialect, err),
    };
    let mut resp = Response::new(Body::from(encoded.body));
    *resp.status_mut() =
        StatusCode::from_u16(encoded.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    for (name, value) in encoded.headers.iter() {
        if let (Ok(n), Ok(v)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            resp.headers_mut().append(n, v);
        }
    }
    stamp(&mut resp, served);
    resp
}

pub fn error(kind: ErrorKind, status: u16, message: impl Into<String>) -> KppError {
    KppError::new(kind, message).with_status(status)
}
