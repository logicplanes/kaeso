//! QwenCloud's own DashScope API, for what its OpenAI-compatible mode doesn't serve.
//!
//! - Video synthesis
//!   (<https://docs.qwencloud.com/api-reference/video-generation/wan-text-to-video/create-task>):
//!   `POST /api/v1/services/aigc/video-generation/video-synthesis` (and
//!   `/api/v1/services/aigc/image2video/video-synthesis` for first and last frames on Wan 2.2
//!   and 2.1) starts a task with `X-DashScope-Async: enable`, and `GET /api/v1/tasks/{task_id}`
//!   reports it. The body's shape depends on the Wan model family, so the model picks it.
//! - Images, speech and transcription: [`images`], [`speech`] and [`transcription`], answered
//!   in the same request.

pub mod images;
pub mod speech;
pub mod transcription;
mod voices;

#[cfg(test)]
mod media_tests;
#[cfg(test)]
mod tests;

use kaeso_kpp::{
    DialectId, ErrorKind, Extras, ImageSize, KppError, Origin, Reply, Request, RequestKind, Source,
    VideoError, VideoImage, VideoImageRole, VideoJob, VideoOperation, VideoOutput, VideoRequest,
    VideoStatus, VideoVariant,
};
use serde_json::{Map, Value};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};
use crate::video_jobs::{
    Fields, body, check_foreign_extras, check_roles, image_link, link_source, no_stream,
    openai_listing, put, put_extras, refuse_unset, refused, video_reply, video_request,
};

const D: DialectId = DialectId::DashScope;
const PROVIDER: &str = "QwenCloud";

/// The path that starts a video task, from the root of QwenCloud's host.
pub const SYNTHESIS_PATH: &str = "/api/v1/services/aigc/video-generation/video-synthesis";
/// The path that starts a video between a first and a last frame on Wan 2.2 and 2.1.
pub const KEYFRAMES_PATH: &str = "/api/v1/services/aigc/image2video/video-synthesis";
/// The path of a task, followed by its id.
pub const TASKS_PATH: &str = "/api/v1/tasks";

// The pixel sizes Wan 2.7's text to video lists for each tier and ratio; its 720P tier is not
// the short side at 1:1 or 4:3, so sizes are looked up, never computed.
const WAN27_SIZES: &[(&str, &str, u32, u32)] = &[
    ("720P", "16:9", 1280, 720),
    ("720P", "9:16", 720, 1280),
    ("720P", "1:1", 960, 960),
    ("720P", "4:3", 1104, 832),
    ("720P", "3:4", 832, 1104),
    ("1080P", "16:9", 1920, 1080),
    ("1080P", "9:16", 1080, 1920),
    ("1080P", "1:1", 1440, 1440),
    ("1080P", "4:3", 1648, 1248),
    ("1080P", "3:4", 1248, 1648),
];

const STATUS: &str = "status";
const NO_CODE: &str = "no_code";
const NO_MESSAGE: &str = "no_message";

/// How a Wan model takes a video request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Family {
    /// Wan 2.6 and older text to video: a pixel `size` as `width*height`.
    TextToVideo,
    /// Wan 2.6 and older image to video: the first frame in `img_url` and a resolution tier.
    FirstFrame,
    /// Wan 2.2 and 2.1 first and last frames, on their own path.
    Keyframes,
    /// Wan 2.7 text to video: a resolution tier and a ratio.
    Wan27Text,
    /// Wan 2.7 image to video: frames in `media` and a resolution tier.
    Wan27Image,
    /// Wan 3.0: frames or reference images in `media`, a tier, a ratio and sound.
    Wan30,
}

/// The family of a Wan model, or `None` for the ones that edit or reference video, which Kaeso
/// doesn't send.
pub fn family(model: &str) -> Option<Family> {
    let m = model.to_ascii_lowercase();
    if m.starts_with("wan3.0-video") {
        Some(Family::Wan30)
    } else if m.starts_with("wan2.7-t2v") {
        Some(Family::Wan27Text)
    } else if m.starts_with("wan2.7-i2v") {
        Some(Family::Wan27Image)
    } else if m.contains("-kf2v-") {
        Some(Family::Keyframes)
    } else if m.contains("-t2v") {
        Some(Family::TextToVideo)
    } else if m.contains("-i2v") {
        Some(Family::FirstFrame)
    } else {
        None
    }
}

