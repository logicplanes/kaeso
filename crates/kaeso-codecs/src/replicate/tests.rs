use kaeso_kpp::{
    DialectId, ErrorKind, ImageBackground, ImageFormat, ImageInput, ImageOperation, ImageQuality,
    ImageRequest, ImageSize, Reply, Request, RequestKind, Source, StreamMode, VideoImage,
    VideoImageRole, VideoOperation, VideoRequest, VideoStatus, VideoVariant,
};
use serde_json::{Value, json};

use super::*;
use crate::test_support::{assert_json_eq, fixtures, sidecar};

const DIR: &str = "replicate";

fn ctx(kind: RequestKind) -> EgressCtx {
    EgressCtx::for_kind(kind, StreamMode::Off)
}

fn fixture(folder: &str, name: &str) -> Vec<u8> {
    fixtures(DIR, folder, "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no fixture {folder}/{name}"))
        .1
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
        let mut req = Replicate::decode_request(kind, &body, &HeaderList::new())
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let model = model_of(folder, &name);
        match &mut req {
            Request::Image(r) => r.model.clone_from(&model),
            Request::Video(r) => r.model.clone_from(&model),
            _ => panic!("{name}"),
        }
        req.validate().unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let mut c = ctx(kind);
        Replicate::check(&req, &c).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let enc =
            Replicate::encode_request(&req, &mut c).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(enc.method, "POST", "{name}");
        assert_eq!(
            enc.path,
            format!("/v1/models/{model}/predictions"),
            "{name}"
        );
        assert_eq!(
            enc.headers.get("prefer"),
            (kind == RequestKind::Image).then_some("wait=60"),
            "{name}"
        );
        assert_json_eq(&name, &body, &enc.body);
        out.push((name, req, enc));
    }
    out
}

fn image(r: &[(String, Request, EncodedRequest)], n: &str) -> ImageRequest {
    match &r.iter().find(|(name, ..)| name == n).unwrap().1 {
        Request::Image(r) => r.clone(),
        _ => unreachable!(),
    }
}

fn video(r: &[(String, Request, EncodedRequest)], n: &str) -> VideoRequest {
    match &r.iter().find(|(name, ..)| name == n).unwrap().1 {
        Request::Video(r) => r.clone(),
        _ => unreachable!(),
    }
}

#[test]
fn every_request_fixture_round_trips_to_its_models_input() {
    let images = round_trip(RequestKind::Image, "requests/images");
    let schnell = image(&images, "flux-schnell-every-field");
    assert_eq!(
        (
            schnell.count,
            schnell.aspect_ratio.as_deref(),
            schnell.format,
            schnell.compression
        ),
        (Some(2), Some("16:9"), Some(ImageFormat::Png), Some(90))
    );
    assert_eq!(
        image(&images, "flux-pro-custom-size").size,
        Some(ImageSize::pixels(1024, 768))
    );
    assert_eq!(
        image(&images, "flux-pro-custom-size").format,
        Some(ImageFormat::Jpeg)
    );
    let group = image(&images, "seedream-group");
    assert_eq!(
        (group.count, group.resolution.as_deref()),
        (Some(4), Some("4K"))
    );
    let edit = image(&images, "seedream-edit-custom");
    assert_eq!(edit.operation, ImageOperation::Edit);
    assert_eq!(edit.size, Some(ImageSize::pixels(2048, 1536)));
    assert_eq!(
        image(&images, "nano-banana-pro-4k").resolution.as_deref(),
        Some("4K")
    );
    assert!(
        image(&images, "flux-dev-own-fields")
            .extras
            .fields
            .contains_key("guidance")
    );

    let videos = round_trip(RequestKind::Video, "requests/videos");
    let frames = video(&videos, "veo31-frames");
    assert_eq!(
        (frames.seconds, frames.size.clone(), frames.audio),
        (Some(6), Some(ImageSize::pixels(1280, 720)), Some(false))
    );
    assert_eq!(frames.images.len(), 2);
    let refs = video(&videos, "veo31-references-published");
    assert_eq!(refs.images_for(VideoImageRole::Reference).count(), 2);
    assert_eq!(refs.size, Some(ImageSize::pixels(1920, 1080)));
    let kling = video(&videos, "kling-published");
    assert_eq!(
        (kling.seconds, kling.aspect_ratio.as_deref()),
        (Some(5), Some("16:9"))
    );
    assert!(kling.extras.fields.contains_key("guidance_scale"));
}

