//! OpenAI's Batch and Files APIs (`/v1/files`, `/v1/batches`) for a project, at providers whose
//! batch API takes OpenAI's batch format (https://developers.openai.com/api/docs/guides/batch).
//!
//! An input file is checked line by line and sent on to the provider with each line's model
//! written as the provider names it. Files and batches get Kaeso ids, and answer only to keys of
//! the project that made them. When a batch ends, each line's usage is recorded at the batch
//! price.

use std::collections::HashSet;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{OriginalUri, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use bytes::Bytes;
use futures::StreamExt;
use kaeso_codecs::multipart::{FormField, FormLimits, FormWriter, form_boundary, read_form};
use kaeso_codecs::{HeaderList, dispatch};
use kaeso_core::catalogue::Fields;
use kaeso_core::catalogue::resolve::Resolved;
use kaeso_core::id::{BatchFileId, BatchId, OrgId, ProjectId, RequestId};
use kaeso_core::provider::AuthMethod;
use kaeso_core::providers::ProviderRef;
use kaeso_kpp::{DialectId, ErrorKind, KppError, Reply, RequestKind};
use kaeso_store::pg::batches::{self as store, BatchRow, FileRow};
use kaeso_upstream::{Call, Outgoing, ResponseBody, Timeouts};
use serde_json::{Value, json};
use time::OffsetDateTime;
use uuid::Uuid;

use super::errors::{Served, error, respond};
use super::hooks::Caller;
use super::pipeline;
use crate::http::client_ip::ClientIp;
use crate::state::AppState;

// Errors answer in OpenAI's shape, which every endpoint here belongs to.
const D: DialectId = DialectId::OpenAiChat;

/// The endpoints a batch line may name, with the format each speaks.
const ENDPOINTS: [(&str, DialectId, RequestKind); 3] = [
    (
        "/v1/chat/completions",
        DialectId::OpenAiChat,
        RequestKind::Generate,
    ),
    (
        "/v1/responses",
        DialectId::OpenAiResponses,
        RequestKind::Generate,
    ),
    (
        "/v1/embeddings",
        DialectId::OpenAiEmbeddings,
        RequestKind::Embed,
    ),
];

// OpenAI's limits for a batch input file: 50,000 requests and 200 MB
// (https://developers.openai.com/api/reference/resources/batches/methods/create).
const MAX_LINES: usize = 50_000;

const ENDED: [&str; 4] = ["completed", "failed", "expired", "cancelled"];

fn invalid(param: &str, message: String) -> KppError {
    error(ErrorKind::InvalidRequest, 400, message).with_param(param)
}

fn at_line(n: usize, message: String) -> KppError {
    invalid("file", format!("Line {n} of the file: {message}"))
}

fn not_found(what: &str, id: &str, param: &str) -> KppError {
    let message = if what == "batch" {
        format!("No batch found with id '{id}'.")
    } else {
        format!("No file found with id '{id}'.")
    };
    error(ErrorKind::NotFound, 404, message).with_param(param)
}

fn unavailable() -> KppError {
    error(
        ErrorKind::ProviderError,
        503,
        "Kaeso cannot read its batches right now. Try again shortly.",
    )
}

fn store_error(e: kaeso_store::StoreError) -> KppError {
    tracing::error!(error = %e, "cannot read or write a batch");
    unavailable()
}

fn replaced() -> KppError {
    error(
        ErrorKind::InvalidRequest,
        409,
        "The provider key this file or batch lives under was replaced, so Kaeso can't reach it.",
    )
}

fn json_answer(status: StatusCode, body: Result<Value, KppError>) -> Response {
    match body {
        Ok(v) => {
            let mut resp = Response::new(Body::from(v.to_string()));
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            resp
        }
        Err(e) => respond(D, &e, &Served::default()),
    }
}

fn segment<'a>(uri: &'a axum::http::Uri, prefix: &str) -> &'a str {
    uri.path()
        .strip_prefix(prefix)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or_default()
}

async fn caller(state: &AppState, headers: &HeaderMap, ip: ClientIp) -> Result<Caller, KppError> {
    let key = super::auth::authenticate(state, headers, ip.0).await?;
    Ok(pipeline::key_caller(&key))
}

fn unix(t: OffsetDateTime) -> i64 {
    t.unix_timestamp()
}