/// DashScope's video synthesis on QwenCloud.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct DashScope;

impl crate::codec::sealed::Sealed for DashScope {}

fn role_of(media_type: &str) -> Option<VideoImageRole> {
    match media_type {
        "first_frame" => Some(VideoImageRole::FirstFrame),
        "last_frame" => Some(VideoImageRole::LastFrame),
        "reference_image" => Some(VideoImageRole::Reference),
        _ => None,
    }
}

fn media_type_of(role: VideoImageRole) -> &'static str {
    match role {
        VideoImageRole::FirstFrame => "first_frame",
        VideoImageRole::LastFrame => "last_frame",
        _ => "reference_image",
    }
}

fn pixels(size: &str) -> Option<ImageSize> {
    let (w, h) = size.split_once('*')?;
    Some(ImageSize::pixels(
        w.trim().parse().ok()?,
        h.trim().parse().ok()?,
    ))
}

fn non_negative(f: &Fields, key: &str) -> bool {
    f.peek(key).and_then(Value::as_u64).is_some()
}

fn decode_request(body: &[u8]) -> Result<Request, KppError> {
    let mut f = Fields::parse(body, D, "request")?;
    let mut extras = Extras::new(Origin::new(D));
    let mut r = VideoRequest::new(
        f.str("model")?.unwrap_or_default(),
        VideoOperation::Generate,
    );
    if let Some(mut input) = f.object("input")? {
        r.prompt = input.str("prompt")?;
        r.negative_prompt = input.str("negative_prompt")?;
        for (key, role) in [
            ("img_url", VideoImageRole::FirstFrame),
            ("first_frame_url", VideoImageRole::FirstFrame),
            ("last_frame_url", VideoImageRole::LastFrame),
        ] {
            if let Some(link) = input.str(key)? {
                r.images.push(VideoImage::new(link_source(link), role));
            }
        }
        for item in input.array("media")?.unwrap_or_default() {
            let Value::Object(map) = item else {
                return Err(
                    KppError::invalid_request("`input.media` lists objects.").with_param("media")
                );
            };
            let mut m = Fields::from_map(map, D, "request");
            let kind = m.str("type")?.unwrap_or_default();
            let Some(role) = role_of(&kind) else {
                return Err(KppError::unsupported_by(
                    format!("`{kind}` media"),
                    D,
                    "Kaeso sends first frames, last frames and reference images only.",
                )
                .with_param("media"));
            };
            let link = m.str("url")?.unwrap_or_default();
            let mut image = VideoImage::new(link_source(link), role);
            image.extras = m.rest(&Origin::new(D));
            r.images.push(image);
        }
        input.rest_into(&mut extras, "input");
    }
    if let Some(mut p) = f.object("parameters")? {
        r.size = p.str("size")?.and_then(|s| pixels(&s));
        r.resolution = p.str("resolution")?;
        r.aspect_ratio = p.str("ratio")?;
        if non_negative(&p, "duration") {
            r.seconds = p.u32("duration")?;
        }
        r.seed = p.i64("seed")?;
        r.audio = p.bool("audio")?;
        p.rest_into(&mut extras, "parameters");
    }
    f.rest_into(&mut extras, "");
    r.extras = extras;
    Ok(Request::Video(r))
}

fn tier(r: &VideoRequest) -> Option<String> {
    r.resolution.as_ref().map(|t| t.to_ascii_uppercase())
}

fn wan27_size(r: &VideoRequest) -> Result<(Option<String>, Option<String>), KppError> {
    let Some(size) = &r.size else {
        return Ok((tier(r), r.aspect_ratio.clone()));
    };
    let Some((w, h)) = size.dimensions() else {
        return Err(refused(
            D,
            PROVIDER,
            &format!("the size `{}`", size.as_wire()),
            "size",
            "use a size QwenCloud lists for Wan 2.7, such as 1280x720.",
        ));
    };
    match WAN27_SIZES
        .iter()
        .find(|(_, _, sw, sh)| (*sw, *sh) == (w, h))
    {
        Some((t, ratio, _, _)) => Ok((Some((*t).to_owned()), Some((*ratio).to_owned()))),
        None => Err(refused(
            D,
            PROVIDER,
            &format!("the size `{}`", size.as_wire()),
            "size",
            "use a size QwenCloud lists for Wan 2.7, such as 1280x720.",
        )),
    }
}