#[test]
fn an_image_prediction_is_polled_by_its_id_until_it_ends() {
    for n in ["starting-built", "processing-built", "starting-doc"] {
        let body = fixture("predictions/images", n);
        let id = pending_task(&body).unwrap_or_else(|| panic!("{n}"));
        let poll = poll_request(&id).unwrap();
        assert_eq!(
            (poll.method, poll.path),
            ("GET", format!("/v1/predictions/{id}"))
        );
        let err =
            Replicate::decode_response(200, &HeaderList::new(), &body, &ctx(RequestKind::Image))
                .unwrap_err();
        assert!(err.message.contains("before the image"), "{n}");
    }
    for n in [
        "flux-schnell-published",
        "failed-built",
        "canceled-built",
        "removed-built",
    ] {
        assert_eq!(pending_task(&fixture("predictions/images", n)), None, "{n}");
    }
    assert_eq!(pending_task(br#"{"id":"../x","status":"starting"}"#), None);
    assert!(poll_request("a/b").is_err());
}

#[test]
fn finished_image_predictions_decode_to_their_links_and_write_back() {
    for n in [
        "flux-schnell-published",
        "seedream-4.5-published",
        "flux-pro-built",
        "nano-banana-pro-built",
    ] {
        let body = fixture("predictions/images", n);
        let reply =
            Replicate::decode_response(200, &HeaderList::new(), &body, &ctx(RequestKind::Image))
                .unwrap_or_else(|e| panic!("{n}: {e:?}"));
        let back = Replicate::encode_response(
            &reply,
            &EncodeCtx::new(RequestKind::Image, StreamMode::Off),
        )
        .unwrap();
        assert_json_eq(n, &body, &back);
        let Reply::Image(r) = reply else { panic!() };
        assert_eq!(r.images.len(), 1, "{n}");
        assert!(
            matches!(&r.images[0].source, Source::Url(u) if u.starts_with("https://replicate.delivery/")),
            "{n}"
        );
        match n {
            "seedream-4.5-published" => assert_eq!(r.settings.resolution.as_deref(), Some("4K")),
            "nano-banana-pro-built" => assert_eq!(r.settings.resolution.as_deref(), Some("4K")),
            "flux-pro-built" => assert_eq!(r.settings.size, Some(ImageSize::pixels(1024, 768))),
            _ => assert_eq!(r.settings.resolution, None),
        }
    }
}

#[test]
fn predictions_that_end_without_output_are_provider_errors() {
    for (n, says) in [
        ("failed-built", "E6716: Timeout starting prediction"),
        ("canceled-built", "canceled"),
        ("removed-built", "an hour"),
    ] {
        let err = Replicate::decode_response(
            200,
            &HeaderList::new(),
            &fixture("predictions/images", n),
            &ctx(RequestKind::Image),
        )
        .unwrap_err();
        assert_eq!(err.kind, ErrorKind::ProviderError, "{n}");
        assert!(err.message.contains(says), "{n}: {}", err.message);
    }
}

#[test]
fn a_video_job_is_a_prediction_and_its_video_is_the_output_link() {
    let started = Replicate::decode_response(
        201,
        &HeaderList::new(),
        &fixture("predictions/videos", "starting-built"),
        &ctx(RequestKind::Video),
    )
    .unwrap();
    let Reply::Video(job) = started else { panic!() };
    assert_eq!(
        (job.id.as_str(), job.status),
        ("ab12cd34ef56gh78ij90kl12mn", VideoStatus::Queued)
    );
    let poll =
        Replicate::encode_video_poll(&job.id, "google/veo-3.1", &ctx(RequestKind::Video)).unwrap();
    assert_eq!(poll.path, format!("/v1/predictions/{}", job.id));
    for (n, status) in [
        ("starting-built", VideoStatus::Queued),
        ("processing-built", VideoStatus::InProgress),
        ("failed-built", VideoStatus::Failed),
        ("veo-3.1-published", VideoStatus::Completed),
        ("kling-published", VideoStatus::Completed),
    ] {
        let body = fixture("predictions/videos", n);
        let job =
            Replicate::decode_video_poll(200, &HeaderList::new(), &body, &ctx(RequestKind::Video))
                .unwrap_or_else(|e| panic!("{n}: {e:?}"));
        assert_eq!(job.status, status, "{n}");
        assert_json_eq(
            n,
            &body,
            &serde_json::to_vec(&videos::encode_job(&job)).unwrap(),
        );
        match status {
            VideoStatus::Completed => {
                let out = job.output(VideoVariant::Video).unwrap();
                assert!(
                    matches!(&out.source, Source::Url(u) if u.starts_with("https://replicate.delivery/") && u.ends_with(".mp4")),
                    "{n}"
                );
                assert!(job.expires_at.is_some(), "{n}");
            }
            VideoStatus::Failed => {
                let e = job.error.unwrap();
                assert_eq!(
                    (e.code.as_str(), e.message.as_str()),
                    ("failed", "E6716: Timeout starting prediction")
                );
            }
            _ => assert!(job.outputs.is_empty(), "{n}"),
        }
    }
}

fn image_request(model: &str) -> ImageRequest {
    ImageRequest::new(model, ImageOperation::Generate).with_prompt("a lighthouse")
}

fn refused_param(req: Request) -> String {
    let err = Replicate::check(&req, &ctx(req.kind())).unwrap_err();
    err.param.unwrap_or_default()
}

/// A change to an image request.
type ImageEdit = fn(&mut ImageRequest);

#[test]
fn image_fields_a_model_lacks_are_refused_before_sending() {
    let cases: Vec<(&str, ImageEdit, &str)> = vec![
        ("acme/other", |_| {}, "model"),
        (
            "black-forest-labs/flux-schnell",
            |r| r.size = Some(ImageSize::pixels(1024, 1024)),
            "size",
        ),
        ("black-forest-labs/flux-schnell", |r| r.count = Some(5), "n"),
        ("black-forest-labs/flux-1.1-pro", |r| r.count = Some(2), "n"),
        (
            "black-forest-labs/flux-1.1-pro",
            |r| r.size = Some(ImageSize::pixels(1000, 1000)),
            "size",
        ),
        (
            "black-forest-labs/flux-1.1-pro",
            |r| r.aspect_ratio = Some("21:9".into()),
            "aspect_ratio",
        ),
        (
            "bytedance/seedream-4.5",
            |r| r.size = Some(ImageSize::pixels(512, 512)),
            "size",
        ),
        ("bytedance/seedream-4.5", |r| r.seed = Some(1), "seed"),
        (
            "bytedance/seedream-4.5",
            |r| r.format = Some(ImageFormat::Png),
            "output_format",
        ),
        (
            "google/nano-banana-pro",
            |r| r.format = Some(ImageFormat::Webp),
            "output_format",
        ),
        (
            "google/nano-banana-pro",
            |r| r.resolution = Some("8K".into()),
            "resolution",
        ),
        (
            "black-forest-labs/flux-dev",
            |r| {
                r.operation = ImageOperation::Edit;
                r.images.push(ImageInput::new(Source::Url(
                    "https://example.com/a.png".into(),
                )));
            },
            "image",
        ),
        (
            "google/nano-banana-pro",
            |r| {
                r.mask = Some(ImageInput::new(Source::Url(
                    "https://example.com/m.png".into(),
                )))
            },
            "mask",
        ),
        (
            "black-forest-labs/flux-dev",
            |r| r.negative_prompt = Some("x".into()),
            "negative_prompt",
        ),
    ];
    for (model, edit, param) in cases {
        let mut r = image_request(model);
        edit(&mut r);
        assert_eq!(refused_param(Request::Image(r)), param, "{model} {param}");
    }
}

#[test]
fn an_openai_size_reaches_the_models_that_take_one_as_a_custom_size() {
    let sent = |r: ImageRequest| -> Value {
        let enc =
            Replicate::encode_request(&Request::Image(r), &mut ctx(RequestKind::Image)).unwrap();
        serde_json::from_slice::<Value>(&enc.body).unwrap()["input"].clone()
    };
    let mut pro = image_request("black-forest-labs/flux-1.1-pro");
    pro.size = Some(ImageSize::pixels(1440, 1024));
    assert_eq!(
        sent(pro),
        json!({"prompt": "a lighthouse", "aspect_ratio": "custom", "width": 1440, "height": 1024})
    );
    let mut seedream = image_request("bytedance/seedream-4.5");
    seedream.count = Some(1);
    seedream.resolution = Some("2k".into());
    assert_eq!(
        sent(seedream),
        json!({"prompt": "a lighthouse", "size": "2K"})
    );
    let mut auto = image_request("black-forest-labs/flux-schnell");
    auto.size = Some(ImageSize::Auto);
    auto.format = Some(ImageFormat::Jpeg);
    assert_eq!(
        sent(auto),
        json!({"prompt": "a lighthouse", "output_format": "jpg"})
    );
}

fn video_request(model: &str, seconds: u32) -> VideoRequest {
    let mut r = VideoRequest::new(model, VideoOperation::Generate);
    r.prompt = Some("a paper boat".into());
    r.seconds = Some(seconds);
    r
}

/// A change to a video request.
type VideoEdit = fn(&mut VideoRequest);

#[test]
fn video_fields_a_model_lacks_are_refused_before_sending() {
    let first = || {
        VideoImage::new(
            Source::Url("https://example.com/f.png".into()),
            VideoImageRole::FirstFrame,
        )
    };
    let reference = || {
        VideoImage::new(
            Source::Url("https://example.com/r.png".into()),
            VideoImageRole::Reference,
        )
    };
    let cases: Vec<(&str, u32, VideoEdit, &str)> = vec![
        ("acme/video", 8, |_| {}, "model"),
        ("google/veo-3.1", 5, |_| {}, "seconds"),
        ("google/veo-3.1", 8, |r| r.seconds = None, "seconds"),
        (
            "google/veo-3.1",
            8,
            |r| r.size = Some(ImageSize::pixels(3840, 2160)),
            "size",
        ),
        (
            "google/veo-3.1",
            8,
            |r| r.aspect_ratio = Some("1:1".into()),
            "aspect_ratio",
        ),
        ("kwaivgi/kling-v2.5-turbo-pro", 8, |_| {}, "seconds"),
        (
            "kwaivgi/kling-v2.5-turbo-pro",
            5,
            |r| r.size = Some(ImageSize::pixels(1280, 720)),
            "size",
        ),
        (
            "kwaivgi/kling-v2.5-turbo-pro",
            5,
            |r| r.seed = Some(1),
            "seed",
        ),
        (
            "kwaivgi/kling-v2.5-turbo-pro",
            5,
            |r| r.audio = Some(true),
            "audio",
        ),
        (
            "kwaivgi/kling-v2.5-turbo-pro",
            5,
            |r| {
                r.images.push(VideoImage::new(
                    Source::Url("https://example.com/f.png".into()),
                    VideoImageRole::FirstFrame,
                ));
                r.aspect_ratio = Some("1:1".into());
            },
            "aspect_ratio",
        ),
        (
            "google/veo-3.1",
            6,
            |r| {
                r.images.push(VideoImage::new(
                    Source::Url("https://example.com/r.png".into()),
                    VideoImageRole::Reference,
                ));
            },
            "input_reference",
        ),
    ];
    for (model, seconds, edit, param) in cases {
        let mut r = video_request(model, seconds);
        edit(&mut r);
        assert_eq!(refused_param(Request::Video(r)), param, "{model} {param}");
    }
    let mut last_only = video_request("google/veo-3.1", 8);
    last_only.images.push(VideoImage::new(
        Source::Url("https://example.com/l.png".into()),
        VideoImageRole::LastFrame,
    ));
    assert_eq!(refused_param(Request::Video(last_only)), "input_reference");
    let mut ok = video_request("google/veo-3.1", 8);
    ok.images.push(reference());
    ok.size = Some(ImageSize::pixels(1280, 720));
    Replicate::check(&Request::Video(ok), &ctx(RequestKind::Video)).unwrap();
    let mut kling = video_request("kwaivgi/kling-v2.5-turbo-pro", 10);
    kling.images.push(first());
    let enc =
        Replicate::encode_request(&Request::Video(kling), &mut ctx(RequestKind::Video)).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&enc.body).unwrap()["input"],
        json!({"prompt": "a paper boat", "duration": 10, "start_image": "https://example.com/f.png"})
    );
}

