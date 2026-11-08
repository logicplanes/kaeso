#[cfg(test)]
mod tests;

use kaeso_kpp::{
    DialectId, ErrorKind, GeneratedImage, ImageDelivery, ImageOperation, ImageRequest,
    ImageResponse, ImageSize, KppError, ModelInfo, ModelList, ModelsQuery, Origin, RawJson, Reply,
    Request, RequestKind, Source, StreamEvent, StreamMode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};
use crate::cross::check_extras;
use crate::wire::{Ext, extra_for, extras_from, parse_provider, to_body, verbatim_from_form};

const D: DialectId = DialectId::MinimaxImages;

/// The path of MiniMax's text-to-image API, from the root of its host
/// (<https://platform.minimax.io/docs/api-reference/image-generation-t2i>).
pub const IMAGE_GENERATION_PATH: &str = "/v1/image_generation";

/// MiniMax's image generation API, `POST /v1/image_generation`: a prompt, an aspect ratio or
/// a size, and up to nine images, answered as links or base64 with errors in `base_resp`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct MinimaxImages;

impl crate::codec::sealed::Sealed for MinimaxImages {}
impl crate::codec::sealed::Sealed for NoStream {}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct RequestW {
    model: String,
    prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    aspect_ratio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    seed: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    n: Option<u32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct ResponseW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    data: Option<Ext<DataW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    base_resp: Option<BaseRespW>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct DataW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    image_urls: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    image_base64: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct BaseRespW {
    #[serde(default)]
    status_code: i64,
    #[serde(default)]
    status_msg: String,
}

// MiniMax's documented status codes (https://platform.minimax.io/docs/api-reference/errorcode).
fn kind_of(code: i64) -> ErrorKind {
    match code {
        1002 | 1039 => ErrorKind::RateLimited,
        1004 | 2049 => ErrorKind::Authentication,
        1008 => ErrorKind::Billing,
        1026 | 1027 => ErrorKind::ContentFiltered,
        2013 => ErrorKind::InvalidRequest,
        _ => ErrorKind::ProviderError,
    }
}

fn invalid(detail: impl std::fmt::Display) -> KppError {
    KppError::invalid_request(format!(
        "The request body is not a valid {} request: {detail}.",
        D.display_name()
    ))
}

fn refused(feature: &str, param: &str, hint: &str) -> KppError {
    KppError::unsupported_by(
        feature.to_owned(),
        D,
        format!("MiniMax does not take {feature}; {hint}"),
    )
    .with_param(param.to_owned())
}

fn decode_request(body: &[u8]) -> Result<Request, KppError> {
    let Ext {
        known: w,
        extra,
        verbatim,
    } = Ext::<RequestW>::from_slice(body)
        .map_err(|e| invalid(crate::wire::without_position(&e)))?;
    let mut r = ImageRequest::new(w.model, ImageOperation::Generate).with_prompt(w.prompt);
    r.aspect_ratio = w.aspect_ratio;
    r.size = match (w.width, w.height) {
        (Some(width), Some(height)) => Some(ImageSize::pixels(width, height)),
        (None, None) => None,
        _ => return Err(invalid("`width` and `height` go together").with_param("width")),
    };
    r.delivery = match w.response_format.as_deref() {
        None => None,
        Some("url") => Some(ImageDelivery::Url),
        Some("base64") => Some(ImageDelivery::Base64),
        Some(other) => {
            return Err(KppError::invalid_request(format!(
                "`response_format` must be `url` or `base64`, not `{other}`."
            ))
            .with_param("response_format"));
        }
    };
    r.seed = w.seed;
    r.count = w.n;
    r.extras = extras_from(&Origin::new(D), extra, verbatim);
    Ok(Request::Image(r))
}

fn check_request(r: &ImageRequest) -> Result<(), KppError> {
    check_extras(&r.extras, D, "", "request")?;
    match r.operation {
        ImageOperation::Generate => {}
        ImageOperation::Variation => {
            return Err(refused(
                "image variations",
                "image",
                "send a prompt to `/v1/images/generations` instead.",
            ));
        }
        _ => {
            return Err(refused(
                "images to edit",
                "image",
                "its image-to-image API takes one character reference, not an image to change.",
            ));
        }
    }
    if r.size
        .as_ref()
        .is_some_and(|s| *s != ImageSize::Auto && s.dimensions().is_none())
    {
        return Err(refused(
            "this size",
            "size",
            "send `WIDTHxHEIGHT` in pixels or `aspect_ratio`.",
        ));
    }
    if r.count.is_some_and(|n| n > 9) {
        return Err(refused("`n` above 9", "n", "ask for fewer images."));
    }
    let unsupported = [
        ("quality", r.quality.is_some()),
        ("output_format", r.format.is_some()),
        ("output_compression", r.compression.is_some()),
        ("background", r.background.is_some()),
        ("resolution", r.resolution.is_some()),
        ("negative_prompt", r.negative_prompt.is_some()),
    ];
    if let Some((field, _)) = unsupported.iter().find(|(_, set)| *set) {
        return Err(refused(&format!("`{field}`"), field, "remove it."));
    }
    if !r.stream.is_off() && r.partial_images.is_some_and(|n| n > 0) {
        return Err(refused(
            "partial images",
            "partial_images",
            "set `partial_images` to 0 to get the finished image as one event.",
        ));
    }
    Ok(())
}

fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    let Request::Image(r) = req else {
        return Err(not_images(req.kind()));
    };
    check_request(r)?;
    ctx.stream = StreamMode::Off;
    if r.user.is_some() {
        ctx.left_out.push("user".to_owned());
    }
    let (width, height) = r
        .size
        .as_ref()
        .and_then(ImageSize::dimensions)
        .map_or((None, None), |(w, h)| (Some(w), Some(h)));
    let w = RequestW {
        model: r.model.clone(),
        prompt: r.prompt.clone().unwrap_or_default(),
        aspect_ratio: r.aspect_ratio.clone(),
        width,
        height,
        response_format: r.delivery.map(|d| match d {
            ImageDelivery::Url => "url".to_owned(),
            _ => "base64".to_owned(),
        }),
        seed: r.seed,
        n: r.count,
    };
    let body = Ext::with(
        w,
        extra_for(&r.extras, D),
        verbatim_from_form(&r.extras, D, ""),
    );
    let path = match &ctx.base_path {
        Some(base) => format!("{base}{IMAGE_GENERATION_PATH}"),
        None => IMAGE_GENERATION_PATH.to_owned(),
    };
    Ok(EncodedRequest::post_json(
        path,
        HeaderList::new().with("content-type", "application/json"),
        to_body(&body)?,
    ))
}

