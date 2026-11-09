use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Path as UrlPath, Query, RawQuery, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{
    Block, Delta, DialectId, EmbeddingVector, ErrorKind, ImageOperation, KppError, ModelInfo,
    ModelLifecycle, ModelList, Reply, RequestKind, Source, StreamEvent, StreamMode,
    TranscriptFormat, TranscriptionOperation,
};
use serde_json::{Value, json};

mod videos;

const CREATED_AT: u64 = 1_760_000_000;

#[derive(Clone)]
struct App {
    fixtures: PathBuf,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let fixtures = std::env::var_os("KPP_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../kaeso-codecs/fixtures"));
    let port: u16 = std::env::var("KPP_CONFORMANCE_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8787);
    let app = App { fixtures };
    let router = Router::new()
        .route(
            "/v1/messages",
            post(|s, h, b| serve(s, h, b, DialectId::AnthropicMessages, RequestKind::Generate)),
        )
        .route(
            "/v1/messages/count_tokens",
            post(|s, h, b| {
                serve(
                    s,
                    h,
                    b,
                    DialectId::AnthropicMessages,
                    RequestKind::CountTokens,
                )
            }),
        )
        .route(
            "/v1/chat/completions",
            post(|s, h, b| serve(s, h, b, DialectId::OpenAiChat, RequestKind::Generate)),
        )
        .route(
            "/v1/responses",
            post(|s, h, b| serve(s, h, b, DialectId::OpenAiResponses, RequestKind::Generate)),
        )
        .route(
            "/v1/responses/input_tokens",
            post(|s, h, b| {
                serve(
                    s,
                    h,
                    b,
                    DialectId::OpenAiResponses,
                    RequestKind::CountTokens,
                )
            }),
        )
        .route(
            "/v1/embeddings",
            post(|s, h, b| serve(s, h, b, DialectId::OpenAiEmbeddings, RequestKind::Embed)),
        )
        .route(
            "/v1/images/generations",
            post(|s, h, b| serve_images(s, h, b, ImageOperation::Generate)),
        )
        .route(
            "/v1/images/edits",
            post(|s, h, b| serve_images(s, h, b, ImageOperation::Edit)),
        )
        .route(
            "/v1/images/variations",
            post(|s, h, b| serve_images(s, h, b, ImageOperation::Variation)),
        )
        .route("/v1/audio/speech", post(serve_speech))
        .route(
            "/v1/audio/transcriptions",
            post(|s, h, b| serve_transcription(s, h, b, TranscriptionOperation::Transcribe)),
        )
        .route(
            "/v1/audio/translations",
            post(|s, h, b| serve_transcription(s, h, b, TranscriptionOperation::Translate)),
        )
        .route("/v1/videos", post(videos::create).get(videos::list))
        .route(
            "/v1/videos/{id}",
            get(videos::retrieve).delete(videos::delete),
        )
        .route("/v1/videos/{id}/remix", post(videos::remix))
        .route("/v1/videos/{id}/content", get(videos::content))
        .route("/v1beta/models", get(list_gemini_models))
        .route(
            "/v1beta/models/{*rest}",
            post(serve_gemini).get(retrieve_gemini_model),
        )
        .route("/v1/models", get(list_models))
        .route("/v1/models/{id}", get(retrieve_model))
        .route("/_scenarios", get(scenarios))
        .route("/_expected", get(expected))
        .with_state(app);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    writeln!(
        std::io::stderr(),
        "kpp-conformance listening on http://{addr}"
    )?;
    axum::serve(listener, router).await
}

fn headers_in(map: &HeaderMap) -> HeaderList {
    map.iter()
        .filter_map(|(k, v)| Some((k.as_str().to_owned(), v.to_str().ok()?.to_owned())))
        .collect()
}

fn respond(status: u16, headers: &HeaderList, body: Vec<u8>) -> Response {
    let mut res = Response::new(Body::from(body));
    *res.status_mut() = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    for (k, v) in headers.iter() {
        if let (Ok(k), Ok(v)) = (HeaderName::try_from(k), HeaderValue::from_str(v)) {
            res.headers_mut().append(k, v);
        }
    }
    res
}

fn error_response(ingress: DialectId, err: &KppError) -> Response {
    let e = dispatch::encode_error(ingress, err);
    respond(e.status, &e.headers, e.body)
}