#[test]
fn a_model_id_that_is_not_owner_and_name_never_reaches_a_path() {
    for model in ["flux", "a/b/c", "../x/y", "a/.."] {
        assert!(
            submit(model, serde_json::Map::new(), true).is_err(),
            "{model}"
        );
    }
}

#[test]
fn errors_decode_with_their_kind_and_write_back() {
    for (name, body) in fixtures(DIR, "errors", "json") {
        let meta = sidecar(DIR, "errors", &name, "meta").unwrap();
        let status = u16::try_from(meta["status"].as_u64().unwrap()).unwrap();
        let err = Replicate::decode_error(status, &HeaderList::new(), &body);
        let kind = if status == 429 {
            ErrorKind::RateLimited
        } else {
            ErrorKind::Authentication
        };
        assert_eq!(err.kind, kind, "{name}");
        assert_eq!(err.status, Some(status), "{name}");
        let enc = Replicate::encode_error(&err);
        assert_eq!(enc.status, status, "{name}");
        assert_json_eq(&name, &body, &enc.body);
    }
    let err = Replicate::decode_error(
        401,
        &HeaderList::new(),
        &fixture("errors", "recorded-invalid-token-401"),
    );
    assert_eq!(
        err.message,
        "Unauthenticated: You did not pass a valid authentication token"
    );
    let ours = Replicate::encode_error(&KppError::invalid_request("No.").with_param("size"));
    assert_eq!(ours.status, 422);
    assert_eq!(
        serde_json::from_slice::<Value>(&ours.body).unwrap(),
        json!({"title": "Invalid input", "detail": "No.", "status": 422})
    );
}

