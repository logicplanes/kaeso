//! Video jobs on Google AI Studio's Interactions API
//! (<https://aistudio.google.com/api-reference/interactions-api/create-interaction>), the API
//! the Gemini Omni models make video through (<https://aistudio.google.com/docs/omni>):
//! `POST /v1beta/interactions` with `background: true` starts a job, `GET
//! /v1beta/interactions/{id}` reports it with the video inline, and `DELETE` removes it.
//! Gemini Enterprise Agent Platform serves the same API in its global location
//! (<https://docs.cloud.google.com/gemini-enterprise-agent-platform/generative-ai/docs/video/generate-videos-from-text>):
//! with [`EgressCtx::base_path`] set to `/v1beta1/projects/{project}/locations/global`, the
//! interactions live under it, and a job is never deleted there, since its reference documents
//! no delete. Only the video part of the API is mapped.

#[cfg(test)]
mod tests;

use kaeso_kpp::{
    DialectId, Extras, KppError, Origin, Reply, Request, RequestKind, Source, Usage, VideoError,
    VideoImage, VideoImageRole, VideoJob, VideoOperation, VideoOutput, VideoRequest, VideoStatus,
    VideoVariant,
};
use serde_json::{Map, Value, json};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};
use crate::video_jobs::{
    Fields, body, check_foreign_extras, check_roles, no_stream, openai_listing, provider_object,
    put, put_extras, refuse_unset, refused, unix_of_rfc3339, video_reply, video_request,
};

const D: DialectId = DialectId::GeminiInteractions;
const PROVIDER: &str = "Gemini Omni";

/// The path of interactions on Google AI Studio; an interaction's own path adds its id.
pub const INTERACTIONS_PATH: &str = "/v1beta/interactions";

/// The aspect ratios `response_format.aspect_ratio` takes for video.
pub const ASPECT_RATIOS: &[&str] = &["16:9", "9:16"];
/// The resolutions `response_format.resolution` takes for video.
pub const RESOLUTIONS: &[&str] = &["360p", "720p", "1080p", "4k"];

const INPUT_TEXT: &str = "input_string";
const NO_TASK: &str = "no_task";
const STATUS: &str = "status";

/// Gemini Omni's video jobs on the Interactions API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct GeminiInteractions;

impl crate::codec::sealed::Sealed for GeminiInteractions {}

fn image_item(map: Map<String, Value>) -> Result<VideoImage, KppError> {
    let mut f = Fields::from_map(map, D, "request");
    f.str("type")?;
    let source = match (f.str("data")?, f.str("uri")?) {
        (Some(data), None) => Source::base64(
            f.str("mime_type")?.unwrap_or_else(|| "image/png".into()),
            data,
        ),
        (None, Some(uri)) => Source::Url(uri),
        _ => {
            return Err(
                KppError::invalid_request("An image item takes `data` or `uri`.")
                    .with_param("input"),
            );
        }
    };
    let mut image = VideoImage::new(source, VideoImageRole::FirstFrame);
    image.extras = f.rest(&Origin::new(D));
    Ok(image)
}

