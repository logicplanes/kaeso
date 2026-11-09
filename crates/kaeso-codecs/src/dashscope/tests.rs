use kaeso_kpp::{
    ErrorKind, ImageSize, Request, Source, VideoImage, VideoImageRole, VideoOperation,
    VideoRequest, VideoStatus, VideoVariant,
};

use super::*;
use crate::video_jobs::round_trips;

const DIR: &str = "dashscope";

fn encode(r: &VideoRequest) -> Result<EncodedRequest, KppError> {
    let req = Request::Video(r.clone());
    let mut ctx = EgressCtx::new(&req);
    DashScope::encode_request(&req, &mut ctx)
}

fn sent(r: &VideoRequest) -> Value {
    serde_json::from_slice(&encode(r).unwrap().body).unwrap()
}

fn param(r: &VideoRequest) -> String {
    encode(r).unwrap_err().param.unwrap_or_default()
}

fn link(url: &str, role: VideoImageRole) -> VideoImage {
    VideoImage::new(Source::Url(url.into()), role)
}

#[test]
fn every_fixture_round_trips() {
    for (name, enc) in round_trips::requests::<DashScope>(DIR) {
        let path = if name.contains("wan22-first-last") {
            KEYFRAMES_PATH
        } else {
            SYNTHESIS_PATH
        };
        assert_eq!(enc.path, path, "{name}");
        assert_eq!(
            enc.headers.get("x-dashscope-async"),
            Some("enable"),
            "{name}"
        );
    }
    let started = round_trips::started::<DashScope>(DIR);
    assert_eq!(started[0].1.status, VideoStatus::Queued);
    assert!(started[0].1.id.starts_with("0385dc79"));
    let polls = round_trips::polls::<DashScope>(DIR, encode_job);
    let by = |n: &str| polls.iter().find(|(name, _)| name == n).unwrap().1.clone();
    let done = by("succeeded");
    assert_eq!(done.status, VideoStatus::Completed);
    assert!(
        matches!(&done.output(VideoVariant::Video).unwrap().source, Source::Url(u) if u.contains(".mp4"))
    );
    assert_eq!(
        (
            done.seconds,
            done.resolution.as_deref(),
            done.aspect_ratio.as_deref()
        ),
        (Some(10), Some("720P"), Some("16:9"))
    );
    assert_eq!(done.created_at, Some(1_759_126_732));
    assert_eq!(done.completed_at, Some(1_759_127_019));
    assert_eq!(done.expires_at, Some(1_759_127_019 + 86_400));
    let failed = by("failed");
    assert_eq!(failed.error.unwrap().code, "InvalidParameter");
    let unknown = by("unknown");
    assert_eq!(unknown.status, VideoStatus::Failed);
    assert_eq!(unknown.error.unwrap().code, "UNKNOWN");
    assert_eq!(by("running").status, VideoStatus::InProgress);
    let errors = round_trips::errors::<DashScope>(DIR);
    let kind = |n: &str| {
        errors
            .iter()
            .find(|(name, _)| name == n)
            .unwrap()
            .1
            .kind
            .clone()
    };
    assert_eq!(kind("invalid-api-key-401"), ErrorKind::Authentication);
    assert_eq!(kind("rate-quota-429"), ErrorKind::RateLimited);
}