#[test]
fn an_envelope_field_other_than_input_is_refused() {
    let err = Replicate::decode_request(
        RequestKind::Image,
        br#"{"input": {"prompt": "x"}, "webhook": "https://example.com/hook"}"#,
        &HeaderList::new(),
    )
    .unwrap_err();
    assert_eq!(err.param.as_deref(), Some("webhook"));
    assert!(
        Replicate::decode_request(
            RequestKind::Image,
            br#"{"prompt": "x"}"#,
            &HeaderList::new()
        )
        .is_err()
    );
}

#[test]
fn the_model_check_reads_the_official_models_collection() {
    let req = Replicate::encode_models_request(&ModelsQuery::default()).unwrap();
    assert_eq!(
        (req.method, req.path.as_str()),
        ("GET", "/v1/collections/official")
    );
    let list = Replicate::decode_models(
        200,
        &HeaderList::new(),
        &fixture("models", "official-collection-built"),
    )
    .unwrap();
    let ids: Vec<&str> = list.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["black-forest-labs/flux-schnell", "google/veo-3.1"]);
    assert_eq!(
        Replicate::encode_model_request("google/veo-3.1").path,
        "/v1/models/google/veo-3.1"
    );
    assert_eq!(
        Replicate::encode_model_request("a/../b").path,
        "/v1/models/a/..%2Fb"
    );
    assert_eq!(
        Replicate::encode_model_request("../x").path,
        "/v1/models/%2E%2E/x"
    );
    let err = Replicate::decode_models(
        401,
        &HeaderList::new(),
        &fixture("errors", "recorded-invalid-token-401"),
    )
    .unwrap_err();
    assert_eq!(err.kind, ErrorKind::Authentication);
}

