use kaeso_kpp::{
    ErrorKind, Request, Source, VideoImage, VideoImageRole, VideoOperation, VideoRequest,
    VideoStatus, VideoVariant,
};

use super::*;
use crate::video_jobs::round_trips;

const DIR: &str = "openrouter_videos";

fn encode(r: &VideoRequest) -> Result<EncodedRequest, KppError> {
    let req = Request::Video(r.clone());
    let mut ctx = EgressCtx::new(&req);
    OpenRouterVideos::encode_request(&req, &mut ctx)
}

#[test]
fn every_fixture_round_trips() {
    for (name, enc) in round_trips::requests::<OpenRouterVideos>(DIR) {
        assert_eq!(enc.path, JOBS_PATH, "{name}");
    }
    let started = round_trips::started::<OpenRouterVideos>(DIR);
    assert_eq!(started[0].1.status, VideoStatus::Queued);
    let polls = round_trips::polls::<OpenRouterVideos>(DIR, encode_job);
    let by = |n: &str| polls.iter().find(|(name, _)| name == n).unwrap().1.clone();
    let link = |n: &str| match &by(n).output(VideoVariant::Video).unwrap().source {
        Source::Url(u) => u.clone(),
        other => panic!("{other:?}"),
    };
    assert!(link("completed").starts_with("https://openrouter.ai/api/v1/videos/"));
    assert!(link("completed-signed").starts_with("https://storage.example.com/"));
    assert_eq!(by("completed-signed").expires_at, Some(1_789_567_274));
    assert_eq!(by("in-progress").status, VideoStatus::InProgress);
    assert_eq!(
        by("failed").error.unwrap().message,
        "The provider rejected the prompt."
    );
    assert_eq!(by("expired").error.unwrap().code, "expired");
    let errors = round_trips::errors::<OpenRouterVideos>(DIR);
    let kinds: Vec<_> = errors.iter().map(|(_, e)| e.kind.clone()).collect();
    assert!(kinds.contains(&ErrorKind::Billing));
}

#[test]
fn a_finished_job_without_links_is_fetched_from_its_content_path() {
    let body =
        br#"{"id":"gen-vid-1-a","polling_url":"/api/v1/videos/gen-vid-1-a","status":"completed"}"#;
    let job =
        OpenRouterVideos::decode_video_poll(200, &HeaderList::new(), body, &round_trips::ctx())
            .unwrap();
    assert!(
        matches!(&job.output(VideoVariant::Video).unwrap().source, Source::Url(u) if u == "/v1/videos/gen-vid-1-a/content?index=0")
    );
}

#[test]
fn openrouter_refuses_what_it_lacks_before_sending() {
    let base = VideoRequest::new("google/veo-3.1", VideoOperation::Generate).with_prompt("p");
    let param = |r: &VideoRequest| encode(r).unwrap_err().param.unwrap_or_default();
    let mut neg = base.clone();
    neg.negative_prompt = Some("x".into());
    assert_eq!(param(&neg), "negative_prompt");
    let mut ratio = base.clone();
    ratio.aspect_ratio = Some("5:2".into());
    assert_eq!(param(&ratio), "aspect_ratio");
    let mut res = base.clone();
    res.resolution = Some("8K".into());
    assert_eq!(param(&res), "resolution");
    let mut remix = VideoRequest::new("m", VideoOperation::Remix).with_prompt("p");
    remix.remix_of = Some("j".into());
    assert_eq!(param(&remix), "video_id");
    let file = base.clone().with_image(VideoImage::new(
        Source::file_id(kaeso_kpp::DialectId::OpenAiVideos, "f"),
        VideoImageRole::FirstFrame,
    ));
    assert_eq!(param(&file), "input_reference");
    let both = br#"{"model":"m","prompt":"p","input_references":[{"type":"audio_url","audio_url":{"url":"a"}}]}"#;
    let Request::Video(mut r) =
        OpenRouterVideos::decode_request(RequestKind::Video, both, &HeaderList::new()).unwrap()
    else {
        panic!()
    };
    r.images.push(VideoImage::new(
        Source::Url("u".into()),
        VideoImageRole::Reference,
    ));
    assert_eq!(param(&r), "input_references");
    let ctx = round_trips::ctx();
    assert!(
        OpenRouterVideos::decode_video_poll(
            200,
            &HeaderList::new(),
            br#"{"id":"x","status":"later"}"#,
            &ctx
        )
        .is_err()
    );
    assert!(
        OpenRouterVideos::decode_response(
            200,
            &HeaderList::new(),
            br#"{"status":"pending"}"#,
            &ctx
        )
        .is_err()
    );
    assert!(
        OpenRouterVideos::decode_request(
            RequestKind::Video,
            br#"{"frame_images":[{"image_url":{"url":"u"}}]}"#,
            &HeaderList::new()
        )
        .is_err()
    );
    assert!(
        OpenRouterVideos::encode_video_delete("j", "m", &ctx)
            .unwrap()
            .is_none()
    );
}