fn decode_request(body: &[u8]) -> Result<Request, KppError> {
    let mut f = Fields::parse(body, D, "request")?;
    let mut extras = Extras::new(Origin::new(D));
    let mut r = VideoRequest::new(
        f.str("model")?.unwrap_or_default(),
        VideoOperation::Generate,
    );
    r.remix_of = f.str("previous_interaction_id")?;
    if r.remix_of.is_some() {
        r.operation = VideoOperation::Remix;
    }
    let mut images = Vec::new();
    match f.take("input") {
        Some(Value::String(s)) => {
            extras.form.insert(INPUT_TEXT.into(), Value::Bool(true));
            r.prompt = Some(s);
        }
        Some(Value::Array(items)) => {
            for item in items {
                let Value::Object(map) = item else {
                    return Err(
                        KppError::invalid_request("`input` lists objects.").with_param("input")
                    );
                };
                match map.get("type").and_then(Value::as_str) {
                    Some("text") if r.prompt.is_none() => {
                        r.prompt = map.get("text").and_then(Value::as_str).map(str::to_owned);
                    }
                    Some("image") => images.push(image_item(map)?),
                    _ => {
                        return Err(KppError::unsupported_by(
                            "`input` other than a prompt and images",
                            D,
                            "Kaeso maps the Interactions API only for video jobs.",
                        )
                        .with_param("input"));
                    }
                }
            }
        }
        None => {}
        Some(_) => {
            return Err(KppError::invalid_request("`input` is text or a list.").with_param("input"));
        }
    }
    let mut task = None;
    if let Some(mut config) = f.object("generation_config")? {
        if let Some(mut video) = config.object("video_config")? {
            task = video.str("task")?;
            video.rest_into(&mut extras, "generation_config.video_config");
        }
        config.rest_into(&mut extras, "generation_config");
    }
    if task.is_none() && !images.is_empty() {
        extras.form.insert(NO_TASK.into(), Value::Bool(true));
    }
    let references = task.as_deref() == Some("reference_to_video");
    if let Some(t) = task
        .as_deref()
        .filter(|t| !matches!(*t, "image_to_video" | "reference_to_video"))
    {
        extras.form.insert("task".into(), t.into());
    }
    for (i, mut image) in images.into_iter().enumerate() {
        image.role = match (references, i) {
            (true, _) => VideoImageRole::Reference,
            (false, 0) => VideoImageRole::FirstFrame,
            _ => VideoImageRole::LastFrame,
        };
        r.images.push(image);
    }
    if let Some(mut format) = f.object("response_format")? {
        if format.str("type")?.is_some_and(|t| t != "video") {
            return Err(KppError::unsupported_by(
                "a response format other than video",
                D,
                "Kaeso maps the Interactions API only for video jobs.",
            )
            .with_param("response_format"));
        }
        r.aspect_ratio = format.str("aspect_ratio")?;
        r.resolution = format.str("resolution")?;
        if let Some(d) = format.str("duration")? {
            r.seconds = Some(
                d.strip_suffix('s')
                    .and_then(crate::video_jobs::digits)
                    .ok_or_else(|| {
                        KppError::invalid_request("`duration` is whole seconds, such as \"10s\".")
                            .with_param("response_format")
                    })?,
            );
        }
        format.rest_into(&mut extras, "response_format");
    }
    f.rest_into(&mut extras, "");
    r.extras = extras;
    Ok(Request::Video(r))
}

fn tier_and_ratio(r: &VideoRequest) -> Result<(Option<String>, Option<String>), KppError> {
    let (mut resolution, mut ratio) = (r.resolution.clone(), r.aspect_ratio.clone());
    if let Some(size) = &r.size {
        let tier = crate::video_jobs::tier_of(size)
            .map(|t| if t == "2160p" { "4k".to_owned() } else { t });
        let aspect = crate::video_jobs::aspect_of(size);
        match (tier, aspect) {
            (Some(t), Some(a))
                if RESOLUTIONS.contains(&t.as_str()) && ASPECT_RATIOS.contains(&a.as_str()) =>
            {
                resolution.get_or_insert(t);
                ratio.get_or_insert(a);
            }
            _ => {
                return Err(refused(
                    D,
                    PROVIDER,
                    &format!("the size `{}`", size.as_wire()),
                    "size",
                    "use 640x360, 1280x720, 1920x1080 or 3840x2160, or their portrait forms.",
                ));
            }
        }
    }
    Ok((resolution, ratio))
}

