//! Z.ai's CogVideoX API (<https://docs.z.ai/api-reference/video/generate-video>):
//! `POST /api/paas/v4/videos/generations` starts a job, and
//! `GET /api/paas/v4/async-result/{id}` reports it with the video's and its cover's links.

#[cfg(test)]
mod tests;

use kaeso_kpp::{
    DialectId, Extras, ImageSize, KppError, Origin, Reply, Request, RequestKind, Source,
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
    openai_listing, provider_object, put, put_extras, refuse_unset, refused, video_reply,
    video_request,
};

const D: DialectId = DialectId::ZaiVideos;
const PROVIDER: &str = "Z.ai";

/// The path that starts a video, from the root of Z.ai's host.
pub const GENERATIONS_PATH: &str = "/api/paas/v4/videos/generations";
/// The path of a job's result, followed by its id.
pub const RESULT_PATH: &str = "/api/paas/v4/async-result";

/// The sizes `size` takes.
pub const SIZES: &[&str] = &[
    "1280x720",
    "720x1280",
    "1024x1024",
    "1920x1080",
    "1080x1920",
    "2048x1080",
    "3840x2160",
];

const ONE_IMAGE: &str = "image_url_string";

/// Z.ai's CogVideoX API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ZaiVideos;

impl crate::codec::sealed::Sealed for ZaiVideos {}

fn decode_request(body: &[u8]) -> Result<Request, KppError> {
    let mut f = Fields::parse(body, D, "request")?;
    let origin = Origin::new(D);
    let mut extras = Extras::new(origin);
    let mut r = VideoRequest::new(
        f.str("model")?.unwrap_or_default(),
        VideoOperation::Generate,
    );
    r.prompt = f.str("prompt")?;
    r.audio = f.bool("with_audio")?;
    r.size = f.str("size")?.map(|s| ImageSize::from_wire(&s));
    r.seconds = f.u32("duration")?;
    r.user = f.str("user_id")?;
    let links =
        match f.take("image_url") {
            None => Vec::new(),
            Some(Value::String(s)) => {
                extras.form.insert(ONE_IMAGE.to_owned(), Value::Bool(true));
                vec![s]
            }
            Some(Value::Array(items)) => items
                .into_iter()
                .map(|v| match v {
                    Value::String(s) => Ok(s),
                    _ => Err(KppError::invalid_request("`image_url` lists links.")
                        .with_param("image_url")),
                })
                .collect::<Result<_, _>>()?,
            Some(_) => {
                return Err(
                    KppError::invalid_request("`image_url` lists links.").with_param("image_url")
                );
            }
        };
    if links.len() > 2 {
        return Err(KppError::invalid_request(
            "`image_url` takes a first and a last frame at most.",
        )
        .with_param("image_url"));
    }
    for (i, link) in links.into_iter().enumerate() {
        let role = if i == 0 {
            VideoImageRole::FirstFrame
        } else {
            VideoImageRole::LastFrame
        };
        r.images.push(VideoImage::new(link_source(link), role));
    }
    f.rest_into(&mut extras, "");
    r.extras = extras;
    Ok(Request::Video(r))
}

