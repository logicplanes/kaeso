//! Veo on Gemini Enterprise Agent Platform
//! (<https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/video/generate-videos-from-text>):
//! `{model}:predictLongRunning` starts a long-running operation, and
//! `{model}:fetchPredictOperation` reports it, with the videos inline when no Cloud Storage
//! bucket was named. The request's fields are `VideoGenerationModelInstance` and
//! `VideoGenerationModelParams`.

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
    Fields, body, check_foreign_extras, check_roles, no_stream, openai_listing, provider_object,
    put, put_extras, refuse_unset, refused, video_reply, video_request,
};

const D: DialectId = DialectId::Veo;
const PROVIDER: &str = "Veo";

/// The aspect ratios `aspectRatio` takes.
pub const ASPECT_RATIOS: &[&str] = &["16:9", "9:16"];
/// The resolutions `resolution` takes; `4k` only on `veo-3.1-generate-001`.
pub const RESOLUTIONS: &[&str] = &["720p", "1080p", "4k"];
/// The lengths `durationSeconds` takes on Veo 3.1.
pub const DURATIONS: &[u32] = &[4, 6, 8];

const NOT_DONE: &str = "done_false";
const FILTERED: &str = "filtered";

/// Veo's `predictLongRunning` and `fetchPredictOperation`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Veo;

impl crate::codec::sealed::Sealed for Veo {}

fn image_of(v: Value, role: VideoImageRole) -> Result<VideoImage, KppError> {
    let Value::Object(map) = v else {
        return Err(KppError::invalid_request("An image is an object.").with_param("image"));
    };
    let mut f = Fields::from_map(map, D, "request");
    let source = match (f.str("bytesBase64Encoded")?, f.str("gcsUri")?) {
        (Some(data), None) => Source::base64(
            f.str("mimeType")?.unwrap_or_else(|| "image/png".into()),
            data,
        ),
        (None, Some(uri)) => Source::Url(uri),
        _ => {
            return Err(KppError::invalid_request(
                "An image takes exactly one of `bytesBase64Encoded` and `gcsUri`.",
            )
            .with_param("image"));
        }
    };
    let mut image = VideoImage::new(source, role);
    image.extras = f.rest(&Origin::new(D));
    Ok(image)
}

fn image_value(image: &VideoImage) -> Value {
    let mut out = Map::new();
    match &image.source {
        Source::Base64(b) => {
            out.insert("bytesBase64Encoded".into(), b.data.clone().into());
            out.insert("mimeType".into(), b.media_type.clone().into());
        }
        Source::Url(u) => {
            out.insert("gcsUri".into(), u.clone().into());
        }
        _ => {}
    }
    put_extras(&mut out, &image.extras, D, "");
    Value::Object(out)
}

