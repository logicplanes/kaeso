use kaeso_kpp::{
    ErrorKind, ImageSize, Request, Source, VideoImage, VideoImageRole, VideoOperation,
    VideoRequest, VideoStatus, VideoVariant,
};

use super::*;
use crate::video_jobs::round_trips;

const DIR: &str = "gemini_interactions";

fn encode(r: &VideoRequest) -> Result<EncodedRequest, KppError> {
    let req = Request::Video(r.clone());
    let mut ctx = EgressCtx::new(&req);
    GeminiInteractions::encode_request(&req, &mut ctx)
}

#[test]
fn every_fixture_round_trips() {
    for (name, enc) in round_trips::requests::<GeminiInteractions>(DIR) {
        assert_eq!(enc.path, INTERACTIONS_PATH, "{name}");
    }
    let started = round_trips::started::<GeminiInteractions>(DIR);
    assert!(
        started
            .iter()
            .all(|(_, j)| j.status == VideoStatus::InProgress)
    );
    let recorded = &started
        .iter()
        .find(|(n, _)| n == "recorded-started")
        .unwrap()
        .1;
    assert_eq!(recorded.created_at, Some(1_794_181_628));
    let polls = round_trips::polls::<GeminiInteractions>(DIR, encode_interaction);
    let by = |n: &str| polls.iter().find(|(name, _)| name == n).unwrap().1.clone();
    let done = by("completed");
    assert_eq!(done.status, VideoStatus::Completed);
    assert!(
        matches!(&done.output(VideoVariant::Video).unwrap().source, Source::Base64(b) if b.data == "AAAAIGZ0eXBpc29t")
    );
    let usage = done.usage.unwrap();
    assert_eq!(
        (
            usage.input_tokens,
            usage.output_tokens,
            usage.reasoning_tokens
        ),
        (26, 29_285, Some(453))
    );
    assert_eq!(done.completed_at, Some(1_780_021_180));
    assert!(
        matches!(&by("completed-uri").outputs[0].source, Source::Url(u) if u.contains("files/abc123"))
    );
    let quota = by("recorded-failed-quota");
    assert_eq!(quota.status, VideoStatus::Failed);
    assert_eq!(quota.error.unwrap().code, "RESOURCE_EXHAUSTED");
    assert_eq!(by("failed").error.unwrap().code, "INVALID_ARGUMENT");
    assert_eq!(by("in-progress").completed_at, None);
    let errors = round_trips::errors::<GeminiInteractions>(DIR);
    assert_eq!(errors[0].1.kind, ErrorKind::RateLimited);
}

#[test]
fn apps_requests_go_out_in_the_background_with_the_video_format() {
    let mut r =
        VideoRequest::new("gemini-omni-1.2-flash", VideoOperation::Generate).with_prompt("A fox");
    r.size = Some(ImageSize::pixels(1280, 720));
    r.seconds = Some(8);
    r.audio = Some(true);
    let body: Value = serde_json::from_slice(&encode(&r).unwrap().body).unwrap();
    assert_eq!(
        body,
        json!({
            "model": "gemini-omni-1.2-flash",
            "input": "A fox",
            "response_format": {"type": "video", "aspect_ratio": "16:9", "resolution": "720p", "duration": "8s"},
            "background": true,
        })
    );
    let frames = VideoRequest::new("gemini-omni-1.2-flash", VideoOperation::Generate)
        .with_prompt("p")
        .with_image(VideoImage::new(
            Source::base64("image/png", "AAAA"),
            VideoImageRole::FirstFrame,
        ));
    let body: Value = serde_json::from_slice(&encode(&frames).unwrap().body).unwrap();
    assert_eq!(
        body["generation_config"]["video_config"]["task"],
        "image_to_video"
    );
    assert_eq!(body["input"][1]["text"], "p");
    let refs = VideoRequest::new("gemini-omni-1.2-flash", VideoOperation::Generate)
        .with_prompt("p")
        .with_image(VideoImage::new(
            Source::Url("https://example.com/a.png".into()),
            VideoImageRole::Reference,
        ));
    let body: Value = serde_json::from_slice(&encode(&refs).unwrap().body).unwrap();
    assert_eq!(
        body["generation_config"]["video_config"]["task"],
        "reference_to_video"
    );
    let mut remix =
        VideoRequest::new("gemini-omni-1.2-flash", VideoOperation::Remix).with_prompt("snow");
    remix.remix_of = Some("v1_prev".into());
    let body: Value = serde_json::from_slice(&encode(&remix).unwrap().body).unwrap();
    assert_eq!(body["previous_interaction_id"], "v1_prev");
    let ctx = round_trips::ctx();
    assert_eq!(
        GeminiInteractions::encode_video_poll("v1_a", "m", &ctx)
            .unwrap()
            .path,
        "/v1beta/interactions/v1_a"
    );
    let del = GeminiInteractions::encode_video_delete("v1_a", "m", &ctx)
        .unwrap()
        .unwrap();
    assert_eq!(del.method, "DELETE");
}