fn check(req: &Request) -> Result<(), KppError> {
    let r = video_request(req, D)?;
    check_foreign_extras(r, D)?;
    if r.audio == Some(false) {
        return Err(refused(
            D,
            PROVIDER,
            "video without sound",
            "audio",
            "Gemini Omni always makes sound.",
        ));
    }
    let mut without_audio = r.clone();
    without_audio.audio = None;
    refuse_unset(
        &without_audio,
        D,
        PROVIDER,
        &["seconds", "size", "aspect_ratio", "resolution"],
    )?;
    let frames = r.images.iter().any(|i| i.role != VideoImageRole::Reference);
    let references = r.images.iter().any(|i| i.role == VideoImageRole::Reference);
    if frames && references {
        return Err(refused(
            D,
            PROVIDER,
            "frames and references together",
            "input_reference",
            "send either frames or reference images.",
        ));
    }
    // The first Omni preview starts from a prompt or reference images only, not from frames
    // (https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/video/generate-videos-from-text).
    if frames && r.model.starts_with("gemini-omni-flash-preview") {
        return Err(refused(
            D,
            PROVIDER,
            "a first or last frame on `gemini-omni-flash-preview`",
            "input_reference",
            "use `gemini-omni-1.2-flash`, or send the image as a reference.",
        ));
    }
    if r.operation == VideoOperation::Remix {
        check_roles(r, D, PROVIDER, &[VideoImageRole::Reference])?;
        if r.remix_of.is_none() {
            return Err(refused(
                D,
                PROVIDER,
                "a remix without its earlier video",
                "video_id",
                "remix a video Gemini Omni made.",
            ));
        }
    }
    for image in &r.images {
        if !matches!(image.source, Source::Base64(_) | Source::Url(_)) {
            return Err(refused(
                D,
                PROVIDER,
                "file ids",
                "input_reference",
                "send the image as a link or upload.",
            ));
        }
    }
    if let Some(s) = r.seconds
        && !(3..=15).contains(&s)
    {
        return Err(refused(
            D,
            PROVIDER,
            &format!("{s} seconds"),
            "seconds",
            "choose 3 to 15 seconds.",
        ));
    }
    if frames
        && r.images_for(VideoImageRole::LastFrame).next().is_some()
        && r.seconds.is_some_and(|s| s > 10)
    {
        return Err(refused(
            D,
            PROVIDER,
            "a first and last frame over 10 seconds",
            "seconds",
            "choose 3 to 10 seconds.",
        ));
    }
    let (resolution, ratio) = tier_and_ratio(r)?;
    if let Some(res) = &resolution
        && !RESOLUTIONS.contains(&res.as_str())
    {
        return Err(refused(
            D,
            PROVIDER,
            &format!("the resolution `{res}`"),
            "resolution",
            "use 360p, 720p, 1080p or 4k.",
        ));
    }
    if let Some(a) = &ratio
        && !ASPECT_RATIOS.contains(&a.as_str())
    {
        return Err(refused(
            D,
            PROVIDER,
            &format!("the aspect ratio `{a}`"),
            "aspect_ratio",
            "use 16:9 or 9:16.",
        ));
    }
    Ok(())
}

fn image_value(image: &VideoImage) -> Value {
    let mut out = Map::new();
    out.insert("type".into(), "image".into());
    match &image.source {
        Source::Base64(b) => {
            out.insert("data".into(), b.data.clone().into());
            out.insert("mime_type".into(), b.media_type.clone().into());
        }
        Source::Url(u) => {
            out.insert("uri".into(), u.clone().into());
        }
        _ => {}
    }
    put_extras(&mut out, &image.extras, D, "");
    Value::Object(out)
}

fn interactions(ctx: &EgressCtx) -> String {
    match &ctx.base_path {
        Some(base) => format!("{base}/interactions"),
        None => INTERACTIONS_PATH.to_owned(),
    }
}

