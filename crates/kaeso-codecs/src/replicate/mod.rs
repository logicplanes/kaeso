//! Replicate's predictions (<https://replicate.com/docs/reference/http>, the HTTP API's OpenAPI
//! document, read 2026-11-14): `POST /v1/models/{owner}/{name}/predictions` with
//! `{"input": {...}}` starts a prediction of an official model; `GET /v1/predictions/{id}` tells
//! how it stands until its `status` is `succeeded`, `failed`, `canceled` or `aborted`, and its
//! `output` names the made files on `replicate.delivery`, deleted after an hour. Every model has
//! its own input (its Cog OpenAPI schema), so the dialect maps OpenAI's image and video fields
//! per model, for the models in [`IMAGE_MODELS`] and [`VIDEO_MODELS`] only.

mod error;
mod images;
mod videos;

#[cfg(test)]
mod tests;

use kaeso_kpp::{
    DialectId, KppError, ModelInfo, ModelList, ModelsQuery, Origin, Reply, Request, RequestKind,
    VideoJob,
};
use serde_json::{Map, Value};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};
use crate::video_jobs::{Fields, no_stream, path_segment, video_reply};

pub use images::IMAGE_MODELS;
pub use videos::VIDEO_MODELS;

const D: DialectId = DialectId::Replicate;
const PROVIDER: &str = "Replicate";

/// The hosts Replicate serves made files from: `replicate.delivery` and its subdomains
/// (<https://replicate.com/docs/topics/predictions/output-files>).
pub const OUTPUT_HOSTS: &[&str] = &["replicate.delivery", "*.replicate.delivery"];

/// How long a prediction request waits for its output before Replicate answers with the
/// prediction still running: `Prefer: wait=n` takes 1 to 60 seconds (the OpenAPI document's
/// `Prefer` header).
const WAIT: (&str, &str) = ("prefer", "wait=60");

const STARTING: &str = "starting";
const PROCESSING: &str = "processing";
const SUCCEEDED: &str = "succeeded";
const FAILED: &str = "failed";
const CANCELED: &str = "canceled";
const ABORTED: &str = "aborted";

/// Replicate's HTTP API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Replicate;

impl crate::codec::sealed::Sealed for Replicate {}

