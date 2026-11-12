//! OpenRouter's video generation API
//! (<https://openrouter.ai/docs/api/api-reference/video-generation/submit-a-video-generation-request>):
//! `POST /api/v1/videos` starts a job, `GET /api/v1/videos/{jobId}` reports it, and `GET
//! /api/v1/videos/{jobId}/content` streams what it made.

#[cfg(test)]
mod tests;

use kaeso_kpp::{
    DialectId, Extras, KppError, Origin, RawJson, Reply, Request, RequestKind, Source, VideoError,
    VideoImage, VideoImageRole, VideoJob, VideoOperation, VideoOutput, VideoRequest, VideoStatus,
    VideoVariant,
};
use serde_json::{Map, Value, json};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};
use crate::video_jobs::{
    Fields, body, check_foreign_extras, check_roles, image_link, link_source, no_stream,
    openai_listing, provider_object, put, put_extras, refuse_unset, refused, video_reply,
    video_request,
};

const D: DialectId = DialectId::OpenRouterVideos;
const PROVIDER: &str = "OpenRouter";

/// The path of video jobs, after OpenRouter's base URL (`https://openrouter.ai/api`); a job's
/// own path adds its id.
pub const JOBS_PATH: &str = "/v1/videos";

/// The aspect ratios `aspect_ratio` takes.
pub const ASPECT_RATIOS: &[&str] = &[
    "16:9", "9:16", "1:1", "4:3", "3:4", "3:2", "2:3", "21:9", "9:21",
];
/// The resolutions `resolution` takes.
pub const RESOLUTIONS: &[&str] = &["360p", "480p", "720p", "768p", "1080p", "1K", "2K", "4K"];

const TERMINAL: &str = "terminal_status";
const STATUS_ERROR: &str = "error_from_status";

/// OpenRouter's video generation API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct OpenRouterVideos;

impl crate::codec::sealed::Sealed for OpenRouterVideos {}

fn part_url(v: &Value, key: &str) -> Option<String> {
    v.get(key)?.get("url")?.as_str().map(str::to_owned)
}

fn decode_request(body: &[u8]) -> Result<Request, KppError> {
    let mut f = Fields::parse(body, D, "request")?;
    let origin = Origin::new(D);
    let mut extras = Extras::new(origin);
    let mut r = VideoRequest::new(
        f.str("model")?.unwrap_or_default(),
        VideoOperation::Generate,
    );
    r.prompt = f.str("prompt")?;
    r.aspect_ratio = f.str("aspect_ratio")?;
    r.seconds = f.u32("duration")?;
    r.resolution = f.str("resolution")?;
    r.size = f.str("size")?.map(|s| kaeso_kpp::ImageSize::from_wire(&s));
    r.seed = f.i64("seed")?;
    r.audio = f.bool("generate_audio")?;
    r.user = f.str("user")?;
    for frame in f.array("frame_images")?.unwrap_or_default() {
        let url = part_url(&frame, "image_url").ok_or_else(|| {
            KppError::invalid_request("A frame image needs an `image_url` with a `url`.")
                .with_param("frame_images")
        })?;
        let role = match frame.get("frame_type").and_then(Value::as_str) {
            Some("first_frame") => VideoImageRole::FirstFrame,
            Some("last_frame") => VideoImageRole::LastFrame,
            _ => {
                return Err(KppError::invalid_request(
                    "A frame image's `frame_type` is `first_frame` or `last_frame`.",
                )
                .with_param("frame_images"));
            }
        };
        r.images.push(VideoImage::new(link_source(url), role));
    }
    if let Some(references) = f.array("input_references")? {
        let images_only = references
            .iter()
            .all(|v| v.get("type").and_then(Value::as_str) == Some("image_url"));
        if images_only {
            for v in &references {
                let url = part_url(v, "image_url").ok_or_else(|| {
                    KppError::invalid_request("A reference needs an `image_url` with a `url`.")
                        .with_param("input_references")
                })?;
                r.images
                    .push(VideoImage::new(link_source(url), VideoImageRole::Reference));
            }
        } else if let Some(raw) = crate::video_jobs::raw(&Value::Array(references)) {
            extras.fields.insert("input_references".to_owned(), raw);
        }
    }
    f.rest_into(&mut extras, "");
    r.extras = extras;
    Ok(Request::Video(r))
}

