use kaeso_kpp::{
    ErrorKind, ImageSize, Request, Source, VideoImage, VideoImageRole, VideoOperation,
    VideoRequest, VideoStatus, VideoVariant,
};

use super::*;
use crate::video_jobs::round_trips;

const DIR: &str = "zai_videos";

fn encode(r: &VideoRequest) -> Result<EncodedRequest, KppError> {
    let req = Request::Video(r.clone());
    let mut ctx = EgressCtx::new(&req);
    ZaiVideos::encode_request(&req, &mut ctx)
}

#[test]
fn every_fixture_round_trips() {
    for (name, enc) in round_trips::requests::<ZaiVideos>(DIR) {
        assert_eq!(enc.path, GENERATIONS_PATH, "{name}");
    }
    let started = round_trips::started::<ZaiVideos>(DIR);
    assert_eq!(started[0].1.status, VideoStatus::InProgress);
    assert!(started[0].1.id.starts_with("8270"));
    let polls = round_trips::polls::<ZaiVideos>(DIR, encode_job);
    let by = |n: &str| polls.iter().find(|(name, _)| name == n).unwrap().1.clone();
    let done = by("success");
    assert_eq!(done.status, VideoStatus::Completed);
    assert!(done.output(VideoVariant::Video).is_some());
    assert!(
        matches!(&done.output(VideoVariant::Thumbnail).unwrap().source, Source::Url(u) if u.ends_with(".jpeg"))
    );
    assert_eq!(by("fail").status, VideoStatus::Failed);
    assert_eq!(by("processing").status, VideoStatus::InProgress);
    let errors = round_trips::errors::<ZaiVideos>(DIR);
    assert_eq!(errors[0].1.kind, ErrorKind::RateLimited);
}

#[test]
fn a_resolution_and_ratio_become_a_size_zai_lists() {
    let mut r = VideoRequest::new("cogvideox-3", VideoOperation::Generate).with_prompt("p");
    r.resolution = Some("1080p".into());
    r.aspect_ratio = Some("16:9".into());
    let body: Value = serde_json::from_slice(&encode(&r).unwrap().body).unwrap();
    assert_eq!(body["size"], "1920x1080");
    r.aspect_ratio = Some("4:3".into());
    assert_eq!(encode(&r).unwrap_err().param.as_deref(), Some("size"));
    r.aspect_ratio = None;
    assert_eq!(encode(&r).unwrap_err().param.as_deref(), Some("resolution"));
    let mut s = VideoRequest::new("cogvideox-3", VideoOperation::Generate).with_prompt("p");
    s.size = Some(ImageSize::pixels(640, 480));
    assert_eq!(encode(&s).unwrap_err().param.as_deref(), Some("size"));
}

#[test]
fn zai_refuses_what_it_lacks_before_sending() {
    let base = VideoRequest::new("cogvideox-3", VideoOperation::Generate).with_prompt("p");
    let param = |r: &VideoRequest| encode(r).unwrap_err().param.unwrap_or_default();
    let mut s = base.clone();
    s.seconds = Some(8);
    assert_eq!(param(&s), "seconds");
    let mut seed = base.clone();
    seed.seed = Some(1);
    assert_eq!(param(&seed), "seed");
    let refs = base.clone().with_image(VideoImage::new(
        Source::Url("u".into()),
        VideoImageRole::Reference,
    ));
    assert_eq!(param(&refs), "input_reference");
    let file = base.clone().with_image(VideoImage::new(
        Source::file_id(kaeso_kpp::DialectId::OpenAiVideos, "file-1"),
        VideoImageRole::FirstFrame,
    ));
    assert_eq!(param(&file), "input_reference");
    let mut remix = VideoRequest::new("cogvideox-3", VideoOperation::Remix).with_prompt("p");
    remix.remix_of = Some("j".into());
    assert_eq!(param(&remix), "video_id");
    let two = base.with_image(VideoImage::new(
        Source::base64("image/png", "AAAA").unwrap(),
        VideoImageRole::FirstFrame,
    ));
    let body: Value = serde_json::from_slice(&encode(&two).unwrap().body).unwrap();
    assert_eq!(body["image_url"][0], "data:image/png;base64,AAAA");
    let ctx = round_trips::ctx();
    assert_eq!(
        ZaiVideos::encode_video_poll("t-1", "m", &ctx).unwrap().path,
        "/api/paas/v4/async-result/t-1"
    );
    assert!(
        ZaiVideos::decode_response(
            200,
            &HeaderList::new(),
            br#"{"task_status":"PROCESSING"}"#,
            &ctx
        )
        .is_err()
    );
    assert!(
        ZaiVideos::decode_video_poll(
            200,
            &HeaderList::new(),
            br#"{"task_status":"WAITING"}"#,
            &ctx
        )
        .is_err()
    );
    assert!(
        ZaiVideos::decode_request(
            RequestKind::Video,
            br#"{"image_url":["a","b","c"]}"#,
            &HeaderList::new()
        )
        .is_err()
    );
    assert!(
        ZaiVideos::decode_request(
            RequestKind::Video,
            br#"{"image_url":[1]}"#,
            &HeaderList::new()
        )
        .is_err()
    );
}
