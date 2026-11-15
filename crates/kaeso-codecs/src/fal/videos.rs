//! Video jobs through fal's queue: Veo 3.1, Kling 4 and Wan 3.0, each with the input of its
//! OpenAPI document (`https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=<model>`, read
//! 2026-11-14). A job is the request's result path at fal; its status is that path followed by
//! `/status`, and the finished video is named in the result.

use kaeso_kpp::{
    Extras, ImageSize, KppError, Origin, Request, Source, VideoError, VideoImage, VideoImageRole,
    VideoJob, VideoOperation, VideoOutput, VideoRequest, VideoStatus, VideoVariant,
};
use serde_json::{Map, Value};

use super::{COMPLETED, D, IN_PROGRESS, IN_QUEUE, PROVIDER, queued, submit};
use crate::codec::{EgressCtx, EncodedRequest};
use crate::video_jobs::{
    Fields, body, check_foreign_extras, image_link, link_source, provider_object, put, put_extras,
    refuse_unset, refused, size_of,
};

/// The video models Kaeso maps to fal's inputs.
pub const VIDEO_MODELS: &[&str] = &[
    "fal-ai/veo3.1",
    "kling/v4/text-to-video",
    "kling/v4/image-to-video",
    "alibaba/wan-3.0/text-to-video",
    "alibaba/wan-3.0/image-to-video",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Family {
    Veo,
    KlingText,
    KlingImage,
    WanText,
    WanImage,
}

/// What one family's input takes.
struct Spec {
    seconds: std::ops::RangeInclusive<u32>,
    /// Veo takes only 4, 6 or 8 seconds.
    even_seconds: bool,
    tiers: &'static [u32],
    ratios: &'static [&'static str],
    audio: Option<&'static str>,
    seed: bool,
    negative_prompt: bool,
    frames: bool,
    prompt_required: bool,
}

impl Family {
    fn of(model: &str) -> Option<Family> {
        Some(match model {
            "fal-ai/veo3.1" => Family::Veo,
            "kling/v4/text-to-video" => Family::KlingText,
            "kling/v4/image-to-video" => Family::KlingImage,
            "alibaba/wan-3.0/text-to-video" => Family::WanText,
            "alibaba/wan-3.0/image-to-video" => Family::WanImage,
            _ => return None,
        })
    }

    fn spec(self) -> Spec {
        const WAN_RATIOS: &[&str] = &["16:9", "4:3", "1:1", "3:4", "9:16"];
        match self {
            Family::Veo => Spec {
                seconds: 4..=8,
                even_seconds: true,
                tiers: &[720, 1080, 2160],
                ratios: &["16:9", "9:16"],
                audio: Some("generate_audio"),
                seed: true,
                negative_prompt: true,
                frames: false,
                prompt_required: true,
            },
            Family::KlingText | Family::KlingImage => Spec {
                seconds: 3..=30,
                even_seconds: false,
                tiers: &[720, 1080],
                // Kling's image-to-video input has no `aspect_ratio`: the first frame's shape
                // decides the video's.
                ratios: if self == Family::KlingText {
                    &["16:9", "9:16", "1:1", "21:9"]
                } else {
                    &[]
                },
                audio: None,
                seed: false,
                negative_prompt: false,
                frames: self == Family::KlingImage,
                prompt_required: self == Family::KlingText,
            },
            Family::WanText | Family::WanImage => Spec {
                seconds: 2..=30,
                even_seconds: false,
                tiers: &[480, 720, 1080],
                ratios: WAN_RATIOS,
                audio: Some("audio"),
                seed: true,
                negative_prompt: false,
                frames: self == Family::WanImage,
                prompt_required: self == Family::WanText,
            },
        }
    }

    fn image_model(self) -> Option<&'static str> {
        match self {
            Family::KlingText => Some("kling/v4/image-to-video"),
            Family::WanText => Some("alibaba/wan-3.0/image-to-video"),
            _ => None,
        }
    }
}

fn no(feature: &str, param: &str, hint: &str) -> KppError {
    refused(D, PROVIDER, feature, param, hint)
}

fn tier_text(short: u32) -> String {
    if short == 2160 {
        "4k".to_owned()
    } else {
        format!("{short}p")
    }
}

fn same_ratio(w: u32, h: u32, ratio: &str) -> bool {
    let Some((a, b)) = ratio.split_once(':') else {
        return false;
    };
    let (Ok(a), Ok(b)) = (a.parse::<u64>(), b.parse::<u64>()) else {
        return false;
    };
    u64::from(w) * b == u64::from(h) * a
}

/// The resolution and aspect ratio fal takes for `size`.
fn tier_and_ratio(
    model: &str,
    spec: &Spec,
    size: &ImageSize,
) -> Result<(String, &'static str), KppError> {
    let found = size.dimensions().and_then(|(w, h)| {
        let short = w.min(h);
        let ratio = spec.ratios.iter().copied().find(|r| same_ratio(w, h, r))?;
        spec.tiers
            .contains(&short)
            .then(|| (tier_text(short), ratio))
    });
    found.ok_or_else(|| {
        let tiers: Vec<String> = spec.tiers.iter().map(|t| tier_text(*t)).collect();
        no(
            &format!("the size `{}` on `{model}`", size.as_wire()),
            "size",
            &format!(
                "use {} at {}, such as 1280x720.",
                tiers.join(", "),
                spec.ratios.join(", ")
            ),
        )
    })
}