fn check(req: &Request) -> Result<(), KppError> {
    let r = video_request(req, D)?;
    check_foreign_extras(r, D)?;
    if r.operation == VideoOperation::Remix {
        return Err(refused(
            D,
            PROVIDER,
            "remixes",
            "video_id",
            "create a new video instead; OpenRouter's `previous_job_id` edits or extends, which Kaeso can't tell apart.",
        ));
    }
    refuse_unset(
        r,
        D,
        PROVIDER,
        &[
            "seconds",
            "size",
            "aspect_ratio",
            "resolution",
            "seed",
            "audio",
            "user",
        ],
    )?;
    check_roles(
        r,
        D,
        PROVIDER,
        &[
            VideoImageRole::FirstFrame,
            VideoImageRole::LastFrame,
            VideoImageRole::Reference,
        ],
    )?;
    if r.extras.is_from(D)
        && r.extras.fields.contains_key("input_references")
        && r.images_for(VideoImageRole::Reference).next().is_some()
    {
        return Err(KppError::invalid_request("Send references in one list.")
            .with_param("input_references"));
    }
    for image in &r.images {
        if image_link(image).is_none() {
            return Err(refused(
                D,
                PROVIDER,
                "file ids",
                "input_reference",
                "send the image as a link or upload.",
            ));
        }
    }
    if let Some(a) = &r.aspect_ratio
        && !ASPECT_RATIOS.contains(&a.as_str())
    {
        return Err(refused(
            D,
            PROVIDER,
            &format!("the aspect ratio `{a}`"),
            "aspect_ratio",
            "use a ratio OpenRouter lists, such as 16:9.",
        ));
    }
    if let Some(res) = &r.resolution
        && !RESOLUTIONS.contains(&res.as_str())
    {
        return Err(refused(
            D,
            PROVIDER,
            &format!("the resolution `{res}`"),
            "resolution",
            "use a resolution OpenRouter lists, such as 720p.",
        ));
    }
    Ok(())
}

fn encode_request(req: &Request) -> Result<EncodedRequest, KppError> {
    check(req)?;
    let r = video_request(req, D)?;
    let mut out = Map::new();
    out.insert("model".into(), r.model.clone().into());
    put(&mut out, "prompt", r.prompt.clone());
    put(&mut out, "aspect_ratio", r.aspect_ratio.clone());
    put(&mut out, "duration", r.seconds);
    put(&mut out, "resolution", r.resolution.clone());
    put(
        &mut out,
        "size",
        r.size.as_ref().map(|s| s.as_wire().to_owned()),
    );
    put(&mut out, "seed", r.seed);
    put(&mut out, "generate_audio", r.audio);
    put(&mut out, "user", r.user.clone());
    let frames: Vec<Value> = r
        .images
        .iter()
        .filter(|i| i.role != VideoImageRole::Reference)
        .map(|i| {
            let frame_type = if i.role == VideoImageRole::LastFrame {
                "last_frame"
            } else {
                "first_frame"
            };
            json!({ "type": "image_url", "image_url": { "url": image_link(i) }, "frame_type": frame_type })
        })
        .collect();
    if !frames.is_empty() {
        out.insert("frame_images".into(), Value::Array(frames));
    }
    let references: Vec<Value> = r
        .images_for(VideoImageRole::Reference)
        .map(|i| json!({ "type": "image_url", "image_url": { "url": image_link(i) } }))
        .collect();
    if !references.is_empty() {
        out.insert("input_references".into(), Value::Array(references));
    }
    put_extras(&mut out, &r.extras, D, "");
    let headers = HeaderList::new().with("content-type", crate::codec::JSON_MEDIA_TYPE);
    Ok(EncodedRequest::post_json(JOBS_PATH, headers, body(&out)?))
}

fn keep(extras: &mut Extras, key: &str, value: Option<Value>) -> Option<Value> {
    let value = value?;
    if let Ok(raw) = RawJson::from_serialize(&value) {
        extras.fields.insert(key.to_owned(), raw);
    }
    Some(value)
}