#[test]
fn every_status_names_its_kind_and_odd_bodies_still_read() {
    for (status, kind) in [
        (400, ErrorKind::InvalidRequest),
        (409, ErrorKind::InvalidRequest),
        (422, ErrorKind::InvalidRequest),
        (401, ErrorKind::Authentication),
        (402, ErrorKind::Billing),
        (403, ErrorKind::PermissionDenied),
        (404, ErrorKind::NotFound),
        (413, ErrorKind::RequestTooLarge),
        (429, ErrorKind::RateLimited),
        (503, ErrorKind::Overloaded),
        (504, ErrorKind::Timeout),
        (500, ErrorKind::ProviderError),
    ] {
        let err = Replicate::decode_error(status, &HeaderList::new(), br#"{"detail": "No."}"#);
        assert_eq!((err.kind, err.message.as_str()), (kind, "No."), "{status}");
    }
    let err = Replicate::decode_error(
        500,
        &HeaderList::new(),
        br#"{"title": "Internal server error", "detail": ""}"#,
    );
    assert_eq!(err.message, "Internal server error");
    let err = Replicate::decode_error(502, &HeaderList::new(), b"  <html>bad gateway</html>\n");
    assert_eq!(
        err.message,
        "Replicate returned HTTP 502 with a body Kaeso could not read: <html>bad gateway</html>"
    );
    assert!(err.raw.is_none());
    let err = Replicate::decode_error(
        429,
        &HeaderList::new().with("retry-after", "7"),
        br#"{"detail": "Request was throttled."}"#,
    );
    assert_eq!(err.retry_after, Some(std::time::Duration::from_secs(7)));
    assert_eq!(
        Replicate::encode_error(&err).headers.get("retry-after"),
        Some("7")
    );
}

#[test]
fn errors_of_other_formats_become_problem_details() {
    for (kind, status, title) in [
        (ErrorKind::Authentication, 401, "Unauthenticated"),
        (ErrorKind::Billing, 402, "Invalid input"),
        (ErrorKind::PermissionDenied, 403, "Invalid input"),
        (ErrorKind::NotFound, 404, "Not found"),
        (ErrorKind::RequestTooLarge, 413, "Invalid input"),
        (ErrorKind::RateLimited, 429, "Too many requests"),
        (ErrorKind::ProviderError, 500, "Internal server error"),
        (ErrorKind::Overloaded, 503, "Internal server error"),
        (ErrorKind::Timeout, 504, "Internal server error"),
        (ErrorKind::InvalidRequest, 422, "Invalid input"),
    ] {
        let name = format!("{kind:?}");
        let enc = Replicate::encode_error(&KppError::new(kind, "Elsewhere."));
        assert_eq!(enc.status, status, "{name}");
        assert_eq!(
            enc.headers.get("content-type"),
            Some("application/problem+json")
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&enc.body).unwrap(),
            json!({"title": title, "detail": "Elsewhere.", "status": status}),
            "{name}"
        );
    }
}

