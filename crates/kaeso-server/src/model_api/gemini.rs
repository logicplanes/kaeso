use axum::Router;
use axum::body::Body;
use axum::extract::{Extension, OriginalUri, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::Response;
use axum::routing::get;
use kaeso_codecs::gemini::{GeminiAction, split_model_action};
use kaeso_core::id::RequestId;
use kaeso_kpp::{DialectId, ErrorKind, KppError};

use super::errors::{Served, error, respond};
use super::pipeline::{self, GeminiCall, Incoming};
use super::{hooks, listing};
use crate::http::client_ip::ClientIp;
use crate::state::AppState;

pub const PREFIX: &str = "/v1beta";

const MODELS: &str = "/v1beta/models";

// Google's REST examples put the key in `?key=`; google-genai sends `x-goog-api-key`.
const KEY_PARAM: &str = "key";

const KEY_HEADER: &str = "x-goog-api-key";

// Gemini's listing pages 50 models by default (`models.list`, `pageSize`).
const DEFAULT_PAGE_SIZE: u32 = 50;

pub fn is_gemini_path(path: &str) -> bool {
    path == PREFIX || path.starts_with("/v1beta/")
}

fn refuse(err: KppError) -> Response {
    respond(DialectId::Gemini, &err, &Served::default())
}

fn query_pairs(uri: &Uri) -> Vec<(String, String)> {
    uri.query()
        .map(|q| {
            url::form_urlencoded::parse(q.as_bytes())
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect()
        })
        .unwrap_or_default()
}

// A key in the query becomes the header the rest of the gateway reads, and leaves the query, so
// nothing after this point can log or forward it.
fn lift_key(headers: &mut HeaderMap, pairs: &mut Vec<(String, String)>) {
    let Some(at) = pairs.iter().position(|(k, _)| k == KEY_PARAM) else {
        return;
    };
    let (_, key) = pairs.remove(at);
    pairs.retain(|(k, _)| k != KEY_PARAM);
    if super::auth::presented(headers).is_some() {
        return;
    }
    if let Ok(mut value) = HeaderValue::from_str(key.trim()) {
        value.set_sensitive(true);
        headers.insert(KEY_HEADER, value);
    }
}

fn unknown_parameter(name: &str) -> KppError {
    error(
        ErrorKind::InvalidRequest,
        400,
        format!("Kaeso doesn't take the query parameter `{name}` on Gemini calls; remove it."),
    )
    .with_param(name.to_owned())
}

fn checked_call(rest: &str, pairs: &[(String, String)]) -> Result<GeminiCall, KppError> {
    let Some((model, action)) = split_model_action(rest) else {
        let message = match rest.rsplit_once(':') {
            Some((_, method)) if !method.is_empty() => {
                format!("Kaeso doesn't serve the Gemini method `{method}`.")
            }
            _ => {
                "Name a model and a method, such as `gemini-3.5-flash:generateContent`.".to_owned()
            }
        };
        return Err(error(ErrorKind::NotFound, 404, message));
    };
    for (name, value) in pairs {
        match name.as_str() {
            "alt" if value == "sse" && action == GeminiAction::StreamGenerateContent => {}
            "alt" if value == "json" && action != GeminiAction::StreamGenerateContent => {}
            "alt" => {
                return Err(error(
                    ErrorKind::InvalidRequest,
                    400,
                    format!("Kaeso streams Gemini replies as server-sent events only, not as `alt={value}`; send `alt=sse`."),
                )
                .with_param("alt"));
            }
            other => return Err(unknown_parameter(other)),
        }
    }
    if action == GeminiAction::StreamGenerateContent
        && !pairs.iter().any(|(k, v)| k == "alt" && v == "sse")
    {
        return Err(error(
            ErrorKind::InvalidRequest,
            400,
            "Kaeso streams Gemini replies as server-sent events only; add `alt=sse` to the URL.",
        )
        .with_param("alt"));
    }
    Ok(GeminiCall { model, action })
}

fn rest_of(uri: &Uri) -> &str {
    uri.path()
        .strip_prefix(MODELS)
        .and_then(|r| r.strip_prefix('/'))
        .unwrap_or("")
}

async fn call(
    State(state): State<AppState>,
    Extension(request_id): Extension<RequestId>,
    ip: ClientIp,
    OriginalUri(uri): OriginalUri,
    req: Request,
) -> Response {
    let (parts, body) = req.into_parts();
    let mut headers = parts.headers;
    let mut pairs = query_pairs(&uri);
    lift_key(&mut headers, &mut pairs);
    let call = match checked_call(rest_of(&uri), &pairs) {
        Ok(call) => call,
        Err(e) => return refuse(e),
    };
    let max_body = state.config.body_limits.model_api.as_usize();
    pipeline::serve(
        state,
        Incoming {
            ingress: call.action.dialect(),
            kind: call.action.kind(),
            headers,
            body,
            request_id,
            client_ip: ip.0,
            caller: None,
            source: hooks::Source::Api,
            stream_only: false,
            max_body,
            alias: None,
            gemini: Some(call),
            image: None,
            transcription: None,
            elevenlabs: None,
            video: None,
        },
    )
    .await
}

fn listing_query(uri: &Uri, headers: &mut HeaderMap) -> String {
    let mut pairs = query_pairs(uri);
    lift_key(headers, &mut pairs);
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs)
        .finish()
}

