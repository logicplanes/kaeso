//! Video jobs through Replicate's official models: Veo 3.1 and Kling 2.5 Turbo Pro, each with
//! the input of its Cog OpenAPI schema (`https://replicate.com/{owner}/{name}/api/schema`, read
//! 2026-11-14). A job is a prediction: its id is the job's, and a finished prediction's output
//! is the video's link on `replicate.delivery`, deleted an hour after the video was made.

use kaeso_kpp::{
    Extras, ImageSize, KppError, Origin, Request, Source, VideoError, VideoImage, VideoImageRole,
    VideoJob, VideoOperation, VideoOutput, VideoRequest, VideoStatus, VideoVariant,
};
use serde_json::{Map, Value};

use super::{
    ABORTED, CANCELED, D, FAILED, PROCESSING, PROVIDER, STARTING, SUCCEEDED, prediction_of, submit,
};
use crate::codec::{EgressCtx, EncodedRequest};
use crate::video_jobs::{
    Fields, check_foreign_extras, image_link, link_source, provider_object, put, put_extras,
    refuse_unset, refused, size_of, unix_of_rfc3339,
};

/// The video models Kaeso maps to Replicate's inputs.
pub const VIDEO_MODELS: &[&str] = &["google/veo-3.1", "kwaivgi/kling-v2.5-turbo-pro"];

const VEO: &str = "google/veo-3.1";
// Replicate deletes a prediction's files an hour after it ends
// (https://replicate.com/docs/topics/predictions/data-retention).
const KEPT_SECONDS: i64 = 3600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Family {
    /// Veo 3.1: 4, 6 or 8 seconds at 720p or 1080p, 16:9 or 9:16, a first and a last frame or
    /// 1 to 3 reference images, audio, seed and negative prompt.
    Veo,
    /// Kling 2.5 Turbo Pro: 5 or 10 seconds, 16:9, 9:16 or 1:1 unless a first frame sets the
    /// shape, a first and a last frame and a negative prompt; its schema keeps `seed`,
    /// `resolution` and `generate_audio` but says they are not used.
    Kling,
}

impl Family {
    fn of(model: &str) -> Option<Family> {
        Some(match model {
            VEO => Family::Veo,
            "kwaivgi/kling-v2.5-turbo-pro" => Family::Kling,
            _ => return None,
        })
    }

    fn seconds(self) -> &'static [u32] {
        match self {
            Family::Veo => &[4, 6, 8],
            Family::Kling => &[5, 10],
        }
    }

    fn ratios(self) -> &'static [&'static str] {
        match self {
            Family::Veo => &["16:9", "9:16"],
            Family::Kling => &["16:9", "9:16", "1:1"],
        }
    }

    fn first_frame(self) -> &'static str {
        match self {
            Family::Veo => "image",
            Family::Kling => "start_image",
        }
    }

    fn last_frame(self) -> &'static str {
        match self {
            Family::Veo => "last_frame",
            Family::Kling => "end_image",
        }
    }
}

fn no(feature: &str, param: &str, hint: &str) -> KppError {
    refused(D, PROVIDER, feature, param, hint)
}