#[test]
fn a_prediction_that_ends_unfinished_says_why() {
    for (prediction, message) in [
        (
            json!({"status": "canceled"}),
            "The prediction was canceled at Replicate.",
        ),
        (
            json!({"status": "aborted"}),
            "Replicate aborted the prediction before it started.",
        ),
        (
            json!({"status": "failed"}),
            "Replicate could not finish the prediction.",
        ),
        (
            json!({"status": "failed", "error": "NSFW content detected."}),
            "NSFW content detected.",
        ),
    ] {
        let err = error::failed(&prediction);
        assert_eq!(err.kind, ErrorKind::ProviderError);
        assert_eq!((err.status, err.message.as_str()), (Some(502), message));
        assert_eq!(err.raw.as_ref(), Some(&prediction));
    }
}

#[test]
fn image_requests_replicate_cannot_make_are_refused_before_sending() {
    let cases: Vec<(&str, ImageEdit, &str)> = vec![
        (
            "black-forest-labs/flux-dev",
            |r| r.operation = ImageOperation::Variation,
            "image",
        ),
        (
            "black-forest-labs/flux-dev",
            |r| r.prompt = Some(String::new()),
            "prompt",
        ),
        (
            "black-forest-labs/flux-dev",
            |r| r.stream = StreamMode::on(false),
            "stream",
        ),
        (
            "bytedance/seedream-4.5",
            |r| {
                r.operation = ImageOperation::Edit;
                r.images = (0..15)
                    .map(|_| ImageInput::new(Source::Url("https://example.com/a.png".into())))
                    .collect();
            },
            "image",
        ),
        (
            "bytedance/seedream-4.5",
            |r| {
                r.operation = ImageOperation::Edit;
                r.images.push(ImageInput::new(Source::file_id(
                    DialectId::OpenAiImages,
                    "file-1",
                )));
            },
            "image",
        ),
        (
            "bytedance/seedream-4.5",
            |r| r.compression = Some(80),
            "output_compression",
        ),
        (
            "black-forest-labs/flux-dev",
            |r| r.quality = Some(ImageQuality::High),
            "quality",
        ),
        (
            "black-forest-labs/flux-dev",
            |r| r.background = Some(ImageBackground::Transparent),
            "background",
        ),
    ];
    for (model, edit, param) in cases {
        let mut r = image_request(model);
        edit(&mut r);
        assert_eq!(refused_param(Request::Image(r)), param, "{model} {param}");
    }
}