fn catalog() -> Vec<ModelInfo> {
    let model = |id: &str, name: &str, created: u64, owner: &str, lifecycle: ModelLifecycle| {
        let mut m = ModelInfo::new(id);
        m.display_name = Some(name.to_owned());
        m.created_at = Some(created);
        m.owned_by = Some(owner.to_owned());
        m.lifecycle = Some(lifecycle);
        m
    };
    let mut opus = model(
        "claude-opus-5",
        "Claude Opus 5",
        1_784_851_200,
        "anthropic",
        ModelLifecycle::Active,
    );
    opus.context_window = Some(1_000_000);
    opus.max_output_tokens = Some(128_000);
    let gpt = model(
        "gpt-6-astra",
        "GPT-6 Astra",
        1_777_593_600,
        "openai",
        ModelLifecycle::Active,
    );
    let sonnet = model(
        "claude-sonnet-5",
        "Claude Sonnet 5",
        1_781_049_600,
        "anthropic",
        ModelLifecycle::Active,
    );
    let mut old = model(
        "claude-sonnet-4-5-20250929",
        "Claude Sonnet 4.5",
        1_759_104_000,
        "anthropic",
        ModelLifecycle::Deprecated,
    );
    old.deprecated_at = Some(1_781_049_600);
    old.retires_at = Some(1_796_860_800);
    let mut gone = model(
        "claude-3-5-sonnet-20241022",
        "Claude Sonnet 3.5",
        1_729_555_200,
        "anthropic",
        ModelLifecycle::Retired,
    );
    gone.deprecated_at = Some(1_755_043_200);
    gone.retires_at = Some(1_761_004_800);
    vec![opus, sonnet, gpt, old, gone]
}

async fn list_models(headers: HeaderMap, RawQuery(query): RawQuery) -> Response {
    let dialect = dispatch::models_dialect(&headers_in(&headers));
    let query = match dispatch::decode_models_query(dialect, query.as_deref().unwrap_or("")) {
        Ok(q) => q,
        Err(e) => return error_response(dialect, &e.with_status(400)),
    };
    let default_limit = (dialect == DialectId::AnthropicMessages).then_some(20);
    let page = match ModelList::page(&catalog(), &query, default_limit) {
        Ok(p) => p,
        Err(e) => return error_response(dialect, &e.with_status(400)),
    };
    match dispatch::encode_models(dialect, &page) {
        Ok(b) => respond(
            200,
            &HeaderList::new().with("content-type", "application/json"),
            b,
        ),
        Err(e) => error_response(dialect, &e),
    }
}

async fn retrieve_model(headers: HeaderMap, UrlPath(id): UrlPath<String>) -> Response {
    let dialect = dispatch::models_dialect(&headers_in(&headers));
    let Some(model) = catalog().into_iter().find(|m| m.id == id) else {
        let err = KppError::new(
            ErrorKind::NotFound,
            format!("The model `{id}` does not exist."),
        )
        .with_status(404);
        return error_response(dialect, &err);
    };
    match dispatch::encode_model(dialect, &model) {
        Ok(b) => respond(
            200,
            &HeaderList::new().with("content-type", "application/json"),
            b,
        ),
        Err(e) => error_response(dialect, &e),
    }
}

struct Scenario {
    source: DialectId,
    name: String,
}

fn scenario(model: &str) -> Result<Scenario, KppError> {
    let (source, name) = model.split_once(':').ok_or_else(|| {
        KppError::invalid_request(format!(
            "conformance: model must be `<dialect>:<fixture>`, got `{model}`"
        ))
    })?;
    let source = source
        .parse()
        .map_err(|e| KppError::invalid_request(format!("conformance: {e}")))?;
    if name.contains("..") {
        return Err(KppError::invalid_request("conformance: bad fixture name"));
    }
    Ok(Scenario {
        source,
        name: name.to_owned(),
    })
}

fn read(path: &Path) -> Result<Vec<u8>, KppError> {
    std::fs::read(path).map_err(|e| {
        KppError::new(
            kaeso_kpp::ErrorKind::NotFound,
            format!("conformance: {}: {e}", path.display()),
        )
        .with_status(404)
    })
}

