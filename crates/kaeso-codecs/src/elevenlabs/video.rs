//! ElevenLabs' video generations (`POST /v1/flows/video`, `GET /v1/flows/video/{id}`), whose
//! body depends on `model_id`: Veo 3.1 and the Seedance 2 models. Only what every one of them
//! documents with the same meaning is mapped.

use kaeso_kpp::{
    Extras, KppError, Origin, Request, Source, VideoError, VideoImage, VideoImageRole, VideoJob,
    VideoOperation, VideoOutput, VideoRequest, VideoStatus, VideoVariant,
};
use serde_json::{Map, Value, json};

use super::D;
use crate::codec::{EgressCtx, EncodedRequest, HeaderList};
use crate::video_jobs::{
    Fields, body, check_foreign_extras, check_roles, provider_object, put, put_extras,
    refuse_unset, refused, video_request,
};

/// The path of video generations; a generation's own path adds its id.
pub const VIDEO_PATH: &str = "/v1/flows/video";

const PROVIDER: &str = "ElevenLabs";

type Limits = (
    &'static [&'static str],
    &'static [&'static str],
    Option<&'static [u32]>,
);

/// The aspect ratios, resolutions and lengths each model documents.
fn model_limits(model: &str) -> Option<Limits> {
    const VEO_RATIOS: &[&str] = &["16:9", "9:16"];
    const SEEDANCE_RATIOS: &[&str] = &["auto", "21:9", "16:9", "4:3", "1:1", "3:4", "9:16"];
    Some(match model {
        "veo-3.1-generate-001" | "veo-3.1-fast-generate-001" => {
            (VEO_RATIOS, &["720p", "1080p", "4K"], Some(&[4, 6, 8]))
        }
        "bytedance-seedance-v2" => (SEEDANCE_RATIOS, &["480p", "720p", "1080p", "4k"], None),
        "bytedance-seedance-v2.5" => (SEEDANCE_RATIOS, &["480p", "720p", "1080p"], None),
        "bytedance-seedance-v2-mini" | "bytedance-seedance-v2-fast" => {
            (SEEDANCE_RATIOS, &["480p", "720p"], None)
        }
        _ => return None,
    })
}

fn is_veo(model: &str) -> bool {
    model.starts_with("veo-")
}

fn frame(v: Value, role: VideoImageRole) -> Result<VideoImage, KppError> {
    let Value::Object(map) = v else {
        return Err(
            KppError::invalid_request("A frame is an image reference object.")
                .with_param("start_frame"),
        );
    };
    let mut f = Fields::from_map(map, D, "request");
    let source = match f.str("type")?.as_deref() {
        Some("inline_base64") => crate::media::from_base64(
            f.str("mime_type")?.unwrap_or_default(),
            f.str("content_base64")?.unwrap_or_default(),
            "content_base64",
        )?,
        Some("asset") => Source::file_id(Origin::new(D), f.str("asset_id")?.unwrap_or_default()),
        _ => {
            return Err(KppError::unsupported_by(
                "an image reference other than an upload or an asset",
                D,
                "Send the image inline.",
            )
            .with_param("start_frame"));
        }
    };
    let mut image = VideoImage::new(source, role);
    image.extras = f.rest(&Origin::new(D));
    Ok(image)
}

fn frame_value(image: &VideoImage) -> Value {
    let mut out = Map::new();
    match &image.source {
        Source::Base64(b) => {
            out.insert("type".into(), "inline_base64".into());
            out.insert("content_base64".into(), b.to_base64().into());
            out.insert("mime_type".into(), b.media_type.clone().into());
        }
        Source::FileId(f) => {
            out.insert("type".into(), "asset".into());
            out.insert("asset_id".into(), f.id.clone().into());
        }
        _ => {}
    }
    put_extras(&mut out, &image.extras, D, "");
    Value::Object(out)
}