/// Checks that fal can make `r`, before anything is sent.
pub(super) fn check(r: &VideoRequest) -> Result<Family, KppError> {
    check_foreign_extras(r, D)?;
    let family = Family::of(&r.model).ok_or_else(|| {
        no(
            &format!("the video model `{}` through Kaeso", r.model),
            "model",
            &format!("use one of {}.", VIDEO_MODELS.join(", ")),
        )
    })?;
    let spec = family.spec();
    if r.operation == VideoOperation::Remix {
        return Err(no("remixes", "video_id", "create a new video instead."));
    }
    let mut takes = vec!["seconds"];
    if !spec.ratios.is_empty() {
        takes.push("size");
    }
    if spec.audio.is_some() {
        takes.push("audio");
    }
    if spec.seed {
        takes.push("seed");
    }
    if spec.negative_prompt {
        takes.push("negative_prompt");
    }
    if family == Family::KlingImage && r.size.is_some() {
        return Err(no(
            &format!("`size` on `{}`", r.model),
            "size",
            "the first frame's shape decides the video's; it is made at 1080p.",
        ));
    }
    refuse_unset(r, D, PROVIDER, &takes)?;
    if spec.prompt_required && r.prompt.as_deref().is_none_or(str::is_empty) {
        return Err(
            KppError::invalid_request(format!("`{}` needs a `prompt`.", r.model))
                .with_param("prompt"),
        );
    }
    // Billing is per second of the video fal makes, so the length has to be known.
    let Some(seconds) = r.seconds else {
        return Err(no(
            "a video without a length",
            "seconds",
            &format!(
                "send `seconds`, {} to {}.",
                spec.seconds.start(),
                spec.seconds.end()
            ),
        ));
    };
    if !spec.seconds.contains(&seconds) || (spec.even_seconds && seconds % 2 != 0) {
        let hint = if spec.even_seconds {
            "send 4, 6 or 8.".to_owned()
        } else {
            format!("send {} to {}.", spec.seconds.start(), spec.seconds.end())
        };
        return Err(no(
            &format!("{seconds} seconds on `{}`", r.model),
            "seconds",
            &hint,
        ));
    }
    if let Some(size) = &r.size {
        tier_and_ratio(&r.model, &spec, size)?;
    }
    if !spec.frames && !r.images.is_empty() {
        let hint = match family.image_model() {
            Some(m) => format!("use `{m}`."),
            None => "send the request without it.".to_owned(),
        };
        return Err(no(
            &format!("images on `{}`", r.model),
            "input_reference",
            &hint,
        ));
    }
    if spec.frames {
        for image in &r.images {
            if !matches!(
                image.role,
                VideoImageRole::FirstFrame | VideoImageRole::LastFrame
            ) {
                return Err(no(
                    "reference images",
                    "input_reference",
                    "send a first frame and, at most, a last frame.",
                ));
            }
            if image_link(image).is_none() {
                return Err(no(
                    "file ids",
                    "input_reference",
                    "upload the image or send its link.",
                ));
            }
        }
        if r.images_for(VideoImageRole::FirstFrame).next().is_none() {
            return Err(KppError::invalid_request(format!(
                "`{}` starts from an image; send it as `input_reference`.",
                r.model
            ))
            .with_param("input_reference"));
        }
    }
    Ok(family)
}

/// The input of the model `r` names.
pub(super) fn encode_request(r: &VideoRequest) -> Result<EncodedRequest, KppError> {
    let family = check(r)?;
    let spec = family.spec();
    let mut out = Map::new();
    put(&mut out, "prompt", r.prompt.clone());
    if let Some(s) = r.seconds {
        let duration = if family == Family::Veo {
            Value::from(format!("{s}s"))
        } else {
            Value::from(s)
        };
        out.insert("duration".into(), duration);
    }
    if let Some(size) = &r.size {
        let (tier, ratio) = tier_and_ratio(&r.model, &spec, size)?;
        out.insert("resolution".into(), tier.into());
        out.insert("aspect_ratio".into(), ratio.into());
    }
    if let Some(key) = spec.audio {
        put(&mut out, key, r.audio);
    }
    put(&mut out, "seed", r.seed);
    put(&mut out, "negative_prompt", r.negative_prompt.clone());
    let link = |role| r.images_for(role).next().and_then(image_link);
    put(
        &mut out,
        "start_image_url",
        link(VideoImageRole::FirstFrame),
    );
    put(&mut out, "end_image_url", link(VideoImageRole::LastFrame));
    put_extras(&mut out, &r.extras, D, "");
    submit(&r.model, body(&out)?)
}