async fn serve(
    State(app): State<App>,
    headers: HeaderMap,
    body: Bytes,
    ingress: DialectId,
    kind: RequestKind,
) -> Response {
    let req = match dispatch::decode_request(ingress, kind, &body, &headers_in(&headers)) {
        Ok(r) => r,
        Err(e) => return error_response(ingress, &e.with_status(400)),
    };
    answer(&app, req, ingress, kind)
}

async fn serve_images(
    State(app): State<App>,
    headers: HeaderMap,
    body: Bytes,
    operation: ImageOperation,
) -> Response {
    let ingress = DialectId::OpenAiImages;
    match dispatch::decode_image_request(operation, &body, &headers_in(&headers)) {
        Ok(req) => answer(&app, req, ingress, RequestKind::Image),
        Err(e) => error_response(ingress, &e.with_status(400)),
    }
}

// A fixture of `source`, where `providers/<id>/<name>` names a provider's own folder.
fn fixture_path(dir: &Path, folder: &str, name: &str, ext: &str) -> PathBuf {
    match name
        .strip_prefix("providers/")
        .and_then(|r| r.split_once('/'))
    {
        Some((id, rest)) => dir.join(format!("providers/{id}/{folder}/{rest}.{ext}")),
        None => dir.join(format!("{folder}/{name}.{ext}")),
    }
}

// A speech scenario's reply is `responses/<name>.mp3` (its content type in
// `<name>.mp3.meta.json`) or, for an events request, `streams/<name>.sse`.
async fn serve_speech(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let ingress = DialectId::OpenAiSpeech;
    let req = match dispatch::decode_request(
        ingress,
        RequestKind::Speech,
        &body,
        &headers_in(&headers),
    ) {
        Ok(r) => r,
        Err(e) => return error_response(ingress, &e.with_status(400)),
    };
    let sc = match scenario(req.model()) {
        Ok(s) => s,
        Err(e) => return error_response(ingress, &e.with_status(400)),
    };
    let dir = app.fixtures.join(sc.source.as_str());
    if let Some(name) = sc.name.strip_prefix("errors/") {
        return match error_fixture(&dir, name) {
            Ok((status, h, b)) => {
                error_response(ingress, &dispatch::decode_error(sc.source, status, &h, &b))
            }
            Err(e) => error_response(ingress, &e),
        };
    }
    let ctx = EncodeCtx::for_request(req.clone());
    let mut egress = EgressCtx::new(&req);
    egress.stream = StreamMode::on(true);
    let events = match read(&fixture_path(&dir, "streams", &sc.name, "sse")) {
        Ok(bytes) => decode_stream(sc.source, &egress, &bytes),
        Err(_) => {
            egress.stream = StreamMode::Off;
            let audio = read(&fixture_path(&dir, "responses", &sc.name, "mp3"));
            audio.and_then(|bytes| {
                let mut decoder = dispatch::stream_decoder(sc.source, &egress)?;
                let mut events = decoder.feed(&bytes);
                events.extend(decoder.finish());
                Ok(events)
            })
        }
    };
    let events = match events {
        Ok(ev) => ev,
        Err(e) => return error_response(ingress, &e),
    };
    let mut encoder = match dispatch::stream_encoder(ingress, &ctx) {
        Ok(enc) => enc,
        Err(e) => return error_response(ingress, &e),
    };
    let mut out = Vec::new();
    for ev in &events {
        match encoder.encode(ev) {
            Ok(b) => out.extend(b),
            Err(e) => return error_response(ingress, &e),
        }
    }
    let media = dispatch::stream_media_type(ingress, &ctx);
    respond(200, &HeaderList::new().with("content-type", media), out)
}