fn old_size(r: &VideoRequest) -> Result<Option<String>, KppError> {
    let size = match (&r.size, &r.resolution, &r.aspect_ratio) {
        (Some(s), _, _) => Some(s.clone()),
        (None, Some(t), Some(a)) => crate::video_jobs::size_of(t, a),
        (None, Some(_), None) | (None, None, Some(_)) => {
            let param = if r.resolution.is_some() {
                "resolution"
            } else {
                "aspect_ratio"
            };
            return Err(refused(
                D,
                PROVIDER,
                &format!("`{param}` alone"),
                param,
                "send `size`, or both a resolution and an aspect ratio.",
            ));
        }
        (None, None, None) => None,
    };
    match size.as_ref().map(|s| (s, s.dimensions())) {
        None => Ok(None),
        Some((_, Some((w, h)))) => Ok(Some(format!("{w}*{h}"))),
        Some((s, None)) => Err(refused(
            D,
            PROVIDER,
            &format!("the size `{}`", s.as_wire()),
            "size",
            "send a size in pixels, such as 1280x720.",
        )),
    }
}

fn check(req: &Request) -> Result<Family, KppError> {
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
    let Some(fam) = family(&r.model) else {
        return Err(KppError::unsupported_by(
            format!("the model `{}`", r.model),
            D,
            "Kaeso sends Wan text to video, image to video and Wan 3.0 jobs only; models that \
             edit or reference video aren't wired.",
        )
        .with_param("model"));
    };
    let frames = [VideoImageRole::FirstFrame, VideoImageRole::LastFrame];
    let (takes, roles): (&[&str], &[VideoImageRole]) = match fam {
        Family::TextToVideo => (
            &[
                "seconds",
                "size",
                "resolution",
                "aspect_ratio",
                "seed",
                "negative_prompt",
            ],
            &[],
        ),
        Family::FirstFrame if r.model.starts_with("wan2.6-i2v-flash") => (
            &["seconds", "resolution", "seed", "negative_prompt", "audio"],
            &[VideoImageRole::FirstFrame],
        ),
        Family::FirstFrame => (
            &["seconds", "resolution", "seed", "negative_prompt"],
            &[VideoImageRole::FirstFrame],
        ),
        Family::Keyframes => (&["resolution", "seed", "negative_prompt"], &frames),
        Family::Wan27Text => (
            &[
                "seconds",
                "size",
                "resolution",
                "aspect_ratio",
                "seed",
                "negative_prompt",
            ],
            &[],
        ),
        Family::Wan27Image => (
            &["seconds", "resolution", "seed", "negative_prompt"],
            &frames,
        ),
        Family::Wan30 => (
            &["seconds", "resolution", "aspect_ratio", "seed", "audio"],
            &[
                VideoImageRole::FirstFrame,
                VideoImageRole::LastFrame,
                VideoImageRole::Reference,
            ],
        ),
    };
    refuse_unset(r, D, PROVIDER, takes)?;
    check_roles(r, D, PROVIDER, roles)?;
    let has = |role| r.images_for(role).next().is_some();
    if matches!(
        fam,
        Family::FirstFrame | Family::Keyframes | Family::Wan27Image
    ) && !has(VideoImageRole::FirstFrame)
    {
        return Err(refused(
            D,
            PROVIDER,
            "this model without a first frame",
            "input_reference",
            "send the first frame as `input_reference`.",
        ));
    }
    if fam == Family::Keyframes && !has(VideoImageRole::LastFrame) {
        return Err(refused(
            D,
            PROVIDER,
            "this model without a last frame",
            "input_reference",
            "send a first and a last frame, or choose an image to video model.",
        ));
    }
    if fam == Family::Wan30
        && has(VideoImageRole::Reference)
        && r.images.iter().any(|i| i.role != VideoImageRole::Reference)
    {
        return Err(refused(
            D,
            PROVIDER,
            "frames together with reference images",
            "input_reference",
            "send either frames or reference images.",
        ));
    }
    for image in &r.images {
        let link = image_link(image);
        let web = matches!(image.source, Source::Url(_));
        if link.is_none() || (fam == Family::Keyframes && !web) {
            return Err(refused(
                D,
                PROVIDER,
                if fam == Family::Keyframes {
                    "frames that aren't web links on this model"
                } else {
                    "file ids"
                },
                "input_reference",
                "send the image as a public link.",
            ));
        }
    }
    match fam {
        Family::TextToVideo => old_size(r).map(|_| fam),
        Family::Wan27Text => wan27_size(r).map(|_| fam),
        _ => Ok(fam),
    }
}