fn listed<T: ToString>(items: &[T]) -> String {
    let items: Vec<String> = items.iter().map(ToString::to_string).collect();
    match items.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
        None => String::new(),
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

/// Veo's resolution and aspect ratio for `size`: 720p or 1080p at 16:9 or 9:16.
fn veo_tier_and_ratio(size: &ImageSize) -> Result<(String, &'static str), KppError> {
    size.dimensions()
        .and_then(|(w, h)| {
            let ratio = Family::Veo
                .ratios()
                .iter()
                .copied()
                .find(|r| same_ratio(w, h, r))?;
            [720, 1080]
                .contains(&w.min(h))
                .then(|| (format!("{}p", w.min(h)), ratio))
        })
        .ok_or_else(|| {
            no(
                &format!("the size `{}` on `{VEO}`", size.as_wire()),
                "size",
                "use 720p or 1080p at 16:9 or 9:16, such as 1280x720.",
            )
        })
}

/// Checks that Replicate can make `r`, before anything is sent.
pub(super) fn check(r: &VideoRequest) -> Result<Family, KppError> {
    check_foreign_extras(r, D)?;
    let family = Family::of(&r.model).ok_or_else(|| {
        no(
            &format!("the video model `{}` through Kaeso", r.model),
            "model",
            &format!("use one of {}.", VIDEO_MODELS.join(", ")),
        )
    })?;
    if r.operation == VideoOperation::Remix {
        return Err(no("remixes", "video_id", "create a new video instead."));
    }
    let takes: &[&str] = match family {
        Family::Veo => &[
            "seconds",
            "size",
            "aspect_ratio",
            "resolution",
            "audio",
            "seed",
            "negative_prompt",
        ],
        Family::Kling => &["seconds", "aspect_ratio", "negative_prompt"],
    };
    if family == Family::Kling && r.size.is_some() {
        return Err(no(
            "a pixel size on `kwaivgi/kling-v2.5-turbo-pro`",
            "size",
            "send `aspect_ratio` instead: 16:9, 9:16 or 1:1.",
        ));
    }
    refuse_unset(r, D, PROVIDER, takes)?;
    if r.prompt.as_deref().is_none_or(str::is_empty) {
        return Err(
            KppError::invalid_request(format!("`{}` needs a `prompt`.", r.model))
                .with_param("prompt"),
        );
    }
    // Billing is per second of the video Replicate makes, so the length has to be known.
    let seconds = r.seconds.ok_or_else(|| {
        no(
            "a video without a length",
            "seconds",
            &format!("send `seconds`, {}.", listed(family.seconds())),
        )
    })?;
    if !family.seconds().contains(&seconds) {
        return Err(no(
            &format!("{seconds} seconds on `{}`", r.model),
            "seconds",
            &format!("send {}.", listed(family.seconds())),
        ));
    }
    if let Some(size) = &r.size {
        veo_tier_and_ratio(size)?;
        if r.aspect_ratio.is_some() || r.resolution.is_some() {
            return Err(KppError::invalid_request(
                "Send either `size` or `aspect_ratio` and `resolution`, not both.",
            )
            .with_param("size"));
        }
    }
    if let Some(ratio) = &r.aspect_ratio
        && !family.ratios().contains(&ratio.as_str())
    {
        return Err(no(
            &format!("the aspect ratio `{ratio}` on `{}`", r.model),
            "aspect_ratio",
            &format!("use {}.", listed(family.ratios())),
        ));
    }
    if let Some(res) = &r.resolution
        && !matches!(res.as_str(), "720p" | "1080p")
    {
        return Err(no(
            &format!("the resolution `{res}` on `{}`", r.model),
            "resolution",
            "use 720p or 1080p.",
        ));
    }
    check_images(r, family, seconds)?;
    Ok(family)
}

fn check_images(r: &VideoRequest, family: Family, seconds: u32) -> Result<(), KppError> {
    for image in &r.images {
        if image_link(image).is_none() {
            return Err(no(
                "file ids",
                "input_reference",
                "upload the image or send its link.",
            ));
        }
        if image.role == VideoImageRole::Reference && family == Family::Kling {
            return Err(no(
                "reference images on `kwaivgi/kling-v2.5-turbo-pro`",
                "input_reference",
                "send a first frame and, at most, a last frame.",
            ));
        }
    }
    let count = |role| r.images_for(role).count();
    let (first, last, references) = (
        count(VideoImageRole::FirstFrame),
        count(VideoImageRole::LastFrame),
        count(VideoImageRole::Reference),
    );
    if first > 1 || last > 1 {
        return Err(no(
            "more than one first or last frame",
            "input_reference",
            "send one of each.",
        ));
    }
    if last == 1 && first == 0 {
        return Err(KppError::invalid_request(format!(
            "`{}` ends on a last frame only after a first frame; send both.",
            r.model
        ))
        .with_param("input_reference"));
    }
    if family == Family::Kling && first == 1 && r.aspect_ratio.is_some() {
        return Err(no(
            "an aspect ratio beside a first frame",
            "aspect_ratio",
            "leave it out: the first frame's shape decides the video's.",
        ));
    }
    // Veo's reference images work only at 16:9 and 8 seconds, and replace the last frame
    // (the `reference_images` description).
    if references > 0 {
        let ratio = r
            .size
            .as_ref()
            .and_then(ImageSize::dimensions)
            .map(|(w, h)| if w >= h { "16:9" } else { "9:16" })
            .or(r.aspect_ratio.as_deref());
        if references > 3 || seconds != 8 || ratio.is_some_and(|r| r != "16:9") || last > 0 {
            return Err(no(
                "these reference images",
                "input_reference",
                "send 1 to 3, for 8 seconds at 16:9, without a last frame.",
            ));
        }
    }
    Ok(())
}

/// The input of the model `r` names.
pub(super) fn encode_request(r: &VideoRequest) -> Result<EncodedRequest, KppError> {
    let family = check(r)?;
    let mut input = Map::new();
    put(&mut input, "prompt", r.prompt.clone());
    put(&mut input, "duration", r.seconds);
    if let Some(size) = &r.size {
        let (tier, ratio) = veo_tier_and_ratio(size)?;
        input.insert("resolution".into(), tier.into());
        input.insert("aspect_ratio".into(), ratio.into());
    }
    put(&mut input, "resolution", r.resolution.clone());
    put(&mut input, "aspect_ratio", r.aspect_ratio.clone());
    put(&mut input, "generate_audio", r.audio);
    put(&mut input, "seed", r.seed);
    put(&mut input, "negative_prompt", r.negative_prompt.clone());
    let link = |role| r.images_for(role).next().and_then(image_link);
    put(
        &mut input,
        family.first_frame(),
        link(VideoImageRole::FirstFrame),
    );
    put(
        &mut input,
        family.last_frame(),
        link(VideoImageRole::LastFrame),
    );
    let references: Vec<Value> = r
        .images_for(VideoImageRole::Reference)
        .filter_map(image_link)
        .map(Value::from)
        .collect();
    if !references.is_empty() {
        input.insert("reference_images".into(), Value::Array(references));
    }
    put_extras(&mut input, &r.extras, D, "");
    submit(&r.model, input, false)
}

/// A Replicate video model's input, as the dialect's fixtures write it; the model is in the
/// URL, so each field is read by its name alone.
pub(super) fn decode_request(mut f: Fields) -> Result<Request, KppError> {
    let mut r = VideoRequest::new(String::new(), VideoOperation::Generate);
    r.prompt = f.str("prompt")?;
    r.seconds = f.u32("duration")?;
    let text = |f: &Fields, k: &str| f.peek(k).and_then(Value::as_str).map(str::to_owned);
    if let (Some(tier), Some(ratio)) = (text(&f, "resolution"), text(&f, "aspect_ratio"))
        && let Some(size) = size_of(&tier, &ratio)
    {
        r.size = Some(size);
        f.take("resolution");
        f.take("aspect_ratio");
    }
    r.resolution = f.str("resolution")?;
    r.aspect_ratio = f.str("aspect_ratio")?;
    r.audio = f.bool("generate_audio")?;
    r.seed = f.i64("seed")?;
    r.negative_prompt = f.str("negative_prompt")?;
    for (key, role) in [
        ("image", VideoImageRole::FirstFrame),
        ("start_image", VideoImageRole::FirstFrame),
        ("last_frame", VideoImageRole::LastFrame),
        ("end_image", VideoImageRole::LastFrame),
    ] {
        if let Some(url) = f.str(key)? {
            r.images.push(VideoImage::new(link_source(url), role));
        }
    }
    for url in f.array("reference_images")?.unwrap_or_default() {
        let Value::String(url) = url else {
            return Err(KppError::invalid_request("`reference_images` lists links.")
                .with_param("reference_images"));
        };
        r.images
            .push(VideoImage::new(link_source(url), VideoImageRole::Reference));
    }
    r.extras = f.rest(&Origin::new(D));
    Ok(Request::Video(r))
}

fn status_of(text: &str) -> VideoStatus {
    match text {
        STARTING => VideoStatus::Queued,
        PROCESSING => VideoStatus::InProgress,
        SUCCEEDED => VideoStatus::Completed,
        _ => VideoStatus::Failed,
    }
}

/// The job a prediction is: started, running, finished with its video, or ended without one.
/// `error` stays among the extras as Replicate wrote it, so a job written back keeps it.
pub(super) fn decode_job(body: &[u8], ctx: &EgressCtx) -> Result<VideoJob, KppError> {
    let (v, p) = prediction_of(body)?;
    let mut f = provider_object(body, D)?;
    f.take("id");
    f.take("status");
    f.take("output");
    let mut job = VideoJob::new(p.id.clone(), status_of(&p.status));
    if p.ended_badly() {
        let message = super::error::failed(&v).message;
        job.error = Some(VideoError::new(p.status.clone(), message));
    }
    if job.status == VideoStatus::Completed {
        let link = p
            .links()
            .and_then(|l| l.into_iter().next())
            .ok_or_else(|| {
                KppError::provider(format!("{PROVIDER} finished the video without its link."))
            })?;
        job.outputs
            .push(VideoOutput::new(VideoVariant::Video, link_source(link)));
        job.expires_at = v
            .get("completed_at")
            .and_then(Value::as_str)
            .and_then(unix_of_rfc3339)
            .map(|t| t + KEPT_SECONDS);
    }
    let mut extras = Extras::new(ctx.origin(D));
    f.rest_into(&mut extras, "");
    job.extras = extras;
    Ok(job)
}

/// A job as a prediction, as Replicate writes it.
pub(super) fn encode_job(job: &VideoJob) -> Value {
    let status = match job.status {
        VideoStatus::Queued => STARTING,
        VideoStatus::InProgress => PROCESSING,
        VideoStatus::Completed => SUCCEEDED,
        _ => match job.error.as_ref().map(|e| e.code.as_str()) {
            Some(CANCELED) => CANCELED,
            Some(ABORTED) => ABORTED,
            _ => FAILED,
        },
    };
    let mut out = Map::new();
    out.insert("id".into(), job.id.clone().into());
    out.insert("status".into(), status.into());
    let link = job.outputs.iter().find_map(|o| match &o.source {
        Source::Url(u) => Some(u.clone()),
        Source::Base64(b) => Some(b.data_url()),
        _ => None,
    });
    out.insert("output".into(), link.map_or(Value::Null, Value::from));
    if let Some(e) = &job.error {
        out.insert("error".into(), e.message.clone().into());
    }
    put_extras(&mut out, &job.extras, D, "");
    Value::Object(out)
}
