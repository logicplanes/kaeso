//! SpaceX's video API (<https://docs.x.ai/developers/rest-api-reference/inference/videos>):
//! `POST /v1/videos/generations` and `POST /v1/videos/edits` start a job and answer with its
//! `request_id`; `GET /v1/videos/{request_id}` reports it.

#[cfg(test)]
mod tests;

use kaeso_kpp::{
    DialectId, Extras, KppError, Origin, Reply, Request, RequestKind, Source, VideoError,
    VideoImage, VideoImageRole, VideoJob, VideoOperation, VideoOutput, VideoRequest, VideoStatus,
    VideoVariant,
};
use serde_json::{Map, Value};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};
use crate::video_jobs::{
    Fields, body, check_foreign_extras, check_roles, image_link, link_source, no_stream,
    openai_listing, provider_object, put, put_extras, refuse_unset, refused, video_reply,
    video_request,
};

const D: DialectId = DialectId::XaiVideos;
const PROVIDER: &str = "SpaceX";

/// The path that starts a video from a prompt and images.
pub const GENERATIONS_PATH: &str = "/v1/videos/generations";
/// The path that starts a video made by changing an earlier one.
pub const EDITS_PATH: &str = "/v1/videos/edits";
/// The path of a job, followed by its `request_id`.
pub const JOBS_PATH: &str = "/v1/videos";

/// The aspect ratios `aspect_ratio` takes.
pub const ASPECT_RATIOS: &[&str] = &[
    "1:1", "16:9", "9:16", "4:3", "3:4", "3:2", "2:3", "21:9", "5:2",
];
/// The resolutions `resolution` takes.
pub const RESOLUTIONS: &[&str] = &["480p", "720p", "1080p"];

const EXPIRED: &str = "expired";
const MODERATED: &str = "moderated";
const EDIT: &str = "edit";

/// SpaceX's video API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct XaiVideos;

impl crate::codec::sealed::Sealed for XaiVideos {}

fn image_of(v: Value, role: VideoImageRole, origin: &Origin) -> Result<VideoImage, KppError> {
    let Value::Object(map) = v else {
        return Err(KppError::invalid_request(
            "An image must be an object with a `url` or a `file_id`.",
        )
        .with_param("image"));
    };
    let mut f = Fields::from_map(map, D, "request");
    let source = match (f.str("url")?, f.str("file_id")?) {
        (Some(url), None) => link_source(url),
        (None, Some(id)) => Source::file_id(origin.clone(), id),
        _ => {
            return Err(KppError::invalid_request(
                "An image takes exactly one of `url` and `file_id`.",
            )
            .with_param("image"));
        }
    };
    let mut image = VideoImage::new(source, role);
    image.extras = f.rest(origin);
    Ok(image)
}

fn image_value(image: &VideoImage) -> Value {
    let mut out = Map::new();
    match &image.source {
        Source::FileId(f) => {
            out.insert("file_id".into(), f.id.clone().into());
        }
        _ => put(&mut out, "url", image_link(image)),
    }
    put_extras(&mut out, &image.extras, D, "");
    Value::Object(out)
}

fn decode_request(body: &[u8], edit: bool) -> Result<Request, KppError> {
    let mut f = Fields::parse(body, D, "request")?;
    let origin = Origin::new(D);
    let operation = if edit {
        VideoOperation::Remix
    } else {
        VideoOperation::Generate
    };
    let mut r = VideoRequest::new(f.str("model")?.unwrap_or_default(), operation);
    r.prompt = f.str("prompt")?;
    r.user = f.str("user")?;
    if edit {
        if let Some(mut video) = f.object("video")? {
            r.source_video = video.str("url")?.map(Source::Url);
            if r.source_video.is_none() {
                return Err(KppError::invalid_request("`video` needs a `url`.").with_param("video"));
            }
            video.rest_into(&mut r.extras, "video");
        }
    } else {
        r.seconds = f.u32("duration")?;
        r.aspect_ratio = f.str("aspect_ratio")?;
        r.resolution = f.str("resolution")?;
        if let Some(image) = f.take("image") {
            r.images
                .push(image_of(image, VideoImageRole::FirstFrame, &origin)?);
        }
        for image in f.array("reference_images")?.unwrap_or_default() {
            r.images
                .push(image_of(image, VideoImageRole::Reference, &origin)?);
        }
    }
    let mut extras = std::mem::replace(&mut r.extras, Extras::new(origin.clone()));
    extras.origin = Some(origin.clone());
    if edit {
        extras.form.insert(EDIT.to_owned(), Value::Bool(true));
    }
    f.rest_into(&mut extras, "");
    r.extras = extras;
    Ok(Request::Video(r))
}