/// The provider key a file or batch lives under, while the project still uses it.
async fn reach(
    state: &AppState,
    caller: &Caller,
    provider: &str,
    credential: Uuid,
) -> Result<(ProviderRef, Arc<crate::services::credentials::Resolved>), KppError> {
    let provider = kaeso_core::provider::provider(provider)
        .map(ProviderRef::from)
        .ok_or_else(replaced)?;
    let resolved = crate::services::credentials::for_provider(
        state,
        OrgId::from_uuid(caller.org_id),
        ProjectId::from_uuid(caller.project_id),
        &provider,
    )
    .await
    .map_err(|_| replaced())?;
    if resolved.id.uuid() != credential {
        return Err(replaced());
    }
    Ok((provider, resolved))
}

struct Sent {
    status: u16,
    headers: HeaderList,
    body: Bytes,
}

async fn send(
    state: &AppState,
    provider: &ProviderRef,
    credential: &crate::services::credentials::Resolved,
    method: &'static str,
    path: String,
    headers: HeaderList,
    body: Bytes,
) -> Result<Sent, KppError> {
    let base = pipeline::base_url(state, provider).ok_or_else(|| {
        error(
            ErrorKind::ProviderError,
            500,
            "No base URL is configured for this provider.",
        )
    })?;
    let u = &state.config.upstream;
    let resp = state
        .upstream
        .send(Call {
            base_url: base,
            auth: provider.auth(),
            guard: pipeline::guard_for(provider),
            credential: &credential.secret,
            sigv4: None,
            request: Outgoing {
                method,
                path,
                headers,
                body,
            },
            stream: false,
            timeouts: Timeouts {
                request: u.request_timeout,
                first_byte: u.first_byte_timeout,
                idle: u.stream_idle_timeout,
            },
            max_body: state.config.body_limits.provider_response.as_usize(),
        })
        .await
        .map_err(|e| pipeline::upstream_error_from(&e, provider))?;
    let body = match resp.body {
        ResponseBody::Full(b) => b,
        ResponseBody::Stream(mut s) => {
            let mut all = Vec::new();
            while let Some(chunk) = s.next().await {
                all.extend_from_slice(&chunk.map_err(|e| pipeline::upstream_error(&e))?);
            }
            Bytes::from(all)
        }
    };
    Ok(Sent {
        status: resp.status,
        headers: resp.headers,
        body,
    })
}

fn provider_json(sent: &Sent) -> Result<Value, KppError> {
    if !(200..300).contains(&sent.status) {
        return Err(dispatch::decode_error(
            D,
            sent.status,
            &sent.headers,
            &sent.body,
        ));
    }
    serde_json::from_slice(&sent.body).map_err(|_| {
        error(
            ErrorKind::ProviderError,
            502,
            "The provider answered in a form Kaeso can't read.",
        )
    })
}

// ----- the input file -----

/// What every line of an input file asks for, once checked.
struct Checked {
    endpoint: &'static str,
    entry: Uuid,
    model: String,
    upstream_model: String,
    provider: ProviderRef,
    credential: Arc<crate::services::credentials::Resolved>,
    lines: Vec<Vec<u8>>,
}

