use kaeso_kpp::{
    ErrorKind, ImageBackground, ImageInput, ImageOperation, ImageQuality, ImageRequest, ImageSize,
    Origin, Reply, Request, RequestKind, Source, StreamMode,
};

use super::*;
use crate::test_support::{assert_json_eq, fixtures};

fn fixture(folder: &str, name: &str) -> Vec<u8> {
    fixtures("elevenlabs", &format!("image/{folder}"), "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("{folder}/{name}"))
        .1
}

fn sent(r: &ImageRequest) -> Result<EncodedRequest, KppError> {
    let req = Request::Image(r.clone());
    let mut ctx = EgressCtx::new(&req);
    ElevenLabs::check(&req, &ctx)?;
    ElevenLabs::encode_request(&req, &mut ctx)
}

fn image(model: &str) -> ImageRequest {
    ImageRequest::new(model, ImageOperation::Generate).with_prompt(
        "A corgi in a tiny lifeguard chair on a sunlit beach at golden hour, photorealistic",
    )
}

#[test]
fn openai_image_requests_become_the_documented_generation_bodies() {
    let mut edit = image("gpt-image-1.5");
    edit.operation = ImageOperation::Edit;
    edit.images.push(ImageInput::new(
        Source::base64("image/png", "iVBORw0KGgo=").unwrap(),
    ));
    edit.images.push(ImageInput::new(Source::file_id(
        Origin::new(D),
        "5xM2KqOnZyce22SPZ9d4",
    )));
    edit.quality = Some(ImageQuality::High);
    edit.background = Some(ImageBackground::Transparent);
    edit.size = Some(ImageSize::pixels(1536, 1024));
    let enc = sent(&edit).unwrap();
    assert_eq!((enc.method, enc.path.as_str()), ("POST", IMAGE_PATH));
    assert_json_eq(
        "gpt-image-edit",
        &fixture("requests", "gpt-image-edit"),
        &enc.body,
    );

    let mut gemini = image("gemini-3-pro-image");
    gemini.aspect_ratio = Some("16:9".into());
    gemini.resolution = Some("2K".into());
    assert_json_eq(
        "gemini",
        &fixture("requests", "gemini"),
        &sent(&gemini).unwrap().body,
    );

    let mut seedream = image("bytedance-seedream-5-pro");
    seedream.size = Some(ImageSize::pixels(2048, 2048));
    seedream.resolution = Some("2K".into());
    seedream.seed = Some(42);
    assert_json_eq(
        "seedream",
        &fixture("requests", "seedream"),
        &sent(&seedream).unwrap().body,
    );
}

#[test]
fn what_a_model_lacks_is_refused_with_its_field_before_sending() {
    let param = |r: ImageRequest| sent(&r).unwrap_err().param.unwrap_or_default();
    let with = |model: &str, f: fn(&mut ImageRequest)| {
        let mut r = image(model);
        f(&mut r);
        r
    };
    assert_eq!(param(image("dall-e-3")), "model");
    assert_eq!(param(with("gpt-image-1", |r| r.count = Some(2))), "n");
    assert_eq!(
        param(with("gpt-image-1", |r| r.size =
            Some(ImageSize::pixels(1792, 1024)))),
        "size"
    );
    assert_eq!(
        param(with("gpt-image-1", |r| r.aspect_ratio = Some("16:9".into()))),
        "aspect_ratio"
    );
    assert_eq!(
        param(with("gpt-image-1", |r| r.resolution = Some("2K".into()))),
        "resolution"
    );
    assert_eq!(
        param(with("gpt-image-1", |r| r.quality = Some(ImageQuality::Max))),
        "quality"
    );
    assert!(sent(&with("gpt-image-1", |r| r.quality = Some(ImageQuality::Auto))).is_ok());
    assert_eq!(
        param(with("gpt-image-2", |r| r.background =
            Some(ImageBackground::Opaque))),
        "background"
    );
    assert_eq!(param(with("gpt-image-2", |r| r.seed = Some(1))), "seed");
    assert_eq!(
        param(with("gemini-3-pro-image", |r| {
            r.operation = ImageOperation::Edit;
            r.images.push(ImageInput::new(
                Source::base64("image/png", "AA==").unwrap(),
            ));
            r.mask = Some(ImageInput::new(
                Source::base64("image/png", "AA==").unwrap(),
            ));
        })),
        "mask"
    );
    assert_eq!(
        param(with("gpt-image-1", |r| {
            r.operation = ImageOperation::Edit;
            r.images.push(ImageInput::new(Source::Url(
                "https://example.com/a.png".into(),
            )));
        })),
        "image"
    );
    assert_eq!(
        param(with("gemini-2.5-flash-image", |r| {
            for _ in 0..6 {
                r.images.push(ImageInput::new(
                    Source::base64("image/png", "AA==").unwrap(),
                ));
            }
        })),
        "image"
    );
    assert_eq!(
        param(with("gpt-image-1", |r| r.format =
            Some(kaeso_kpp::ImageFormat::Jpeg))),
        "output_format"
    );
    assert_eq!(
        param(with("gpt-image-1", |r| r.operation = ImageOperation::Variation)),
        "image"
    );
    assert_eq!(param(with("gpt-image-1", |r| r.prompt = None)), "prompt");
}

#[test]
fn a_generation_is_polled_until_it_ends_with_a_link_or_a_failure() {
    let started = fixture("responses", "started");
    assert_eq!(
        crate::dispatch::pending_task(D, RequestKind::Image, &started).as_deref(),
        Some("JWr5N6X9ZTqf8jD2LmQb")
    );
    assert!(
        crate::dispatch::pending_task(D, RequestKind::Image, &fixture("polls", "generating"))
            .is_some()
    );
    assert_eq!(
        crate::dispatch::pending_task(D, RequestKind::Image, &fixture("polls", "completed")),
        None
    );
    let poll = crate::dispatch::encode_task_poll(
        D,
        "JWr5N6X9ZTqf8jD2LmQb",
        &EgressCtx::for_kind(RequestKind::Image, StreamMode::Off),
    )
    .unwrap();
    assert_eq!(
        (poll.method, poll.path.as_str()),
        ("GET", "/v1/flows/image/JWr5N6X9ZTqf8jD2LmQb")
    );
    let poll_ctx = EgressCtx::for_kind(RequestKind::Image, StreamMode::Off);
    assert!(crate::dispatch::encode_task_poll(D, "..", &poll_ctx).is_err());
    let odd = crate::dispatch::encode_task_poll(D, "a/../b", &poll_ctx).unwrap();
    assert_eq!(odd.path.matches('/').count(), 4, "{}", odd.path);
    assert_eq!(
        crate::dispatch::task_poll_gap(D),
        std::time::Duration::from_secs(2)
    );

    let ctx = EgressCtx::for_kind(RequestKind::Image, StreamMode::Off);
    let Reply::Image(done) = ElevenLabs::decode_response(
        200,
        &HeaderList::new(),
        &fixture("polls", "completed"),
        &ctx,
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(
        done.images[0].source,
        Source::Url("https://storage.googleapis.com/generations/JWr5N6X9ZTqf8jD2LmQb".into())
    );
    assert!(
        done.images[0]
            .extras
            .fields
            .contains_key("content_mime_type")
    );

    let failed =
        ElevenLabs::decode_response(200, &HeaderList::new(), &fixture("polls", "failed"), &ctx)
            .unwrap_err();
    assert_eq!(
        (failed.kind, failed.status),
        (ErrorKind::Timeout, Some(504))
    );
    assert!(failed.message.starts_with("Timed out while processing."));
    let moderated =
        br#"{"id":"x","status":"failed","failure_reason":"moderated","error_message":"Flagged."}"#;
    let e = ElevenLabs::decode_response(200, &HeaderList::new(), moderated, &ctx).unwrap_err();
    assert_eq!((e.kind, e.status), (ErrorKind::ContentFiltered, Some(400)));
    assert!(
        ElevenLabs::decode_response(
            200,
            &HeaderList::new(),
            &fixture("polls", "generating"),
            &ctx
        )
        .is_err()
    );
    assert!(ElevenLabs::decode_response(200, &HeaderList::new(), b"not json", &ctx).is_err());
    assert!(
        ElevenLabs::decode_response(
            402,
            &HeaderList::new(),
            br#"{"detail":{"code":"paid_plan_required","message":"No."}}"#,
            &ctx
        )
        .is_err()
    );
}