fn decode_request(body: &[u8]) -> Result<Request, KppError> {
    let mut f = Fields::parse(body, D, "request")?;
    let origin = Origin::new(D);
    let mut extras = Extras::new(origin);
    let mut r = VideoRequest::new(String::new(), VideoOperation::Generate);
    let mut instances = f.array("instances")?.unwrap_or_default();
    if instances.len() != 1 {
        return Err(
            KppError::invalid_request("Kaeso sends one instance a request.")
                .with_param("instances"),
        );
    }
    let Some(Value::Object(instance)) = instances.pop() else {
        return Err(KppError::invalid_request("An instance is an object.").with_param("instances"));
    };
    let mut i = Fields::from_map(instance, D, "request");
    r.prompt = i.str("prompt")?;
    if let Some(v) = i.take("image") {
        r.images.push(image_of(v, VideoImageRole::FirstFrame)?);
    }
    if let Some(v) = i.take("lastFrame") {
        r.images.push(image_of(v, VideoImageRole::LastFrame)?);
    }
    for reference in i.array("referenceImages")?.unwrap_or_default() {
        let Value::Object(map) = reference else {
            return Err(KppError::invalid_request("A reference image is an object.")
                .with_param("referenceImages"));
        };
        let mut rf = Fields::from_map(map, D, "request");
        let image = rf.take("image").ok_or_else(|| {
            KppError::invalid_request("A reference image needs its `image`.")
                .with_param("referenceImages")
        })?;
        let mut image = image_of(image, VideoImageRole::Reference)?;
        rf.rest_into(&mut image.extras, "reference");
        r.images.push(image);
    }
    i.rest_into(&mut extras, "instance");
    if let Some(mut p) = f.object("parameters")? {
        r.count = p.u32("sampleCount")?;
        r.seconds = p.u32("durationSeconds")?;
        r.seed = p.i64("seed")?;
        r.aspect_ratio = p.str("aspectRatio")?;
        r.resolution = p.str("resolution")?;
        r.negative_prompt = p.str("negativePrompt")?;
        r.audio = p.bool("generateAudio")?;
        p.rest_into(&mut extras, "parameters");
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
                    "use 1280x720, 1920x1080 or 3840x2160, or their portrait forms.",
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
            "seed",
            "negative_prompt",
            "audio",
            "n",
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
    for image in &r.images {
        let ok = match &image.source {
            Source::Base64(_) => true,
            Source::Url(u) => u.starts_with("gs://"),
            _ => false,
        };
        if !ok {
            return Err(refused(
                D,
                PROVIDER,
                "images by web link",
                "input_reference",
                "upload the image, or link it in Cloud Storage with a `gs://` link.",
            ));
        }
    }
    if let Some(s) = r.seconds
        && !DURATIONS.contains(&s)
    {
        return Err(refused(
            D,
            PROVIDER,
            &format!("{s} seconds"),
            "seconds",
            "choose 4, 6 or 8 seconds.",
        ));
    }
    if let Some(n) = r.count
        && !(1..=4).contains(&n)
    {
        return Err(refused(
            D,
            PROVIDER,
            &format!("{n} videos"),
            "n",
            "make 1 to 4 videos.",
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
            "use 720p, 1080p or 4k.",
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

fn models_path(ctx: &EgressCtx) -> &str {
    ctx.gemini_profile.models_path.as_ref()
}

fn encode_request(req: &Request, ctx: &EgressCtx) -> Result<EncodedRequest, KppError> {
    check(req)?;
    let r = video_request(req, D)?;
    let mut instance = Map::new();
    put(&mut instance, "prompt", r.prompt.clone());
    if let Some(first) = r.images_for(VideoImageRole::FirstFrame).next() {
        instance.insert("image".into(), image_value(first));
    }
    if let Some(last) = r.images_for(VideoImageRole::LastFrame).next() {
        instance.insert("lastFrame".into(), image_value(last));
    }
    let references: Vec<Value> = r
        .images_for(VideoImageRole::Reference)
        .map(|i| {
            let mut out = Map::new();
            out.insert("image".into(), image_value(i));
            if i.extras.is_from(D) {
                put_extras(&mut out, &i.extras, D, "reference");
            }
            if !out.contains_key("referenceType") {
                out.insert("referenceType".into(), "asset".into());
            }
            Value::Object(out)
        })
        .collect();
    if !references.is_empty() {
        instance.insert("referenceImages".into(), Value::Array(references));
    }
    put_extras(&mut instance, &r.extras, D, "instance");
    let mut parameters = Map::new();
    put(&mut parameters, "sampleCount", r.count);
    put(&mut parameters, "durationSeconds", r.seconds);
    put(&mut parameters, "seed", r.seed);
    let (resolution, ratio) = tier_and_ratio(r)?;
    put(&mut parameters, "aspectRatio", ratio);
    put(&mut parameters, "resolution", resolution);
    put(&mut parameters, "negativePrompt", r.negative_prompt.clone());
    put(&mut parameters, "generateAudio", r.audio);
    put_extras(&mut parameters, &r.extras, D, "parameters");
    let mut out = Map::new();
    out.insert("instances".into(), json!([Value::Object(instance)]));
    if !parameters.is_empty() {
        out.insert("parameters".into(), Value::Object(parameters));
    }
    put_extras(&mut out, &r.extras, D, "");
    let path = format!(
        "{}/{}:predictLongRunning",
        models_path(ctx),
        crate::video_jobs::path_segment(&r.model)
    );
    let headers = HeaderList::new().with("content-type", crate::codec::JSON_MEDIA_TYPE);
    Ok(EncodedRequest::post_json(path, headers, body(&out)?))
}

fn decode_operation(body: &[u8], ctx: &EgressCtx) -> Result<VideoJob, KppError> {
    let mut f = provider_object(body, D)?;
    let mut extras = Extras::new(ctx.origin(D));
    let name = f.str("name")?.unwrap_or_default();
    let done = f.bool("done")?;
    if done == Some(false) {
        extras.form.insert(NOT_DONE.into(), Value::Bool(true));
    }
    let mut job = VideoJob::new(name, VideoStatus::InProgress);
    if let Some(mut e) = f.object("error")? {
        let code = e.take("code").map(|c| c.to_string()).unwrap_or_default();
        job.error = Some(VideoError::new(code, e.str("message")?.unwrap_or_default()));
        e.rest_into(&mut extras, "error");
        job.status = VideoStatus::Failed;
    }
    if let Some(mut response) = f.object("response")? {
        for video in response.array("videos")?.unwrap_or_default() {
            let Value::Object(map) = video else { continue };
            let mut v = Fields::from_map(map, D, "reply");
            let media_type = v.str("mimeType")?.unwrap_or_else(|| "video/mp4".into());
            let source = match (v.str("bytesBase64Encoded")?, v.str("gcsUri")?) {
                (Some(data), _) => Source::base64(media_type.clone(), data),
                (None, Some(uri)) => Source::Url(uri),
                (None, None) => continue,
            };
            job.outputs
                .push(VideoOutput::new(VideoVariant::Video, source).with_media_type(media_type));
            v.rest_into(&mut extras, "response.videos");
        }
        let filtered = response.u32("raiMediaFilteredCount")?;
        if let Some(n) = filtered {
            extras.form.insert(FILTERED.into(), n.into());
        }
        response.rest_into(&mut extras, "response");
        if job.status != VideoStatus::Failed {
            job.status = if !job.outputs.is_empty() {
                VideoStatus::Completed
            } else {
                job.error = Some(VideoError::new(
                    "moderation_blocked",
                    "Veo filtered every video under its responsible AI rules.",
                ));
                VideoStatus::Failed
            };
        }
    } else if done == Some(true) && job.status != VideoStatus::Failed {
        return Err(KppError::provider(
            "Veo finished an operation without a response or an error.",
        ));
    }
    f.rest_into(&mut extras, "");
    job.extras = extras;
    Ok(job)
}

pub(crate) fn encode_operation(job: &VideoJob) -> Value {
    let mut out = Map::new();
    if !job.id.is_empty() {
        out.insert("name".into(), job.id.clone().into());
    }
    let done = !job.extras.form_flag(D, NOT_DONE) && job.status.is_done();
    if done || job.extras.form_flag(D, NOT_DONE) {
        out.insert("done".into(), done.into());
    }
    let has_response =
        job.status == VideoStatus::Completed || job.extras.form_value(D, FILTERED).is_some();
    if job.status == VideoStatus::Failed
        && !has_response
        && let Some(e) = &job.error
    {
        let mut error = Map::new();
        let code: Value = e
            .code
            .parse::<i64>()
            .map_or_else(|_| e.code.clone().into(), Value::from);
        error.insert("code".into(), code);
        error.insert("message".into(), e.message.clone().into());
        put_extras(&mut error, &job.extras, D, "error");
        out.insert("error".into(), Value::Object(error));
    }
    if has_response {
        let mut response = Map::new();
        if let Some(n) = job.extras.form_value(D, FILTERED) {
            response.insert("raiMediaFilteredCount".into(), n.clone());
        }
        let videos: Vec<Value> = job
            .outputs
            .iter()
            .map(|o| {
                let mut v = Map::new();
                match &o.source {
                    Source::Base64(b) => {
                        v.insert("bytesBase64Encoded".into(), b.data.clone().into());
                    }
                    Source::Url(u) => {
                        v.insert("gcsUri".into(), u.clone().into());
                    }
                    _ => {}
                }
                put(&mut v, "mimeType", o.media_type.clone());
                put_extras(&mut v, &job.extras, D, "response.videos");
                Value::Object(v)
            })
            .collect();
        if !videos.is_empty() {
            response.insert("videos".into(), Value::Array(videos));
        }
        put_extras(&mut response, &job.extras, D, "response");
        out.insert("response".into(), Value::Object(response));
    }
    put_extras(&mut out, &job.extras, D, "");
    Value::Object(out)
}

impl Codec for Veo {
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
        crate::wire::to_body(&encode_operation(video_reply(reply, D)?))
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
        let job = decode_operation(body, ctx)?;
        if job.id.is_empty() {
            return Err(KppError::provider(
                "Veo started an operation without naming it.",
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
        model: &str,
        ctx: &EgressCtx,
    ) -> Result<EncodedRequest, KppError> {
        let path = format!(
            "{}/{}:fetchPredictOperation",
            models_path(ctx),
            crate::video_jobs::path_segment(model)
        );
        let headers = HeaderList::new().with("content-type", crate::codec::JSON_MEDIA_TYPE);
        Ok(EncodedRequest::post_json(
            path,
            headers,
            crate::wire::to_body(&json!({ "operationName": job }))?,
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
        decode_operation(body, ctx)
    }

    openai_listing!(D);
}
