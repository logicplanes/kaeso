//! MiniMax's video APIs, from the root of its host: the Hailuo API
//! (<https://platform.minimax.io/docs/api-reference/video-generation-t2v>), `POST
//! /v1/video_generation`, `GET /v1/query/video_generation` and `GET /v1/files/retrieve`; and
//! the MiniMax H3 API (<https://platform.minimax.io/docs/api-reference/video-generation-v2-create>),
//! `POST /v2/video_generation`, `GET /v2/query/video_generation/{task_id}` and `DELETE
//! /v2/video_generation/{task_id}`. The model decides which one a request goes to.

#[cfg(test)]
mod tests;

use kaeso_kpp::{
    DialectId, Extras, KppError, Origin, Reply, Request, RequestKind, Source, VideoError,
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

const D: DialectId = DialectId::MinimaxVideos;
const PROVIDER: &str = "MiniMax";

/// The Hailuo API's path that starts a video.
pub const V1_PATH: &str = "/v1/video_generation";
/// The Hailuo API's path that reports a job, with `task_id` in the query.
pub const V1_QUERY_PATH: &str = "/v1/query/video_generation";
/// The path that turns a finished Hailuo job's `file_id` into a download link.
pub const FILES_PATH: &str = "/v1/files/retrieve";
/// The H3 API's path that starts a video; a job's own path adds its id.
pub const V2_PATH: &str = "/v2/video_generation";
/// The H3 API's path that reports a job, followed by its id.
pub const V2_QUERY_PATH: &str = "/v2/query/video_generation";

/// The resolutions the H3 models take.
pub const V2_RESOLUTIONS: &[&str] = &["480P", "768P", "2K"];
/// The aspect ratios the H3 models take, `adaptive` aside.
pub const V2_RATIOS: &[&str] = &["21:9", "16:9", "4:3", "1:1", "3:4", "9:16"];

const CANCELLED: &str = "cancelled";
const UPDATED_AT: &str = "updated_at";

/// Whether `model` is served by the H3 API rather than the Hailuo API.
pub fn is_h3(model: &str) -> bool {
    model.starts_with("MiniMax-H3")
}

/// MiniMax's video APIs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct MinimaxVideos;

impl crate::codec::sealed::Sealed for MinimaxVideos {}

fn decode_v1(mut f: Fields, r: &mut VideoRequest) -> Result<(), KppError> {
    r.prompt = f.str("prompt")?;
    r.seconds = f.u32("duration")?;
    r.resolution = f.str("resolution")?;
    if let Some(url) = f.str("first_frame_image")? {
        r.images.push(VideoImage::new(
            link_source(url),
            VideoImageRole::FirstFrame,
        ));
    }
    if let Some(url) = f.str("last_frame_image")? {
        r.images
            .push(VideoImage::new(link_source(url), VideoImageRole::LastFrame));
    }
    f.rest_into(&mut r.extras, "");
    Ok(())
}

fn role_text(role: VideoImageRole) -> &'static str {
    match role {
        VideoImageRole::LastFrame => "last_frame",
        VideoImageRole::Reference => "reference_image",
        _ => "first_frame",
    }
}

fn decode_v2(mut f: Fields, r: &mut VideoRequest) -> Result<(), KppError> {
    r.seconds = f.u32("duration")?;
    r.resolution = f.str("resolution")?;
    r.aspect_ratio = f.str("ratio")?;
    for item in f.array("content")?.unwrap_or_default() {
        let Value::Object(map) = item else {
            return Err(KppError::invalid_request("`content` lists objects.").with_param("content"));
        };
        let mut c = Fields::from_map(map, D, "request");
        match c.str("type")?.as_deref() {
            Some("text") if r.prompt.is_none() => r.prompt = c.str("text")?,
            Some("image_url") => {
                let url = c
                    .object("image_url")?
                    .and_then(|mut u| u.str("url").ok().flatten())
                    .ok_or_else(|| {
                        KppError::invalid_request("An `image_url` item needs a `url`.")
                            .with_param("content")
                    })?;
                let role = match c.str("role")?.as_deref() {
                    None | Some("first_frame") => VideoImageRole::FirstFrame,
                    Some("last_frame") => VideoImageRole::LastFrame,
                    Some("reference_image") => VideoImageRole::Reference,
                    Some(other) => {
                        return Err(KppError::invalid_request(format!(
                            "Kaeso does not know the image role `{other}`."
                        ))
                        .with_param("content"));
                    }
                };
                r.images.push(VideoImage::new(link_source(url), role));
            }
            _ => {
                return Err(KppError::unsupported_by(
                    "a `content` item other than text and images",
                    D,
                    "Send the reference video or audio to MiniMax directly.",
                )
                .with_param("content"));
            }
        }
    }
    f.rest_into(&mut r.extras, "");
    Ok(())
}