/// The resolution and aspect ratio a size names, when SpaceX lists both.
fn tier_and_ratio(r: &VideoRequest) -> Result<(Option<String>, Option<String>), KppError> {
    let (mut resolution, mut ratio) = (r.resolution.clone(), r.aspect_ratio.clone());
    if let Some(size) = &r.size {
        let tier = crate::video_jobs::tier_of(size);
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
                    "use 480p, 720p or 1080p at a ratio SpaceX lists, such as 1280x720.",
                ));
            }
        }
    }
    Ok((resolution, ratio))
}

fn check(req: &Request) -> Result<(), KppError> {
    let r = video_request(req, D)?;
    check_foreign_extras(r, D)?;
    if r.operation == VideoOperation::Remix {
        refuse_unset(r, D, PROVIDER, &["user"])?;
        if !r.images.is_empty() {
            return Err(refused(
                D,
                PROVIDER,
                "images in a remix",
                "input_reference",
                "remix with a prompt only.",
            ));
        }
        if !matches!(r.source_video, Some(Source::Url(_))) {
            return Err(refused(
                D,
                PROVIDER,
                "a remix without the earlier video's link",
                "video_id",
                "wait until the earlier video is completed.",
            ));
        }
        return Ok(());
    }
    refuse_unset(
        r,
        D,
        PROVIDER,
        &["seconds", "size", "aspect_ratio", "resolution", "user"],
    )?;
    check_roles(
        r,
        D,
        PROVIDER,
        &[VideoImageRole::FirstFrame, VideoImageRole::Reference],
    )?;
    if let Some(s) = r.seconds
        && !(1..=15).contains(&s)
    {
        return Err(refused(
            D,
            PROVIDER,
            &format!("{s} seconds"),
            "seconds",
            "choose 1 to 15 seconds.",
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
            "use 480p, 720p or 1080p.",
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
            "use a ratio SpaceX lists, such as 16:9.",
        ));
    }
    Ok(())
}

fn encode_request(req: &Request) -> Result<EncodedRequest, KppError> {
    check(req)?;
    let r = video_request(req, D)?;
    let mut out = Map::new();
    if !r.model.is_empty() {
        out.insert("model".into(), r.model.clone().into());
    }
    put(&mut out, "prompt", r.prompt.clone());
    let path = if r.operation == VideoOperation::Remix {
        let mut video = Map::new();
        if let Some(Source::Url(u)) = &r.source_video {
            video.insert("url".into(), u.clone().into());
        }
        put_extras(&mut video, &r.extras, D, "video");
        out.insert("video".into(), Value::Object(video));
        EDITS_PATH
    } else {
        put(&mut out, "duration", r.seconds);
        let (resolution, ratio) = tier_and_ratio(r)?;
        put(&mut out, "aspect_ratio", ratio);
        put(&mut out, "resolution", resolution);
        if let Some(first) = r.images_for(VideoImageRole::FirstFrame).next() {
            out.insert("image".into(), image_value(first));
        }
        let references: Vec<Value> = r
            .images_for(VideoImageRole::Reference)
            .map(image_value)
            .collect();
        if !references.is_empty() {
            out.insert("reference_images".into(), Value::Array(references));
        }
        GENERATIONS_PATH
    };
    put(&mut out, "user", r.user.clone());
    put_extras(&mut out, &r.extras, D, "");
    let headers = HeaderList::new().with("content-type", crate::codec::JSON_MEDIA_TYPE);
    Ok(EncodedRequest::post_json(path, headers, body(&out)?))
}

fn decode_started(body: &[u8], ctx: &EgressCtx) -> Result<VideoJob, KppError> {
    let mut f = provider_object(body, D)?;
    let id = f.str("request_id")?.ok_or_else(|| {
        KppError::provider("SpaceX started a video job without naming its `request_id`.")
    })?;
    let mut job = VideoJob::new(id, VideoStatus::Queued);
    job.extras = f.rest(&ctx.origin(D));
    Ok(job)
}

fn encode_started(job: &VideoJob) -> Value {
    let mut out = Map::new();
    out.insert("request_id".into(), job.id.clone().into());
    put_extras(&mut out, &job.extras, D, "");
    Value::Object(out)
}