fn encode_request(req: &Request) -> Result<EncodedRequest, KppError> {
    let fam = check(req)?;
    let r = video_request(req, D)?;
    let mut input = Map::new();
    put(&mut input, "prompt", r.prompt.clone());
    put(&mut input, "negative_prompt", r.negative_prompt.clone());
    let link = |role| r.images_for(role).next().and_then(image_link);
    let mut params = Map::new();
    match fam {
        Family::TextToVideo => {
            put(&mut params, "size", old_size(r)?);
        }
        Family::FirstFrame => {
            put(&mut input, "img_url", link(VideoImageRole::FirstFrame));
            put(&mut params, "resolution", tier(r));
        }
        Family::Keyframes => {
            put(
                &mut input,
                "first_frame_url",
                link(VideoImageRole::FirstFrame),
            );
            put(
                &mut input,
                "last_frame_url",
                link(VideoImageRole::LastFrame),
            );
            put(&mut params, "resolution", tier(r));
        }
        Family::Wan27Text => {
            let (t, ratio) = wan27_size(r)?;
            put(&mut params, "resolution", t);
            put(&mut params, "ratio", ratio);
        }
        Family::Wan27Image | Family::Wan30 => {
            let media: Vec<Value> = r
                .images
                .iter()
                .map(|i| {
                    let mut item = Map::new();
                    item.insert("type".into(), media_type_of(i.role).into());
                    put(&mut item, "url", image_link(i));
                    put_extras(&mut item, &i.extras, D, "");
                    Value::Object(item)
                })
                .collect();
            if !media.is_empty() {
                input.insert("media".into(), Value::Array(media));
            }
            put(&mut params, "resolution", tier(r));
            if fam == Family::Wan30 {
                put(&mut params, "ratio", r.aspect_ratio.clone());
            }
        }
    }
    put(&mut params, "duration", r.seconds);
    put(&mut params, "seed", r.seed);
    put(&mut params, "audio", r.audio);
    put_extras(&mut input, &r.extras, D, "input");
    put_extras(&mut params, &r.extras, D, "parameters");
    let mut out = Map::new();
    out.insert("model".into(), r.model.clone().into());
    out.insert("input".into(), Value::Object(input));
    if !params.is_empty() {
        out.insert("parameters".into(), Value::Object(params));
    }
    put_extras(&mut out, &r.extras, D, "");
    let path = if fam == Family::Keyframes {
        KEYFRAMES_PATH
    } else {
        SYNTHESIS_PATH
    };
    // Video synthesis runs only as an asynchronous task (the create-task references above).
    let headers = HeaderList::new()
        .with("content-type", crate::codec::JSON_MEDIA_TYPE)
        .with("x-dashscope-async", "enable");
    Ok(EncodedRequest::post_json(path, headers, body(&out)?))
}

fn status_of(s: &str) -> Result<VideoStatus, KppError> {
    match s {
        "PENDING" => Ok(VideoStatus::Queued),
        "RUNNING" => Ok(VideoStatus::InProgress),
        "SUCCEEDED" => Ok(VideoStatus::Completed),
        "FAILED" | "CANCELED" | "UNKNOWN" => Ok(VideoStatus::Failed),
        other => Err(KppError::provider(format!(
            "QwenCloud sent the unknown task status `{other}`."
        ))),
    }
}

/// Unix seconds of a DashScope time such as `2025-09-29 14:18:52.331`, which the task
/// references give in UTC+8.
pub(crate) fn unix_of_beijing(s: &str) -> Option<i64> {
    let (date, clock) = s.trim().split_once(' ')?;
    crate::video_jobs::unix_of_rfc3339(&format!("{date}T{clock}+08:00"))
}

