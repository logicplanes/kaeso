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
        matches!(&done.output(VideoVariant::Video).unwrap().source, Source::Base64(b) if b.to_base64() == "AAAAIGZ0eXBpc29t")
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
            Source::base64("image/png", "AAAA").unwrap(),
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
    let framed = |model: &str| {
        let mut r = base.clone().with_image(VideoImage::new(
            Source::Url("https://example.com/a.png".into()),
            VideoImageRole::FirstFrame,
        ));
        r.model = model.into();
        r
    };
    assert_eq!(
        param(&framed("gemini-omni-flash-preview")),
        "input_reference"
    );
    assert!(encode(&framed("gemini-omni-1.1-flash-preview")).is_ok());
    let mut referenced = base.clone().with_image(VideoImage::new(
        Source::Url("https://example.com/a.png".into()),
        VideoImageRole::Reference,
    ));
    referenced.model = "gemini-omni-flash-preview".into();
    assert!(encode(&referenced).is_ok());
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

#[test]
fn an_input_that_is_neither_a_prompt_nor_images_is_refused_naming_input() {
    let refused = |body: &[u8]| {
        GeminiInteractions::decode_request(RequestKind::Video, body, &HeaderList::new())
            .unwrap_err()
    };
    let no_source = refused(br#"{"model":"m","input":[{"type":"image","mime_type":"image/png"}]}"#);
    assert_eq!(no_source.message, "An image item takes `data` or `uri`.");
    assert_eq!(no_source.param.as_deref(), Some("input"));
    let bad_data = refused(br#"{"model":"m","input":[{"type":"image","data":"not base64!"}]}"#);
    assert_eq!(bad_data.kind, ErrorKind::InvalidRequest);
    assert_eq!(bad_data.param.as_deref(), Some("data"));
    let strings = refused(br#"{"model":"m","input":["A fox"]}"#);
    assert_eq!(strings.message, "`input` lists objects.");
    assert_eq!(strings.param.as_deref(), Some("input"));
    let number = refused(br#"{"model":"m","input":7}"#);
    assert_eq!(number.message, "`input` is text or a list.");
    assert_eq!(number.param.as_deref(), Some("input"));
}

#[test]
fn images_without_a_task_or_prompt_go_back_as_the_app_sent_them() {
    let sent = json!({
        "model": "gemini-omni-1.2-flash",
        "input": [{"type": "image", "uri": "https://example.com/a.png"}],
        "generation_config": {"temperature": 0.5},
    });
    let req = GeminiInteractions::decode_request(
        RequestKind::Video,
        &serde_json::to_vec(&sent).unwrap(),
        &HeaderList::new(),
    )
    .unwrap();
    let Request::Video(r) = &req else { panic!() };
    assert_eq!(r.prompt, None);
    assert_eq!(r.images[0].role, VideoImageRole::FirstFrame);
    let mut ctx = EgressCtx::new(&req);
    let body: Value = serde_json::from_slice(
        &GeminiInteractions::encode_request(&req, &mut ctx)
            .unwrap()
            .body,
    )
    .unwrap();
    let mut expected = sent;
    expected["response_format"] = json!({"type": "video"});
    assert_eq!(body, expected);

    let extension = json!({
        "model": "gemini-omni-1.2-flash",
        "input": [{"type": "text", "text": "Go on"}, {"type": "image", "uri": "https://example.com/a.png"}],
        "generation_config": {"video_config": {"task": "video_extension"}},
        "response_format": {"type": "video"},
    });
    let req = GeminiInteractions::decode_request(
        RequestKind::Video,
        &serde_json::to_vec(&extension).unwrap(),
        &HeaderList::new(),
    )
    .unwrap();
    let mut ctx = EgressCtx::new(&req);
    let body: Value = serde_json::from_slice(
        &GeminiInteractions::encode_request(&req, &mut ctx)
            .unwrap()
            .body,
    )
    .unwrap();
    assert_eq!(
        body["generation_config"]["video_config"]["task"],
        "video_extension"
    );
}

#[test]
fn a_remix_without_its_video_file_ids_and_unknown_formats_are_refused_before_sending() {
    let base =
        VideoRequest::new("gemini-omni-1.2-flash", VideoOperation::Generate).with_prompt("p");
    let refused = |r: &VideoRequest| {
        let err = encode(r).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
        (err.param.unwrap_or_default(), err.message)
    };
    let mut remix = base.clone();
    remix.operation = VideoOperation::Remix;
    let (param, message) = refused(&remix);
    assert_eq!(param, "video_id");
    assert!(
        message.contains("a remix without its earlier video"),
        "{message}"
    );
    let file = base.clone().with_image(VideoImage::new(
        Source::file_id(Origin::new(D), "files/abc"),
        VideoImageRole::FirstFrame,
    ));
    let (param, message) = refused(&file);
    assert_eq!(param, "input_reference");
    assert!(message.contains("file ids"), "{message}");
    let mut res = base.clone();
    res.resolution = Some("8k".into());
    let (param, message) = refused(&res);
    assert_eq!(param, "resolution");
    assert!(message.contains("`8k`"), "{message}");
    let mut square = base;
    square.aspect_ratio = Some("1:1".into());
    let (param, message) = refused(&square);
    assert_eq!(param, "aspect_ratio");
    assert!(message.contains("`1:1`"), "{message}");
}

#[test]
fn only_video_outputs_that_carry_their_bytes_or_link_are_read() {
    let ctx = round_trips::ctx();
    let body = json!({
        "id": "v1_a",
        "status": "completed",
        "steps": [
            {"type": "user_input", "content": [{"type": "video", "uri": "https://example.com/in.mp4"}]},
            {"type": "model_output", "content": [
                {"type": "text", "text": "Here it is."},
                {"type": "video", "data": "not base64!"},
                {"type": "video", "mime_type": "video/webm"},
                {"type": "video", "uri": "https://example.com/out.mp4"},
            ]},
        ],
        "usage": {"total_input_tokens": 5, "total_output_tokens": 7},
    });
    let job = GeminiInteractions::decode_video_poll(
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&body).unwrap(),
        &ctx,
    )
    .unwrap();
    assert_eq!(job.status, VideoStatus::Completed);
    assert_eq!(job.outputs.len(), 1);
    assert_eq!(
        job.outputs[0].source,
        Source::Url("https://example.com/out.mp4".into())
    );
    let usage = job.usage.unwrap();
    assert_eq!(
        (
            usage.input_tokens,
            usage.output_tokens,
            usage.reasoning_tokens
        ),
        (5, 7, None)
    );
}

#[test]
fn an_interaction_that_ends_without_a_video_or_error_names_its_status() {
    let ctx = round_trips::ctx();
    let job = GeminiInteractions::decode_video_poll(
        200,
        &HeaderList::new(),
        br#"{"id":"v1_a","status":"cancelled"}"#,
        &ctx,
    )
    .unwrap();
    assert_eq!(job.status, VideoStatus::Failed);
    let error = job.error.unwrap();
    assert_eq!(error.code, "cancelled");
    assert_eq!(
        error.message,
        "The interaction ended as `cancelled` without a video."
    );
    let unnamed = GeminiInteractions::decode_response(
        200,
        &HeaderList::new(),
        br#"{"status":"in_progress"}"#,
        &ctx,
    )
    .unwrap_err();
    assert_eq!(unnamed.kind, ErrorKind::ProviderError);
    assert_eq!(
        unnamed.message,
        "Google started an interaction without naming its `id`."
    );
}

#[test]
fn a_job_from_another_format_is_written_with_its_status() {
    let ctx = EncodeCtx::new(RequestKind::Video, kaeso_kpp::StreamMode::Off);
    for (status, wire) in [
        (VideoStatus::Completed, "completed"),
        (VideoStatus::Failed, "failed"),
        (VideoStatus::InProgress, "in_progress"),
    ] {
        let job = kaeso_kpp::VideoJob::new("", status);
        let body = GeminiInteractions::encode_response(&Reply::Video(job), &ctx).unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v, json!({"status": wire}));
    }
}