// A transcription scenario's reply is `responses/<name>.json`, `.txt`, `.srt` or `.vtt`; a
// streamed request's is `streams/<name>.sse`.
async fn serve_transcription(
    State(app): State<App>,
    headers: HeaderMap,
    body: Bytes,
    operation: TranscriptionOperation,
) -> Response {
    let ingress = DialectId::OpenAiTranscriptions;
    let req = match dispatch::decode_transcription_request(operation, &body, &headers_in(&headers))
    {
        Ok(r) => r,
        Err(e) => return error_response(ingress, &e.with_status(400)),
    };
    let sc = match scenario(req.model()) {
        Ok(s) => s,
        Err(e) => return error_response(ingress, &e.with_status(400)),
    };
    let dir = app.fixtures.join(sc.source.as_str());
    if let Some(name) = sc.name.strip_prefix("errors/") {
        return match error_fixture(&dir, name) {
            Ok((status, h, b)) => {
                error_response(ingress, &dispatch::decode_error(sc.source, status, &h, &b))
            }
            Err(e) => error_response(ingress, &e),
        };
    }
    let ctx = EncodeCtx::for_request(req.clone());
    let mut egress = EgressCtx::new(&req);
    if !req.stream_mode().is_off() {
        let events = read(&fixture_path(&dir, "streams", &sc.name, "sse"))
            .and_then(|bytes| decode_stream(sc.source, &egress, &bytes));
        let events = match events {
            Ok(ev) => ev,
            Err(e) => return error_response(ingress, &e),
        };
        let mut encoder = match dispatch::stream_encoder(ingress, &ctx) {
            Ok(enc) => enc,
            Err(e) => return error_response(ingress, &e),
        };
        let mut out = Vec::new();
        for ev in &events {
            match encoder.encode(ev) {
                Ok(b) => out.extend(b),
                Err(e) => return error_response(ingress, &e),
            }
        }
        return respond(
            200,
            &HeaderList::new().with("content-type", "text/event-stream"),
            out,
        );
    }
    let mut forms = vec![
        ("json", None),
        ("txt", Some(TranscriptFormat::Text)),
        ("srt", Some(TranscriptFormat::Srt)),
        ("vtt", Some(TranscriptFormat::Vtt)),
    ];
    let asked = match &req {
        kaeso_kpp::Request::Transcription(t) => t.format_or_default(),
        _ => TranscriptFormat::Json,
    };
    forms.sort_by_key(|(_, f)| f.as_ref() != Some(&asked));
    let found = forms.into_iter().find_map(|(ext, f)| {
        Some((
            read(&fixture_path(&dir, "responses", &sc.name, ext)).ok()?,
            f,
        ))
    });
    let Some((bytes, format)) = found else {
        let err = KppError::new(
            ErrorKind::NotFound,
            format!("conformance: no transcript {}", sc.name),
        )
        .with_status(404);
        return error_response(ingress, &err);
    };
    egress.transcript_format = Some(match format {
        Some(f) => f,
        None if sc
            .name
            .rsplit('/')
            .next()
            .is_some_and(|n| n.starts_with("verbose")) =>
        {
            TranscriptFormat::VerboseJson
        }
        None if sc.name.starts_with("diarized") => TranscriptFormat::DiarizedJson,
        None => TranscriptFormat::Json,
    });
    let reply = match dispatch::decode_response(sc.source, 200, &HeaderList::new(), &bytes, &egress)
    {
        Ok(r) => r,
        Err(e) => return error_response(ingress, &e),
    };
    match dispatch::encode_response(ingress, &reply, &ctx) {
        Ok(b) => {
            let media = dispatch::response_media_type(ingress, &reply, &ctx);
            respond(200, &HeaderList::new().with("content-type", media), b)
        }
        Err(e) => error_response(ingress, &e),
    }
}

async fn serve_gemini(
    State(app): State<App>,
    UrlPath(rest): UrlPath<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some((model, action)) = kaeso_codecs::gemini::split_model_action(&rest) else {
        let err = KppError::new(
            ErrorKind::NotFound,
            format!("conformance: no method in `{rest}`"),
        )
        .with_status(404);
        return error_response(DialectId::Gemini, &err);
    };
    let ingress = action.dialect();
    match dispatch::decode_gemini_request(&model, action, &body, &headers_in(&headers)) {
        Ok(req) => answer(&app, req, ingress, action.kind()),
        Err(e) => error_response(ingress, &e.with_status(400)),
    }
}

async fn list_gemini_models(RawQuery(query): RawQuery) -> Response {
    let dialect = DialectId::Gemini;
    let page = dispatch::decode_models_query(dialect, query.as_deref().unwrap_or(""))
        .and_then(|q| ModelList::page(&catalog(), &q, None));
    match page.and_then(|p| dispatch::encode_models(dialect, &p)) {
        Ok(b) => respond(
            200,
            &HeaderList::new().with("content-type", "application/json"),
            b,
        ),
        Err(e) => error_response(dialect, &e.with_status(400)),
    }
}

