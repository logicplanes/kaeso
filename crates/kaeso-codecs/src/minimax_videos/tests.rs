use kaeso_kpp::{
    ErrorKind, ImageSize, Request, Source, VideoImage, VideoImageRole, VideoOperation,
    VideoRequest, VideoStatus, VideoVariant,
};

use super::*;
use crate::test_support::fixtures;
use crate::video_jobs::round_trips;

const DIR: &str = "minimax_videos";

fn encode(r: &VideoRequest) -> Result<EncodedRequest, KppError> {
    let req = Request::Video(r.clone());
    let mut ctx = EgressCtx::new(&req);
    MinimaxVideos::encode_request(&req, &mut ctx)
}

#[test]
fn every_fixture_round_trips() {
    for (name, enc) in round_trips::requests::<MinimaxVideos>(DIR) {
        let path = if name.starts_with("h3") {
            V2_PATH
        } else {
            V1_PATH
        };
        assert_eq!(enc.path, path, "{name}");
    }
    let started = round_trips::started::<MinimaxVideos>(DIR);
    assert!(
        started
            .iter()
            .all(|(_, j)| j.status == VideoStatus::Queued && !j.id.is_empty())
    );
    let polls = round_trips::polls::<MinimaxVideos>(DIR, encode_poll);
    let by = |n: &str| polls.iter().find(|(name, _)| name == n).unwrap().1.clone();
    let hailuo = by("hailuo-success");
    assert_eq!(hailuo.size, Some(ImageSize::pixels(1920, 1080)));
    assert!(
        matches!(&hailuo.output(VideoVariant::Video).unwrap().source, Source::FileId(f) if f.id == "176844028768320")
    );
    assert_eq!(by("hailuo-preparing").status, VideoStatus::Queued);
    assert_eq!(by("hailuo-processing").status, VideoStatus::InProgress);
    assert_eq!(by("hailuo-fail").status, VideoStatus::Failed);
    let h3 = by("h3-succeeded");
    assert_eq!(
        (h3.seconds, h3.resolution.as_deref(), h3.completed_at),
        (Some(5), Some("2K"), Some(1_785_125_946))
    );
    assert!(matches!(
        &h3.output(VideoVariant::Video).unwrap().source,
        Source::Url(_)
    ));
    assert_eq!(by("h3-running").completed_at, None);
    assert_eq!(by("h3-failed").error.unwrap().code, "1026");
    assert_eq!(by("h3-cancelled").error.unwrap().code, "cancelled");
    let errors = round_trips::errors::<MinimaxVideos>(DIR);
    let by_err = |n: &str| errors.iter().find(|(name, _)| name == n).unwrap().1.clone();
    assert_eq!(
        by_err("sensitive-input-200").kind,
        ErrorKind::ContentFiltered
    );
    assert_eq!(by_err("h3-auth-401").kind, ErrorKind::Authentication);
}