fn decode_job(body: &[u8], ctx: &EgressCtx) -> Result<VideoJob, KppError> {
    let mut f = crate::video_jobs::provider_object(body, D)?;
    let mut extras = Extras::new(ctx.origin(D));
    let mut output = f.object("output")?.ok_or_else(|| {
        KppError::provider("QwenCloud answered a video task without its `output`.")
    })?;
    let raw_status = output.str("task_status")?.unwrap_or_default();
    let status = status_of(&raw_status)?;
    let mut job = VideoJob::new(output.str("task_id")?.unwrap_or_default(), status);
    if matches!(raw_status.as_str(), "CANCELED" | "UNKNOWN") {
        extras.form.insert(STATUS.into(), raw_status.clone().into());
    }
    let time = |key| {
        output
            .peek(key)
            .and_then(Value::as_str)
            .and_then(unix_of_beijing)
    };
    job.created_at = time("submit_time");
    job.completed_at = time("end_time");
    // The video's link is valid for 24 hours (the task references' `video_url`).
    job.expires_at = job
        .completed_at
        .filter(|_| status == VideoStatus::Completed)
        .map(|t| t + 86_400);
    job.prompt = output
        .peek("orig_prompt")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if let Some(url) = output.str("video_url")? {
        job.outputs.push(
            VideoOutput::new(VideoVariant::Video, Source::Url(url)).with_media_type("video/mp4"),
        );
    }
    if status == VideoStatus::Failed {
        let (code, message) = (output.str("code")?, output.str("message")?);
        if code.is_none() {
            extras.form.insert(NO_CODE.into(), Value::Bool(true));
        }
        if message.is_none() {
            extras.form.insert(NO_MESSAGE.into(), Value::Bool(true));
        }
        let text = match raw_status.as_str() {
            "CANCELED" => "The video task was canceled.",
            "UNKNOWN" => "QwenCloud no longer knows the task; tasks expire after 24 hours.",
            _ => "QwenCloud could not make the video.",
        };
        job.error = Some(VideoError::new(
            code.unwrap_or_else(|| raw_status.clone()),
            message.unwrap_or_else(|| text.to_owned()),
        ));
    }
    output.rest_into(&mut extras, "output");
    if let Some(Value::Object(usage)) = f.peek("usage") {
        job.seconds = usage
            .get("output_video_duration")
            .or_else(|| usage.get("duration"))
            .and_then(Value::as_u64)
            .filter(|s| *s > 0)
            .and_then(|s| u32::try_from(s).ok());
        job.resolution = usage
            .get("SR")
            .and_then(Value::as_u64)
            .map(|sr| format!("{sr}P"));
        job.aspect_ratio = usage
            .get("ratio")
            .and_then(Value::as_str)
            .map(str::to_owned);
    }
    f.rest_into(&mut extras, "");
    job.extras = extras;
    Ok(job)
}

pub(crate) fn encode_job(job: &VideoJob) -> Value {
    let mut output = Map::new();
    if !job.id.is_empty() {
        output.insert("task_id".into(), job.id.clone().into());
    }
    let status = match job.status {
        VideoStatus::Queued => "PENDING",
        VideoStatus::InProgress => "RUNNING",
        VideoStatus::Completed => "SUCCEEDED",
        _ => job.extras.form_str(D, STATUS).unwrap_or("FAILED"),
    };
    output.insert("task_status".into(), status.into());
    if let Some(Source::Url(u)) = job.output(VideoVariant::Video).map(|o| &o.source) {
        output.insert("video_url".into(), u.clone().into());
    }
    if let Some(e) = &job.error {
        if !job.extras.form_flag(D, NO_CODE) {
            output.insert("code".into(), e.code.clone().into());
        }
        if !job.extras.form_flag(D, NO_MESSAGE) {
            output.insert("message".into(), e.message.clone().into());
        }
    }
    put_extras(&mut output, &job.extras, D, "output");
    let mut out = Map::new();
    out.insert("output".into(), Value::Object(output));
    put_extras(&mut out, &job.extras, D, "");
    Value::Object(out)
}