pub(super) fn decode(body: &[u8]) -> Result<Request, KppError> {
    let mut f = Fields::parse(body, D, "request")?;
    let mut r = VideoRequest::new(
        f.str("model_id")?.unwrap_or_default(),
        VideoOperation::Generate,
    );
    r.prompt = f.str("prompt")?;
    r.negative_prompt = f.str("negative_prompt")?;
    r.seed = f.i64("seed")?;
    r.seconds = f.u32("duration_secs")?;
    r.aspect_ratio = f.str("aspect_ratio")?;
    r.resolution = f.str("resolution")?;
    r.audio = f.bool("generate_audio")?;
    if let Some(v) = f.take("start_frame") {
        r.images.push(frame(v, VideoImageRole::FirstFrame)?);
    }
    if let Some(v) = f.take("end_frame") {
        r.images.push(frame(v, VideoImageRole::LastFrame)?);
    }
    if is_veo(&r.model) {
        for item in f.array("images")?.unwrap_or_default() {
            let Value::Object(map) = item else { continue };
            let mut i = Fields::from_map(map, D, "request");
            let role = i.str("role")?;
            let image = i.take("image").ok_or_else(|| {
                KppError::invalid_request("A reference needs its `image`.").with_param("images")
            })?;
            let mut image = frame(image, VideoImageRole::Reference)?;
            if role.as_deref() != Some("subject") {
                put_role(&mut image.extras, role);
            }
            r.images.push(image);
        }
    }
    r.extras = f.rest(&Origin::new(D));
    Ok(Request::Video(r))
}

fn put_role(extras: &mut Extras, role: Option<String>) {
    if let Some(role) = role {
        extras.form.insert("role".into(), role.into());
    }
}

pub(super) fn check(r: &VideoRequest) -> Result<(), KppError> {
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
    let Some((ratios, resolutions, durations)) = model_limits(&r.model) else {
        return Err(refused(
            D,
            PROVIDER,
            &format!("the video model `{}`", r.model),
            "model",
            "choose a Veo 3.1 or Seedance 2 model.",
        ));
    };
    let mut takes = vec!["seconds", "aspect_ratio", "resolution", "seed", "audio"];
    if is_veo(&r.model) {
        takes.push("negative_prompt");
    }
    refuse_unset(r, D, PROVIDER, &takes)?;
    let roles: &[VideoImageRole] = if is_veo(&r.model) {
        &[
            VideoImageRole::FirstFrame,
            VideoImageRole::LastFrame,
            VideoImageRole::Reference,
        ]
    } else {
        &[VideoImageRole::FirstFrame, VideoImageRole::LastFrame]
    };
    check_roles(r, D, PROVIDER, roles)?;
    for image in &r.images {
        let usable = match &image.source {
            Source::Base64(_) => true,
            Source::FileId(f) => f.origin.is(D),
            _ => false,
        };
        if !usable {
            return Err(refused(
                D,
                PROVIDER,
                "images by link",
                "input_reference",
                "upload the image instead.",
            ));
        }
    }
    if let (Some(s), Some(allowed)) = (r.seconds, durations)
        && !allowed.contains(&s)
    {
        return Err(refused(
            D,
            PROVIDER,
            &format!("{s} seconds"),
            "seconds",
            "choose 4, 6 or 8 seconds.",
        ));
    }
    if let Some(a) = &r.aspect_ratio
        && !ratios.contains(&a.as_str())
    {
        return Err(refused(
            D,
            PROVIDER,
            &format!("the aspect ratio `{a}`"),
            "aspect_ratio",
            "use a ratio the model lists.",
        ));
    }
    if let Some(res) = &r.resolution
        && !resolutions.contains(&res.as_str())
    {
        return Err(refused(
            D,
            PROVIDER,
            &format!("the resolution `{res}`"),
            "resolution",
            "use a resolution the model lists.",
        ));
    }
    Ok(())
}