#[test]
fn a_hailuo_file_id_becomes_its_download_link() {
    let ctx = round_trips::ctx();
    let req = MinimaxVideos::encode_video_file("176844028768320", &ctx).unwrap();
    assert_eq!(req.path, "/v1/files/retrieve?file_id=176844028768320");
    let (_, body) = fixtures(DIR, "files", "json").remove(0);
    let link = MinimaxVideos::decode_video_file(200, &HeaderList::new(), &body).unwrap();
    assert!(matches!(link, Source::Url(u) if u.ends_with("output_aigc.mp4")));
    assert!(MinimaxVideos::decode_video_file(200, &HeaderList::new(), br#"{"file":{}}"#).is_err());
    assert!(
        MinimaxVideos::decode_video_file(
            200,
            &HeaderList::new(),
            br#"{"base_resp":{"status_code":1004,"status_msg":"bad key"}}"#
        )
        .is_err()
    );
    assert!(MinimaxVideos::decode_video_file(500, &HeaderList::new(), b"{}").is_err());
}

#[test]
fn the_model_picks_the_api_and_its_paths() {
    let ctx = round_trips::ctx();
    assert_eq!(
        MinimaxVideos::encode_video_poll("1", "MiniMax-H3", &ctx)
            .unwrap()
            .path,
        "/v2/query/video_generation/1"
    );
    assert_eq!(
        MinimaxVideos::encode_video_poll("1", "MiniMax-Hailuo-2.3", &ctx)
            .unwrap()
            .path,
        "/v1/query/video_generation?task_id=1"
    );
    let del = MinimaxVideos::encode_video_delete("1", "MiniMax-H3-Lite", &ctx)
        .unwrap()
        .unwrap();
    assert_eq!(
        (del.method, del.path.as_str()),
        ("DELETE", "/v2/video_generation/1")
    );
    assert!(
        MinimaxVideos::encode_video_delete("1", "MiniMax-Hailuo-02", &ctx)
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_size_becomes_an_h3_tier_and_ratio_within_a_percent() {
    let mut r = VideoRequest::new("MiniMax-H3", VideoOperation::Generate).with_prompt("p");
    r.size = Some(ImageSize::pixels(1366, 768));
    r.seconds = Some(5);
    let body: Value = serde_json::from_slice(&encode(&r).unwrap().body).unwrap();
    assert_eq!(
        (body["resolution"].as_str(), body["ratio"].as_str()),
        (Some("768P"), Some("16:9"))
    );
    r.size = Some(ImageSize::pixels(1280, 720));
    assert_eq!(encode(&r).unwrap_err().param.as_deref(), Some("size"));
    r.size = None;
    assert_eq!(encode(&r).unwrap_err().param.as_deref(), Some("size"));
    r.resolution = Some("768P".into());
    r.seconds = None;
    assert_eq!(encode(&r).unwrap_err().param.as_deref(), Some("seconds"));
    r.seconds = Some(6);
    r.prompt = None;
    r.images.push(VideoImage::new(
        Source::Url("u".into()),
        VideoImageRole::FirstFrame,
    ));
    assert_eq!(encode(&r).unwrap_err().param.as_deref(), Some("prompt"));
}

#[test]
fn minimax_refuses_what_it_lacks_before_sending() {
    let base = VideoRequest::new("MiniMax-Hailuo-2.3", VideoOperation::Generate).with_prompt("p");
    let param = |r: &VideoRequest| {
        let err = encode(r).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
        err.param.unwrap_or_default()
    };
    let mut size = base.clone();
    size.size = Some(ImageSize::pixels(1280, 720));
    assert_eq!(param(&size), "size");
    let refs = base.clone().with_image(VideoImage::new(
        Source::Url("u".into()),
        VideoImageRole::Reference,
    ));
    assert_eq!(param(&refs), "input_reference");
    let mut remix = VideoRequest::new("MiniMax-H3", VideoOperation::Remix).with_prompt("p");
    remix.remix_of = Some("j".into());
    assert_eq!(param(&remix), "video_id");
    let file = base.with_image(VideoImage::new(
        Source::file_id(kaeso_kpp::DialectId::OpenAiVideos, "f"),
        VideoImageRole::FirstFrame,
    ));
    assert_eq!(param(&file), "input_reference");
    let audio =
        br#"{"model":"MiniMax-H3","content":[{"type":"audio_url","audio_url":{"url":"u"}}]}"#;
    assert!(MinimaxVideos::decode_request(RequestKind::Video, audio, &HeaderList::new()).is_err());
    let role = br#"{"model":"MiniMax-H3","content":[{"type":"image_url","image_url":{"url":"u"},"role":"reference_video"}]}"#;
    assert!(MinimaxVideos::decode_request(RequestKind::Video, role, &HeaderList::new()).is_err());
    let ctx = round_trips::ctx();
    assert!(
        MinimaxVideos::decode_video_poll(200, &HeaderList::new(), br#"{"status":"Waiting"}"#, &ctx)
            .is_err()
    );
    assert!(
        MinimaxVideos::decode_video_poll(
            200,
            &HeaderList::new(),
            br#"{"task":{"status":"waiting"}}"#,
            &ctx
        )
        .is_err()
    );
    assert!(
        MinimaxVideos::decode_response(200, &HeaderList::new(), br#"{"task_id":""}"#, &ctx)
            .is_err()
    );
}
