use axum::Router;
use axum::body::Body;
use axum::extract::{Extension, OriginalUri, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::response::Response;
use axum::routing::get;
use kaeso_codecs::dispatch;
use kaeso_codecs::gemini::{GeminiAction, split_model_action};
use kaeso_core::id::RequestId;
use kaeso_kpp::{DialectId, ErrorKind, KppError, RequestKind};

use super::errors::{Served, error, respond};
use super::pipeline::{self, GeminiCall, Incoming};
use super::{hooks, listing};
use crate::http::client_ip::ClientIp;
use crate::state::AppState;

pub const PREFIX: &str = "/v1beta";

const MODELS: &str = "/v1beta/models";

// google-genai 3 calls Gemini's stable version, `/v1`, by default; Google serves the same model
// paths and bodies there (https://ai.google.dev/gemini-api/docs/api-versions).
const V1_MODELS: &str = "/v1/models";

const DECISIONS: &str = "/v1beta/decisions";

// Google's REST examples put the key in `?key=`; google-genai sends `x-goog-api-key`.
const KEY_PARAM: &str = "key";

const KEY_HEADER: &str = "x-goog-api-key";

pub fn is_gemini_path(path: &str) -> bool {
    path == PREFIX || path.starts_with("/v1beta/") || names_method(path)
}

// `/v1/models/{model}:{method}` with a method only Gemini has; `/v1/models/{id}` alone is also
// OpenAI's and Anthropic's model path.
fn names_method(path: &str) -> bool {
    path.strip_prefix(V1_MODELS)
        .and_then(|r| r.strip_prefix('/'))
        .and_then(|r| r.rsplit_once(':'))
        .is_some_and(|(_, method)| GeminiAction::from_wire(method).is_some())
}

// On a path several APIs share, an app speaks Gemini when it sends `x-goog-api-key` (or `?key=`,
// once `lift_query` has moved it there) and no `authorization`, as google-genai does.
pub(super) fn speaks_gemini(headers: &HeaderMap) -> bool {
    dispatch::models_dialect(&pipeline::header_list(headers)) == DialectId::Gemini
}

pub(super) fn lift_query(uri: &Uri, headers: &mut HeaderMap) -> Vec<(String, String)> {
    let mut pairs = query_pairs(uri);
    lift_key(headers, &mut pairs);
    pairs
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
    let path = uri.path();
    path.strip_prefix(MODELS)
        .or_else(|| path.strip_prefix(V1_MODELS))
        .and_then(|r| r.strip_prefix('/'))
        .unwrap_or("")
}

// An app that speaks Gemini gets Gemini's 404 for a method Kaeso doesn't serve; any other app
// gets its own 405 for a POST to `/v1/models/{id}`.
pub(super) fn is_v1_call(uri: &Uri, headers: &HeaderMap) -> bool {
    if names_method(uri.path()) {
        return true;
    }
    let mut headers = headers.clone();
    lift_query(uri, &mut headers);
    speaks_gemini(&headers)
}

pub(super) async fn call(
    State(state): State<AppState>,
    Extension(request_id): Extension<RequestId>,
    ip: ClientIp,
    OriginalUri(uri): OriginalUri,
    req: Request,
) -> Response {
    let (parts, body) = req.into_parts();
    let mut headers = parts.headers;
    let pairs = lift_query(&uri, &mut headers);
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
            generation: None,
        },
    )
    .await
}

// Gemini's Decisions API names the model in the body, not the path (`POST /v1beta/decisions`).
async fn decisions(
    State(state): State<AppState>,
    Extension(request_id): Extension<RequestId>,
    ip: ClientIp,
    OriginalUri(uri): OriginalUri,
    req: Request,
) -> Response {
    let (parts, body) = req.into_parts();
    let mut headers = parts.headers;
    let pairs = lift_query(&uri, &mut headers);
    decide(state, request_id, ip, headers, &pairs, body).await
}