pub(super) fn encode(req: &Request) -> Result<EncodedRequest, KppError> {
    let r = video_request(req, D)?;
    check(r)?;
    let mut out = Map::new();
    out.insert("model_id".into(), r.model.clone().into());
    put(&mut out, "prompt", r.prompt.clone());
    put(&mut out, "negative_prompt", r.negative_prompt.clone());
    put(&mut out, "seed", r.seed);
    put(&mut out, "duration_secs", r.seconds);
    put(&mut out, "aspect_ratio", r.aspect_ratio.clone());
    put(&mut out, "resolution", r.resolution.clone());
    put(&mut out, "generate_audio", r.audio);
    if let Some(first) = r.images_for(VideoImageRole::FirstFrame).next() {
        out.insert("start_frame".into(), frame_value(first));
    }
    if let Some(last) = r.images_for(VideoImageRole::LastFrame).next() {
        out.insert("end_frame".into(), frame_value(last));
    }
    let references: Vec<Value> = r
        .images_for(VideoImageRole::Reference)
        .map(|i| {
            let role = i.extras.form_str(D, "role").unwrap_or("subject");
            json!({ "image": frame_value(i), "role": role })
        })
        .collect();
    if !references.is_empty() {
        out.insert("images".into(), Value::Array(references));
    }
    put_extras(&mut out, &r.extras, D, "");
    let headers = HeaderList::new().with("content-type", crate::codec::JSON_MEDIA_TYPE);
    Ok(EncodedRequest::post_json(VIDEO_PATH, headers, body(&out)?))
}

pub(super) fn decode_job(body: &[u8], ctx: &EgressCtx) -> Result<VideoJob, KppError> {
    let mut f = provider_object(body, D)?;
    let mut extras = Extras::new(ctx.origin(D));
    let id = f.str("id")?.unwrap_or_default();
    let status_text = f.str("status")?.unwrap_or_default();
    let status = match status_text.as_str() {
        "pending" => VideoStatus::Queued,
        "generating" => VideoStatus::InProgress,
        "completed" => VideoStatus::Completed,
        "failed" => VideoStatus::Failed,
        other => {
            return Err(KppError::provider(format!(
                "ElevenLabs sent the unknown generation status `{other}`."
            )));
        }
    };
    let mut job = VideoJob::new(id, status);
    if let Some(url) = f.str("content_url")? {
        let mut output = VideoOutput::new(VideoVariant::Video, Source::Url(url));
        output.media_type = f.str("content_mime_type")?;
        job.outputs.push(output);
    }
    if status == VideoStatus::Failed {
        job.error = Some(VideoError::new(
            f.str("failure_reason")?.unwrap_or_default(),
            f.str("error_message")?.unwrap_or_default(),
        ));
    }
    f.rest_into(&mut extras, "");
    job.extras = extras;
    Ok(job)
}

pub(crate) fn encode_job(job: &VideoJob) -> Value {
    let mut out = Map::new();
    out.insert("id".into(), job.id.clone().into());
    let status = match job.status {
        VideoStatus::Queued => "pending",
        VideoStatus::InProgress => "generating",
        VideoStatus::Completed => "completed",
        _ => "failed",
    };
    out.insert("status".into(), status.into());
    if let Some(VideoOutput {
        source: Source::Url(u),
        media_type,
        ..
    }) = job.output(VideoVariant::Video)
    {
        out.insert("content_url".into(), u.clone().into());
        put(&mut out, "content_mime_type", media_type.clone());
    }
    if let Some(e) = &job.error {
        out.insert("failure_reason".into(), e.code.clone().into());
        out.insert("error_message".into(), e.message.clone().into());
    }
    put_extras(&mut out, &job.extras, D, "");
    Value::Object(out)
}

pub(super) fn poll_request(job: &str) -> EncodedRequest {
    EncodedRequest::get(
        format!("{VIDEO_PATH}/{}", crate::video_jobs::path_segment(job)),
        HeaderList::new(),
    )
}