fn size_for(r: &VideoRequest) -> Result<Option<String>, KppError> {
    let size = match (&r.size, &r.resolution, &r.aspect_ratio) {
        (Some(s), _, _) => Some(s.clone()),
        (None, Some(res), Some(ratio)) => crate::video_jobs::size_of(res, ratio),
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
    match size {
        Some(s) if !SIZES.contains(&s.as_wire()) => Err(refused(
            D,
            PROVIDER,
            &format!("the size `{}`", s.as_wire()),
            "size",
            "use a size Z.ai lists, such as 1920x1080.",
        )),
        other => Ok(other.map(|s| s.as_wire().to_owned())),
    }
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
    refuse_unset(
        r,
        D,
        PROVIDER,
        &[
            "seconds",
            "size",
            "aspect_ratio",
            "resolution",
            "audio",
            "user",
        ],
    )?;
    check_roles(
        r,
        D,
        PROVIDER,
        &[VideoImageRole::FirstFrame, VideoImageRole::LastFrame],
    )?;
    if let Some(s) = r.seconds
        && s != 5
        && s != 10
    {
        return Err(refused(
            D,
            PROVIDER,
            &format!("{s} seconds"),
            "seconds",
            "choose 5 or 10 seconds.",
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
    size_for(r).map(|_| ())
}

fn encode_request(req: &Request) -> Result<EncodedRequest, KppError> {
    check(req)?;
    let r = video_request(req, D)?;
    let mut out = Map::new();
    out.insert("model".into(), r.model.clone().into());
    put(&mut out, "prompt", r.prompt.clone());
    put(&mut out, "with_audio", r.audio);
    let links: Vec<Value> = r
        .images
        .iter()
        .filter_map(image_link)
        .map(Value::String)
        .collect();
    match links.as_slice() {
        [] => {}
        [one] if r.extras.form_flag(D, ONE_IMAGE) => {
            out.insert("image_url".into(), one.clone());
        }
        _ => {
            out.insert("image_url".into(), Value::Array(links));
        }
    }
    put(&mut out, "size", size_for(r)?);
    put(&mut out, "duration", r.seconds);
    put(&mut out, "user_id", r.user.clone());
    put_extras(&mut out, &r.extras, D, "");
    let headers = HeaderList::new().with("content-type", crate::codec::JSON_MEDIA_TYPE);
    Ok(EncodedRequest::post_json(
        GENERATIONS_PATH,
        headers,
        body(&out)?,
    ))
}

fn status_of(s: &str) -> Result<VideoStatus, KppError> {
    match s {
        "PROCESSING" => Ok(VideoStatus::InProgress),
        "SUCCESS" => Ok(VideoStatus::Completed),
        "FAIL" => Ok(VideoStatus::Failed),
        other => Err(KppError::provider(format!(
            "Z.ai sent the unknown task status `{other}`."
        ))),
    }
}

fn status_text(s: VideoStatus) -> &'static str {
    match s {
        VideoStatus::Completed => "SUCCESS",
        VideoStatus::Failed => "FAIL",
        _ => "PROCESSING",
    }
}

fn decode_job(body: &[u8], ctx: &EgressCtx) -> Result<VideoJob, KppError> {
    let mut f = provider_object(body, D)?;
    let mut extras = Extras::new(ctx.origin(D));
    let status = status_of(&f.str("task_status")?.unwrap_or_default())?;
    let mut job = VideoJob::new(f.str("id")?.unwrap_or_default(), status);
    job.model = f.str("model")?.unwrap_or_default();
    if status == VideoStatus::Failed {
        job.error = Some(VideoError::new("FAIL", "Z.ai could not make the video."));
    }
    for item in f.array("video_result")?.unwrap_or_default() {
        let Value::Object(map) = item else { continue };
        let mut v = Fields::from_map(map, D, "reply");
        if let Some(url) = v.str("url")? {
            job.outputs.push(
                VideoOutput::new(VideoVariant::Video, Source::Url(url))
                    .with_media_type("video/mp4"),
            );
        }
        if let Some(url) = v.str("cover_image_url")? {
            job.outputs
                .push(VideoOutput::new(VideoVariant::Thumbnail, Source::Url(url)));
        }
        v.rest_into(&mut extras, "video_result");
    }
    f.rest_into(&mut extras, "");
    job.extras = extras;
    Ok(job)
}

pub(crate) fn encode_job(job: &VideoJob) -> Value {
    let mut out = Map::new();
    if !job.model.is_empty() {
        out.insert("model".into(), job.model.clone().into());
    }
    if !job.id.is_empty() {
        out.insert("id".into(), job.id.clone().into());
    }
    out.insert("task_status".into(), status_text(job.status).into());
    let link = |variant| {
        job.output(variant).and_then(|o| match &o.source {
            Source::Url(u) => Some(u.clone()),
            _ => None,
        })
    };
    let (video, cover) = (link(VideoVariant::Video), link(VideoVariant::Thumbnail));
    if video.is_some() || cover.is_some() {
        let mut item = Map::new();
        put(&mut item, "url", video);
        put(&mut item, "cover_image_url", cover);
        put_extras(&mut item, &job.extras, D, "video_result");
        out.insert(
            "video_result".into(),
            Value::Array(vec![Value::Object(item)]),
        );
    }
    put_extras(&mut out, &job.extras, D, "");
    Value::Object(out)
}

impl Codec for ZaiVideos {
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
                "Z.ai started a video job without naming its `id`.",
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
            format!("{RESULT_PATH}/{}", crate::video_jobs::path_segment(job)),
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
