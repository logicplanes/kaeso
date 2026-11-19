use kaeso_kpp::{
    ErrorKind, ImageBackground, ImageDelivery, ImageFormat, ImageInput, ImageOperation,
    ImageQuality, ImageRequest, ImageSize, Reply, Request, RequestKind, Source, StreamMode,
    VideoImage, VideoImageRole, VideoOperation, VideoRequest, VideoStatus, VideoVariant,
};
use serde_json::{Value, json};

use super::*;
use crate::test_support::{assert_json_eq, fixtures, sidecar};

const DIR: &str = "fal";

fn ctx(kind: RequestKind) -> EgressCtx {
    EgressCtx::for_kind(kind, StreamMode::Off)
}

fn model_of(folder: &str, name: &str) -> String {
    sidecar(DIR, folder, name, "meta").unwrap()["model"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn round_trip(kind: RequestKind, folder: &str) -> Vec<(String, Request, EncodedRequest)> {
    let mut out = Vec::new();
    for (name, body) in fixtures(DIR, folder, "json") {
        let mut req = Fal::decode_request(kind, &body, &HeaderList::new())
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let model = model_of(folder, &name);
        match &mut req {
            Request::Image(r) => r.model.clone_from(&model),
            Request::Video(r) => r.model.clone_from(&model),
            _ => panic!("{name}"),
        }
        req.validate().unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let mut c = ctx(kind);
        Fal::check(&req, &c).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let enc = Fal::encode_request(&req, &mut c).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(enc.method, "POST", "{name}");
        assert_eq!(enc.path, format!("/{model}"), "{name}");
        assert_eq!(enc.headers.get("x-fal-store-io"), Some("0"), "{name}");
        assert_json_eq(&name, &body, &enc.body);
        out.push((name, req, enc));
    }
    out
}

#[test]
fn every_request_fixture_round_trips_to_its_models_input() {
    let images = round_trip(RequestKind::Image, "requests/images");
    let by = |n: &str| match &images.iter().find(|(name, ..)| name == n).unwrap().1 {
        Request::Image(r) => r.clone(),
        _ => unreachable!(),
    };
    let sync = by("flux-pro-sync");
    assert_eq!(sync.delivery, Some(ImageDelivery::Base64));
    assert_eq!(sync.size, Some(ImageSize::pixels(1024, 1024)));
    assert_eq!(sync.count, Some(2));
    let edit = by("seedream-edit");
    assert_eq!(edit.operation, ImageOperation::Edit);
    assert!(matches!(edit.images[1].source, Source::Base64(_)));
    assert_eq!(
        by("seedream-transparent").background,
        Some(ImageBackground::Transparent)
    );
    let videos = round_trip(RequestKind::Video, "requests/videos");
    let by = |n: &str| match &videos.iter().find(|(name, ..)| name == n).unwrap().1 {
        Request::Video(r) => r.clone(),
        _ => unreachable!(),
    };
    let veo = by("veo31");
    assert_eq!(
        (veo.seconds, veo.size.clone(), veo.audio),
        (Some(8), Some(ImageSize::pixels(1920, 1080)), Some(false))
    );
    assert_eq!(by("veo31-4k").size, Some(ImageSize::pixels(2160, 3840)));
    assert_eq!(by("kling-text").size, Some(ImageSize::pixels(1680, 720)));
    assert_eq!(by("kling-image").images.len(), 2);
    assert_eq!(by("wan-text").audio, Some(false));
}

#[test]
fn image_results_decode_with_their_links_or_bytes_and_write_back() {
    for (name, body) in fixtures(DIR, "results/images", "json") {
        assert_eq!(pending_task(&body), None, "{name}");
        let reply = Fal::decode_response(200, &HeaderList::new(), &body, &ctx(RequestKind::Image))
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let back =
            Fal::encode_response(&reply, &EncodeCtx::new(RequestKind::Image, StreamMode::Off))
                .unwrap();
        assert_json_eq(&name, &body, &back);
        let Reply::Image(r) = reply else { panic!() };
        match name.as_str() {
            "flux-sync" => {
                assert_eq!(r.images.len(), 2);
                assert!(
                    matches!(&r.images[0].source, Source::Base64(b) if b.media_type == "image/png")
                );
                assert_eq!(r.settings.size, Some(ImageSize::pixels(1024, 1024)));
            }
            "seedream" => {
                assert!(
                    matches!(&r.images[0].source, Source::Url(u) if u.starts_with("https://v3b.fal.media/"))
                );
                assert_eq!(r.settings.size, None);
            }
            _ => assert_eq!(r.settings.size, Some(ImageSize::pixels(1024, 1024))),
        }
    }
}

#[test]
fn a_black_safety_image_is_a_content_filter_error() {
    let body = &fixtures(DIR, "moderated", "json")[0].1;
    let err =
        Fal::decode_response(200, &HeaderList::new(), body, &ctx(RequestKind::Image)).unwrap_err();
    assert_eq!(err.kind, ErrorKind::ContentFiltered);
    assert_eq!(err.status, Some(400));
    assert!(err.message.contains("image 1"), "{}", err.message);
}

#[test]
fn an_image_request_follows_the_queue_to_its_result() {
    let fixture = |n: &str| {
        fixtures(DIR, "queue", "json")
            .into_iter()
            .find(|(name, _)| name == n)
            .unwrap()
            .1
    };
    let base = "/fal-ai/flux/requests/764cabcf-b745-4b3e-ae38-1200304cf45b";
    assert_eq!(
        pending_task(&fixture("submitted-short")).as_deref(),
        Some(format!("{base}/status").as_str())
    );
    assert_eq!(
        pending_task(&fixture("submitted")).as_deref(),
        Some("/blackforestlabs/flux-3-max/requests/019a1b2c-3d4e-7f80-9a1b-2c3d4e5f6a7b/status")
    );
    for n in ["in-queue", "in-progress"] {
        assert_eq!(
            pending_task(&fixture(n)).as_deref(),
            Some(format!("{base}/status").as_str()),
            "{n}"
        );
    }
    assert_eq!(pending_task(&fixture("completed")).as_deref(), Some(base));
    let poll =
        crate::dispatch::encode_task_poll(DialectId::Fal, base, &ctx(RequestKind::Image)).unwrap();
    assert_eq!((poll.method, poll.path.as_str()), ("GET", base));
    for (n, kind) in [
        ("failed", ErrorKind::Timeout),
        ("cancelled", ErrorKind::InvalidRequest),
    ] {
        let body = fixture(n);
        assert_eq!(pending_task(&body), None, "{n}");
        let err = Fal::decode_response(200, &HeaderList::new(), &body, &ctx(RequestKind::Image))
            .unwrap_err();
        assert_eq!(err.kind, kind, "{n}");
    }
    let early = Fal::decode_response(
        200,
        &HeaderList::new(),
        &fixture("in-queue"),
        &ctx(RequestKind::Image),
    )
    .unwrap_err();
    assert_eq!(early.kind, ErrorKind::ProviderError);
}

#[test]
fn urls_are_followed_only_on_falls_queue_hosts() {
    assert_eq!(
        queue_path("https://queue.fal.run/fal-ai/flux/requests/1").as_deref(),
        Some("/fal-ai/flux/requests/1")
    );
    assert_eq!(
        queue_path("https://QUEUE.FALRUN.COM/a/b").as_deref(),
        Some("/a/b")
    );
    for url in [
        "http://queue.fal.run/a",
        "https://evil.example.com/a",
        "https://queue.fal.run.example.com/a",
        "https://queue.fal.run:8443/a",
        "https://user@queue.fal.run/a",
        "https://queue.fal.run/a/../b",
        "https://queue.fal.run/a?x=1",
        "https://queue.fal.run/a%2F..",
        "https://queue.fal.run//a",
        "https://queue.fal.run",
    ] {
        assert_eq!(queue_path(url), None, "{url}");
    }
    let elsewhere = json!({
        "status": "IN_QUEUE",
        "request_id": "1",
        "response_url": "https://evil.example.com/a/requests/1",
        "status_url": "https://evil.example.com/a/requests/1/status",
    });
    assert_eq!(pending_task(&serde_json::to_vec(&elsewhere).unwrap()), None);
    let c = ctx(RequestKind::Video);
    for job in ["/a/../b", "a/b", "/a/b?x", "/a//b"] {
        let err = Fal::encode_video_poll(job, "m", &c).unwrap_err();
        assert_eq!(err.status, Some(400), "{job}");
        assert!(Fal::encode_video_file(job, &c).is_err(), "{job}");
        assert!(poll_request(job).is_err(), "{job}");
    }
}

#[test]
fn video_jobs_start_poll_and_name_their_result() {
    let c = ctx(RequestKind::Video);
    for (name, body) in fixtures(DIR, "queue", "json") {
        let job = Fal::decode_video_poll(200, &HeaderList::new(), &body, &c)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_json_eq(
            &name,
            &body,
            &serde_json::to_vec(&videos::encode_started(&job)).unwrap(),
        );
        let expected = match name.as_str() {
            "in-progress" => VideoStatus::InProgress,
            "completed" => VideoStatus::Completed,
            "failed" | "cancelled" => VideoStatus::Failed,
            _ => VideoStatus::Queued,
        };
        assert_eq!(job.status, expected, "{name}");
        if name.starts_with("submitted") {
            let reply = Fal::decode_response(200, &HeaderList::new(), &body, &c).unwrap();
            let back =
                Fal::encode_response(&reply, &EncodeCtx::new(RequestKind::Video, StreamMode::Off))
                    .unwrap();
            assert_json_eq(&name, &body, &back);
        }
    }
    let poll = |n: &str| {
        let body = fixtures(DIR, "queue", "json")
            .into_iter()
            .find(|(name, _)| name == n)
            .unwrap()
            .1;
        Fal::decode_video_poll(200, &HeaderList::new(), &body, &c).unwrap()
    };
    let base = "/fal-ai/flux/requests/764cabcf-b745-4b3e-ae38-1200304cf45b";
    let started = poll("submitted-short");
    assert_eq!(started.id, base);
    let status = crate::dispatch::encode_video_poll(DialectId::Fal, &started.id, "m", &c).unwrap();
    assert_eq!(
        (status.method, status.path),
        ("GET", format!("{base}/status"))
    );
    let done = poll("completed");
    let Source::FileId(file) = &done.output(VideoVariant::Video).unwrap().source else {
        panic!()
    };
    assert_eq!(file.id, base);
    let get = crate::dispatch::encode_video_file(DialectId::Fal, &file.id, &c).unwrap();
    assert_eq!((get.method, get.path.as_str()), ("GET", base));
    let failed = poll("failed").error.unwrap();
    assert_eq!(
        (failed.code.as_str(), failed.message.as_str()),
        ("request_timeout", "Request timed out")
    );
    assert_eq!(poll("cancelled").error.unwrap().code, "client_cancelled");
    for (name, body) in fixtures(DIR, "results/videos", "json") {
        let link = Fal::decode_video_file(200, &HeaderList::new(), &body).unwrap();
        assert!(
            matches!(&link, Source::Url(u) if u.starts_with("https://v3b.fal.media/")),
            "{name}"
        );
    }
    assert!(Fal::decode_video_file(200, &HeaderList::new(), b"{}").is_err());
    assert!(Fal::decode_video_file(404, &HeaderList::new(), b"{}").is_err());
    let no_result = br#"{"status":"IN_QUEUE","request_id":"1"}"#;
    assert!(Fal::decode_response(200, &HeaderList::new(), no_result, &c).is_err());
    assert!(Fal::decode_video_poll(200, &HeaderList::new(), b"{}", &c).is_err());
    let done = br#"{"status":"COMPLETED","request_id":"1"}"#;
    assert!(Fal::decode_video_poll(200, &HeaderList::new(), done, &c).is_err());
}

#[test]
fn errors_keep_their_kind_message_and_field() {
    let mut seen = Vec::new();
    for (name, body) in fixtures(DIR, "errors", "json") {
        let status = sidecar(DIR, "errors", &name, "meta").unwrap()["status"]
            .as_u64()
            .unwrap() as u16;
        let err = Fal::decode_error(status, &HeaderList::new(), &body);
        assert!(!err.message.is_empty(), "{name}");
        assert_eq!(err.status, Some(status), "{name}");
        let back = Fal::encode_error(&err);
        assert_eq!(back.status, status, "{name}");
        assert_json_eq(&name, &body, &back.body);
        assert!(
            Fal::decode_response(status, &HeaderList::new(), &body, &ctx(RequestKind::Image))
                .is_err()
        );
        assert!(
            Fal::decode_video_poll(status, &HeaderList::new(), &body, &ctx(RequestKind::Video))
                .is_err()
        );
        seen.push((name, err));
    }
    let by = |n: &str| seen.iter().find(|(name, _)| name == n).unwrap().1.clone();
    assert_eq!(
        by("recorded-auth-required-401").kind,
        ErrorKind::Authentication
    );
    assert_eq!(by("recorded-unknown-model-404").kind, ErrorKind::NotFound);
    let policy = by("content-policy-422");
    assert_eq!(policy.kind, ErrorKind::ContentFiltered);
    assert_eq!(policy.param.as_deref(), Some("prompt"));
    assert_eq!(by("generation-timeout-504").kind, ErrorKind::Timeout);
    assert_eq!(by("generation-timeout-504").param, None);
    assert_eq!(by("request-timeout-504").kind, ErrorKind::Timeout);
    let platform = Fal::decode_error(
        401,
        &HeaderList::new(),
        br#"{"error":{"type":"authorization_error","message":"API key authentication required"}}"#,
    );
    assert_eq!(platform.message, "API key authentication required");
    let typed = Fal::decode_error(
        503,
        &HeaderList::new().with("x-fal-error-type", "runner_disconnected"),
        br#"{"detail":"The runner disconnected."}"#,
    );
    assert_eq!(typed.kind, ErrorKind::Overloaded);
    let garbled = Fal::decode_error(502, &HeaderList::new().with("retry-after", "3"), b"<html>");
    assert_eq!(garbled.kind, ErrorKind::ProviderError);
    assert!(garbled.retry_after.is_some());
    let mine = Fal::encode_error(&KppError::invalid_request("No.").with_param("size"));
    let v: Value = serde_json::from_slice(&mine.body).unwrap();
    assert_eq!(mine.status, 422);
    assert_eq!(v["detail"][0]["loc"], json!(["body", "size"]));
    for (kind, status) in [
        (ErrorKind::Authentication, 401),
        (ErrorKind::RateLimited, 429),
        (ErrorKind::Timeout, 504),
        (ErrorKind::ContentFiltered, 422),
    ] {
        assert_eq!(Fal::encode_error(&KppError::new(kind, "x")).status, status);
    }
}

fn image(model: &str) -> ImageRequest {
    ImageRequest::new(model, ImageOperation::Generate).with_prompt("a fox")
}

fn image_param(r: &ImageRequest) -> String {
    let req = Request::Image(r.clone());
    let err = Fal::encode_request(&req, &mut ctx(RequestKind::Image)).unwrap_err();
    err.param.unwrap_or_default()
}

fn sent(r: &ImageRequest) -> Value {
    let req = Request::Image(r.clone());
    let enc = Fal::encode_request(&req, &mut ctx(RequestKind::Image)).unwrap();
    serde_json::from_slice(&enc.body).unwrap()
}

/// A change to an image request.
type Edit = Box<dyn Fn(&mut ImageRequest)>;

#[test]
fn image_fields_map_per_model_and_what_a_model_lacks_is_refused() {
    let mut r = image("fal-ai/flux/dev");
    r.size = Some(ImageSize::pixels(1536, 1024));
    r.count = Some(4);
    r.format = Some(ImageFormat::Jpeg);
    r.quality = Some(ImageQuality::Auto);
    r.background = Some(ImageBackground::Auto);
    r.delivery = Some(ImageDelivery::Url);
    assert_eq!(
        sent(&r),
        json!({"prompt": "a fox", "image_size": {"width": 1536, "height": 1024}, "num_images": 4, "output_format": "jpeg"})
    );
    r.size = Some(ImageSize::Auto);
    r.delivery = Some(ImageDelivery::Base64);
    assert_eq!(sent(&r)["sync_mode"], json!(true));
    assert!(sent(&r).get("image_size").is_none());

    let cases: Vec<(&str, Edit)> = vec![
        ("model", Box::new(|r| r.model = "fal-ai/flux-2-pro".into())),
        ("n", Box::new(|r| r.count = Some(5))),
        (
            "size",
            Box::new(|r| r.size = Some(ImageSize::pixels(20000, 512))),
        ),
        (
            "quality",
            Box::new(|r| r.quality = Some(ImageQuality::High)),
        ),
        (
            "output_format",
            Box::new(|r| r.format = Some(ImageFormat::Webp)),
        ),
        (
            "background",
            Box::new(|r| r.background = Some(ImageBackground::Transparent)),
        ),
        ("output_compression", Box::new(|r| r.compression = Some(50))),
        (
            "negative_prompt",
            Box::new(|r| r.negative_prompt = Some("x".into())),
        ),
        ("user", Box::new(|r| r.user = Some("u".into()))),
        (
            "aspect_ratio",
            Box::new(|r| r.aspect_ratio = Some("1:1".into())),
        ),
        ("resolution", Box::new(|r| r.resolution = Some("2K".into()))),
        ("partial_images", Box::new(|r| r.partial_images = Some(1))),
        ("stream", Box::new(|r| r.stream = StreamMode::on(false))),
        ("prompt", Box::new(|r| r.prompt = None)),
        (
            "mask",
            Box::new(|r| {
                r.mask = Some(ImageInput::new(Source::Url(
                    "https://example.com/m.png".into(),
                )))
            }),
        ),
        (
            "image",
            Box::new(|r| {
                r.images.push(ImageInput::new(Source::Url(
                    "https://example.com/a.png".into(),
                )))
            }),
        ),
        (
            "image",
            Box::new(|r| r.operation = ImageOperation::Variation),
        ),
    ];
    for (param, change) in cases {
        let mut r = image("fal-ai/flux/dev");
        change(&mut r);
        assert_eq!(image_param(&r), param);
    }

    let mut s = image("bytedance/seedream/v5.5/text-to-image");
    s.size = Some(ImageSize::pixels(1024, 1024));
    s.count = Some(6);
    s.background = Some(ImageBackground::Opaque);
    assert_eq!(
        sent(&s),
        json!({"prompt": "a fox", "image_size": {"width": 1024, "height": 1024}, "num_images": 6, "background": "opaque"})
    );
    s.background = Some(ImageBackground::Transparent);
    s.format = Some(ImageFormat::Jpeg);
    assert_eq!(image_param(&s), "background");
    s.format = None;
    s.seed = Some(1);
    assert_eq!(image_param(&s), "seed");
    s.seed = None;
    s.size = Some(ImageSize::pixels(512, 512));
    assert_eq!(image_param(&s), "size");
    s.size = Some(ImageSize::pixels(4096, 240));
    assert_eq!(image_param(&s), "size");

    let mut e = image("bytedance/seedream/v5.5/edit");
    e.operation = ImageOperation::Edit;
    assert_eq!(image_param(&e), "image");
    e.images = vec![
        ImageInput::new(Source::file_id(
            Origin::new(DialectId::OpenAiImages),
            "file-1"
        ));
        1
    ];
    assert_eq!(image_param(&e), "image");
    e.images = vec![ImageInput::new(Source::Url("https://example.com/a.png".into())); 11];
    assert_eq!(image_param(&e), "image");
    e.images.truncate(2);
    assert_eq!(
        sent(&e)["image_urls"],
        json!(["https://example.com/a.png", "https://example.com/a.png"])
    );
    let mut foreign = image("fal-ai/flux/dev");
    foreign.extras = kaeso_kpp::Extras::new(Origin::new(DialectId::OpenAiImages));
    foreign.extras.fields.insert(
        "style".into(),
        kaeso_kpp::RawJson::from_serialize(&"vivid").unwrap(),
    );
    let req = Request::Image(foreign);
    let err = Fal::encode_request(&req, &mut ctx(RequestKind::Image)).unwrap_err();
    assert!(err.message.contains("style"), "{}", err.message);
}

fn video(model: &str) -> VideoRequest {
    let mut r = VideoRequest::new(model, VideoOperation::Generate).with_prompt("a fox");
    r.seconds = Some(8);
    r
}

fn video_param(r: &VideoRequest) -> String {
    let req = Request::Video(r.clone());
    let err = Fal::encode_request(&req, &mut ctx(RequestKind::Video)).unwrap_err();
    err.param.unwrap_or_default()
}

fn video_sent(r: &VideoRequest) -> Value {
    let req = Request::Video(r.clone());
    let enc = Fal::encode_request(&req, &mut ctx(RequestKind::Video)).unwrap();
    serde_json::from_slice(&enc.body).unwrap()
}

#[test]
fn video_fields_map_per_model_and_what_a_model_lacks_is_refused() {
    let mut veo = video("fal-ai/veo3.1");
    veo.size = Some(ImageSize::pixels(720, 1280));
    assert_eq!(
        video_sent(&veo),
        json!({"prompt": "a fox", "duration": "8s", "resolution": "720p", "aspect_ratio": "9:16"})
    );
    veo.seconds = Some(5);
    assert_eq!(video_param(&veo), "seconds");
    veo.seconds = None;
    assert_eq!(video_param(&veo), "seconds");
    veo.seconds = Some(6);
    veo.size = Some(ImageSize::pixels(1024, 1024));
    assert_eq!(video_param(&veo), "size");
    veo.size = None;
    veo.count = Some(2);
    assert_eq!(video_param(&veo), "n");
    veo.count = None;
    veo.user = Some("u".into());
    assert_eq!(video_param(&veo), "user");
    veo.user = None;
    veo.images.push(VideoImage::new(
        Source::Url("https://example.com/a.png".into()),
        VideoImageRole::FirstFrame,
    ));
    assert_eq!(video_param(&veo), "input_reference");

    let mut kling = video("kling/v4/text-to-video");
    kling.size = Some(ImageSize::pixels(1920, 1080));
    assert_eq!(
        video_sent(&kling),
        json!({"prompt": "a fox", "duration": 8, "resolution": "1080p", "aspect_ratio": "16:9"})
    );
    kling.audio = Some(true);
    assert_eq!(video_param(&kling), "audio");
    kling.audio = None;
    kling.seed = Some(1);
    assert_eq!(video_param(&kling), "seed");
    kling.seed = None;
    kling.images.push(VideoImage::new(
        Source::Url("https://example.com/a.png".into()),
        VideoImageRole::FirstFrame,
    ));
    let req = Request::Video(kling.clone());
    let err = Fal::encode_request(&req, &mut ctx(RequestKind::Video)).unwrap_err();
    assert!(
        err.message.contains("kling/v4/image-to-video"),
        "{}",
        err.message
    );

    let mut i2v = video("kling/v4/image-to-video");
    assert_eq!(video_param(&i2v), "input_reference");
    i2v.images.push(VideoImage::new(
        Source::base64("image/png", "iVBORw0KGgo=").unwrap(),
        VideoImageRole::FirstFrame,
    ));
    assert_eq!(
        video_sent(&i2v)["start_image_url"],
        json!("data:image/png;base64,iVBORw0KGgo=")
    );
    i2v.size = Some(ImageSize::pixels(1280, 720));
    assert_eq!(video_param(&i2v), "size");
    i2v.size = None;
    i2v.images.push(VideoImage::new(
        Source::Url("https://example.com/r.png".into()),
        VideoImageRole::Reference,
    ));
    assert_eq!(video_param(&i2v), "input_reference");
    i2v.images.pop();
    i2v.images[0].source = Source::file_id(Origin::new(DialectId::OpenAiVideos), "file-1");
    assert_eq!(video_param(&i2v), "input_reference");

    let mut wan = video("alibaba/wan-3.0/text-to-video");
    wan.audio = Some(true);
    wan.seed = Some(3);
    assert_eq!(
        video_sent(&wan),
        json!({"prompt": "a fox", "duration": 8, "audio": true, "seed": 3})
    );
    wan.seconds = Some(31);
    assert_eq!(video_param(&wan), "seconds");
    wan.seconds = Some(8);
    wan.negative_prompt = Some("x".into());
    assert_eq!(video_param(&wan), "negative_prompt");
    wan.negative_prompt = None;
    wan.prompt = None;
    assert_eq!(video_param(&wan), "prompt");
    let mut remix = video("fal-ai/veo3.1");
    remix.operation = VideoOperation::Remix;
    assert_eq!(video_param(&remix), "video_id");
    assert_eq!(video_param(&video("fal-ai/veo3")), "model");
}

#[test]
fn other_kinds_and_streams_are_refused() {
    let h = HeaderList::new();
    assert!(Fal::decode_request(RequestKind::Speech, b"{}", &h).is_err());
    let speech = Request::Speech(kaeso_kpp::SpeechRequest::new(
        "m",
        kaeso_kpp::SpeechOperation::Speech,
    ));
    assert!(Fal::check(&speech, &ctx(RequestKind::Speech)).is_err());
    assert!(Fal::encode_request(&speech, &mut ctx(RequestKind::Speech)).is_err());
    assert!(Fal::decode_response(200, &h, b"{}", &ctx(RequestKind::Speech)).is_err());
    let mut s = Fal::stream_decoder(&ctx(RequestKind::Image));
    assert_eq!(s.feed(b"x").len(), 1);
    assert!(
        Fal::stream_encoder(&EncodeCtx::new(RequestKind::Image, StreamMode::Off))
            .finish()
            .unwrap()
            .is_empty()
    );
    assert!(Fal::decode_response(200, &h, b"nope", &ctx(RequestKind::Image)).is_err());
    assert!(Fal::decode_response(200, &h, br#"{"seed":1}"#, &ctx(RequestKind::Image)).is_err());
    assert!(Fal::decode_response(200, &h, br#"{"images":[1]}"#, &ctx(RequestKind::Image)).is_err());
    assert!(
        Fal::decode_response(
            200,
            &h,
            br#"{"images":[{"url":""}]}"#,
            &ctx(RequestKind::Image)
        )
        .is_err()
    );
    assert!(Fal::decode_request(RequestKind::Image, br#"{"image_urls":[1]}"#, &h).is_err());
    let err = submit("a/../b", Vec::new()).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("model"));
    assert_eq!(IMAGE_MODELS.len() + VIDEO_MODELS.len(), 10);
}