#[cfg(test)]
mod tests {
    use kaeso_kpp::{
        ErrorKind, Request, RequestKind, Source, VideoImage, VideoImageRole, VideoOperation,
        VideoRequest, VideoStatus, VideoVariant,
    };

    use super::*;
    use crate::codec::Codec;
    use crate::elevenlabs::ElevenLabs;
    use crate::video_jobs::round_trips;

    const DIR: &str = "elevenlabs/video";

    fn encode_r(r: &VideoRequest) -> Result<EncodedRequest, KppError> {
        let req = Request::Video(r.clone());
        let mut ctx = EgressCtx::new(&req);
        ElevenLabs::encode_request(&req, &mut ctx)
    }

    #[test]
    fn every_fixture_round_trips() {
        for (name, enc) in round_trips::requests::<ElevenLabs>(DIR) {
            assert_eq!(enc.path, VIDEO_PATH, "{name}");
        }
        let started = round_trips::started::<ElevenLabs>(DIR);
        assert_eq!(started[0].1.status, VideoStatus::Queued);
        let polls = round_trips::polls::<ElevenLabs>(DIR, encode_job);
        let by = |n: &str| polls.iter().find(|(name, _)| name == n).unwrap().1.clone();
        assert_eq!(by("generating").status, VideoStatus::InProgress);
        let done = by("completed");
        assert_eq!(
            done.output(VideoVariant::Video)
                .unwrap()
                .media_type
                .as_deref(),
            Some("video/mp4")
        );
        assert_eq!(by("failed").error.unwrap().code, "timeout");
        let errors = round_trips::errors::<ElevenLabs>(DIR);
        assert!(!errors[0].1.message.is_empty());
        assert_eq!(
            ElevenLabs::encode_video_poll("a", "m", &round_trips::ctx())
                .unwrap()
                .path,
            "/v1/flows/video/a"
        );
    }

    #[test]
    fn elevenlabs_refuses_what_its_models_lack() {
        let base = VideoRequest::new("bytedance-seedance-v2-mini", VideoOperation::Generate)
            .with_prompt("p");
        let param = |r: &VideoRequest| {
            let err = encode_r(r).unwrap_err();
            assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
            err.param.unwrap_or_default()
        };
        let mut neg = base.clone();
        neg.negative_prompt = Some("x".into());
        assert_eq!(param(&neg), "negative_prompt");
        let mut res = base.clone();
        res.resolution = Some("1080p".into());
        assert_eq!(param(&res), "resolution");
        let refs = base.clone().with_image(VideoImage::new(
            Source::base64("image/png", "AAAA").unwrap(),
            VideoImageRole::Reference,
        ));
        assert_eq!(param(&refs), "input_reference");
        let link = base.clone().with_image(VideoImage::new(
            Source::Url("https://example.com/a.png".into()),
            VideoImageRole::FirstFrame,
        ));
        assert_eq!(param(&link), "input_reference");
        let mut unknown = base.clone();
        unknown.model = "creatify-aurora".into();
        assert_eq!(param(&unknown), "model");
        let mut veo =
            VideoRequest::new("veo-3.1-generate-001", VideoOperation::Generate).with_prompt("p");
        veo.seconds = Some(5);
        assert_eq!(param(&veo), "seconds");
        let mut size = base;
        size.size = Some(kaeso_kpp::ImageSize::pixels(1280, 720));
        assert_eq!(param(&size), "size");
        assert!(
            ElevenLabs::decode_video_poll(
                200,
                &HeaderList::new(),
                br#"{"id":"x","status":"queued"}"#,
                &round_trips::ctx()
            )
            .is_err()
        );
        assert!(ElevenLabs::decode_request(RequestKind::Video, br#"{"model_id":"veo-3.1-generate-001","start_frame":{"type":"generation","generation_id":"g"}}"#, &HeaderList::new()).is_err());
    }
}