fn encode_request(req: &Request, ctx: &EgressCtx) -> Result<EncodedRequest, KppError> {
    check(req)?;
    let r = video_request(req, D)?;
    let mut out = Map::new();
    out.insert("model".into(), r.model.clone().into());
    let only_text = r.images.is_empty();
    if only_text && (r.extras.form_flag(D, INPUT_TEXT) || !r.extras.is_from(D)) {
        put(&mut out, "input", r.prompt.clone());
    } else {
        let mut items: Vec<Value> = r.images.iter().map(image_value).collect();
        if let Some(p) = &r.prompt {
            items.push(json!({ "type": "text", "text": p }));
        }
        out.insert("input".into(), Value::Array(items));
    }
    put(&mut out, "previous_interaction_id", r.remix_of.clone());
    let task = match r.extras.form_str(D, "task") {
        Some(t) => Some(t.to_owned()),
        None if r.images.is_empty() || r.extras.form_flag(D, NO_TASK) => None,
        None if r.images.iter().any(|i| i.role == VideoImageRole::Reference) => {
            Some("reference_to_video".to_owned())
        }
        None => Some("image_to_video".to_owned()),
    };
    let mut video_config = Map::new();
    put(&mut video_config, "task", task);
    put_extras(
        &mut video_config,
        &r.extras,
        D,
        "generation_config.video_config",
    );
    let mut config = Map::new();
    if !video_config.is_empty() {
        config.insert("video_config".into(), Value::Object(video_config));
    }
    put_extras(&mut config, &r.extras, D, "generation_config");
    if !config.is_empty() {
        out.insert("generation_config".into(), Value::Object(config));
    }
    let mut format = Map::new();
    format.insert("type".into(), "video".into());
    let (resolution, ratio) = tier_and_ratio(r)?;
    put(&mut format, "aspect_ratio", ratio);
    put(&mut format, "resolution", resolution);
    put(&mut format, "duration", r.seconds.map(|s| format!("{s}s")));
    put_extras(&mut format, &r.extras, D, "response_format");
    out.insert("response_format".into(), Value::Object(format));
    if !r.extras.is_from(D) {
        out.insert("background".into(), Value::Bool(true));
    }
    put_extras(&mut out, &r.extras, D, "");
    let headers = HeaderList::new().with("content-type", crate::codec::JSON_MEDIA_TYPE);
    Ok(EncodedRequest::post_json(
        interactions(ctx),
        headers,
        body(&out)?,
    ))
}

fn number(v: Option<&Value>) -> u64 {
    v.and_then(Value::as_u64).unwrap_or(0)
}

fn usage_of(v: &Value) -> Usage {
    let input = number(v.get("total_input_tokens"));
    let output = number(v.get("total_output_tokens"));
    let thoughts = number(v.get("total_thought_tokens"));
    let mut usage = Usage::new(input, output.saturating_add(thoughts));
    if thoughts > 0 {
        usage.reasoning_tokens = Some(thoughts);
    }
    usage
}

fn outputs_of(steps: &Value) -> Vec<VideoOutput> {
    let mut out = Vec::new();
    for step in steps.as_array().into_iter().flatten() {
        if step.get("type").and_then(Value::as_str) != Some("model_output") {
            continue;
        }
        for item in step
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if item.get("type").and_then(Value::as_str) != Some("video") {
                continue;
            }
            let media_type = item
                .get("mime_type")
                .and_then(Value::as_str)
                .unwrap_or("video/mp4");
            let source = match (
                item.get("data").and_then(Value::as_str),
                item.get("uri").and_then(Value::as_str),
            ) {
                (Some(data), _) => Source::base64(media_type, data),
                (None, Some(uri)) => Source::Url(uri.to_owned()),
                _ => continue,
            };
            out.push(VideoOutput::new(VideoVariant::Video, source).with_media_type(media_type));
        }
    }
    out
}