async fn retrieve_gemini_model(UrlPath(rest): UrlPath<String>) -> Response {
    let id = rest.strip_prefix("models/").unwrap_or(&rest);
    let Some(model) = catalog().into_iter().find(|m| m.id == id) else {
        let err = KppError::new(
            ErrorKind::NotFound,
            format!("The model `{id}` does not exist."),
        )
        .with_status(404);
        return error_response(DialectId::Gemini, &err);
    };
    match dispatch::encode_model(DialectId::Gemini, &model) {
        Ok(b) => respond(
            200,
            &HeaderList::new().with("content-type", "application/json"),
            b,
        ),
        Err(e) => error_response(DialectId::Gemini, &e),
    }
}

fn answer(app: &App, req: kaeso_kpp::Request, ingress: DialectId, kind: RequestKind) -> Response {
    let sc = match scenario(req.model()) {
        Ok(s) => s,
        Err(e) => return error_response(ingress, &e.with_status(400)),
    };
    let dir = app.fixtures.join(sc.source.as_str());
    if let Some(name) = sc.name.strip_prefix("errors/") {
        let (status, h, body) = match error_fixture(&dir, name) {
            Ok(x) => x,
            Err(e) => return error_response(ingress, &e),
        };
        let err = dispatch::decode_error(sc.source, status, &h, &body);
        return error_response(ingress, &err);
    }
    let ctx = EncodeCtx::for_request(req.clone()).with_created_at(CREATED_AT);
    let mut egress = EgressCtx::for_kind(kind, req.stream_mode());
    egress.audio_format = EgressCtx::new(&req).audio_format;
    if let StreamMode::On { .. } = req.stream_mode() {
        let bytes = match read(&dir.join(format!("streams/{}.sse", sc.name))) {
            Ok(b) => b,
            Err(e) => return error_response(ingress, &e),
        };
        let events = match decode_stream(sc.source, &egress, &bytes) {
            Ok(ev) => ev,
            Err(e) => return error_response(ingress, &e),
        };
        let mut encoder = match dispatch::stream_encoder(ingress, &ctx) {
            Ok(enc) => enc,
            Err(e) => return error_response(ingress, &e),
        };
        let mut out = Vec::new();
        for ev in &events {
            match encoder.encode(ev) {
                Ok(b) => out.extend(b),
                Err(e) => return error_response(ingress, &e),
            }
        }
        match encoder.finish() {
            Ok(b) => out.extend(b),
            Err(e) => return error_response(ingress, &e),
        }
        let h = HeaderList::new().with("content-type", "text/event-stream");
        return respond(200, &h, out);
    }
    let reply = match decode_fixture(sc.source, &dir, &sc.name, &egress) {
        Ok(r) => r,
        Err(e) => return error_response(ingress, &e),
    };
    match dispatch::encode_response(ingress, &reply, &ctx) {
        Ok(b) => respond(
            200,
            &HeaderList::new().with("content-type", "application/json"),
            b,
        ),
        Err(e) => error_response(ingress, &e),
    }
}

fn error_fixture(dir: &Path, name: &str) -> Result<(u16, HeaderList, Vec<u8>), KppError> {
    let body = read(&dir.join(format!("errors/{name}.json")))?;
    let meta: Value = std::fs::read(dir.join(format!("errors/{name}.meta.json")))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null);
    let status = meta
        .get("status")
        .and_then(Value::as_u64)
        .and_then(|s| u16::try_from(s).ok())
        .unwrap_or(500);
    let mut h = HeaderList::new();
    if let Some(map) = meta.get("headers").and_then(Value::as_object) {
        for (k, v) in map {
            h.push(k.clone(), v.as_str().unwrap_or_default().to_owned());
        }
    }
    Ok((status, h, body))
}

fn decode_fixture(
    source: DialectId,
    dir: &Path,
    name: &str,
    egress: &EgressCtx,
) -> Result<Reply, KppError> {
    let body = read(&dir.join(format!("responses/{name}.json")))?;
    dispatch::decode_response(source, 200, &HeaderList::new(), &body, egress)
}

fn decode_stream(
    source: DialectId,
    egress: &EgressCtx,
    bytes: &[u8],
) -> Result<Vec<StreamEvent>, KppError> {
    let mut decoder = dispatch::stream_decoder(source, egress)?;
    let mut events = decoder.feed(bytes);
    events.extend(decoder.finish());
    Ok(events)
}