async fn models_list(
    State(state): State<AppState>,
    ip: ClientIp,
    mut headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let query = listing_query(&uri, &mut headers);
    listing::list_as(
        &state,
        &headers,
        ip.0,
        &query,
        DialectId::Gemini,
        Some(DEFAULT_PAGE_SIZE),
    )
    .await
}

async fn models_get(
    State(state): State<AppState>,
    ip: ClientIp,
    mut headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let mut pairs = query_pairs(&uri);
    lift_key(&mut headers, &mut pairs);
    if let Some((name, _)) = pairs.first() {
        return refuse(unknown_parameter(name));
    }
    let id = percent_encoding::percent_decode_str(rest_of(&uri))
        .decode_utf8_lossy()
        .into_owned();
    listing::get_as(&state, &headers, ip.0, &id, DialectId::Gemini).await
}

pub(super) async fn not_found(OriginalUri(uri): OriginalUri, _body: Body) -> Response {
    refuse(error(
        ErrorKind::NotFound,
        404,
        format!("Kaeso does not serve `{}`.", uri.path()),
    ))
}

pub(super) fn method_not_allowed(path: &str, method: &axum::http::Method) -> Response {
    let mut resp = refuse(error(
        ErrorKind::InvalidRequest,
        405,
        format!("`{path}` does not accept `{method}` requests."),
    ));
    *resp.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
    resp
}

// google-genai with `api_version="v1"` calls `/v1/models/{model}:{method}`, where `/v1/models`
// belongs to the OpenAI and Anthropic listings; such an app gets pointed to `/v1beta`.
pub(super) fn on_v1(method_path: &str) -> Option<Response> {
    let (_, method) = method_path.rsplit_once(':')?;
    GeminiAction::from_wire(method)?;
    Some(refuse(error(
        ErrorKind::NotFound,
        404,
        "Kaeso serves the Gemini API under `/v1beta`; set the SDK's API version to `v1beta`.",
    )))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(MODELS, get(models_list))
        .route("/v1beta/models/{*rest}", get(models_get).post(call))
        .route(PREFIX, axum::routing::any(not_found))
        .route("/v1beta/{*rest}", axum::routing::any(not_found))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(q: &[(&str, &str)]) -> Vec<(String, String)> {
        q.iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn a_key_in_the_query_becomes_the_header_and_leaves_the_query() {
        let mut headers = HeaderMap::new();
        let mut q = pairs(&[("key", "sk-kso-abc"), ("alt", "sse"), ("key", "sk-kso-abc")]);
        lift_key(&mut headers, &mut q);
        assert_eq!(q, pairs(&[("alt", "sse")]));
        let sent = headers.get(KEY_HEADER).unwrap();
        assert!(sent.is_sensitive());
        assert_eq!(sent, "sk-kso-abc");

        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer sk-kso-own"),
        );
        let mut q = pairs(&[("key", "sk-kso-other")]);
        lift_key(&mut headers, &mut q);
        assert!(q.is_empty());
        assert!(headers.get(KEY_HEADER).is_none());
    }

    #[test]
    fn a_call_names_a_method_kaeso_serves_and_streams_only_as_server_sent_events() {
        let call = checked_call("gemini-3.5-flash:generateContent", &[]).unwrap();
        assert_eq!(
            (call.model.as_str(), call.action),
            ("gemini-3.5-flash", GeminiAction::GenerateContent)
        );
        let call = checked_call(
            "openrouter/a%3Ab:streamGenerateContent",
            &pairs(&[("alt", "sse")]),
        )
        .unwrap();
        assert_eq!(call.model, "openrouter/a:b");
        let refused = |rest: &str, q: &[(&str, &str)]| checked_call(rest, &pairs(q)).unwrap_err();
        let e = refused("gemini-3.5-flash:streamGenerateContent", &[]);
        assert_eq!((e.status, e.param.as_deref()), (Some(400), Some("alt")));
        let e = refused("gemini-3.5-flash:streamGenerateContent", &[("alt", "json")]);
        assert!(e.message.contains("`alt=json`"), "{}", e.message);
        let e = refused("gemini-3.5-flash:batchGenerateContent", &[]);
        assert_eq!((e.kind, e.status), (ErrorKind::NotFound, Some(404)));
        assert!(
            e.message.contains("`batchGenerateContent`"),
            "{}",
            e.message
        );
        let e = refused("gemini-3.5-flash", &[]);
        assert_eq!(e.status, Some(404));
        let e = refused("gemini-3.5-flash:countTokens", &[("$alt", "json")]);
        assert_eq!((e.status, e.param.as_deref()), (Some(400), Some("$alt")));
        checked_call("m:countTokens", &pairs(&[("alt", "json")])).unwrap();
    }

    #[test]
    fn only_gemini_methods_on_v1_are_pointed_to_v1beta() {
        assert!(on_v1("gemini-3.5-flash:generateContent").is_some());
        assert!(on_v1("gpt-4o").is_none());
        assert!(on_v1("a:b").is_none());
        assert!(is_gemini_path("/v1beta"));
        assert!(is_gemini_path("/v1beta/models"));
        assert!(!is_gemini_path("/v1betax"));
        assert!(!is_gemini_path("/v1/models"));
    }
}