fn decode_interaction(body: &[u8], ctx: &EgressCtx) -> Result<VideoJob, KppError> {
    let mut f = provider_object(body, D)?;
    let mut extras = Extras::new(ctx.origin(D));
    let id = f.str("id")?.unwrap_or_default();
    let status_text = f.str(STATUS)?.unwrap_or_default();
    let mut job = VideoJob::new(id, VideoStatus::InProgress);
    job.model = f.str("model")?.unwrap_or_default();
    job.created_at = f
        .peek("created")
        .and_then(Value::as_str)
        .and_then(unix_of_rfc3339);
    let updated = f
        .peek("updated")
        .and_then(Value::as_str)
        .and_then(unix_of_rfc3339);
    if let Some(steps) = f.peek("steps") {
        job.outputs = outputs_of(steps);
    }
    if let Some(usage) = f.peek("usage") {
        job.usage = Some(usage_of(usage));
    }
    let error = f
        .peek("errors")
        .and_then(Value::as_array)
        .and_then(|e| e.first())
        .or_else(|| f.peek("error"))
        .cloned();
    job.status = match status_text.as_str() {
        "in_progress" => VideoStatus::InProgress,
        "completed" if !job.outputs.is_empty() => VideoStatus::Completed,
        "completed"
        | "failed"
        | "cancelled"
        | "incomplete"
        | "requires_action"
        | "requires_continuation" => {
            let message = error
                .as_ref()
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .map_or_else(
                    || format!("The interaction ended as `{status_text}` without a video."),
                    str::to_owned,
                );
            let code = error.as_ref().and_then(|e| e.get("code")).map_or_else(
                || status_text.clone(),
                |c| c.as_str().map_or_else(|| c.to_string(), str::to_owned),
            );
            job.error = Some(VideoError::new(code, message));
            VideoStatus::Failed
        }
        other => {
            return Err(KppError::provider(format!(
                "Google sent the unknown interaction status `{other}`."
            )));
        }
    };
    if job.status.is_done() {
        job.completed_at = updated;
    }
    extras.form.insert(STATUS.into(), status_text.into());
    f.rest_into(&mut extras, "");
    job.extras = extras;
    Ok(job)
}

pub(crate) fn encode_interaction(job: &VideoJob) -> Value {
    let mut out = Map::new();
    if !job.id.is_empty() {
        out.insert("id".into(), job.id.clone().into());
    }
    let status = job.extras.form_str(D, STATUS).map_or_else(
        || match job.status {
            VideoStatus::Completed => "completed".to_owned(),
            VideoStatus::Failed => "failed".to_owned(),
            _ => "in_progress".to_owned(),
        },
        str::to_owned,
    );
    out.insert(STATUS.into(), status.into());
    if !job.model.is_empty() {
        out.insert("model".into(), job.model.clone().into());
    }
    put_extras(&mut out, &job.extras, D, "");
    Value::Object(out)
}

impl Codec for GeminiInteractions {
    const DIALECT: DialectId = D;

    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        _headers: &HeaderList,
    ) -> Result<Request, KppError> {
        if kind != RequestKind::Video {
            return Err(crate::video_jobs::not_videos(D, kind));
        }
        crate::limits::request_shape(D, body)?;
        decode_request(body)
    }

    fn encode_response(reply: &Reply, _ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        crate::wire::to_body(&encode_interaction(video_reply(reply, D)?))
    }

    fn stream_encoder(_ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        no_stream(Origin::new(D))
    }

    fn encode_error(err: &KppError) -> EncodedError {
        crate::gemini::error::encode_error(err, D)
    }

    fn check(req: &Request, _ctx: &EgressCtx) -> Result<(), KppError> {
        check(req)
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
        if !(200..300).contains(&status) {
            return Err(Self::decode_error(status, headers, body));
        }
        let job = decode_interaction(body, ctx)?;
        if job.id.is_empty() {
            return Err(KppError::provider(
                "Google started an interaction without naming its `id`.",
            ));
        }
        Ok(Reply::Video(job))
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        no_stream(ctx.origin(D))
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        crate::gemini::error::decode_error_value(status, headers, body, D)
    }

    fn encode_video_poll(
        job: &str,
        _model: &str,
        ctx: &EgressCtx,
    ) -> Result<EncodedRequest, KppError> {
        Ok(EncodedRequest::get(
            format!(
                "{}/{}",
                interactions(ctx),
                crate::video_jobs::path_segment(job)
            ),
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
        decode_interaction(body, ctx)
    }

    fn encode_video_delete(
        job: &str,
        _model: &str,
        ctx: &EgressCtx,
    ) -> Result<Option<EncodedRequest>, KppError> {
        if ctx.base_path.is_some() {
            return Ok(None);
        }
        Ok(Some(EncodedRequest::delete(
            format!(
                "{INTERACTIONS_PATH}/{}",
                crate::video_jobs::path_segment(job)
            ),
            HeaderList::new(),
        )))
    }

    openai_listing!(D);
}
