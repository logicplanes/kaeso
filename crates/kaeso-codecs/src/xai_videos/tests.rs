use kaeso_kpp::{
    ErrorKind, ImageSize, Request, Source, VideoImage, VideoImageRole, VideoOperation,
    VideoRequest, VideoStatus, VideoVariant,
};

use super::*;
use crate::video_jobs::round_trips;

const DIR: &str = "xai_videos";

fn encode(r: &VideoRequest) -> Result<EncodedRequest, KppError> {
    let req = Request::Video(r.clone());
    let mut ctx = EgressCtx::new(&req);
    XaiVideos::encode_request(&req, &mut ctx)
}

#[test]
fn every_fixture_round_trips() {
    for (name, enc) in round_trips::requests::<XaiVideos>(DIR) {
        let path = if name == "edit" {
            EDITS_PATH
        } else {
            GENERATIONS_PATH
        };
        assert_eq!(enc.path, path, "{name}");
    }
    let started = round_trips::started::<XaiVideos>(DIR);
    assert_eq!(started[0].1.id, "a3d1008e-4544-40d4-d075-11527e794e4a");
    assert_eq!(started[0].1.status, VideoStatus::Queued);
    let polls = round_trips::polls::<XaiVideos>(DIR, encode_poll);
    let by = |n: &str| polls.iter().find(|(name, _)| name == n).unwrap().1.clone();
    let done = by("done");
    assert_eq!(
        (done.status, done.seconds),
        (VideoStatus::Completed, Some(6))
    );
    assert!(
        matches!(&done.output(VideoVariant::Video).unwrap().source, Source::Url(u) if u.ends_with(".mp4"))
    );
    assert_eq!(by("pending").status, VideoStatus::InProgress);
    assert_eq!(by("queued").status, VideoStatus::Queued);
    assert_eq!(by("failed").error.unwrap().code, "invalid_argument");
    let expired = by("expired");
    assert_eq!(
        (expired.status, expired.error.unwrap().code.as_str()),
        (VideoStatus::Failed, "expired")
    );
    let moderated = by("moderated");
    assert_eq!(moderated.status, VideoStatus::Failed);
    assert!(moderated.outputs.is_empty());
    let errors = round_trips::errors::<XaiVideos>(DIR);
    assert!(errors[0].1.message.contains("does not exist"));
}

#[test]
fn a_size_becomes_a_resolution_and_a_ratio_spacex_lists() {
    let mut r =
        VideoRequest::new("grok-imagine-video-1.5", VideoOperation::Generate).with_prompt("p");
    r.size = Some(ImageSize::pixels(1280, 720));
    let body: Value = serde_json::from_slice(&encode(&r).unwrap().body).unwrap();
    assert_eq!(
        (body["resolution"].as_str(), body["aspect_ratio"].as_str()),
        (Some("720p"), Some("16:9"))
    );
    r.size = Some(ImageSize::pixels(1024, 1792));
    assert_eq!(encode(&r).unwrap_err().param.as_deref(), Some("size"));
}

#[test]
fn spacex_refuses_what_it_lacks_before_sending() {
    let base =
        VideoRequest::new("grok-imagine-video-1.5", VideoOperation::Generate).with_prompt("p");
    let param = |r: &VideoRequest| {
        let err = encode(r).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
        err.param.unwrap_or_default()
    };
    let mut seed = base.clone();
    seed.seed = Some(3);
    assert_eq!(param(&seed), "seed");
    let mut long = base.clone();
    long.seconds = Some(20);
    assert_eq!(param(&long), "seconds");
    let mut res = base.clone();
    res.resolution = Some("4k".into());
    assert_eq!(param(&res), "resolution");
    let mut ratio = base.clone();
    ratio.aspect_ratio = Some("7:3".into());
    assert_eq!(param(&ratio), "aspect_ratio");
    let last = base
        .clone()
        .with_image(VideoImage::new(
            Source::Url("u".into()),
            VideoImageRole::FirstFrame,
        ))
        .with_image(VideoImage::new(
            Source::Url("v".into()),
            VideoImageRole::LastFrame,
        ));
    assert_eq!(param(&last), "input_reference");
    let mut remix = VideoRequest::new("grok-imagine-video", VideoOperation::Remix).with_prompt("p");
    remix.remix_of = Some("job".into());
    assert_eq!(param(&remix), "video_id");
    remix.source_video = Some(Source::Url("https://vidgen.x.ai/v.mp4".into()));
    let enc = encode(&remix).unwrap();
    assert_eq!(enc.path, EDITS_PATH);
    let poll = XaiVideos::encode_video_poll("a b", "m", &round_trips::ctx()).unwrap();
    assert_eq!(poll.path, "/v1/videos/a%20b");
    assert!(
        XaiVideos::encode_video_delete("j", "m", &round_trips::ctx())
            .unwrap()
            .is_none()
    );
}

#[test]
fn unknown_statuses_and_missing_ids_are_provider_errors() {
    let ctx = round_trips::ctx();
    assert!(
        XaiVideos::decode_video_poll(200, &HeaderList::new(), br#"{"status":"thinking"}"#, &ctx)
            .is_err()
    );
    assert!(XaiVideos::decode_response(200, &HeaderList::new(), b"{}", &ctx).is_err());
    assert!(XaiVideos::decode_request(RequestKind::Image, b"{}", &HeaderList::new()).is_err());
    assert!(
        XaiVideos::decode_request(RequestKind::Video, br#"{"image":"x"}"#, &HeaderList::new())
            .is_err()
    );
    assert!(
        XaiVideos::decode_request(RequestKind::Video, br#"{"video":{}}"#, &HeaderList::new())
            .is_err()
    );
}
