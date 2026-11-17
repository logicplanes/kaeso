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
pub const ADAPTED_HEADER: &str = "kaeso-adapted";
pub const ROUTE_HEADER: &str = "kaeso-route";
pub const TIMING_HEADER: &str = "server-timing";
pub const UPSTREAM_REQUEST_ID_HEADER: &str = "kaeso-upstream-request-id";
pub const DEPRECATION_HEADER: &str = "deprecation";
pub const SUNSET_HEADER: &str = "sunset";

#[derive(Clone, Debug, Default)]
pub struct Served {
    pub model: Option<String>,
    pub provider: Option<String>,
    pub left_out: Vec<String>,
    pub adapted: Vec<String>,
    pub route: Vec<Walked>,
    /// The id the provider gave its answer, for its support: `request-id`, `x-request-id` or
    /// Cloudflare AI Gateway's `cf-aig-log-id`. Absent on a cache hit and before any answer.
    pub upstream_request_id: Option<String>,
    /// For a model its provider retires: the dates and links its replies carry.
    pub lifecycle: Option<LifecycleHeaders>,
}

/// What a reply of a deprecated model says about its retirement.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LifecycleHeaders {
    pub deprecated_on: Option<time::Date>,
    pub shutdown_on: Option<time::Date>,
    /// The replacement's page.
    pub successor: Option<String>,
    /// The provider's announcement.
    pub announcement: Option<String>,
}

impl LifecycleHeaders {
    /// `Deprecation` as an RFC 9651 date (`@` and Unix seconds), `Sunset` as an HTTP date and
    /// `Link` with `successor-version` and `deprecation` relations.
    pub fn stamp(&self, resp: &mut Response) {
        if let Some(d) = self.deprecated_on {
            let at = d.midnight().assume_utc().unix_timestamp();
            if let Ok(v) = HeaderValue::from_str(&format!("@{at}")) {
                resp.headers_mut().insert(DEPRECATION_HEADER, v);
            }
        }
        if let Some(d) = self.shutdown_on
            && let Ok(v) = HeaderValue::from_str(&http_date(d))
        {
            resp.headers_mut().insert(SUNSET_HEADER, v);
        }
        let links: Vec<String> = [
            self.successor
                .as_ref()
                .map(|u| format!("<{u}>; rel=\"successor-version\"")),
            self.announcement
                .as_ref()
                .map(|u| format!("<{u}>; rel=\"deprecation\"; type=\"text/html\"")),
        ]
        .into_iter()
        .flatten()
        .collect();
        if !links.is_empty()
            && let Ok(v) = HeaderValue::from_str(&links.join(", "))
        {
            resp.headers_mut().append(axum::http::header::LINK, v);
        }
    }
}