#[test]
fn the_model_picks_the_family_and_its_body() {
    for (model, family_) in [
        ("wan3.0-video-prime", Some(Family::Wan30)),
        ("wan2.7-t2v-2026-04-25", Some(Family::Wan27Text)),
        ("wan2.7-i2v", Some(Family::Wan27Image)),
        ("wan2.1-kf2v-plus", Some(Family::Keyframes)),
        ("wanx2.1-t2v-turbo", Some(Family::TextToVideo)),
        ("wan2.5-i2v-preview", Some(Family::FirstFrame)),
        ("wan2.7-r2v", None),
        ("wan2.7-videoedit", None),
        ("wan2.2-animate-mix", None),
    ] {
        assert_eq!(family(model), family_, "{model}");
    }
    let mut r = VideoRequest::new("wan2.6-t2v", VideoOperation::Generate).with_prompt("p");
    r.size = Some(ImageSize::pixels(1920, 1080));
    r.seconds = Some(5);
    assert_eq!(
        sent(&r)["parameters"],
        json_of(r#"{"size":"1920*1080","duration":5}"#)
    );
    let mut r = VideoRequest::new("wan2.7-t2v", VideoOperation::Generate).with_prompt("p");
    r.size = Some(ImageSize::pixels(1104, 832));
    assert_eq!(
        sent(&r)["parameters"],
        json_of(r#"{"resolution":"720P","ratio":"4:3"}"#)
    );
    r.size = Some(ImageSize::pixels(1024, 768));
    assert_eq!(param(&r), "size");
    let mut r = VideoRequest::new("wan2.6-i2v", VideoOperation::Generate)
        .with_prompt("p")
        .with_image(link(
            "https://example.com/a.png",
            VideoImageRole::FirstFrame,
        ));
    r.resolution = Some("1080p".into());
    let body = sent(&r);
    assert_eq!(body["input"]["img_url"], "https://example.com/a.png");
    assert_eq!(body["parameters"]["resolution"], "1080P");
    let r = VideoRequest::new("wan3.0-video", VideoOperation::Generate)
        .with_prompt("p")
        .with_image(link(
            "https://example.com/a.png",
            VideoImageRole::FirstFrame,
        ))
        .with_image(VideoImage::new(
            Source::base64("image/png", "iVBORw0KGgo=").unwrap(),
            VideoImageRole::LastFrame,
        ));
    let body = sent(&r);
    assert_eq!(body["input"]["media"][0]["type"], "first_frame");
    assert_eq!(
        body["input"]["media"][1]["url"],
        "data:image/png;base64,iVBORw0KGgo="
    );
}

fn json_of(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}

#[test]
fn qwencloud_refuses_what_a_model_lacks_before_sending() {
    let text = VideoRequest::new("wan2.6-t2v", VideoOperation::Generate).with_prompt("p");
    let mut s = text.clone();
    s.audio = Some(true);
    assert_eq!(param(&s), "audio");
    let mut s = text.clone();
    s.resolution = Some("720p".into());
    assert_eq!(param(&s), "resolution");
    let s = text.clone().with_image(link(
        "https://example.com/a.png",
        VideoImageRole::FirstFrame,
    ));
    assert_eq!(param(&s), "input_reference");
    let mut s = text.clone();
    s.operation = VideoOperation::Remix;
    assert_eq!(param(&s), "video_id");

    let i2v = VideoRequest::new("wan2.6-i2v", VideoOperation::Generate).with_prompt("p");
    assert_eq!(param(&i2v), "input_reference");
    let mut s = i2v.clone().with_image(link(
        "https://example.com/a.png",
        VideoImageRole::FirstFrame,
    ));
    s.size = Some(ImageSize::pixels(1280, 720));
    assert_eq!(param(&s), "size");
    let mut s = i2v.clone().with_image(link(
        "https://example.com/a.png",
        VideoImageRole::FirstFrame,
    ));
    s.audio = Some(false);
    assert_eq!(param(&s), "audio");

    let kf = VideoRequest::new("wan2.1-kf2v-plus", VideoOperation::Generate)
        .with_prompt("p")
        .with_image(link(
            "https://example.com/a.png",
            VideoImageRole::FirstFrame,
        ));
    assert_eq!(param(&kf), "input_reference");
    let s = kf.clone().with_image(VideoImage::new(
        Source::base64("image/png", "iVBORw0KGgo=").unwrap(),
        VideoImageRole::LastFrame,
    ));
    assert_eq!(param(&s), "input_reference");

    let wan30 = VideoRequest::new("wan3.0-video", VideoOperation::Generate)
        .with_prompt("p")
        .with_image(link(
            "https://example.com/a.png",
            VideoImageRole::FirstFrame,
        ))
        .with_image(link("https://example.com/b.png", VideoImageRole::Reference));
    assert_eq!(param(&wan30), "input_reference");
    let mut s = VideoRequest::new("wan3.0-video", VideoOperation::Generate).with_prompt("p");
    s.negative_prompt = Some("blur".into());
    assert_eq!(param(&s), "negative_prompt");

    let r2v = VideoRequest::new("wan2.7-r2v", VideoOperation::Generate).with_prompt("p");
    assert_eq!(param(&r2v), "model");
}

#[test]
fn decoding_refuses_media_kaeso_cannot_carry_and_odd_replies() {
    let body = br#"{"model":"wan2.7-i2v","input":{"media":[{"type":"driving_audio","url":"https://example.com/a.mp3"}]}}"#;
    let err = DashScope::decode_request(RequestKind::Video, body, &HeaderList::new()).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("media"));
    let body = br#"{"model":"wan2.7-i2v","input":{"media":["https://example.com/a.png"]}}"#;
    assert!(DashScope::decode_request(RequestKind::Video, body, &HeaderList::new()).is_err());
    assert!(DashScope::decode_request(RequestKind::Generate, b"{}", &HeaderList::new()).is_err());
    let ctx = round_trips::ctx();
    let unknown = br#"{"output":{"task_id":"t","task_status":"PAUSED"}}"#;
    assert!(DashScope::decode_video_poll(200, &HeaderList::new(), unknown, &ctx).is_err());
    assert!(DashScope::decode_video_poll(200, &HeaderList::new(), b"{}", &ctx).is_err());
    let unnamed = br#"{"output":{"task_status":"PENDING"}}"#;
    assert!(DashScope::decode_response(200, &HeaderList::new(), unnamed, &ctx).is_err());
    let canceled = br#"{"output":{"task_id":"t","task_status":"CANCELED"}}"#;
    let job = DashScope::decode_video_poll(200, &HeaderList::new(), canceled, &ctx).unwrap();
    assert_eq!(job.error.as_ref().unwrap().code, "CANCELED");
    assert_eq!(
        encode_job(&job),
        json_of(std::str::from_utf8(canceled).unwrap())
    );
    let plain = DashScope::decode_error(502, &HeaderList::new(), b"upstream failed");
    assert_eq!(plain.kind, ErrorKind::ProviderError);
    let filtered = DashScope::decode_error(
        400,
        &HeaderList::new(),
        br#"{"code":"DataInspectionFailed","message":"Input data may contain inappropriate content."}"#,
    );
    assert_eq!(filtered.kind, ErrorKind::ContentFiltered);
    let poll = DashScope::encode_video_poll("a/b", "wan2.7-t2v", &ctx).unwrap();
    assert_eq!(poll.path, "/api/v1/tasks/a%2Fb");
    assert!(
        DashScope::encode_video_delete("t", "wan2.7-t2v", &ctx)
            .unwrap()
            .is_none()
    );
}

#[test]
fn beijing_times_read_as_unix_seconds() {
    assert_eq!(
        unix_of_beijing("2025-09-29 14:18:52.331"),
        Some(1_759_126_732)
    );
    assert_eq!(unix_of_beijing("2025-09-29"), None);
}