/// Checks each line as the endpoint it names would check a live request, and writes it again
/// with the model as the provider names it. Every line must name the same endpoint and model,
/// as OpenAI's batches require, and the key must be allowed that model.
async fn check(state: &AppState, caller: &Caller, file: &[u8]) -> Result<Checked, KppError> {
    let models = super::listing::project_models(
        state,
        caller.project_id,
        caller.key_id,
        caller.service_account_id,
    )
    .await?;
    let mut first: Option<(&'static str, DialectId, RequestKind, String)> = None;
    let mut target = None;
    let mut ids = HashSet::new();
    let mut lines = Vec::new();
    for (i, raw) in file.split(|b| *b == b'\n').enumerate() {
        let n = i + 1;
        let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
        if raw.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        if lines.len() == MAX_LINES {
            return Err(invalid(
                "file",
                format!("A batch file holds at most {MAX_LINES} requests."),
            ));
        }
        let mut v: Value = serde_json::from_slice(raw)
            .map_err(|_| at_line(n, "it isn't a JSON object.".to_owned()))?;
        let custom_id = v
            .get("custom_id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| at_line(n, "it has no `custom_id`.".to_owned()))?
            .to_owned();
        if !ids.insert(custom_id.clone()) {
            return Err(at_line(
                n,
                format!("`custom_id` `{custom_id}` is used before."),
            ));
        }
        if v.get("method").and_then(Value::as_str) != Some("POST") {
            return Err(at_line(n, "`method` must be `POST`.".to_owned()));
        }
        let url = v.get("url").and_then(Value::as_str).unwrap_or_default();
        let Some(&(endpoint, dialect, kind)) = ENDPOINTS.iter().find(|(e, _, _)| *e == url) else {
            return Err(at_line(
                n,
                format!(
                    "`url` `{url}` isn't one Kaeso runs batches of: `/v1/chat/completions`, `/v1/responses` or `/v1/embeddings`."
                ),
            ));
        };
        let body = v
            .get("body")
            .filter(|b| b.is_object())
            .ok_or_else(|| at_line(n, "`body` must be an object.".to_owned()))?;
        if body.get("stream").and_then(Value::as_bool) == Some(true) {
            return Err(at_line(
                n,
                "a batch can't stream; leave `stream` out.".to_owned(),
            ));
        }
        let model = body
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        match &first {
            None => first = Some((endpoint, dialect, kind, model.clone())),
            Some((e, _, _, _)) if *e != endpoint => {
                return Err(at_line(
                    n,
                    format!(
                        "it asks for `{endpoint}`, but the file's first line asks for `{e}`; a batch runs one endpoint."
                    ),
                ));
            }
            Some((_, _, _, m)) if *m != model => {
                return Err(at_line(
                    n,
                    format!(
                        "it names `{model}`, but the file's first line names `{m}`; a batch runs one model."
                    ),
                ));
            }
            Some(_) => {}
        }
        let body_bytes = serde_json::to_vec(body).unwrap_or_default();
        dispatch::decode_request(dialect, kind, &body_bytes, &HeaderList::new())
            .map_err(|e| at_line(n, e.message))?;
        if target.is_none() {
            target = Some(resolve(state, caller, &models, n, &model, dialect).await?);
        }
        let Some((_, eff, _)) = &target else {
            continue;
        };
        if let Some(b) = v.get_mut("body").and_then(Value::as_object_mut) {
            b.insert("model".into(), Value::from(eff.sent_model()));
        }
        lines.push(serde_json::to_vec(&v).unwrap_or_default());
    }
    let (Some((endpoint, _, _, model)), Some((entry, eff, credential))) = (first, target) else {
        return Err(invalid("file", "The file holds no requests.".to_owned()));
    };
    Ok(Checked {
        endpoint,
        entry,
        upstream_model: eff.sent_model().to_owned(),
        model,
        provider: eff.provider.clone(),
        credential,
        lines,
    })
}

type Target = (
    Uuid,
    kaeso_core::catalogue::Effective,
    Arc<crate::services::credentials::Resolved>,
);

async fn resolve(
    state: &AppState,
    caller: &Caller,
    models: &kaeso_store::pg::catalogue::ProjectModels,
    n: usize,
    model: &str,
    dialect: DialectId,
) -> Result<Target, KppError> {
    let unknown = || at_line(n, format!("no model `{model}` is available to this key."));
    let entry = match models.catalogue.resolve(model) {
        Ok(Resolved::Model(entry, _)) => entry,
        Ok(Resolved::Alias(_)) => {
            return Err(at_line(
                n,
                format!("`{model}` is an alias; a batch names one model."),
            ));
        }
        Err(_) => return Err(unknown()),
    };
    if !models
        .access(caller.key_id, caller.user_id, caller.service_account_id)
        .allows(entry.id)
    {
        return Err(unknown());
    }
    let eff = crate::services::catalogue::effective(state, entry, &models.providers)
        .ok_or_else(unknown)?;
    if kaeso_core::cost::batch_percent(eff.provider.id()).is_none() {
        return Err(at_line(
            n,
            format!(
                "{provider} runs no batches Kaeso can send.",
                provider = eff.provider.name()
            ),
        ));
    }
    // A batch line goes in its endpoint's format, which the provider may take for the model even
    // where live requests use another, as OpenAI takes chat models in Chat Completions and in
    // Responses.
    let embeds = dialect == DialectId::OpenAiEmbeddings;
    if eff.is_embeddings() != embeds || !eff.provider.dialects().contains(&dialect) {
        return Err(at_line(
            n,
            format!(
                "`{model}` isn't served in this endpoint's format; Kaeso doesn't translate batches yet."
            ),
        ));
    }
    let credential = crate::services::credentials::for_provider(
        state,
        OrgId::from_uuid(caller.org_id),
        ProjectId::from_uuid(caller.project_id),
        &eff.provider,
    )
    .await
    .map_err(|e| invalid("file", e.to_string()))?;
    if credential.method != AuthMethod::ApiKey {
        return Err(at_line(
            n,
            format!(
                "this project's {provider} key is an account sign-in, which runs no batches.",
                provider = eff.provider.name()
            ),
        ));
    }
    Ok((entry.id, eff, credential))
}

fn file_view(f: &FileRow) -> Value {
    json!({
        "id": BatchFileId::from_uuid(f.id).to_string(),
        "object": "file",
        "bytes": f.bytes,
        "created_at": unix(f.created_at),
        "filename": f.filename,
        "purpose": f.purpose,
        "status": "processed",
        "expires_at": f.expires_at.map(unix),
    })
}

async fn upload(
    state: &AppState,
    caller: &Caller,
    headers: &HeaderMap,
    body: Body,
) -> Result<Value, KppError> {
    let limit = state.config.body_limits.model_api.as_usize();
    let bytes = crate::http::body::read(body, limit).await.map_err(|e| {
        if crate::http::body_timeout::timed_out(&e) {
            error(
                ErrorKind::InvalidRequest,
                400,
                "The request body did not arrive in time; send it again.",
            )
        } else if crate::http::body::over_limit(&e) {
            error(
                ErrorKind::RequestTooLarge,
                413,
                "The file is larger than this install accepts.",
            )
        } else {
            error(
                ErrorKind::InvalidRequest,
                400,
                "The request body broke off before it was complete; send it again.",
            )
        }
    })?;
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let boundary = form_boundary(content_type).map_err(|e| {
        invalid(
            "file",
            format!("Send the file as `multipart/form-data`: {e}."),
        )
    })?;
    let parts = read_form(&bytes, &boundary, FormLimits::new(8, limit))
        .map_err(|e| invalid("file", format!("The form can't be read: {e}.")))?;
    let purpose = parts
        .iter()
        .find(|p| p.name == "purpose")
        .and_then(|p| p.text())
        .unwrap_or_default();
    if purpose != "batch" {
        return Err(invalid(
            "purpose",
            "Kaeso takes files with the purpose `batch` only.".to_owned(),
        ));
    }
    let file = parts
        .iter()
        .find(|p| p.name == "file")
        .ok_or_else(|| invalid("file", "The form has no `file`.".to_owned()))?;
    let filename = file
        .filename
        .clone()
        .unwrap_or_else(|| "batch.jsonl".to_owned());
    let checked = check(state, caller, file.body).await?;
    pipeline::admit_follow_up(state, caller, Some(checked.entry)).await?;
    let mut joined = checked.lines.join(&b'\n');
    joined.push(b'\n');
    let form = FormWriter::write(&[
        FormField::Text("purpose", "batch"),
        FormField::File("file", &filename, "application/jsonl", &joined),
    ]);
    let content_type = form.content_type();
    let sent = send(
        state,
        &checked.provider,
        &checked.credential,
        "POST",
        "/v1/files".to_owned(),
        HeaderList::new().with("content-type", content_type),
        Bytes::from(form.into_body()),
    )
    .await?;
    let answer = provider_json(&sent)?;
    let upstream_id = answer
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let now = state.clock.now();
    let row = FileRow {
        id: Uuid::now_v7(),
        org_id: caller.org_id,
        project_id: caller.project_id,
        api_key_id: caller.key_id,
        user_id: caller.user_id,
        service_account_id: caller.service_account_id,
        credential_id: checked.credential.id.uuid(),
        provider: checked.provider.id().to_owned(),
        upstream_id,
        purpose: "batch".to_owned(),
        filename,
        bytes: i64::try_from(joined.len()).unwrap_or(i64::MAX),
        lines: i32::try_from(checked.lines.len()).unwrap_or(i32::MAX),
        endpoint: Some(checked.endpoint.to_owned()),
        catalogue_model_id: Some(checked.entry),
        model: Some(checked.model),
        upstream_model: Some(checked.upstream_model),
        created_at: now,
        expires_at: answer
            .get("expires_at")
            .and_then(Value::as_i64)
            .and_then(|t| OffsetDateTime::from_unix_timestamp(t).ok()),
    };
    store::insert_file(&state.pg, &row)
        .await
        .map_err(store_error)?;
    Ok(file_view(&row))
}

async fn load_file(
    state: &AppState,
    caller: &Caller,
    id: &str,
    param: &str,
) -> Result<FileRow, KppError> {
    let parsed: BatchFileId = id.parse().map_err(|_| not_found("file", id, param))?;
    store::file(&state.pg, caller.org_id, caller.project_id, parsed.uuid())
        .await
        .map_err(store_error)?
        .ok_or_else(|| not_found("file", id, param))
}

async fn file_content(state: &AppState, caller: &Caller, id: &str) -> Result<Bytes, KppError> {
    let f = load_file(state, caller, id, "file_id").await?;
    let (provider, credential) = reach(state, caller, &f.provider, f.credential_id).await?;
    let sent = send(
        state,
        &provider,
        &credential,
        "GET",
        format!("/v1/files/{}/content", f.upstream_id),
        HeaderList::new(),
        Bytes::new(),
    )
    .await?;
    if !(200..300).contains(&sent.status) {
        return Err(dispatch::decode_error(
            D,
            sent.status,
            &sent.headers,
            &sent.body,
        ));
    }
    Ok(sent.body)
}

async fn delete_file(state: &AppState, caller: &Caller, id: &str) -> Result<Value, KppError> {
    let f = load_file(state, caller, id, "file_id").await?;
    let (provider, credential) = reach(state, caller, &f.provider, f.credential_id).await?;
    let sent = send(
        state,
        &provider,
        &credential,
        "DELETE",
        format!("/v1/files/{}", f.upstream_id),
        HeaderList::new(),
        Bytes::new(),
    )
    .await?;
    if !(200..300).contains(&sent.status) && sent.status != 404 {
        return Err(dispatch::decode_error(
            D,
            sent.status,
            &sent.headers,
            &sent.body,
        ));
    }
    store::delete_file(&state.pg, caller.project_id, f.id)
        .await
        .map_err(store_error)?;
    Ok(json!({ "id": id, "object": "file", "deleted": true }))
}

// ----- batches -----

/// The batch as OpenAI's API shows it, with Kaeso's ids and the model as the app named it.
fn batch_view(row: &BatchRow) -> Value {
    let mut v = row.view.clone();
    if let Some(o) = v.as_object_mut() {
        o.insert("id".into(), BatchId::from_uuid(row.id).to_string().into());
        o.insert(
            "input_file_id".into(),
            BatchFileId::from_uuid(row.input_file_id).to_string().into(),
        );
        for (key, id) in [
            ("output_file_id", row.output_file_id),
            ("error_file_id", row.error_file_id),
        ] {
            o.insert(
                key.into(),
                id.map_or(Value::Null, |id| {
                    BatchFileId::from_uuid(id).to_string().into()
                }),
            );
        }
        if o.contains_key("model") {
            o.insert("model".into(), row.model.clone().into());
        }
    }
    v
}

async fn create(state: &AppState, caller: &Caller, body: &[u8]) -> Result<BatchRow, KppError> {
    let asked: Value = serde_json::from_slice(body)
        .ok()
        .filter(Value::is_object)
        .ok_or_else(|| invalid("body", "The body must be a JSON object.".to_owned()))?;
    let file_id = asked
        .get("input_file_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("input_file_id", "`input_file_id` is required.".to_owned()))?;
    let file = load_file(state, caller, file_id, "input_file_id").await?;
    let (Some(endpoint), Some(entry), Some(model), Some(upstream_model)) = (
        file.endpoint.clone(),
        file.catalogue_model_id,
        file.model.clone(),
        file.upstream_model.clone(),
    ) else {
        return Err(not_found("file", file_id, "input_file_id"));
    };
    if asked.get("endpoint").and_then(Value::as_str) != Some(endpoint.as_str()) {
        return Err(invalid(
            "endpoint",
            format!("The file's requests ask for `{endpoint}`; give that `endpoint`."),
        ));
    }
    if asked.get("completion_window").and_then(Value::as_str) != Some("24h") {
        return Err(invalid(
            "completion_window",
            "`completion_window` must be `24h`.".to_owned(),
        ));
    }
    let models = super::listing::project_models(
        state,
        caller.project_id,
        caller.key_id,
        caller.service_account_id,
    )
    .await?;
    let unknown = || not_found("file", file_id, "input_file_id");
    let entry_ref = models.catalogue.entry(entry).ok_or_else(unknown)?;
    if !models
        .access(caller.key_id, caller.user_id, caller.service_account_id)
        .allows(entry)
    {
        return Err(unknown());
    }
    let eff = crate::services::catalogue::effective(state, entry_ref, &models.providers)
        .ok_or_else(unknown)?;
    let percent = kaeso_core::cost::batch_percent(eff.provider.id()).ok_or_else(unknown)?;
    let (provider, credential) = reach(state, caller, &file.provider, file.credential_id).await?;
    pipeline::admit_follow_up(state, caller, Some(entry)).await?;
    let mut sent_body = json!({
        "input_file_id": file.upstream_id,
        "endpoint": endpoint,
        "completion_window": "24h",
    });
    if let (Some(m), Some(o)) = (
        asked.get("metadata").filter(|m| !m.is_null()),
        sent_body.as_object_mut(),
    ) {
        o.insert("metadata".into(), m.clone());
    }
    let sent = send(
        state,
        &provider,
        &credential,
        "POST",
        "/v1/batches".to_owned(),
        HeaderList::new().with("content-type", "application/json"),
        Bytes::from(sent_body.to_string()),
    )
    .await?;
    let answer = provider_json(&sent)?;
    let prices: Fields = kaeso_core::cost::at_share(&eff.fields, percent);
    let row = BatchRow {
        id: Uuid::now_v7(),
        org_id: caller.org_id,
        project_id: caller.project_id,
        api_key_id: caller.key_id,
        user_id: caller.user_id,
        service_account_id: caller.service_account_id,
        credential_id: credential.id.uuid(),
        provider: provider.id().to_owned(),
        upstream_id: answer
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        input_file_id: file.id,
        output_file_id: None,
        error_file_id: None,
        endpoint,
        catalogue_model_id: entry,
        model,
        upstream_model,
        prices: serde_json::to_value(&prices).ok(),
        metadata: asked.get("metadata").cloned(),
        status: answer
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("validating")
            .to_owned(),
        view: answer,
        created_at: state.clock.now(),
        accounted_at: None,
    };
    store::insert(&state.pg, &row).await.map_err(store_error)?;
    Ok(row)
}

async fn load_batch(state: &AppState, caller: &Caller, id: &str) -> Result<BatchRow, KppError> {
    let parsed: BatchId = id.parse().map_err(|_| not_found("batch", id, "batch_id"))?;
    store::get(&state.pg, caller.org_id, caller.project_id, parsed.uuid())
        .await
        .map_err(store_error)?
        .ok_or_else(|| not_found("batch", id, "batch_id"))
}

fn caller_of(row: &BatchRow) -> Caller {
    Caller {
        org_id: row.org_id,
        project_id: row.project_id,
        key_id: row.api_key_id,
        user_id: row.user_id,
        service_account_id: row.service_account_id,
    }
}

/// The Kaeso id of an output or error file the provider names, registered the first time.
async fn output_file(
    state: &AppState,
    row: &BatchRow,
    upstream: Option<&str>,
) -> Result<Option<Uuid>, KppError> {
    let Some(upstream) = upstream.filter(|u| !u.is_empty()) else {
        return Ok(None);
    };
    if let Some(id) = store::file_by_upstream(&state.pg, row.project_id, &row.provider, upstream)
        .await
        .map_err(store_error)?
    {
        return Ok(Some(id));
    }
    let f = FileRow {
        id: Uuid::now_v7(),
        org_id: row.org_id,
        project_id: row.project_id,
        api_key_id: row.api_key_id,
        user_id: row.user_id,
        service_account_id: row.service_account_id,
        credential_id: row.credential_id,
        provider: row.provider.clone(),
        upstream_id: upstream.to_owned(),
        purpose: "batch_output".to_owned(),
        filename: format!("{upstream}.jsonl"),
        bytes: 0,
        lines: 0,
        endpoint: None,
        catalogue_model_id: None,
        model: None,
        upstream_model: None,
        created_at: state.clock.now(),
        expires_at: None,
    };
    store::insert_file(&state.pg, &f)
        .await
        .map_err(store_error)?;
    store::file_by_upstream(&state.pg, row.project_id, &row.provider, upstream)
        .await
        .map_err(store_error)
}

/// Takes the provider's answer about the batch into its row.
async fn take(state: &AppState, mut row: BatchRow, answer: Value) -> Result<BatchRow, KppError> {
    let status = answer
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or(&row.status)
        .to_owned();
    row.output_file_id = output_file(
        state,
        &row,
        answer.get("output_file_id").and_then(Value::as_str),
    )
    .await?;
    row.error_file_id = output_file(
        state,
        &row,
        answer.get("error_file_id").and_then(Value::as_str),
    )
    .await?;
    store::update(
        &state.pg,
        row.id,
        &status,
        &answer,
        row.output_file_id,
        row.error_file_id,
    )
    .await
    .map_err(store_error)?;
    row.status = status;
    row.view = answer;
    Ok(row)
}

/// Asks the provider how the batch stands, and records its usage once it ended.
pub(crate) async fn refresh(state: &AppState, row: BatchRow) -> Result<BatchRow, KppError> {
    if row.accounted_at.is_some() {
        return Ok(row);
    }
    let caller = caller_of(&row);
    let (provider, credential) = reach(state, &caller, &row.provider, row.credential_id).await?;
    let sent = send(
        state,
        &provider,
        &credential,
        "GET",
        format!("/v1/batches/{}", row.upstream_id),
        HeaderList::new(),
        Bytes::new(),
    )
    .await?;
    let answer = provider_json(&sent)?;
    let row = take(state, row, answer).await?;
    if ENDED.contains(&row.status.as_str()) {
        account(state, &row, &provider, &credential).await?;
    }
    Ok(row)
}

/// Records each line of an ended batch's output as a request at the batch price, once.
async fn account(
    state: &AppState,
    row: &BatchRow,
    provider: &ProviderRef,
    credential: &crate::services::credentials::Resolved,
) -> Result<(), KppError> {
    let output = match row.view.get("output_file_id").and_then(Value::as_str) {
        Some(up) if !up.is_empty() => {
            let sent = send(
                state,
                provider,
                credential,
                "GET",
                format!("/v1/files/{up}/content"),
                HeaderList::new(),
                Bytes::new(),
            )
            .await?;
            if !(200..300).contains(&sent.status) {
                return Err(dispatch::decode_error(
                    D,
                    sent.status,
                    &sent.headers,
                    &sent.body,
                ));
            }
            sent.body
        }
        _ => Bytes::new(),
    };
    if !store::mark_accounted(&state.pg, row.id, state.clock.now())
        .await
        .map_err(store_error)?
    {
        return Ok(());
    }
    let Some(&(_, dialect, kind)) = ENDPOINTS.iter().find(|(e, _, _)| *e == row.endpoint) else {
        return Ok(());
    };
    let prices: Option<Fields> = row
        .prices
        .clone()
        .and_then(|p| serde_json::from_value(p).ok());
    let ectx = kaeso_codecs::EgressCtx::for_kind(kind, kaeso_kpp::StreamMode::Off)
        .with_provider(provider.id())
        .with_responses_profile(provider.responses());
    for line in output.split(|b| *b == b'\n') {
        let Ok(v) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        let response = v.get("response").unwrap_or(&Value::Null);
        let reply = response.get("body").unwrap_or(&Value::Null);
        let status = response
            .get("status_code")
            .and_then(Value::as_u64)
            .and_then(|s| u16::try_from(s).ok())
            .unwrap_or(500);
        let body = serde_json::to_vec(reply).unwrap_or_default();
        let usage =
            match dispatch::decode_response(dialect, status, &HeaderList::new(), &body, &ectx) {
                Ok(Reply::Generate(r)) => Some(r.usage),
                Ok(Reply::Embed(e)) => Some(e.usage),
                _ => None,
            };
        let mut o = pipeline::blank_outcome(RequestId::new());
        o.caller = Some(caller_of(row));
        o.ingress = dialect;
        o.egress = Some(dialect);
        o.kind = kind;
        o.provider = provider.built_in().map(|p| p.id);
        o.requested_model = Some(row.model.clone());
        o.catalogue_model_id = Some(row.catalogue_model_id);
        o.model_id = Some(row.upstream_model.clone());
        o.response_model = reply
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_owned);
        o.prices = prices.clone();
        o.status = status;
        o.usage = usage;
        o.started_at = state.clock.now();
        o.upstream_request_id = v
            .get("custom_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        state.sink.finish(state, o).await;
    }
    Ok(())
}

/// Asks the providers about the batches whose usage isn't recorded yet.
pub(crate) async fn settle_open(state: &AppState) -> Result<String, kaeso_store::StoreError> {
    let open = store::open(&state.pg, 200).await?;
    let mut ended = 0;
    for row in open {
        match refresh(state, row).await {
            Ok(r) if r.accounted_at.is_some() || ENDED.contains(&r.status.as_str()) => ended += 1,
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e.message, "cannot ask the provider about a batch"),
        }
    }
    Ok(format!("{ended} batches ended"))
}

async fn cancel(state: &AppState, caller: &Caller, id: &str) -> Result<BatchRow, KppError> {
    let row = load_batch(state, caller, id).await?;
    let (provider, credential) = reach(state, caller, &row.provider, row.credential_id).await?;
    let sent = send(
        state,
        &provider,
        &credential,
        "POST",
        format!("/v1/batches/{}/cancel", row.upstream_id),
        HeaderList::new(),
        Bytes::new(),
    )
    .await?;
    let answer = provider_json(&sent)?;
    take(state, row, answer).await
}

async fn list(state: &AppState, caller: &Caller, query: Option<&str>) -> Result<Value, KppError> {
    let mut after = None;
    let mut limit = 20_i64;
    for (k, v) in url::form_urlencoded::parse(query.unwrap_or_default().as_bytes()) {
        match k.as_ref() {
            "after" => {
                after = Some(
                    v.parse::<BatchId>()
                        .map_err(|_| invalid("after", "`after` isn't a batch id.".to_owned()))?
                        .uuid(),
                );
            }
            "limit" => {
                limit = v
                    .parse::<i64>()
                    .ok()
                    .filter(|n| (1..=100).contains(n))
                    .ok_or_else(|| invalid("limit", "`limit` must be 1 to 100.".to_owned()))?;
            }
            _ => {}
        }
    }
    let rows = store::list(
        &state.pg,
        caller.org_id,
        caller.project_id,
        after,
        limit + 1,
    )
    .await
    .map_err(store_error)?;
    let has_more = rows.len() > usize::try_from(limit).unwrap_or(usize::MAX);
    let data: Vec<Value> = rows
        .iter()
        .take(usize::try_from(limit).unwrap_or(usize::MAX))
        .map(batch_view)
        .collect();
    Ok(json!({
        "object": "list",
        "first_id": data.first().and_then(|b| b["id"].as_str()),
        "last_id": data.last().and_then(|b| b["id"].as_str()),
        "has_more": has_more,
        "data": data,
    }))
}

// ----- handlers -----

pub(super) async fn files_create(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let answer = async {
        let c = caller(&state, &headers, ip).await?;
        upload(&state, &c, &headers, body).await
    }
    .await;
    json_answer(StatusCode::OK, answer)
}

pub(super) async fn files_get(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let id = segment(&uri, "/v1/files/");
    let answer = async {
        let c = caller(&state, &headers, ip).await?;
        load_file(&state, &c, id, "file_id")
            .await
            .map(|f| file_view(&f))
    }
    .await;
    json_answer(StatusCode::OK, answer)
}

pub(super) async fn files_content(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let id = segment(&uri, "/v1/files/");
    let answer = async {
        let c = caller(&state, &headers, ip).await?;
        file_content(&state, &c, id).await
    }
    .await;
    match answer {
        Ok(bytes) => {
            let mut resp = Response::new(Body::from(bytes));
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            );
            resp
        }
        Err(e) => respond(D, &e, &Served::default()),
    }
}

pub(super) async fn files_delete(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let id = segment(&uri, "/v1/files/").to_owned();
    let answer = async {
        let c = caller(&state, &headers, ip).await?;
        delete_file(&state, &c, &id).await
    }
    .await;
    json_answer(StatusCode::OK, answer)
}

pub(super) async fn batches_create(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let answer = async {
        let c = caller(&state, &headers, ip).await?;
        create(&state, &c, &body).await.map(|r| batch_view(&r))
    }
    .await;
    json_answer(StatusCode::OK, answer)
}

pub(super) async fn batches_list(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let answer = async {
        let c = caller(&state, &headers, ip).await?;
        list(&state, &c, uri.query()).await
    }
    .await;
    json_answer(StatusCode::OK, answer)
}

pub(super) async fn batches_get(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let id = segment(&uri, "/v1/batches/").to_owned();
    let answer = async {
        let c = caller(&state, &headers, ip).await?;
        let row = load_batch(&state, &c, &id).await?;
        refresh(&state, row).await.map(|r| batch_view(&r))
    }
    .await;
    json_answer(StatusCode::OK, answer)
}

pub(super) async fn batches_cancel(
    State(state): State<AppState>,
    ip: ClientIp,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let id = segment(&uri, "/v1/batches/").to_owned();
    let answer = async {
        let c = caller(&state, &headers, ip).await?;
        cancel(&state, &c, &id).await.map(|r| batch_view(&r))
    }
    .await;
    json_answer(StatusCode::OK, answer)
}