fn decode_poll(body: &[u8], ctx: &EgressCtx) -> Result<VideoJob, KppError> {
    let mut f = provider_object(body, D)?;
    let mut extras = Extras::new(ctx.origin(D));
    let status = f.str("status")?.unwrap_or_default();
    let mut job = VideoJob::new(String::new(), VideoStatus::Queued);
    job.model = f.str("model")?.unwrap_or_default();
    job.progress = f.u32("progress")?;
    job.status = match status.as_str() {
        "pending" if job.progress.unwrap_or(0) == 0 => VideoStatus::Queued,
        "pending" => VideoStatus::InProgress,
        "done" => VideoStatus::Completed,
        "failed" => VideoStatus::Failed,
        EXPIRED => {
            extras.form.insert(EXPIRED.to_owned(), Value::Bool(true));
            job.error = Some(VideoError::new(EXPIRED, "The video expired at SpaceX."));
            VideoStatus::Failed
        }
        other => {
            return Err(KppError::provider(format!(
                "SpaceX sent the unknown video status `{other}`."
            )));
        }
    };
    if let Some(mut e) = f.object("error")? {
        job.error = Some(VideoError::new(
            e.str("code")?.unwrap_or_default(),
            e.str("message")?.unwrap_or_default(),
        ));
        e.rest_into(&mut extras, "error");
    }
    if let Some(mut video) = f.object("video")? {
        job.seconds = video.u32("duration")?;
        let url = video.str("url")?;
        let respects = video.bool("respect_moderation")?;
        if respects == Some(false) {
            extras.form.insert(MODERATED.to_owned(), Value::Bool(true));
            job.status = VideoStatus::Failed;
            job.error.get_or_insert_with(|| {
                VideoError::new(
                    "moderation_blocked",
                    "SpaceX withheld the video under its moderation rules.",
                )
            });
        } else if let Some(url) = url.filter(|u| !u.is_empty()) {
            job.outputs.push(
                VideoOutput::new(VideoVariant::Video, Source::Url(url))
                    .with_media_type("video/mp4"),
            );
        }
        video.rest_into(&mut extras, "video");
    }
    f.rest_into(&mut extras, "");
    job.extras = extras;
    Ok(job)
}

#[cfg(test)]
pub(crate) fn encode_poll(job: &VideoJob) -> Value {
    let flag = |k: &str| job.extras.form_flag(D, k);
    let mut out = Map::new();
    let status = match job.status {
        VideoStatus::Queued | VideoStatus::InProgress => "pending",
        VideoStatus::Completed => "done",
        VideoStatus::Failed if flag(EXPIRED) => EXPIRED,
        VideoStatus::Failed if flag(MODERATED) => "done",
        _ => "failed",
    };
    out.insert("status".into(), status.into());
    if let Some(e) = job
        .error
        .as_ref()
        .filter(|_| !flag(EXPIRED) && !flag(MODERATED))
    {
        let mut error = Map::new();
        error.insert("code".into(), e.code.clone().into());
        error.insert("message".into(), e.message.clone().into());
        put_extras(&mut error, &job.extras, D, "error");
        out.insert("error".into(), Value::Object(error));
    }
    if !job.model.is_empty() {
        out.insert("model".into(), job.model.clone().into());
    }
    put(&mut out, "progress", job.progress);
    let url = job
        .output(VideoVariant::Video)
        .and_then(|o| match &o.source {
            Source::Url(u) => Some(u.clone()),
            _ => None,
        });
    if url.is_some() || flag(MODERATED) || job.seconds.is_some() {
        let mut video = Map::new();
        put(&mut video, "url", url);
        put(&mut video, "duration", job.seconds);
        video.insert("respect_moderation".into(), Value::Bool(!flag(MODERATED)));
        put_extras(&mut video, &job.extras, D, "video");
        out.insert("video".into(), Value::Object(video));
    }
    put_extras(&mut out, &job.extras, D, "");
    Value::Object(out)
}

impl Codec for XaiVideos {
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
        let edit = serde_json::from_slice::<Value>(body)
            .ok()
            .is_some_and(|v| v.get("video").is_some());
        decode_request(body, edit)
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
        decode_started(body, ctx).map(Reply::Video)
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
        decode_poll(body, ctx)
    }

    openai_listing!(D);
}