/// An IMF-fixdate at midnight UTC, as `Sunset` takes it.
pub fn http_date(d: time::Date) -> String {
    d.format(time::macros::format_description!(
        "[weekday repr:short], [day] [month repr:short] [year] 00:00:00 GMT"
    ))
    .unwrap_or_default()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Walked {
    pub path: String,
    pub model: Option<String>,
    pub tried: Option<Tried>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tried {
    pub status: u16,
    pub duration_ms: Option<u64>,
    /// How many requests the step sent: its retries and the keys of the project's pool it tried.
    pub tries: u32,
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
    if !served.adapted.is_empty()
        && let Ok(v) = HeaderValue::from_str(&served.adapted.join(", "))
    {
        resp.headers_mut().insert(ADAPTED_HEADER, v);
    }
    if let Some(id) = &served.upstream_request_id
        && let Ok(v) = HeaderValue::from_str(id)
    {
        resp.headers_mut().insert(UPSTREAM_REQUEST_ID_HEADER, v);
    }
    if let Some(l) = &served.lifecycle {
        l.stamp(resp);
    }
    stamp_route(resp, &served.route);
}

/// How much of each enforced rate limit and hard budget the app has left, as an RFC 9651 list:
/// `"key.requests.minute";limit=60;remaining=41;reset=17`. Spend is a USD string.
pub const LIMITS_HEADER: &str = "kaeso-limits";

/// The `kaeso-limits` value for what admission found; spend only for someone who may see costs.
pub fn limits_header(left: &[super::hooks::Left], with_spend: bool) -> Option<HeaderValue> {
    let usd = |nano: i64| {
        let d = rust_decimal::Decimal::new(nano, 9).normalize();
        format!("\"{d}\"")
    };
    let items: Vec<String> = left
        .iter()
        .filter(|l| with_spend || !l.spend)
        .map(|l| {
            let mut item = format!("\"{}\"", l.name);
            if l.model {
                item.push_str(";model");
            }
            if l.spend {
                item.push_str(&format!(
                    ";limit={};remaining={}",
                    usd(l.limit),
                    usd(l.remaining)
                ));
            } else {
                item.push_str(&format!(";limit={};remaining={}", l.limit, l.remaining));
            }
            if let Some(reset) = l.reset {
                item.push_str(&format!(";reset={reset}"));
            }
            item
        })
        .collect();
    if items.is_empty() {
        return None;
    }
    HeaderValue::from_str(&items.join(", ")).ok()
}

pub fn stamp_route(resp: &mut Response, route: &[Walked]) {
    if let Some(v) = route_header(route) {
        resp.headers_mut().insert(ROUTE_HEADER, v);
    }
}

// An RFC 9651 structured-field list: each step as a string with parameters, so apps can
// parse it with any structured-field library.
fn route_header(route: &[Walked]) -> Option<HeaderValue> {
    if route.is_empty() {
        return None;
    }
    let items: Vec<String> = route
        .iter()
        .map(|w| {
            let mut item = sf_string(&w.path).unwrap_or_else(|| "\"\"".to_owned());
            if let Some(model) = w.model.as_deref().and_then(sf_string) {
                item.push_str(";model=");
                item.push_str(&model);
            }
            match w.tried {
                Some(t) => {
                    item.push_str(&format!(";status={}", t.status));
                    if let Some(ms) = t.duration_ms {
                        item.push_str(&format!(";dur={ms}"));
                    }
                    if t.tries > 1 {
                        item.push_str(&format!(";tries={}", t.tries));
                    }
                }
                None => item.push_str(";skipped"),
            }
            item
        })
        .collect();
    HeaderValue::from_str(&items.join(", ")).ok()
}

fn sf_string(text: &str) -> Option<String> {
    if !text.chars().all(|c| matches!(c, ' '..='~')) {
        return None;
    }
    Some(format!(
        "\"{}\"",
        text.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}

pub fn server_timing(
    upload: Option<Duration>,
    kaeso: Duration,
    upstream: Option<Duration>,
) -> HeaderValue {
    let entry = |name: &str, d: Duration| format!("{name};dur={:.3}", d.as_secs_f64() * 1000.0);
    let value = [
        upload.map(|d| entry("upload", d)),
        Some(entry("kaeso", kaeso)),
        upstream.map(|d| entry("upstream", d)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(", ");
    HeaderValue::from_str(&value).unwrap_or_else(|_| HeaderValue::from_static("kaeso"))
}

pub fn respond(dialect: DialectId, err: &KppError, served: &Served) -> Response {
    let encoded = match request_id::current() {
        Some(id) => dispatch::encode_error_for_request(dialect, err, &id.to_string()),
        None => dispatch::encode_error(dialect, err),
    };
    let mut resp = Response::new(Body::from(encoded.body));
    // The codecs choose a status by the error's kind, and no kind means 415: the gateway's own
    // refusal of a content encoding keeps it, as a refused method keeps 405, and its refusal of
    // a retired model keeps 410.
    *resp.status_mut() = match err.status {
        Some(415) if err.origin.is_none() => StatusCode::UNSUPPORTED_MEDIA_TYPE,
        Some(410) if err.origin.is_none() => StatusCode::GONE,
        _ => StatusCode::from_u16(encoded.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
    };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn step(path: &str, model: Option<&str>, tried: Option<Tried>) -> Walked {
        Walked {
            path: path.to_owned(),
            model: model.map(str::to_owned),
            tried,
        }
    }

    #[test]
    fn the_route_header_is_a_structured_field_list_of_the_steps_tried() {
        assert!(route_header(&[]).is_none());
        let value = route_header(&[
            step("route", None, None),
            step(
                "route.fallback",
                Some("anthropic/claude-sonnet-4-5"),
                Some(Tried {
                    status: 529,
                    duration_ms: Some(1204),
                    tries: 2,
                }),
            ),
            step(
                "route.fallback.fallback",
                Some("custom/a \"quoted\" \\ model"),
                Some(Tried {
                    status: 200,
                    duration_ms: None,
                    tries: 1,
                }),
            ),
            step(
                "route.else",
                Some("custom/modèle"),
                Some(Tried {
                    status: 200,
                    duration_ms: None,
                    tries: 1,
                }),
            ),
        ])
        .unwrap();
        assert_eq!(
            value.to_str().unwrap(),
            r#""route";skipped, "route.fallback";model="anthropic/claude-sonnet-4-5";status=529;dur=1204;tries=2, "route.fallback.fallback";model="custom/a \"quoted\" \\ model";status=200, "route.else";status=200"#
        );
    }
}