fn decode_job(body: &[u8], ctx: &EgressCtx) -> Result<VideoJob, KppError> {
    let mut f = provider_object(body, D)?;
    let mut extras = Extras::new(ctx.origin(D));
    let id = f.str("id")?.unwrap_or_default();
    let status_text = f.str("status")?.unwrap_or_default();
    let status = match status_text.as_str() {
        "pending" => VideoStatus::Queued,
        "in_progress" => VideoStatus::InProgress,
        "completed" => VideoStatus::Completed,
        "failed" => VideoStatus::Failed,
        "cancelled" | "expired" => {
            extras
                .form
                .insert(TERMINAL.into(), status_text.clone().into());
            VideoStatus::Failed
        }
        other => {
            return Err(KppError::provider(format!(
                "OpenRouter sent the unknown video status `{other}`."
            )));
        }
    };
    let mut job = VideoJob::new(id, status);
    if let Some(message) = f.str("error")? {
        job.error = Some(VideoError::new(status_text.clone(), message));
    } else if extras.form_str(D, TERMINAL).is_some() {
        extras.form.insert(STATUS_ERROR.into(), Value::Bool(true));
        job.error = Some(VideoError::new(
            status_text.clone(),
            format!("The video job is {status_text} at OpenRouter."),
        ));
    }
    job.expires_at = f.i64("expires_at")?;
    let first = |v: &Option<Value>| {
        v.as_ref()
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let signed = keep(&mut extras, "urls", f.take("urls"));
    let unsigned = keep(&mut extras, "unsigned_urls", f.take("unsigned_urls"));
    if status == VideoStatus::Completed {
        let url = first(&signed)
            .or_else(|| first(&unsigned))
            .unwrap_or_else(|| {
                format!(
                    "{JOBS_PATH}/{}/content?index=0",
                    crate::video_jobs::path_segment(&job.id)
                )
            });
        job.outputs
            .push(VideoOutput::new(VideoVariant::Video, Source::Url(url)));
    }
    f.rest_into(&mut extras, "");
    job.extras = extras;
    Ok(job)
}

pub(crate) fn encode_job(job: &VideoJob) -> Value {
    let mut out = Map::new();
    out.insert("id".into(), job.id.clone().into());
    let status = match (job.status, job.extras.form_str(D, TERMINAL)) {
        (VideoStatus::Failed, Some(t)) => t.to_owned(),
        (s, _) => match s {
            VideoStatus::Queued => "pending",
            VideoStatus::InProgress => "in_progress",
            VideoStatus::Completed => "completed",
            _ => "failed",
        }
        .to_owned(),
    };
    out.insert("status".into(), status.into());
    if let Some(e) = job
        .error
        .as_ref()
        .filter(|_| !job.extras.form_flag(D, STATUS_ERROR))
    {
        out.insert("error".into(), e.message.clone().into());
    }
    put(&mut out, "expires_at", job.expires_at);
    put_extras(&mut out, &job.extras, D, "");
    Value::Object(out)
}

impl Codec for OpenRouterVideos {
    const DIALECT: DialectId = D;

    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        _headers: &HeaderList,
    ) -> Result<Request, KppError> {
        crate::decisions::refuse_kind(kind, Self::DIALECT)?;
        if kind != RequestKind::Video {
            return Err(crate::video_jobs::not_videos(D, kind));
        }
        crate::limits::request_shape(D, body)?;
        decode_request(body)
    }

    fn encode_response(reply: &Reply, _ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        crate::decisions::refuse_reply(reply, Self::DIALECT)?;
        crate::wire::to_body(&encode_job(video_reply(reply, D)?))
    }

    fn stream_encoder(_ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        no_stream(Origin::new(D))
    }

    fn encode_error(err: &KppError) -> EncodedError {
        crate::openai_responses::error::encode(err)
    }

    fn check(req: &Request, _ctx: &EgressCtx) -> Result<(), KppError> {
        crate::decisions::refuse_request(req, Self::DIALECT)?;
        check(req)
    }

    fn encode_request(req: &Request, _ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
        crate::decisions::refuse_request(req, Self::DIALECT)?;
        encode_request(req)
    }

    fn decode_response(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<Reply, KppError> {
        if !(200..300).contains(&status) {
            return Err(Self::decode_error(status, headers, body));
        }
        let job = decode_job(body, ctx)?;
        if job.id.is_empty() {
            return Err(KppError::provider(
                "OpenRouter started a video job without naming its `id`.",
            ));
        }
        Ok(Reply::Video(job))
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        no_stream(ctx.origin(D))
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        crate::openai_responses::error::decode(D, status, headers, body)
    }

    fn encode_video_poll(
        job: &str,
        _model: &str,
        _ctx: &EgressCtx,
    ) -> Result<EncodedRequest, KppError> {
        Ok(EncodedRequest::get(
            format!("{JOBS_PATH}/{}", crate::video_jobs::path_segment(job)),
            HeaderList::new(),
        ))
    }

    fn decode_video_poll(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<VideoJob, KppError> {
        if !(200..300).contains(&status) {
            return Err(Self::decode_error(status, headers, body));
        }
        decode_job(body, ctx)
    }

    openai_listing!(D);
}