/// A fal-format video input, as the dialect's fixtures write it; the model is in the URL.
pub(super) fn decode_request(body: &[u8]) -> Result<Request, KppError> {
    let mut f = Fields::parse(body, D, "request")?;
    let mut r = VideoRequest::new(String::new(), VideoOperation::Generate);
    r.prompt = f.str("prompt")?;
    r.seconds = match f.take("duration") {
        None => None,
        Some(Value::Number(n)) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        Some(Value::String(s)) => crate::video_jobs::digits(s.strip_suffix('s').unwrap_or(&s)),
        Some(_) => None,
    };
    let tier = f
        .peek("resolution")
        .and_then(Value::as_str)
        .map(|t| match t {
            "4k" => "2160p".to_owned(),
            other => other.to_owned(),
        });
    let ratio = f
        .peek("aspect_ratio")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if let (Some(tier), Some(ratio)) = (tier, ratio)
        && let Some(size) = size_of(&tier, &ratio)
    {
        r.size = Some(size);
        f.take("resolution");
        f.take("aspect_ratio");
    }
    r.audio = match f.bool("generate_audio")? {
        Some(a) => Some(a),
        None => f.bool("audio")?,
    };
    r.seed = f.i64("seed")?;
    r.negative_prompt = f.str("negative_prompt")?;
    if let Some(url) = f.str("start_image_url")? {
        r.images.push(VideoImage::new(
            link_source(url),
            VideoImageRole::FirstFrame,
        ));
    }
    if let Some(url) = f.str("end_image_url")? {
        r.images
            .push(VideoImage::new(link_source(url), VideoImageRole::LastFrame));
    }
    r.extras = f.rest(&Origin::new(D));
    Ok(Request::Video(r))
}

fn status_of(text: &str, failed: bool) -> VideoStatus {
    match text {
        IN_QUEUE => VideoStatus::Queued,
        IN_PROGRESS => VideoStatus::InProgress,
        _ if failed => VideoStatus::Failed,
        _ => VideoStatus::Completed,
    }
}

fn job_of(body: &[u8], ctx: &EgressCtx) -> Result<(VideoJob, Option<String>), KppError> {
    let v: Value = serde_json::from_slice(body).map_err(|_| {
        KppError::provider(format!("{PROVIDER} sent a queue status that is not JSON."))
    })?;
    let q = queued(&v).ok_or_else(|| {
        KppError::provider(format!(
            "{PROVIDER} sent a queue status Kaeso doesn't know."
        ))
    })?;
    let mut f = provider_object(body, D)?;
    f.take("status");
    let status = status_of(&q.status, q.failed);
    let mut job = VideoJob::new(q.result_path.clone().unwrap_or_default(), status);
    if status == VideoStatus::Failed {
        let err = super::error::failed(&v);
        let code = v
            .get("error_type")
            .and_then(Value::as_str)
            .unwrap_or("failed");
        job.error = Some(VideoError::new(code, err.message));
    }
    let mut extras = Extras::new(ctx.origin(D));
    f.rest_into(&mut extras, "");
    job.extras = extras;
    Ok((job, q.result_path))
}

/// The job fal's queue just took.
pub(super) fn decode_started(body: &[u8], ctx: &EgressCtx) -> Result<VideoJob, KppError> {
    let (job, result) = job_of(body, ctx)?;
    if result.is_none() {
        return Err(KppError::provider(format!(
            "{PROVIDER} took the video without naming a result URL on its queue."
        )));
    }
    Ok(job)
}

/// The job as its queue status reports it; a completed job names its result as a file.
pub(super) fn decode_poll(body: &[u8], ctx: &EgressCtx) -> Result<VideoJob, KppError> {
    let (mut job, result) = job_of(body, ctx)?;
    if job.status == VideoStatus::Completed {
        let result = result.ok_or_else(|| {
            KppError::provider(format!(
                "{PROVIDER} finished the video without naming its result URL."
            ))
        })?;
        job.outputs.push(VideoOutput::new(
            VideoVariant::Video,
            Source::file_id(ctx.origin(D), result),
        ));
    }
    Ok(job)
}

/// A job's queue status, as fal wrote it.
pub(super) fn encode_started(job: &VideoJob) -> Value {
    let status = match job.status {
        VideoStatus::Queued => IN_QUEUE,
        VideoStatus::InProgress => IN_PROGRESS,
        _ => COMPLETED,
    };
    let mut out = Map::new();
    out.insert("status".into(), status.into());
    put_extras(&mut out, &job.extras, D, "");
    Value::Object(out)
}

/// The finished video a result names.
pub(super) fn decode_file(body: &[u8]) -> Result<Source, KppError> {
    let v: Value = serde_json::from_slice(body)
        .map_err(|_| KppError::provider(format!("{PROVIDER} sent a result that is not JSON.")))?;
    if queued(&v).is_some_and(|q| q.failed) {
        return Err(super::error::failed(&v));
    }
    v.get("video")
        .and_then(|video| video.get("url"))
        .and_then(Value::as_str)
        .filter(|u| !u.is_empty())
        .map(|u| link_source(u.to_owned()))
        .ok_or_else(|| KppError::provider(format!("{PROVIDER} named no link for the video.")))
}