fn not_images(kind: RequestKind) -> KppError {
    KppError::unsupported_by(
        format!("{kind:?} request"),
        D,
        "MiniMax's image API only generates images.",
    )
}

fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    base_resp_error(D, status, headers, body)
}

/// A MiniMax error: a failing `base_resp`, which MiniMax may send with HTTP 200, or else an
/// OpenAI-style error body.
pub(crate) fn base_resp_error(
    dialect: DialectId,
    status: u16,
    headers: &HeaderList,
    body: &[u8],
) -> KppError {
    let value: Option<Value> = serde_json::from_slice(body).ok();
    let base = value
        .as_ref()
        .and_then(|v| v.get("base_resp"))
        .and_then(|b| serde_json::from_value::<BaseRespW>(b.clone()).ok());
    let mut err = match base {
        Some(b) if b.status_code != 0 => {
            let message = if b.status_msg.is_empty() {
                format!("MiniMax answered with status code {}.", b.status_code)
            } else {
                b.status_msg
            };
            KppError::new(kind_of(b.status_code), message)
        }
        _ => {
            return crate::openai_responses::error::decode(dialect, status, headers, body);
        }
    };
    if (400..600).contains(&status) {
        err = err.with_status(status);
    }
    if let Some(v) = value {
        err = err.with_raw(v);
    }
    err.with_origin(Origin::new(dialect))
}