/// DashScope's error body, `{"request_id", "code", "message"}`, by its status and code.
pub(super) fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    let value: Option<Value> = serde_json::from_slice(body).ok();
    let field = |k: &str| {
        value
            .as_ref()
            .and_then(|v| v.get(k))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    };
    let (Some(code), message) = (field("code"), field("message")) else {
        return crate::openai_responses::error::decode(D, status, headers, body);
    };
    let kind = match code {
        "InvalidApiKey" => ErrorKind::Authentication,
        c if c.starts_with("Throttling") => ErrorKind::RateLimited,
        c if c.starts_with("DataInspection") => ErrorKind::ContentFiltered,
        _ => crate::openai_responses::error::kind_for(Some(status), None, None),
    };
    let mut err = KppError::new(
        kind,
        message.map_or_else(
            || format!("QwenCloud answered with `{code}`."),
            str::to_owned,
        ),
    );
    if (400..600).contains(&status) {
        err = err.with_status(status);
    }
    if let Some(v) = value {
        err = err.with_raw(v);
    }
    err.with_origin(Origin::new(D))
}

fn not_served(kind: RequestKind) -> KppError {
    KppError::unsupported_by(
        format!("{kind:?} request"),
        D,
        "Kaeso sends QwenCloud images, speech, transcriptions and video jobs to DashScope.",
    )
}

impl Codec for DashScope {
    const DIALECT: DialectId = D;

    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        _headers: &HeaderList,
    ) -> Result<Request, KppError> {
        crate::limits::request_shape(D, body)?;
        match kind {
            RequestKind::Video => decode_request(body),
            RequestKind::Image => images::decode_request(body),
            RequestKind::Speech => speech::decode_request(body),
            RequestKind::Transcription => transcription::decode_request(body),
            other => Err(not_served(other)),
        }
    }

    fn encode_response(reply: &Reply, _ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        match reply {
            Reply::Image(r) => images::encode_response(r),
            Reply::Speech(r) => speech::encode_response(r),
            Reply::Transcription(r) => transcription::encode_response(r),
            other => crate::wire::to_body(&encode_job(video_reply(other, D)?)),
        }
    }

    fn stream_encoder(_ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        no_stream(Origin::new(D))
    }

    fn encode_error(err: &KppError) -> EncodedError {
        crate::openai_responses::error::encode(err)
    }

    fn check(req: &Request, _ctx: &EgressCtx) -> Result<(), KppError> {
        match req {
            Request::Image(r) => images::check(r).map(|_| ()),
            Request::Speech(r) => speech::check(r).map(|_| ()),
            Request::Transcription(r) => transcription::check(r),
            Request::Video(_) => check(req).map(|_| ()),
            other => Err(not_served(other.kind())),
        }
    }

    fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
        match req {
            Request::Image(r) => images::encode_request(r, ctx),
            Request::Speech(r) => speech::encode_request(r, ctx),
            Request::Transcription(r) => transcription::encode_request(r, ctx),
            Request::Video(_) => encode_request(req),
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
            return Err(decode_error(status, headers, body));
        }
        match ctx.kind {
            RequestKind::Image => return images::decode_response(body, ctx),
            RequestKind::Speech => return speech::decode_response(body, ctx),
            RequestKind::Transcription => return transcription::decode_response(body, ctx),
            _ => {}
        }
        let job = decode_job(body, ctx)?;
        if job.id.is_empty() {
            return Err(KppError::provider(
                "QwenCloud started a video task without naming its `task_id`.",
            ));
        }
        Ok(Reply::Video(job))
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        if ctx.kind == RequestKind::Speech {
            return Box::new(speech::Decoder::new(ctx));
        }
        no_stream(ctx.origin(D))
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        decode_error(status, headers, body)
    }

    fn encode_video_poll(
        job: &str,
        _model: &str,
        _ctx: &EgressCtx,
    ) -> Result<EncodedRequest, KppError> {
        Ok(EncodedRequest::get(
            format!("{TASKS_PATH}/{}", crate::video_jobs::path_segment(job)),
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
            return Err(decode_error(status, headers, body));
        }
        decode_job(body, ctx)
    }

    openai_listing!(D);
}