fn decode_request(body: &[u8]) -> Result<Request, KppError> {
    let mut f = Fields::parse(body, D, "request")?;
    let model = f.str("model")?.unwrap_or_default();
    let mut r = VideoRequest::new(model, VideoOperation::Generate);
    r.extras = Extras::new(Origin::new(D));
    if is_h3(&r.model) {
        decode_v2(f, &mut r)?;
    } else {
        decode_v1(f, &mut r)?;
    }
    Ok(Request::Video(r))
}

/// The aspect ratio among `ratios` closest to a size's, within 1%, as MiniMax's 768P is
/// 1366x768 for 16:9.
fn ratio_near(w: u32, h: u32, ratios: &[&'static str]) -> Option<&'static str> {
    let actual = f64::from(w) / f64::from(h);
    ratios.iter().copied().find(|r| {
        let Some((a, b)) = r.split_once(':') else {
            return false;
        };
        let (Ok(a), Ok(b)) = (a.parse::<f64>(), b.parse::<f64>()) else {
            return false;
        };
        ((a / b) - actual).abs() <= 0.01 * (a / b)
    })
}

fn v2_tier_and_ratio(r: &VideoRequest) -> Result<(Option<String>, Option<String>), KppError> {
    let (mut resolution, mut ratio) = (r.resolution.clone(), r.aspect_ratio.clone());
    if let Some(size) = &r.size {
        let found = size.dimensions().and_then(|(w, h)| {
            let tier = match w.min(h) {
                480 => "480P",
                768 => "768P",
                1440 | 1536 => "2K",
                _ => return None,
            };
            Some((tier, ratio_near(w, h, V2_RATIOS)?))
        });
        let Some((tier, near)) = found else {
            return Err(refused(
                D,
                PROVIDER,
                &format!("the size `{}`", size.as_wire()),
                "size",
                "use 480P, 768P or 2K at a ratio MiniMax lists.",
            ));
        };
        resolution.get_or_insert_with(|| tier.to_owned());
        ratio.get_or_insert_with(|| near.to_owned());
    }
    Ok((resolution, ratio))
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
            "create a new video instead.",
        ));
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
    if is_h3(&r.model) {
        refuse_unset(
            r,
            D,
            PROVIDER,
            &["seconds", "size", "aspect_ratio", "resolution"],
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
        let (resolution, _) = v2_tier_and_ratio(r)?;
        if resolution.is_none() {
            return Err(refused(
                D,
                PROVIDER,
                "a video without a resolution",
                "size",
                "send a size, such as 1366x768.",
            ));
        }
        if r.seconds.is_none() {
            return Err(refused(
                D,
                PROVIDER,
                "a video without a length",
                "seconds",
                "send `seconds`, 4 to 15.",
            ));
        }
        let prompt = r.prompt.as_deref().is_some_and(|p| !p.is_empty());
        if !prompt {
            return Err(refused(
                D,
                PROVIDER,
                "a video without a prompt",
                "prompt",
                "send `prompt`.",
            ));
        }
    } else {
        refuse_unset(r, D, PROVIDER, &["seconds", "resolution"])?;
        check_roles(
            r,
            D,
            PROVIDER,
            &[VideoImageRole::FirstFrame, VideoImageRole::LastFrame],
        )?;
    }
    Ok(())
}

fn encode_request(req: &Request) -> Result<EncodedRequest, KppError> {
    check(req)?;
    let r = video_request(req, D)?;
    let mut out = Map::new();
    out.insert("model".into(), r.model.clone().into());
    let path = if is_h3(&r.model) {
        let mut content = Vec::new();
        if let Some(p) = &r.prompt {
            content.push(json!({ "type": "text", "text": p }));
        }
        for image in &r.images {
            content.push(json!({
                "type": "image_url",
                "image_url": { "url": image_link(image) },
                "role": role_text(image.role),
            }));
        }
        out.insert("content".into(), Value::Array(content));
        let (resolution, ratio) = v2_tier_and_ratio(r)?;
        put(&mut out, "resolution", resolution);
        put(&mut out, "duration", r.seconds);
        put(&mut out, "ratio", ratio);
        V2_PATH
    } else {
        put(&mut out, "prompt", r.prompt.clone());
        put(&mut out, "duration", r.seconds);
        put(&mut out, "resolution", r.resolution.clone());
        let link = |role| r.images_for(role).next().and_then(image_link);
        put(
            &mut out,
            "first_frame_image",
            link(VideoImageRole::FirstFrame),
        );
        put(
            &mut out,
            "last_frame_image",
            link(VideoImageRole::LastFrame),
        );
        V1_PATH
    };
    put_extras(&mut out, &r.extras, D, "");
    let headers = HeaderList::new().with("content-type", crate::codec::JSON_MEDIA_TYPE);
    Ok(EncodedRequest::post_json(path, headers, body(&out)?))
}

fn decode_started(
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<VideoJob, KppError> {
    if let Some(err) = base_resp_failure(status, headers, body) {
        return Err(err);
    }
    let mut f = provider_object(body, D)?;
    let id = f.str("task_id")?.filter(|t| !t.is_empty()).ok_or_else(|| {
        KppError::provider("MiniMax started a video job without naming its `task_id`.")
    })?;
    let mut job = VideoJob::new(id, VideoStatus::Queued);
    job.extras = f.rest(&ctx.origin(D));
    Ok(job)
}

fn base_resp_failure(status: u16, headers: &HeaderList, body: &[u8]) -> Option<KppError> {
    let value: Value = serde_json::from_slice(body).ok()?;
    let code = value.get("base_resp")?.get("status_code")?.as_i64()?;
    (code != 0).then(|| crate::minimax_images::base_resp_error(D, status, headers, body))
}

pub(crate) fn encode_started(job: &VideoJob) -> Value {
    let mut out = Map::new();
    out.insert("task_id".into(), job.id.clone().into());
    put_extras(&mut out, &job.extras, D, "");
    Value::Object(out)
}

fn decode_v1_poll(mut f: Fields, origin: Origin) -> Result<VideoJob, KppError> {
    let mut extras = Extras::new(origin.clone());
    let status = match f.str("status")?.as_deref() {
        Some("Preparing") => {
            extras.form.insert("status".into(), "Preparing".into());
            VideoStatus::Queued
        }
        Some("Queueing") => VideoStatus::Queued,
        Some("Processing") => VideoStatus::InProgress,
        Some("Success") => VideoStatus::Completed,
        Some("Fail") => VideoStatus::Failed,
        other => {
            return Err(KppError::provider(format!(
                "MiniMax sent the unknown task status `{}`.",
                other.unwrap_or_default()
            )));
        }
    };
    let mut job = VideoJob::new(f.str("task_id")?.unwrap_or_default(), status);
    if let Some(file) = f.str("file_id")? {
        job.outputs.push(
            VideoOutput::new(VideoVariant::Video, Source::file_id(origin, file))
                .with_media_type("video/mp4"),
        );
    }
    let (w, h) = (f.u32("video_width")?, f.u32("video_height")?);
    if let (Some(w), Some(h)) = (w, h) {
        job.size = Some(kaeso_kpp::ImageSize::pixels(w, h));
    }
    if status == VideoStatus::Failed {
        job.error = Some(VideoError::new("Fail", "MiniMax could not make the video."));
    }
    f.rest_into(&mut extras, "");
    job.extras = extras;
    Ok(job)
}

fn decode_v2_poll(mut outer: Fields, origin: Origin) -> Result<VideoJob, KppError> {
    let mut f = outer
        .object("task")?
        .ok_or_else(|| KppError::provider("MiniMax reported a video job without its `task`."))?;
    let mut extras = Extras::new(origin);
    let status = match f.str("status")?.as_deref() {
        Some("queued") => VideoStatus::Queued,
        Some("running") => VideoStatus::InProgress,
        Some("succeeded") => VideoStatus::Completed,
        Some("failed") => VideoStatus::Failed,
        Some(CANCELLED) => {
            extras.form.insert(CANCELLED.into(), Value::Bool(true));
            VideoStatus::Failed
        }
        other => {
            return Err(KppError::provider(format!(
                "MiniMax sent the unknown task status `{}`.",
                other.unwrap_or_default()
            )));
        }
    };
    let mut job = VideoJob::new(f.str("id")?.unwrap_or_default(), status);
    job.model = f.str("model")?.unwrap_or_default();
    if let Some(mut e) = f.object("error")? {
        job.error = Some(VideoError::new(
            e.str("code")?.unwrap_or_default(),
            e.str("message")?.unwrap_or_default(),
        ));
        e.rest_into(&mut extras, "task.error");
    } else if extras.form_flag(D, CANCELLED) {
        job.error = Some(VideoError::new(CANCELLED, "The video job was cancelled."));
    }
    job.created_at = f.i64("created_at")?;
    if let Some(updated) = f.i64(UPDATED_AT)? {
        extras.form.insert(UPDATED_AT.into(), updated.into());
        if status.is_done() {
            job.completed_at = Some(updated);
        }
    }
    if let Some(mut content) = f.object("content")? {
        if let Some(url) = content.str("url")? {
            job.outputs.push(
                VideoOutput::new(VideoVariant::Video, Source::Url(url))
                    .with_media_type("video/mp4"),
            );
        }
        content.rest_into(&mut extras, "task.content");
    }
    job.resolution = f.str("resolution")?;
    job.seconds = f.u32("duration")?;
    job.aspect_ratio = f.str("ratio")?;
    f.rest_into(&mut extras, "task");
    outer.rest_into(&mut extras, "");
    job.extras = extras;
    Ok(job)
}

#[cfg(test)]
pub(crate) fn encode_poll(job: &VideoJob) -> Value {
    let flag = |k: &str| job.extras.form_flag(D, k);
    if !is_h3(&job.model) {
        let mut out = Map::new();
        out.insert("task_id".into(), job.id.clone().into());
        let status = match job.status {
            VideoStatus::Queued if job.extras.form_str(D, "status") == Some("Preparing") => {
                "Preparing"
            }
            VideoStatus::Queued => "Queueing",
            VideoStatus::InProgress => "Processing",
            VideoStatus::Completed => "Success",
            _ => "Fail",
        };
        out.insert("status".into(), status.into());
        if let Some(VideoOutput {
            source: Source::FileId(f),
            ..
        }) = job.output(VideoVariant::Video)
        {
            out.insert("file_id".into(), f.id.clone().into());
        }
        if let Some((w, h)) = job.size.as_ref().and_then(kaeso_kpp::ImageSize::dimensions) {
            out.insert("video_width".into(), w.into());
            out.insert("video_height".into(), h.into());
        }
        put_extras(&mut out, &job.extras, D, "");
        return Value::Object(out);
    }
    let mut task = Map::new();
    task.insert("id".into(), job.id.clone().into());
    task.insert("model".into(), job.model.clone().into());
    let status = match job.status {
        VideoStatus::Queued => "queued",
        VideoStatus::InProgress => "running",
        VideoStatus::Completed => "succeeded",
        _ if flag(CANCELLED) => CANCELLED,
        _ => "failed",
    };
    task.insert("status".into(), status.into());
    if let Some(e) = job.error.as_ref().filter(|_| !flag(CANCELLED)) {
        let mut error = Map::new();
        error.insert("code".into(), e.code.clone().into());
        error.insert("message".into(), e.message.clone().into());
        put_extras(&mut error, &job.extras, D, "task.error");
        task.insert("error".into(), Value::Object(error));
    }
    put(&mut task, "created_at", job.created_at);
    if let Some(updated) = job.extras.form_value(D, UPDATED_AT) {
        task.insert(UPDATED_AT.into(), updated.clone());
    }
    if let Some(VideoOutput {
        source: Source::Url(u),
        ..
    }) = job.output(VideoVariant::Video)
    {
        let mut content = Map::new();
        content.insert("url".into(), u.clone().into());
        put_extras(&mut content, &job.extras, D, "task.content");
        task.insert("content".into(), Value::Object(content));
    }
    put(&mut task, "resolution", job.resolution.clone());
    put(&mut task, "duration", job.seconds);
    put(&mut task, "ratio", job.aspect_ratio.clone());
    put_extras(&mut task, &job.extras, D, "task");
    let mut out = Map::new();
    out.insert("task".into(), Value::Object(task));
    put_extras(&mut out, &job.extras, D, "");
    Value::Object(out)
}

impl Codec for MinimaxVideos {
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
        crate::wire::to_body(&encode_started(video_reply(reply, D)?))
    }

    fn stream_encoder(_ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        no_stream(Origin::new(D))
    }

    fn encode_error(err: &KppError) -> EncodedError {
        crate::openai_responses::error::encode(err)
    }

    fn check(req: &Request, _ctx: &EgressCtx) -> Result<(), KppError> {
        check(req)
    }

    fn encode_request(req: &Request, _ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
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
        decode_started(status, headers, body, ctx).map(Reply::Video)
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        no_stream(ctx.origin(D))
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        crate::minimax_images::base_resp_error(D, status, headers, body)
    }

    fn encode_video_poll(
        job: &str,
        model: &str,
        _ctx: &EgressCtx,
    ) -> Result<EncodedRequest, KppError> {
        let id = crate::video_jobs::path_segment(job);
        let path = if is_h3(model) {
            format!("{V2_QUERY_PATH}/{id}")
        } else {
            format!("{V1_QUERY_PATH}?task_id={id}")
        };
        Ok(EncodedRequest::get(path, HeaderList::new()))
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
        if let Some(err) = base_resp_failure(status, headers, body) {
            return Err(err);
        }
        let f = provider_object(body, D)?;
        if f_has_task(body) {
            decode_v2_poll(f, ctx.origin(D))
        } else {
            decode_v1_poll(f, ctx.origin(D))
        }
    }

    fn encode_video_delete(
        job: &str,
        model: &str,
        _ctx: &EgressCtx,
    ) -> Result<Option<EncodedRequest>, KppError> {
        Ok(is_h3(model).then(|| {
            EncodedRequest::delete(
                format!("{V2_PATH}/{}", crate::video_jobs::path_segment(job)),
                HeaderList::new(),
            )
        }))
    }

    fn encode_video_file(file: &str, _ctx: &EgressCtx) -> Result<EncodedRequest, KppError> {
        Ok(EncodedRequest::get(
            format!(
                "{FILES_PATH}?file_id={}",
                crate::video_jobs::path_segment(file)
            ),
            HeaderList::new(),
        ))
    }

    fn decode_video_file(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
    ) -> Result<Source, KppError> {
        if !(200..300).contains(&status) {
            return Err(Self::decode_error(status, headers, body));
        }
        if let Some(err) = base_resp_failure(status, headers, body) {
            return Err(err);
        }
        let mut f = provider_object(body, D)?;
        f.object("file")?
            .and_then(|mut file| file.str("download_url").ok().flatten())
            .filter(|u| !u.is_empty())
            .map(Source::Url)
            .ok_or_else(|| KppError::provider("MiniMax named no download link for the video."))
    }

    openai_listing!(D);
}

fn f_has_task(body: &[u8]) -> bool {
    serde_json::from_slice::<Value>(body)
        .ok()
        .is_some_and(|v| v.get("task").is_some())
}