fn safe_segment(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// The request that starts a prediction of `model`, an official model's `owner/name`.
fn submit(model: &str, input: Map<String, Value>, wait: bool) -> Result<EncodedRequest, KppError> {
    let path = match model.split_once('/') {
        Some((owner, name)) if safe_segment(owner) && safe_segment(name) => {
            format!("/v1/models/{owner}/{name}/predictions")
        }
        _ => {
            return Err(KppError::invalid_request(format!(
                "The model id `{model}` is not a Replicate model's `owner/name`."
            ))
            .with_param("model")
            .with_status(400));
        }
    };
    let mut out = Map::new();
    out.insert("input".into(), Value::Object(input));
    let mut headers = HeaderList::new().with("content-type", crate::codec::JSON_MEDIA_TYPE);
    if wait {
        headers = headers.with(WAIT.0, WAIT.1);
    }
    Ok(EncodedRequest::post_json(
        path,
        headers,
        crate::video_jobs::body(&out)?,
    ))
}

/// A `GET` of the prediction `id`.
fn get(id: &str, param: &'static str) -> Result<EncodedRequest, KppError> {
    if !safe_segment(id) {
        return Err(
            KppError::invalid_request(format!("`{id}` is not a Replicate prediction id."))
                .with_param(param)
                .with_status(400),
        );
    }
    Ok(EncodedRequest::get(
        format!("/v1/predictions/{id}"),
        HeaderList::new(),
    ))
}

/// A prediction as Replicate writes it: its id, status and output.
struct Prediction {
    id: String,
    status: String,
    output: Value,
}

impl Prediction {
    fn of(v: &Value) -> Option<Prediction> {
        let status = v.get("status")?.as_str()?.to_owned();
        if !matches!(
            status.as_str(),
            STARTING | PROCESSING | SUCCEEDED | FAILED | CANCELED | ABORTED
        ) {
            return None;
        }
        let id = v.get("id")?.as_str()?.to_owned();
        Some(Prediction {
            id,
            status,
            output: v.get("output").cloned().unwrap_or(Value::Null),
        })
    }

    fn running(&self) -> bool {
        matches!(self.status.as_str(), STARTING | PROCESSING)
    }

    fn ended_badly(&self) -> bool {
        matches!(self.status.as_str(), FAILED | CANCELED | ABORTED)
    }

    /// The links in `output`: one link, or a list of them.
    fn links(&self) -> Option<Vec<String>> {
        match &self.output {
            Value::String(s) if !s.is_empty() => Some(vec![s.clone()]),
            Value::Array(items) if !items.is_empty() => items
                .iter()
                .map(|i| i.as_str().filter(|s| !s.is_empty()).map(str::to_owned))
                .collect(),
            _ => None,
        }
    }
}

fn prediction_of(body: &[u8]) -> Result<(Value, Prediction), KppError> {
    let v: Value = serde_json::from_slice(body).map_err(|_| {
        KppError::provider(format!("{PROVIDER} sent a prediction that is not JSON."))
    })?;
    let p = Prediction::of(&v).ok_or_else(|| {
        KppError::provider(format!(
            "{PROVIDER} sent a prediction Kaeso can't read: no id or no known status."
        ))
    })?;
    Ok((v, p))
}

/// The prediction to ask about next while Replicate makes an image: its id while it is
/// `starting` or `processing`, `None` once it has ended.
pub(crate) fn pending_task(body: &[u8]) -> Option<String> {
    let v: Value = serde_json::from_slice(body).ok()?;
    let p = Prediction::of(&v)?;
    (p.running() && safe_segment(&p.id)).then_some(p.id)
}

/// The request that asks how the prediction [`pending_task`] named stands.
pub(crate) fn poll_request(task: &str) -> Result<EncodedRequest, KppError> {
    get(task, "task_id")
}

/// A Replicate request's envelope: its `input`, which holds the model's fields. Webhooks and
/// the other envelope fields are Kaeso's to set, so they are refused.
fn input_of(body: &[u8]) -> Result<Fields, KppError> {
    let mut envelope = Fields::parse(body, D, "request")?;
    let input = envelope.object("input")?.ok_or_else(|| {
        KppError::invalid_request("A Replicate request needs its `input`.").with_param("input")
    })?;
    let rest = envelope.rest(&Origin::new(D));
    if let Some(name) = rest.fields.keys().next() {
        return Err(crate::video_jobs::refused(
            D,
            "Kaeso",
            &format!("`{name}`"),
            name,
            "send the model's fields in `input`.",
        ));
    }
    Ok(input)
}

/// The collection of Replicate's official models, the only ones Kaeso maps: "always on" and
/// priced per output (<https://replicate.com/docs/topics/models/official-models>). It names
/// each model's `owner` and `name` (the OpenAPI document's `schemas_collection_response`).
const OFFICIAL: &str = "/v1/collections/official";

fn model_of(v: &Value) -> Option<ModelInfo> {
    let owner = v.get("owner")?.as_str()?;
    let name = v.get("name")?.as_str()?;
    Some(ModelInfo::new(format!("{owner}/{name}")))
}

fn listing_object(status: u16, headers: &HeaderList, body: &[u8]) -> Result<Value, KppError> {
    if !(200..300).contains(&status) {
        return Err(error::decode(status, headers, body));
    }
    serde_json::from_slice(body).map_err(|_| {
        KppError::provider(format!("{PROVIDER} sent a model listing that is not JSON."))
    })
}

fn not_served(kind: RequestKind) -> KppError {
    KppError::unsupported_by(
        format!("{kind:?} request"),
        D,
        "Kaeso maps Replicate's image and video models only.",
    )
}

impl Codec for Replicate {
    const DIALECT: DialectId = D;

    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        _headers: &HeaderList,
    ) -> Result<Request, KppError> {
        crate::decisions::refuse_kind(kind, Self::DIALECT)?;
        crate::limits::request_shape(D, body)?;
        match kind {
            RequestKind::Image => images::decode_request(input_of(body)?),
            RequestKind::Video => videos::decode_request(input_of(body)?),
            other => Err(not_served(other)),
        }
    }

    fn encode_response(reply: &Reply, _ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        crate::decisions::refuse_reply(reply, Self::DIALECT)?;
        match reply {
            Reply::Image(r) => images::encode_response(r),
            other => crate::wire::to_body(&videos::encode_job(video_reply(other, D)?)),
        }
    }

    fn stream_encoder(_ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        no_stream(Origin::new(D))
    }

    fn encode_error(err: &KppError) -> EncodedError {
        error::encode(err)
    }

    fn check(req: &Request, _ctx: &EgressCtx) -> Result<(), KppError> {
        crate::decisions::refuse_request(req, Self::DIALECT)?;
        match req {
            Request::Image(r) => images::check(r).map(|_| ()),
            Request::Video(r) => videos::check(r).map(|_| ()),
            other => Err(not_served(other.kind())),
        }
    }

    fn encode_request(req: &Request, _ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
        crate::decisions::refuse_request(req, Self::DIALECT)?;
        match req {
            Request::Image(r) => images::encode_request(r),
            Request::Video(r) => videos::encode_request(r),
            other => Err(not_served(other.kind())),
        }
    }

    fn decode_response(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<Reply, KppError> {
        if !(200..300).contains(&status) {
            return Err(error::decode(status, headers, body));
        }
        match ctx.kind {
            RequestKind::Image => images::decode_response(body, ctx).map(Reply::Image),
            RequestKind::Video => videos::decode_job(body, ctx).map(Reply::Video),
            other => Err(not_served(other)),
        }
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        no_stream(ctx.origin(D))
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        error::decode(status, headers, body)
    }

    fn encode_video_poll(
        job: &str,
        _model: &str,
        _ctx: &EgressCtx,
    ) -> Result<EncodedRequest, KppError> {
        get(job, "video_id")
    }

    fn decode_video_poll(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<VideoJob, KppError> {
        if !(200..300).contains(&status) {
            return Err(error::decode(status, headers, body));
        }
        videos::decode_job(body, ctx)
    }

    fn decode_models_query(query: &str) -> Result<ModelsQuery, KppError> {
        crate::openai_models::decode_query(D, query)
    }

    fn encode_models(list: &ModelList) -> Result<Vec<u8>, KppError> {
        crate::openai_models::encode_models(list)
    }

    fn encode_model(model: &ModelInfo) -> Result<Vec<u8>, KppError> {
        crate::openai_models::encode_model(model)
    }

    fn decode_models(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
    ) -> Result<ModelList, KppError> {
        let v = listing_object(status, headers, body)?;
        let models = v.get("models").and_then(Value::as_array).ok_or_else(|| {
            KppError::provider(format!("{PROVIDER} sent a collection without `models`."))
        })?;
        Ok(ModelList::new(models.iter().filter_map(model_of).collect()))
    }

    fn decode_model(status: u16, headers: &HeaderList, body: &[u8]) -> Result<ModelInfo, KppError> {
        let v = listing_object(status, headers, body)?;
        model_of(&v).ok_or_else(|| {
            KppError::provider(format!(
                "{PROVIDER} sent a model without its owner and name."
            ))
        })
    }

    fn encode_model_request(model: &str) -> EncodedRequest {
        // A segment of dots would climb the path once a URL parser reads it.
        let segment = |s: &str| match s {
            "." | ".." => s.replace('.', "%2E"),
            other => path_segment(other),
        };
        let path = match model.split_once('/') {
            Some((owner, name)) => format!("/v1/models/{}/{}", segment(owner), segment(name)),
            None => format!("/v1/models/{}", segment(model)),
        };
        EncodedRequest::get(path, HeaderList::new())
    }

    fn encode_models_request(_query: &ModelsQuery) -> Result<EncodedRequest, KppError> {
        Ok(EncodedRequest::get(OFFICIAL, HeaderList::new()))
    }
}