#[test]
fn omni_refuses_what_it_lacks_before_sending() {
    let base =
        VideoRequest::new("gemini-omni-1.2-flash", VideoOperation::Generate).with_prompt("p");
    let param = |r: &VideoRequest| {
        let err = encode(r).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
        err.param.unwrap_or_default()
    };
    let mut silent = base.clone();
    silent.audio = Some(false);
    assert_eq!(param(&silent), "audio");
    let mut neg = base.clone();
    neg.negative_prompt = Some("x".into());
    assert_eq!(param(&neg), "negative_prompt");
    let mut s = base.clone();
    s.seconds = Some(20);
    assert_eq!(param(&s), "seconds");
    let mut size = base.clone();
    size.size = Some(ImageSize::pixels(1024, 1792));
    assert_eq!(param(&size), "size");
    let mixed = base
        .clone()
        .with_image(VideoImage::new(
            Source::Url("a".into()),
            VideoImageRole::FirstFrame,
        ))
        .with_image(VideoImage::new(
            Source::Url("b".into()),
            VideoImageRole::Reference,
        ));
    assert_eq!(param(&mixed), "input_reference");
    let mut long_frames = base
        .with_image(VideoImage::new(
            Source::Url("a".into()),
            VideoImageRole::FirstFrame,
        ))
        .with_image(VideoImage::new(
            Source::Url("b".into()),
            VideoImageRole::LastFrame,
        ));
    long_frames.seconds = Some(12);
    assert_eq!(param(&long_frames), "seconds");
    let ctx = round_trips::ctx();
    assert!(
        GeminiInteractions::decode_video_poll(
            200,
            &HeaderList::new(),
            br#"{"id":"x","status":"paused"}"#,
            &ctx
        )
        .is_err()
    );
    assert!(
        GeminiInteractions::decode_request(
            RequestKind::Video,
            br#"{"input":[{"type":"audio"}]}"#,
            &HeaderList::new()
        )
        .is_err()
    );
    assert!(
        GeminiInteractions::decode_request(
            RequestKind::Video,
            br#"{"response_format":{"type":"text"}}"#,
            &HeaderList::new()
        )
        .is_err()
    );
    assert!(
        GeminiInteractions::decode_request(
            RequestKind::Video,
            br#"{"response_format":{"duration":"ten"}}"#,
            &HeaderList::new()
        )
        .is_err()
    );
}

#[test]
fn the_agent_platform_serves_interactions_under_its_global_location_without_deletes() {
    let base = "/v1beta1/projects/kaeso-example/locations/global";
    let req = Request::Video(
        VideoRequest::new("gemini-omni-1.2-flash", VideoOperation::Generate)
            .with_prompt("A marble rolling"),
    );
    let mut ctx = EgressCtx::new(&req).with_base_path(base);
    let enc = GeminiInteractions::encode_request(&req, &mut ctx).unwrap();
    assert_eq!(enc.path, format!("{base}/interactions"));
    let poll = GeminiInteractions::encode_video_poll("v1_abc", "m", &ctx).unwrap();
    assert_eq!(poll.path, format!("{base}/interactions/v1_abc"));
    assert!(
        GeminiInteractions::encode_video_delete("v1_abc", "m", &ctx)
            .unwrap()
            .is_none()
    );
    let studio = EgressCtx::new(&req);
    let delete = GeminiInteractions::encode_video_delete("v1_abc", "m", &studio).unwrap();
    assert_eq!(delete.unwrap().path, "/v1beta/interactions/v1_abc");
}