#[derive(serde::Deserialize)]
struct ExpectedQuery {
    source: String,
    name: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    stream: bool,
}

async fn expected(State(app): State<App>, Query(q): Query<ExpectedQuery>) -> Response {
    let json_response = |v: Value| {
        respond(
            200,
            &HeaderList::new().with("content-type", "application/json"),
            serde_json::to_vec(&v).unwrap_or_default(),
        )
    };
    let Ok(source) = q.source.parse::<DialectId>() else {
        return json_response(json!({"error": "unknown source"}));
    };
    let kind = match q.kind.as_deref() {
        Some("count_tokens") => RequestKind::CountTokens,
        Some("embed") => RequestKind::Embed,
        Some("image") => RequestKind::Image,
        _ => RequestKind::Generate,
    };
    let dir = app.fixtures.join(source.as_str());
    if q.kind.as_deref() == Some("video") {
        return json_response(videos::expected(&dir, source, &q.name));
    }
    if let Some(name) = q.name.strip_prefix("errors/") {
        let (status, headers, body) = match error_fixture(&dir, name) {
            Ok(x) => x,
            Err(e) => return json_response(json!({"error": e.message})),
        };
        let err = dispatch::decode_error(source, status, &headers, &body);
        return json_response(json!({
            "error_kind": err.kind.as_str(),
            "status": status,
            "retry_after": err.retry_after.map(|d| d.as_secs_f64()),
        }));
    }
    let mode = if q.stream {
        StreamMode::on(true)
    } else {
        StreamMode::Off
    };
    let egress = EgressCtx::for_kind(kind, mode);
    if q.stream {
        let Ok(bytes) = read(&dir.join(format!("streams/{}.sse", q.name))) else {
            return json_response(json!({"error": "missing"}));
        };
        return match decode_stream(source, &egress, &bytes) {
            Ok(events) => json_response(summarize_stream(&events)),
            Err(e) => json_response(json!({"error": e.message})),
        };
    }
    match decode_fixture(source, &dir, &q.name, &egress) {
        Ok(reply) => json_response(summarize_reply(&reply)),
        Err(e) => json_response(json!({"error": e.message})),
    }
}

fn summarize_reply(reply: &Reply) -> Value {
    match reply {
        Reply::Generate(r) => {
            let mut text = String::new();
            let mut refusal = String::new();
            let mut tools = Vec::new();
            let mut reasoning = Vec::new();
            for b in &r.blocks {
                match b {
                    Block::Text(t) => text.push_str(&t.text),
                    Block::Reasoning(rb) => {
                        let joined = readable(rb.summary.iter().chain(rb.text.iter()));
                        if !joined.is_empty() {
                            reasoning.push(joined);
                        }
                    }
                    Block::Refusal(t) => refusal.push_str(&t.text),
                    Block::ToolCall(c) => tools.push(json!({
                        "name": c.name,
                        "arguments": c.arguments.to_value().unwrap_or(Value::Null),
                    })),
                    _ => {}
                }
            }
            json!({"text": text, "refusal": refusal, "tool_calls": tools, "reasoning": reasoning, "stop": format!("{:?}", r.stop)})
        }
        Reply::CountTokens(c) => json!({"input_tokens": c.input_tokens}),
        Reply::Embed(e) => json!({
            "count": e.data.len(),
            "dims": e.data.first().map(|d| match &d.vector {
                EmbeddingVector::Float(v) => v.len(),
                _ => 0,
            }),
        }),
        Reply::Image(r) => json!({
            "count": r.images.len(),
            "links": r.images.iter().filter(|i| matches!(i.source, Source::Url(_))).count(),
            "input_tokens": r.usage.total_input_tokens(),
            "output_tokens": r.usage.output_tokens,
        }),
        _ => json!({}),
    }
}