fn decode_response(
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<Reply, KppError> {
    if !(200..300).contains(&status) {
        return Err(decode_error(status, headers, body));
    }
    if ctx.kind != RequestKind::Image {
        return Err(not_images(ctx.kind));
    }
    let origin = ctx.origin(D);
    let Ext {
        known: w,
        extra,
        verbatim,
    } = parse_provider::<ResponseW>(body, D)?;
    // MiniMax answers HTTP 200 with the failure in `base_resp`.
    if w.base_resp.as_ref().is_some_and(|b| b.status_code != 0) {
        return Err(decode_error(status, headers, body));
    }
    let data = w.data.unwrap_or_default();
    let mut images: Vec<GeneratedImage> = data
        .image_urls
        .iter()
        .flatten()
        .map(|u| GeneratedImage::new(Source::Url(u.clone())))
        .collect();
    images.extend(data.image_base64.iter().flatten().map(|b| {
        let media = crate::media::sniff_image(b).unwrap_or("application/octet-stream");
        GeneratedImage::new(Source::base64(media, b.clone()))
    }));
    let mut resp = ImageResponse::new(String::new(), images);
    let mut extras = extras_from(&origin, extra, verbatim);
    if let Some(b) = &w.base_resp
        && let Ok(raw) = RawJson::from_serialize(b)
    {
        extras.fields.insert("base_resp".to_owned(), raw);
    }
    for (k, v) in data.extra {
        extras.fields.insert(format!("data.{k}"), v);
    }
    resp.extras = extras;
    Ok(Reply::Image(resp))
}

fn encode_response(reply: &Reply) -> Result<Vec<u8>, KppError> {
    let Reply::Image(r) = reply else {
        return Err(not_images(reply.kind()));
    };
    let mut data = Map::new();
    let urls: Vec<&str> = r
        .images
        .iter()
        .filter_map(|i| match &i.source {
            Source::Url(u) => Some(u.as_str()),
            _ => None,
        })
        .collect();
    let b64: Vec<&str> = r
        .images
        .iter()
        .filter_map(|i| match &i.source {
            Source::Base64(b) => Some(b.data.as_str()),
            _ => None,
        })
        .collect();
    if !urls.is_empty() {
        data.insert("image_urls".to_owned(), json!(urls));
    }
    if !b64.is_empty() {
        data.insert("image_base64".to_owned(), json!(b64));
    }
    let own = r.extras.is_from(D);
    if own {
        for (k, v) in r.extras.nested_fields("data") {
            if let Ok(value) = v.to_value() {
                data.insert(k.to_owned(), value);
            }
        }
    }
    let mut out = Map::new();
    for (k, v) in extra_for(&r.extras, D) {
        if let Ok(value) = v.to_value() {
            out.insert(k, value);
        }
    }
    out.insert("data".to_owned(), Value::Object(data));
    out.entry("base_resp")
        .or_insert_with(|| json!({"status_code": 0, "status_msg": "success"}));
    to_body(&out)
}

/// An error in MiniMax's `base_resp` form.
pub(crate) fn encode_error(err: &KppError) -> EncodedError {
    let code = match err.kind {
        ErrorKind::RateLimited => 1002,
        ErrorKind::Authentication => 1004,
        ErrorKind::Billing => 1008,
        ErrorKind::ContentFiltered => 1026,
        ErrorKind::InvalidRequest | ErrorKind::Unsupported { .. } => 2013,
        _ => 1000,
    };
    let body = json!({"base_resp": {"status_code": code, "status_msg": err.message}});
    EncodedError::new(
        err.status.unwrap_or(200),
        HeaderList::new().with("content-type", "application/json"),
        serde_json::to_vec(&body).unwrap_or_default(),
    )
}

struct NoStream {
    origin: Origin,
    done: bool,
}

fn no_streaming() -> KppError {
    KppError::unsupported_by(
        "streaming",
        D,
        "MiniMax's image API answers with the finished images; send the request without streaming.",
    )
}

impl StreamDecoder for NoStream {
    fn feed(&mut self, _bytes: &[u8]) -> Vec<StreamEvent> {
        StreamDecoder::finish(self)
    }

    fn finish(&mut self) -> Vec<StreamEvent> {
        if std::mem::replace(&mut self.done, true) {
            return Vec::new();
        }
        vec![StreamEvent::Error(
            no_streaming().with_origin(self.origin.clone()),
        )]
    }
}

impl StreamEncoder for NoStream {
    fn encode(&mut self, _event: &StreamEvent) -> Result<Vec<u8>, KppError> {
        Err(no_streaming())
    }

    fn finish(&mut self) -> Result<Vec<u8>, KppError> {
        Ok(Vec::new())
    }
}

impl Codec for MinimaxImages {
    const DIALECT: DialectId = D;

    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        _headers: &HeaderList,
    ) -> Result<Request, KppError> {
        if kind != RequestKind::Image {
            return Err(not_images(kind));
        }
        crate::limits::request_shape(D, body)?;
        decode_request(body)
    }

    fn encode_response(reply: &Reply, _ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        encode_response(reply)
    }

    fn stream_encoder(_ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        Box::new(NoStream {
            origin: Origin::new(D),
            done: false,
        })
    }

    fn encode_error(err: &KppError) -> EncodedError {
        encode_error(err)
    }

    fn check(req: &Request, _ctx: &EgressCtx) -> Result<(), KppError> {
        match req {
            Request::Image(r) => check_request(r),
            other => Err(not_images(other.kind())),
        }
    }

    fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
        encode_request(req, ctx)
    }

    fn decode_response(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<Reply, KppError> {
        decode_response(status, headers, body, ctx)
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        Box::new(NoStream {
            origin: ctx.origin(D),
            done: false,
        })
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        decode_error(status, headers, body)
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
        if !(200..300).contains(&status) {
            return Err(decode_error(status, headers, body));
        }
        crate::openai_models::decode_models(D, body)
    }

    fn decode_model(status: u16, headers: &HeaderList, body: &[u8]) -> Result<ModelInfo, KppError> {
        if !(200..300).contains(&status) {
            return Err(decode_error(status, headers, body));
        }
        crate::openai_models::decode_model(D, body)
    }

    fn encode_model_request(model: &str) -> EncodedRequest {
        crate::openai_models::retrieve_request(model)
    }

    fn encode_models_request(query: &ModelsQuery) -> Result<EncodedRequest, KppError> {
        crate::openai_models::list_request(D, query)
    }
}