pub(super) async fn decide(
    state: AppState,
    request_id: RequestId,
    ip: ClientIp,
    headers: HeaderMap,
    pairs: &[(String, String)],
    body: Body,
) -> Response {
    if let Some((name, _)) = pairs.first() {
        return refuse(unknown_parameter(name));
    }
    let max_body = state.config.body_limits.decisions.as_usize();
    pipeline::serve(
        state,
        Incoming {
            ingress: DialectId::GeminiDecisions,
            kind: RequestKind::Decide,
            headers,
            body,
            request_id,
            client_ip: ip.0,
            caller: None,
            source: hooks::Source::Api,
            stream_only: false,
            max_body,
            alias: None,
            gemini: None,
            image: None,
            transcription: None,
            elevenlabs: None,
            video: None,
            generation: None,
        },
    )
    .await
}

pub(super) fn listing_query(uri: &Uri, headers: &mut HeaderMap) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(lift_query(uri, headers))
        .finish()
}

async fn models_list(
    State(state): State<AppState>,
    ip: ClientIp,
    mut headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let query = listing_query(&uri, &mut headers);
    listing::list_as(&state, &headers, ip.0, &query, DialectId::Gemini).await
}

pub(super) async fn models_get(
    State(state): State<AppState>,
    ip: ClientIp,
    mut headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let pairs = lift_query(&uri, &mut headers);
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

pub(super) fn method_not_allowed(path: &str, method: &Method) -> Response {
    let mut resp = refuse(error(
        ErrorKind::InvalidRequest,
        405,
        format!("`{path}` does not accept `{method}` requests."),
    ));
    *resp.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
    resp
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(MODELS, get(models_list))
        .route("/v1beta/models/{*rest}", get(models_get).post(call))
        .route(DECISIONS, axum::routing::post(decisions))
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
    fn a_gemini_path_is_under_v1beta_or_names_a_gemini_method_under_v1() {
        assert!(is_gemini_path("/v1beta"));
        assert!(is_gemini_path("/v1beta/models"));
        assert!(is_gemini_path(
            "/v1/models/gemini-3.5-flash:generateContent"
        ));
        assert!(is_gemini_path("/v1/models/openrouter/a:b:countTokens"));
        assert!(!is_gemini_path("/v1betax"));
        assert!(!is_gemini_path("/v1/models"));
        assert!(!is_gemini_path("/v1/models/gpt-4o"));
        assert!(!is_gemini_path("/v1/models/ft:gpt-4o:acme::abc"));
        assert!(!is_gemini_path("/v1/modelsx/m:generateContent"));
        assert!(!is_gemini_path("/v1/decisions"));
    }

    #[test]
    fn a_post_under_v1_models_is_gemini_by_its_method_or_its_key() {
        let uri = |u: &str| u.parse::<Uri>().unwrap();
        let none = HeaderMap::new();
        assert!(is_v1_call(&uri("/v1/models/gpt-4o:embedContent"), &none));
        assert!(!is_v1_call(
            &uri("/v1/models/m:batchGenerateContent"),
            &none
        ));
        assert!(is_v1_call(
            &uri("/v1/models/m:batchGenerateContent?key=sk-kso-a"),
            &none
        ));
        let mut goog = HeaderMap::new();
        goog.insert(KEY_HEADER, HeaderValue::from_static("sk-kso-a"));
        assert!(is_v1_call(&uri("/v1/models/m"), &goog));
        let mut bearer = goog.clone();
        bearer.insert("authorization", HeaderValue::from_static("Bearer sk-kso-a"));
        assert!(!is_v1_call(
            &uri("/v1/models/m:batchGenerateContent"),
            &bearer
        ));
        assert!(rest_of(&uri("/v1/models/m:countTokens")) == "m:countTokens");
        assert!(rest_of(&uri("/v1beta/models/m:countTokens")) == "m:countTokens");
    }
}