fn readable<'a>(parts: impl Iterator<Item = &'a String>) -> String {
    parts
        .filter(|t| !t.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn summarize_stream(events: &[StreamEvent]) -> Value {
    use std::collections::BTreeMap;
    let mut text = String::new();
    let mut refusal = String::new();
    let mut tools: BTreeMap<u32, (String, String)> = BTreeMap::new();
    let mut reasoning: BTreeMap<u32, (BTreeMap<u32, String>, String)> = BTreeMap::new();
    let mut error = None;
    let (mut images, mut partial_images) = (0u32, 0u32);
    for ev in events {
        match ev {
            StreamEvent::BlockStart(s) => match &s.block {
                kaeso_kpp::BlockHeader::ToolCall(h) => {
                    tools.insert(s.index, (h.name.clone(), String::new()));
                }
                kaeso_kpp::BlockHeader::Reasoning(_) => {
                    reasoning.insert(s.index, (BTreeMap::new(), String::new()));
                }
                _ => {}
            },
            StreamEvent::BlockDelta(d) => match &d.delta {
                Delta::ReasoningText(t) => {
                    if let Some((_, text)) = reasoning.get_mut(&d.index) {
                        text.push_str(t);
                    }
                }
                Delta::ReasoningSummary(sd) => {
                    if let Some((summaries, _)) = reasoning.get_mut(&d.index) {
                        summaries
                            .entry(sd.summary_index)
                            .or_default()
                            .push_str(&sd.text);
                    }
                }
                Delta::Text(t) => text.push_str(t),
                Delta::Refusal(t) => refusal.push_str(t),
                Delta::ToolArguments(a) => {
                    if let Some((_, args)) = tools.get_mut(&d.index) {
                        args.push_str(a);
                    }
                }
                _ => {}
            },
            StreamEvent::Error(e) => error = Some(e.message.clone()),
            StreamEvent::Image(i) if i.partial.is_some() => partial_images += 1,
            StreamEvent::Image(_) => images += 1,
            _ => {}
        }
    }
    let tools: Vec<Value> = tools
        .into_values()
        .map(|(name, args)| json!({"name": name, "arguments": streamed_arguments(args)}))
        .collect();
    let reasoning: Vec<String> = reasoning
        .into_values()
        .map(|(summaries, text)| readable(summaries.values().chain(std::iter::once(&text))))
        .filter(|r| !r.is_empty())
        .collect();
    json!({"text": text, "refusal": refusal, "tool_calls": tools, "reasoning": reasoning, "stream_error": error, "images": images, "partial_images": partial_images})
}

fn streamed_arguments(text: String) -> Value {
    if text.trim().is_empty() {
        return json!({});
    }
    serde_json::from_str(&text).unwrap_or(Value::String(text))
}

async fn scenarios(State(app): State<App>) -> Response {
    let mut out = Vec::new();
    for source in DialectId::ALL {
        let dir = app.fixtures.join(source.as_str());
        for (folder, kind, stream) in [
            ("responses", "generate", false),
            ("responses/count_tokens", "count_tokens", false),
            ("responses/input_tokens", "count_tokens", false),
            ("streams", "generate", true),
            ("errors", "error", false),
        ] {
            let Ok(entries) = std::fs::read_dir(dir.join(folder)) else {
                continue;
            };
            let mut names: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| {
                    (n.ends_with(".json") && !n.ends_with(".meta.json")) || n.ends_with(".sse")
                })
                .map(|n| {
                    n.trim_end_matches(".json")
                        .trim_end_matches(".sse")
                        .to_owned()
                })
                .collect();
            names.sort();
            for name in names {
                let sub = folder.strip_prefix("responses/").map(|s| format!("{s}/"));
                let full = match (kind, sub) {
                    ("error", _) => format!("errors/{name}"),
                    (_, Some(sub)) => format!("{sub}{name}"),
                    _ => name.clone(),
                };
                let embeddings = matches!(
                    source,
                    DialectId::OpenAiEmbeddings | DialectId::GeminiEmbeddings
                );
                let images = matches!(source, DialectId::OpenAiImages | DialectId::MinimaxImages);
                let kind = match kind {
                    "generate" if embeddings => "embed",
                    "generate" if images => "image",
                    other => other,
                };
                out.push(json!({"source": source.as_str(), "name": full, "kind": kind, "stream": stream}));
            }
        }
    }
    for (source, prefix) in videos::SOURCES {
        let dir = app.fixtures.join(source.as_str());
        for folder in ["responses", "polls"] {
            for name in videos::names(&dir, &format!("{prefix}{folder}")) {
                out.push(json!({"source": source.as_str(), "name": format!("{prefix}{folder}/{name}"), "kind": "video", "stream": false}));
            }
        }
    }
    respond(
        200,
        &HeaderList::new().with("content-type", "application/json"),
        serde_json::to_vec(&out).unwrap_or_default(),
    )
}
