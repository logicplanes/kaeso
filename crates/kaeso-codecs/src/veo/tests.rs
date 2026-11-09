use kaeso_kpp::{
    ErrorKind, ImageSize, Request, Source, VideoImage, VideoImageRole, VideoOperation,
    VideoRequest, VideoStatus, VideoVariant,
};

use super::*;
use crate::gemini::GeminiProfile;
use crate::video_jobs::round_trips;

const DIR: &str = "veo";

fn platform() -> EgressCtx {
    round_trips::ctx().with_gemini_profile(GeminiProfile::AGENT_PLATFORM)
}

fn encode(r: &VideoRequest) -> Result<EncodedRequest, KppError> {
    let req = Request::Video(r.clone());
    let mut ctx = platform();
    Veo::encode_request(&req, &mut ctx)
}

#[test]
fn every_fixture_round_trips() {
    for (name, enc) in round_trips::requests_with::<Veo>(DIR, &platform()) {
        assert_eq!(
            enc.path, "/v1/publishers/google/models/:predictLongRunning",
            "{name}"
        );
    }
    let started = round_trips::started::<Veo>(DIR);
    assert!(
        started[0]
            .1
            .id
            .ends_with("operations/a1b07c8e-7b5a-4aba-bb34-3e1ccb8afcc8")
    );
    let polls = round_trips::polls::<Veo>(DIR, encode_operation);
    let by = |n: &str| polls.iter().find(|(name, _)| name == n).unwrap().1.clone();
    assert_eq!(by("running").status, VideoStatus::InProgress);
    assert_eq!(by("running-no-done").status, VideoStatus::InProgress);
    let done = by("done");
    assert_eq!(done.status, VideoStatus::Completed);
    assert!(
        matches!(&done.output(VideoVariant::Video).unwrap().source, Source::Base64(b) if b.media_type == "video/mp4")
    );
    assert!(
        matches!(&by("done-storage").outputs[0].source, Source::Url(u) if u.starts_with("gs://"))
    );
    assert_eq!(by("filtered").error.unwrap().code, "moderation_blocked");
    assert_eq!(by("error").error.unwrap().code, "3");
    let errors = round_trips::errors::<Veo>(DIR);
    assert_eq!(errors[0].1.kind, ErrorKind::PermissionDenied);
}

#[test]
fn the_model_names_the_paths_and_the_operation_goes_in_the_body() {
    let mut r =
        VideoRequest::new("veo-3.1-fast-generate-001", VideoOperation::Generate).with_prompt("p");
    r.size = Some(ImageSize::pixels(1080, 1920));
    let enc = encode(&r).unwrap();
    assert_eq!(
        enc.path,
        "/v1/publishers/google/models/veo-3.1-fast-generate-001:predictLongRunning"
    );
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        (
            body["parameters"]["resolution"].as_str(),
            body["parameters"]["aspectRatio"].as_str()
        ),
        (Some("1080p"), Some("9:16"))
    );
    let ctx = platform().with_gemini_profile(
        GeminiProfile::AGENT_PLATFORM
            .with_models_path("/v1/projects/p/locations/us-central1/publishers/google/models"),
    );
    let poll =
        Veo::encode_video_poll("projects/p/operations/1", "veo-3.1-generate-001", &ctx).unwrap();
    assert_eq!(
        poll.path,
        "/v1/projects/p/locations/us-central1/publishers/google/models/veo-3.1-generate-001:fetchPredictOperation"
    );
    assert_eq!(poll.method, "POST");
    let body: Value = serde_json::from_slice(&poll.body).unwrap();
    assert_eq!(body["operationName"], "projects/p/operations/1");
    assert!(Veo::encode_video_delete("j", "m", &ctx).unwrap().is_none());
}

#[test]
fn veo_refuses_what_it_lacks_before_sending() {
    let base = VideoRequest::new("veo-3.1-generate-001", VideoOperation::Generate).with_prompt("p");
    let param = |r: &VideoRequest| {
        let err = encode(r).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
        err.param.unwrap_or_default()
    };
    let mut s = base.clone();
    s.seconds = Some(5);
    assert_eq!(param(&s), "seconds");
    let mut n = base.clone();
    n.count = Some(5);
    assert_eq!(param(&n), "n");
    let mut size = base.clone();
    size.size = Some(ImageSize::pixels(1024, 1792));
    assert_eq!(param(&size), "size");
    let mut user = base.clone();
    user.user = Some("u".into());
    assert_eq!(param(&user), "user");
    let web = base.clone().with_image(VideoImage::new(
        Source::Url("https://example.com/a.png".into()),
        VideoImageRole::FirstFrame,
    ));
    assert_eq!(param(&web), "input_reference");
    let mut remix =
        VideoRequest::new("veo-3.1-generate-001", VideoOperation::Remix).with_prompt("p");
    remix.remix_of = Some("j".into());
    assert_eq!(param(&remix), "video_id");
    let ctx = round_trips::ctx();
    assert!(
        Veo::decode_video_poll(
            200,
            &HeaderList::new(),
            br#"{"name":"n","done":true}"#,
            &ctx
        )
        .is_err()
    );
    assert!(Veo::decode_response(200, &HeaderList::new(), br#"{"done":false}"#, &ctx).is_err());
    assert!(
        Veo::decode_request(
            RequestKind::Video,
            br#"{"instances":[]}"#,
            &HeaderList::new()
        )
        .is_err()
    );
    assert!(
        Veo::decode_request(
            RequestKind::Video,
            br#"{"instances":[{"image":{}}]}"#,
            &HeaderList::new()
        )
        .is_err()
    );
}
