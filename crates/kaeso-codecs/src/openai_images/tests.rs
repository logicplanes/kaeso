use kaeso_kpp::{
    DialectId, ErrorKind, ImageDelivery, ImageEvent, ImageFormat, ImageInput, ImageOperation,
    ImageQuality, ImageRequest, ImageResponse, ImageSize, Reply, Request, RequestKind, Source,
    StreamEvent, StreamMode, Usage,
};
use serde_json::{Value, json};

use super::*;
use crate::multipart::{FormLimits, form_boundary, read_form};
use crate::test_support::{assert_json_eq, assert_sse_eq, fixtures, sidecar};

const DIR: &str = "openai_images";
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";

fn operation_of(name: &str) -> ImageOperation {
    if name.starts_with("edit") {
        ImageOperation::Edit
    } else {
        ImageOperation::Generate
    }
}

fn json_headers() -> HeaderList {
    HeaderList::new().with("content-type", "application/json")
}

fn decode(operation: ImageOperation, body: &[u8], headers: &HeaderList) -> ImageRequest {
    match OpenAiImages::decode_call(operation, body, headers).unwrap() {
        Request::Image(r) => r,
        other => panic!("{other:?}"),
    }
}

fn egress(profile: ImagesProfile, req: &ImageRequest) -> EgressCtx {
    EgressCtx::new(&Request::Image(req.clone()))
        .with_provider("openai")
        .with_images_profile(profile)
}

fn encode(
    profile: ImagesProfile,
    req: &ImageRequest,
) -> Result<(EncodedRequest, EgressCtx), kaeso_kpp::KppError> {
    let mut ctx = egress(profile, req);
    let enc = OpenAiImages::encode_request(&Request::Image(req.clone()), &mut ctx)?;
    Ok((enc, ctx))
}

fn body_of(enc: &EncodedRequest) -> Value {
    serde_json::from_slice(&enc.body).unwrap()
}

fn generate(prompt: &str) -> ImageRequest {
    ImageRequest::new("m", ImageOperation::Generate).with_prompt(prompt)
}

fn png() -> ImageInput {
    ImageInput::new(Source::base64("image/png", PNG).unwrap()).with_filename("fox.png")
}

fn refusal(profile: ImagesProfile, req: &ImageRequest) -> (String, String) {
    let err = encode(profile, req).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
    (err.param.unwrap_or_default(), err.message)
}

#[test]
fn every_json_request_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "requests", "json") {
        let req = decode(operation_of(&name), &body, &json_headers());
        req.validate().unwrap();
        let (enc, _) = encode(ImagesProfile::OPENAI, &req).unwrap();
        let path = if name.starts_with("edit") {
            "/v1/images/edits"
        } else {
            "/v1/images/generations"
        };
        assert_eq!(enc.path, path, "{name}");
        assert_eq!(enc.headers.get("content-type"), Some("application/json"));
        assert_json_eq(&name, &body, &enc.body);
    }
}

#[test]
fn typed_fields_carry_what_every_provider_shares() {
    let body = fixtures(DIR, "requests", "json")
        .into_iter()
        .find(|(n, _)| n == "generate-every-field")
        .unwrap()
        .1;
    let r = decode(ImageOperation::Generate, &body, &json_headers());
    assert_eq!(r.count, Some(2));
    assert_eq!(
        r.size.as_ref().and_then(ImageSize::dimensions),
        Some((1536, 864))
    );
    assert_eq!(r.quality, Some(ImageQuality::XHigh));
    assert_eq!(r.format, Some(ImageFormat::Webp));
    assert_eq!(r.compression, Some(80));
    assert_eq!(r.partial_images, Some(2));
    assert!(!r.stream.is_off());
    assert_eq!(r.user.as_deref(), Some("user-7f3a"));
    let fields: Vec<&str> = r.extras.fields.keys().map(String::as_str).collect();
    assert_eq!(fields, ["moderation", "complexity", "user"]);
}

fn upload() -> (Vec<u8>, HeaderList) {
    let body = std::fs::read(
        crate::test_support::fixtures_dir().join("openai_images/requests/edit-upload.multipart"),
    )
    .unwrap();
    let meta = sidecar(DIR, "requests", "edit-upload", "meta").unwrap();
    let headers = HeaderList::new().with("content-type", meta["content_type"].as_str().unwrap());
    (body, headers)
}

fn parts_of(content_type: &str, body: &[u8]) -> Vec<(String, Option<String>, Vec<u8>)> {
    let boundary = form_boundary(content_type).unwrap();
    let mut parts: Vec<_> = read_form(body, &boundary, FormLimits::new(64, 1 << 20))
        .unwrap()
        .into_iter()
        .map(|p| (p.name, p.filename, p.body.to_vec()))
        .collect();
    parts.sort();
    parts
}

// A form's boundary and the order of its fields are the writer's own, so the round trip
// compares the parts as a set.
#[test]
fn an_upload_round_trips_part_for_part() {
    let (body, headers) = upload();
    let r = decode(ImageOperation::Edit, &body, &headers);
    r.validate().unwrap();
    assert_eq!(r.model, "gpt-image-1.5");
    assert_eq!(r.images.len(), 2);
    assert_eq!(r.images[0].filename.as_deref(), Some("body-lotion.png"));
    assert!(r.mask.is_some());
    assert_eq!(r.count, Some(2));
    assert_eq!(r.compression, Some(70));
    assert_eq!(r.extras.fields["input_fidelity"].get(), "\"high\"");
    assert_eq!(r.extras.fields["svg"].get(), r#"{"model":"gpt-6-astra"}"#);
    assert_eq!(r.extras.fields["frame_count"].get(), "9");
    let (enc, _) = encode(ImagesProfile::OPENAI, &r).unwrap();
    assert_eq!(enc.path, "/v1/images/edits");
    let ct = enc.headers.get("content-type").unwrap();
    assert!(ct.starts_with("multipart/form-data; boundary="));
    assert_eq!(
        parts_of(ct, &enc.body),
        parts_of(headers.get("content-type").unwrap(), &body)
    );
}

#[test]
fn uploads_that_break_the_rules_are_refused_with_the_field() {
    let form = |fields: &str| {
        let body = format!("--b\r\n{fields}\r\n--b--\r\n");
        let headers = HeaderList::new().with("content-type", "multipart/form-data; boundary=b");
        OpenAiImages::decode_call(ImageOperation::Edit, body.as_bytes(), &headers).unwrap_err()
    };
    let err = form("Content-Disposition: form-data; name=\"n\"\r\n\r\ntwo");
    assert_eq!(err.param.as_deref(), Some("n"));
    let err = form("Content-Disposition: form-data; name=\"stream\"\r\n\r\nyes");
    assert_eq!(err.param.as_deref(), Some("stream"));
    let err = form("Content-Disposition: form-data; name=\"image\"\r\n\r\nnot a file");
    assert_eq!(err.param.as_deref(), Some("image"));
    let err = form("Content-Disposition: form-data; name=\"logo\"; filename=\"a.png\"\r\n\r\nx");
    assert_eq!(err.param.as_deref(), Some("logo"));
    let err = form("Content-Disposition: form-data; name=\"output_compression\"\r\n\r\n300");
    assert_eq!(err.param.as_deref(), Some("output_compression"));
    let headers = HeaderList::new().with("content-type", "multipart/form-data");
    assert!(OpenAiImages::decode_call(ImageOperation::Edit, b"x", &headers).is_err());
    let err =
        OpenAiImages::decode_call(ImageOperation::Variation, b"{}", &json_headers()).unwrap_err();
    assert!(err.message.contains("multipart/form-data"));
    let err = OpenAiImages::decode_call(
        ImageOperation::Generate,
        br#"{"model":"m","prompt":"p","images":[]}"#,
        &json_headers(),
    )
    .unwrap_err();
    assert_eq!(err.param.as_deref(), Some("images"));
    let err = OpenAiImages::decode_call(
        ImageOperation::Generate,
        br#"{"model":"m","n":"two"}"#,
        &json_headers(),
    )
    .unwrap_err();
    assert_eq!(err.param.as_deref(), Some("n"));
    let err = OpenAiImages::decode_call(
        ImageOperation::Generate,
        br#"{"model":"m","response_format":"png"}"#,
        &json_headers(),
    )
    .unwrap_err();
    assert_eq!(err.param.as_deref(), Some("response_format"));
}

#[test]
fn a_body_without_a_path_is_read_by_its_shape() {
    let req = OpenAiImages::decode_request(
        RequestKind::Image,
        br#"{"model":"m","prompt":"p","images":[{"image_url":"https://example.com/a.png"}]}"#,
        &json_headers(),
    )
    .unwrap();
    let Request::Image(r) = req else { panic!() };
    assert_eq!(r.operation, ImageOperation::Edit);
    let (body, headers) = upload();
    let Request::Image(r) =
        OpenAiImages::decode_request(RequestKind::Image, &body, &headers).unwrap()
    else {
        panic!()
    };
    assert_eq!(r.operation, ImageOperation::Edit);
    let Request::Image(r) = OpenAiImages::decode_request(
        RequestKind::Image,
        br#"{"model":"m","prompt":"p"}"#,
        &json_headers(),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(r.operation, ImageOperation::Generate);
}

#[test]
fn an_upload_with_a_link_goes_out_as_json_edits() {
    let mut r = ImageRequest::new("gpt-image-2", ImageOperation::Edit)
        .with_prompt("hat")
        .with_image(png())
        .with_image(ImageInput::new(Source::Url(
            "https://example.com/b.png".into(),
        )));
    r.mask = Some(png());
    let (enc, _) = encode(ImagesProfile::OPENAI, &r).unwrap();
    assert_eq!(
        body_of(&enc),
        json!({
            "model": "gpt-image-2",
            "prompt": "hat",
            "images": [
                {"image_url": format!("data:image/png;base64,{PNG}")},
                {"image_url": "https://example.com/b.png"}
            ],
            "mask": {"image_url": format!("data:image/png;base64,{PNG}")}
        })
    );
    r.images.truncate(1);
    let (enc, _) = encode(ImagesProfile::OPENAI, &r).unwrap();
    let ct = enc.headers.get("content-type").unwrap();
    let parts = parts_of(ct, &enc.body);
    let names: Vec<&str> = parts.iter().map(|p| p.0.as_str()).collect();
    assert_eq!(names, ["image[]", "mask", "model", "prompt"]);
}

#[test]
fn every_response_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "responses", "json") {
        let req = Request::Image(generate("p"));
        let ctx = EgressCtx::new(&req);
        let reply = OpenAiImages::decode_response(200, &HeaderList::new(), &body, &ctx).unwrap();
        let enc_ctx = EncodeCtx::for_request(req).with_created_at(1_700_000_000);
        let out = OpenAiImages::encode_response(&reply, &enc_ctx).unwrap();
        let original: Value = serde_json::from_slice(&body).unwrap();
        let mut written: Value = serde_json::from_slice(&out).unwrap();
        // `created` is required in `ImagesResponse`; a provider that leaves it out gets the
        // time Kaeso received the request.
        if original.get("created").is_none() {
            assert_eq!(written["created"], 1_700_000_000, "{name}");
            written.as_object_mut().unwrap().remove("created");
        }
        assert_json_eq(&name, &body, &serde_json::to_vec(&written).unwrap());
    }
}

#[test]
fn replies_carry_images_usage_and_media_types() {
    let reply = |name: &str| {
        let body = fixtures(DIR, "responses", "json")
            .into_iter()
            .find(|(n, _)| n == name)
            .unwrap()
            .1;
        let req = Request::Image(generate("p"));
        match OpenAiImages::decode_response(200, &HeaderList::new(), &body, &EgressCtx::new(&req))
            .unwrap()
        {
            Reply::Image(r) => r,
            other => panic!("{other:?}"),
        }
    };
    let r = reply("generate-every-field");
    assert_eq!(r.images.len(), 2);
    assert_eq!(r.usage.input_tokens, 22);
    assert_eq!(r.usage.output_tokens, 4228);
    assert_eq!(r.usage.output_image_tokens, Some(4228));
    assert_eq!(r.usage.input_image_tokens, Some(0));
    let Source::Base64(b) = &r.images[0].source else {
        panic!()
    };
    assert_eq!(b.media_type, "image/webp");
    let r = reply("providers/xai/edit-b64");
    assert_eq!(r.usage.input_image_tokens, Some(1289));
    let Source::Base64(b) = &r.images[0].source else {
        panic!()
    };
    assert_eq!(b.media_type, "image/png");
    let r = reply("providers/openrouter/generate");
    assert_eq!((r.usage.input_tokens, r.usage.output_tokens), (15, 1290));
    assert_eq!(r.usage.output_image_tokens, Some(1290));
    let r = reply("legacy-dall-e-3-url");
    assert!(matches!(&r.images[0].source, Source::Url(u) if u.ends_with("otter.png")));
    assert!(r.images[0].revised_prompt.is_some());
    let r = reply("providers/togetherai/generate-url");
    assert_eq!(r.model, "black-forest-labs/FLUX.2-pro");
    assert_eq!(r.settings.resolution, None);
    let r = reply("providers/alibaba/generate");
    assert_eq!(r.settings.resolution.as_deref(), Some("1K"));
    let r = reply("providers/alibaba/generate-2k");
    assert_eq!(r.settings.resolution.as_deref(), Some("2K"));
    let err = OpenAiImages::decode_response(
        200,
        &HeaderList::new(),
        br#"{"data":[{}]}"#,
        &EgressCtx::new(&Request::Image(generate("p"))),
    )
    .unwrap_err();
    assert!(err.message.contains("b64_json"));
}

#[test]
fn a_reply_from_another_format_gets_usage_in_openai_s_shape() {
    let mut r = ImageResponse::new(
        "gemini-3.1-flash-image",
        vec![kaeso_kpp::GeneratedImage::new(
            Source::base64("image/png", PNG).unwrap(),
        )],
    );
    let mut usage = Usage::new(12, 1120);
    usage.output_image_tokens = Some(1120);
    r.usage = usage;
    let ctx = EncodeCtx::for_request(Request::Image(generate("p"))).with_created_at(7);
    let out: Value =
        serde_json::from_slice(&OpenAiImages::encode_response(&Reply::Image(r), &ctx).unwrap())
            .unwrap();
    assert_eq!(
        out,
        json!({
            "created": 7,
            "data": [{"b64_json": PNG}],
            "usage": {
                "input_tokens": 12,
                "output_tokens": 1120,
                "total_tokens": 1132,
                "input_tokens_details": {"text_tokens": 12, "image_tokens": 0},
                "output_tokens_details": {"image_tokens": 1120, "text_tokens": 0}
            }
        })
    );
}

fn stream_ctx(operation: ImageOperation) -> EncodeCtx {
    let mut r = generate("p");
    r.operation = operation;
    r.stream = StreamMode::on(true);
    EncodeCtx::for_request(Request::Image(r))
}

#[test]
fn every_stream_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "streams", "sse") {
        let op = operation_of(&name);
        let req = Request::Image(ImageRequest::new("m", op));
        let mut dec = OpenAiImages::stream_decoder(&EgressCtx::new(&req));
        let mut events = dec.feed(&body);
        events.extend(dec.finish());
        assert!(
            events.iter().all(|e| matches!(e, StreamEvent::Image(_))),
            "{name}: {events:?}"
        );
        let mut enc = OpenAiImages::stream_encoder(&stream_ctx(op));
        let mut out = Vec::new();
        for e in &events {
            out.extend(enc.encode(e).unwrap());
        }
        out.extend(enc.finish().unwrap());
        assert_sse_eq(&name, &body, &out);
    }
}

#[test]
fn streams_carry_partial_and_finished_images() {
    let body = fixtures(DIR, "streams", "sse")
        .into_iter()
        .find(|(n, _)| n == "edit-every-field")
        .unwrap()
        .1;
    let req = Request::Image(ImageRequest::new("m", ImageOperation::Edit));
    let mut dec = OpenAiImages::stream_decoder(&EgressCtx::new(&req));
    let events = dec.feed(&body);
    let StreamEvent::Image(first) = &events[0] else {
        panic!()
    };
    assert_eq!(first.partial, Some(0));
    assert_eq!(first.created, Some(1_620_000_000));
    let StreamEvent::Image(last) = &events[1] else {
        panic!()
    };
    assert_eq!(last.partial, None);
    assert_eq!(
        last.usage.as_ref().map(|u| u.input_image_tokens),
        Some(Some(40))
    );
    let mut dec = OpenAiImages::stream_decoder(&EgressCtx::new(&req));
    let events = dec.feed(b"data: {\"type\":\"image_generation.completed\",\"b64_json\":\"AA==\",\"media_type\":\"image/jpeg\"}\n\ndata: {\"type\":\"error\",\"error\":{\"message\":\"blocked\"}}\n\ndata: [DONE]\n\n");
    let StreamEvent::Image(img) = &events[0] else {
        panic!()
    };
    let Source::Base64(b) = &img.image.source else {
        panic!()
    };
    assert_eq!(b.media_type, "image/jpeg");
    assert!(matches!(&events[1], StreamEvent::Error(e) if e.message == "blocked"));
    assert_eq!(events.len(), 2);
    let mut dec = OpenAiImages::stream_decoder(&EgressCtx::new(&req));
    let events = dec.feed(b": keepalive\n\nevent: x\ndata: {\"type\":\"image_generation.text_chunk\",\"text\":\"a\"}\n\n");
    assert!(events.iter().all(|e| matches!(e, StreamEvent::Extra(_))));
}

#[test]
fn events_from_another_format_say_every_required_field() {
    let mut enc =
        OpenAiImages::stream_encoder(&stream_ctx(ImageOperation::Generate).with_created_at(9));
    let event = ImageEvent::finished(
        kaeso_kpp::GeneratedImage::new(Source::base64("image/jpeg", "AA==").unwrap()),
        Some(Usage::new(1, 2)),
    );
    let out = enc.encode(&StreamEvent::Image(event)).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.starts_with("event: image_generation.completed\n"),
        "{text}"
    );
    let data: Value =
        serde_json::from_str(text.lines().nth(1).unwrap().strip_prefix("data: ").unwrap()).unwrap();
    assert_eq!(data["created_at"], 9);
    assert_eq!(data["size"], "auto");
    assert_eq!(data["quality"], "auto");
    assert_eq!(data["background"], "auto");
    assert_eq!(data["output_format"], "jpeg");
    assert_eq!(data["usage"]["total_tokens"], 3);
    let err = kaeso_kpp::KppError::invalid_request("no");
    let out = String::from_utf8(enc.encode(&StreamEvent::Error(err)).unwrap()).unwrap();
    assert!(
        out.starts_with("event: error\n") && out.contains("\"type\":\"invalid_request_error\"")
    );
}

#[test]
fn a_whole_reply_asked_for_as_a_stream_becomes_finished_events() {
    let mut r = ImageResponse::new(
        "m",
        vec![
            kaeso_kpp::GeneratedImage::new(Source::base64("image/png", PNG).unwrap()),
            kaeso_kpp::GeneratedImage::new(Source::base64("image/png", PNG).unwrap()),
        ],
    );
    r.usage = Usage::new(3, 4);
    let out =
        OpenAiImages::encode_response(&Reply::Image(r), &stream_ctx(ImageOperation::Edit)).unwrap();
    let items = crate::sse::parse_all(&out).unwrap();
    assert_eq!(items.len(), 2);
    let text = String::from_utf8(out).unwrap();
    assert_eq!(text.matches("event: image_edit.completed").count(), 2);
    assert_eq!(text.matches("\"usage\"").count(), 1);
}

#[test]
fn errors_keep_openai_s_kinds() {
    for (name, body) in fixtures(DIR, "errors", "json") {
        let status = sidecar(DIR, "errors", &name, "meta").unwrap()["status"]
            .as_u64()
            .unwrap() as u16;
        let err = OpenAiImages::decode_error(status, &HeaderList::new(), &body);
        assert_eq!(err.status, Some(status), "{name}");
        let kind = match name.as_str() {
            "rate-limit-429" => ErrorKind::RateLimited,
            "moderation-blocked-400" => ErrorKind::InvalidRequest,
            _ => err.kind.clone(),
        };
        assert_eq!(err.kind, kind, "{name}");
        let enc = OpenAiImages::encode_error(&err);
        assert_eq!(enc.status, status, "{name}");
    }
}

#[test]
fn variations_are_refused_everywhere() {
    let r = ImageRequest::new("dall-e-2", ImageOperation::Variation).with_image(png());
    for p in [
        ImagesProfile::OPENAI,
        ImagesProfile::XAI,
        ImagesProfile::VERCEL,
    ] {
        let (param, message) = refusal(p, &r);
        assert_eq!(param, "image");
        assert!(message.contains("/v1/images/edits"), "{message}");
    }
}

#[test]
fn a_stream_the_provider_cannot_send_is_asked_for_whole() {
    let mut r = generate("p");
    r.stream = StreamMode::on(true);
    let (enc, ctx) = encode(ImagesProfile::VERCEL, &r).unwrap();
    assert!(ctx.stream.is_off());
    assert!(body_of(&enc).get("stream").is_none());
    let (enc, ctx) = encode(ImagesProfile::OPENAI, &r).unwrap();
    assert!(!ctx.stream.is_off());
    assert_eq!(body_of(&enc)["stream"], true);
    r.partial_images = Some(2);
    assert_eq!(refusal(ImagesProfile::VERCEL, &r).0, "partial_images");
    r.partial_images = Some(0);
    encode(ImagesProfile::VERCEL, &r).unwrap();
}

#[test]
fn spacex_takes_aspect_ratios_and_json_edits() {
    let mut r = generate("fox");
    r.size = Some(ImageSize::pixels(1024, 1024));
    let (param, message) = refusal(ImagesProfile::XAI, &r);
    assert_eq!(param, "size");
    assert!(message.contains("aspect_ratio"), "{message}");
    r.size = None;
    r.aspect_ratio = Some("16:9".into());
    r.resolution = Some("2k".into());
    r.quality = Some(ImageQuality::Medium);
    r.delivery = Some(ImageDelivery::Base64);
    r.user = Some("u1".into());
    let (enc, _) = encode(ImagesProfile::XAI, &r).unwrap();
    assert_eq!(enc.path, "/v1/images/generations");
    assert_eq!(
        body_of(&enc),
        json!({"model": "m", "prompt": "fox", "quality": "medium", "response_format": "b64_json", "aspect_ratio": "16:9", "resolution": "2k", "user": "u1"})
    );
    r.quality = Some(ImageQuality::High);
    assert_eq!(refusal(ImagesProfile::XAI, &r).0, "quality");
    let mut edit = ImageRequest::new("grok-imagine-image-2.0", ImageOperation::Edit)
        .with_prompt("hat")
        .with_image(png());
    let (enc, _) = encode(ImagesProfile::XAI, &edit).unwrap();
    assert_eq!(enc.path, "/v1/images/edits");
    assert_eq!(
        body_of(&enc)["image"],
        json!({"type": "image_url", "url": format!("data:image/png;base64,{PNG}")})
    );
    edit = edit.with_image(ImageInput::new(Source::Url(
        "https://example.com/b.png".into(),
    )));
    let (enc, _) = encode(ImagesProfile::XAI, &edit).unwrap();
    assert_eq!(
        body_of(&enc)["images"][1]["url"],
        "https://example.com/b.png"
    );
    edit.mask = Some(png());
    assert_eq!(refusal(ImagesProfile::XAI, &edit).0, "mask");
    edit.mask = None;
    for _ in 0..4 {
        edit = edit.with_image(png());
    }
    assert_eq!(refusal(ImagesProfile::XAI, &edit).0, "image");
    let mut moderated = generate("fox");
    moderated.extras = kaeso_kpp::Extras::new(DialectId::OpenAiImages).with_field(
        "moderation",
        kaeso_kpp::RawJson::from_string("\"low\"".into()).unwrap(),
    );
    assert_eq!(refusal(ImagesProfile::XAI, &moderated).0, "moderation");
}

#[test]
fn together_takes_pixels_as_width_and_height_and_links_to_edit() {
    let mut r = generate("fox");
    r.size = Some(ImageSize::pixels(1024, 768));
    r.delivery = Some(ImageDelivery::Base64);
    r.seed = Some(7);
    r.negative_prompt = Some("text".into());
    r.user = Some("u1".into());
    let (enc, ctx) = encode(ImagesProfile::TOGETHER, &r).unwrap();
    assert_eq!(
        body_of(&enc),
        json!({"model": "m", "prompt": "fox", "width": 1024, "height": 768, "response_format": "base64", "seed": 7, "negative_prompt": "text"})
    );
    assert_eq!(ctx.left_out, ["user"]);
    r.size = Some(ImageSize::from_wire("1K"));
    assert_eq!(refusal(ImagesProfile::TOGETHER, &r).0, "size");
    let edit = ImageRequest::new("black-forest-labs/FLUX.1-kontext-pro", ImageOperation::Edit)
        .with_prompt("hat")
        .with_image(png());
    assert_eq!(refusal(ImagesProfile::TOGETHER, &edit).0, "image");
    let linked = ImageRequest::new("black-forest-labs/FLUX.1-kontext-pro", ImageOperation::Edit)
        .with_prompt("hat")
        .with_image(ImageInput::new(Source::Url(
            "https://example.com/a.png".into(),
        )));
    let (enc, _) = encode(ImagesProfile::TOGETHER, &linked).unwrap();
    assert_eq!(enc.path, "/v1/images/generations");
    assert_eq!(body_of(&enc)["image_url"], "https://example.com/a.png");
    let mut many = generate("fox");
    many.count = Some(5);
    assert_eq!(refusal(ImagesProfile::TOGETHER, &many).0, "n");
    let mut quality = generate("fox");
    quality.quality = Some(ImageQuality::High);
    assert_eq!(refusal(ImagesProfile::TOGETHER, &quality).0, "quality");
}

#[test]
fn zai_makes_one_image_per_request_from_the_host_s_root() {
    let mut r = ImageRequest::new("glm-image", ImageOperation::Generate).with_prompt("fox");
    r.quality = Some(ImageQuality::Hd);
    r.size = Some(ImageSize::pixels(1280, 1280));
    r.user = Some("user-123456".into());
    r.count = Some(1);
    let (enc, _) = encode(ImagesProfile::ZAI, &r).unwrap();
    assert_eq!(enc.path, "/api/paas/v4/images/generations");
    assert_eq!(
        body_of(&enc),
        json!({"model": "glm-image", "prompt": "fox", "size": "1280x1280", "quality": "hd", "user_id": "user-123456"})
    );
    r.count = Some(2);
    assert_eq!(refusal(ImagesProfile::ZAI, &r).0, "n");
    r.count = None;
    r.delivery = Some(ImageDelivery::Base64);
    assert_eq!(refusal(ImagesProfile::ZAI, &r).0, "response_format");
    let edit = ImageRequest::new("glm-image", ImageOperation::Edit)
        .with_prompt("hat")
        .with_image(png());
    let (param, message) = refusal(ImagesProfile::ZAI, &edit);
    assert_eq!(param, "image");
    assert!(message.contains("/v1/images/generations"), "{message}");
}

#[test]
fn qwencloud_edits_on_its_generations_endpoint_with_data_urls() {
    let edit = ImageRequest::new("qwen-image-3.0", ImageOperation::Edit)
        .with_prompt("hat")
        .with_image(png());
    let (enc, _) = encode(ImagesProfile::QWENCLOUD, &edit).unwrap();
    assert_eq!(enc.path, "/v1/images/generations");
    assert_eq!(
        body_of(&enc)["image"],
        format!("data:image/png;base64,{PNG}")
    );
    let two = edit.clone().with_image(png());
    let (enc, _) = encode(ImagesProfile::QWENCLOUD, &two).unwrap();
    assert_eq!(body_of(&enc)["image"].as_array().map(Vec::len), Some(2));
    let mut url = generate("fox");
    url.delivery = Some(ImageDelivery::Url);
    let (enc, _) = encode(ImagesProfile::QWENCLOUD, &url).unwrap();
    assert!(body_of(&enc).get("response_format").is_none());
}

#[test]
fn openrouter_takes_references_and_bytes_only() {
    let edit = ImageRequest::new("openai/gpt-image-2.6-flare", ImageOperation::Edit)
        .with_prompt("hat")
        .with_image(png());
    let (enc, _) = encode(ImagesProfile::OPENROUTER, &edit).unwrap();
    assert_eq!(enc.path, "/v1/images");
    assert_eq!(
        body_of(&enc)["input_references"],
        json!([{"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{PNG}")}}])
    );
    let mut url = generate("fox");
    url.delivery = Some(ImageDelivery::Url);
    assert_eq!(
        refusal(ImagesProfile::OPENROUTER, &url).0,
        "response_format"
    );
}

#[test]
fn file_ids_go_only_where_they_were_issued() {
    let mut edit = ImageRequest::new("m", ImageOperation::Edit).with_prompt("hat");
    edit.images.push(ImageInput::new(Source::file_id(
        DialectId::OpenAiImages,
        "file-1",
    )));
    let (enc, _) = encode(ImagesProfile::OPENAI, &edit).unwrap();
    assert_eq!(body_of(&enc)["images"], json!([{"file_id": "file-1"}]));
    assert_eq!(refusal(ImagesProfile::VERCEL, &edit).0, "image");
    let mut foreign = ImageRequest::new("m", ImageOperation::Edit).with_prompt("hat");
    foreign.images.push(ImageInput::new(Source::file_id(
        DialectId::OpenAiResponses,
        "file-1",
    )));
    assert_eq!(refusal(ImagesProfile::OPENAI, &foreign).0, "image");
}

#[test]
fn a_base_path_goes_before_the_provider_s_path() {
    let r = generate("fox");
    let mut ctx = egress(ImagesProfile::OPENAI, &r).with_base_path("/openai/");
    let enc = OpenAiImages::encode_request(&Request::Image(r), &mut ctx).unwrap();
    assert_eq!(enc.path, "/openai/v1/images/generations");
}

#[test]
fn other_requests_are_not_image_requests() {
    let req = Request::Generate(kaeso_kpp::GenerateRequest::new("m"));
    let err = OpenAiImages::check(&req, &EgressCtx::new(&req)).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Unsupported { .. }));
    let reply = Reply::Embed(kaeso_kpp::EmbedResponse::new("m", vec![]));
    assert!(
        OpenAiImages::encode_response(&reply, &EncodeCtx::new(RequestKind::Embed, StreamMode::Off))
            .is_err()
    );
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    assert!(OpenAiImages::decode_response(200, &HeaderList::new(), b"{}", &ctx).is_err());
}

#[test]
fn uploads_keep_markup_nested_fields_and_lists() {
    let png = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, PNG).unwrap();
    let mut body = Vec::new();
    let mut text = |name: &str, value: &str| {
        body.extend_from_slice(
            format!("--b\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                .as_bytes(),
        );
    };
    text("model", "gpt-image-2.6-flare");
    text("prompt", "hat");
    text("svg[reasoning][effort]", "low");
    text("tags[]", "a");
    text("tags[]", "b");
    text("frame_duration_ms", "soon");
    body.extend_from_slice(b"--b\r\nContent-Disposition: form-data; name=\"image\"; filename=\"fox.png\"\r\nContent-Type: image/png\r\n\r\n");
    body.extend_from_slice(&png);
    body.extend_from_slice(b"\r\n--b\r\nContent-Disposition: form-data; name=\"markup\"; filename=\"m.png\"\r\nContent-Type: image/png\r\n\r\n");
    body.extend_from_slice(&png);
    body.extend_from_slice(b"\r\n--b--\r\n");
    let headers = HeaderList::new().with("content-type", "multipart/form-data; boundary=b");
    let r = decode(ImageOperation::Edit, &body, &headers);
    assert_eq!(
        r.extras.fields["svg"].get(),
        r#"{"reasoning":{"effort":"low"}}"#
    );
    assert_eq!(r.extras.fields["tags"].get(), r#"["a","b"]"#);
    assert_eq!(r.extras.fields["frame_duration_ms"].get(), r#""soon""#);
    assert!(
        r.extras.fields["markup"]
            .get()
            .contains("data:image/png;base64,")
    );
    let (enc, _) = encode(ImagesProfile::OPENAI, &r).unwrap();
    let ct = enc.headers.get("content-type").unwrap().to_owned();
    let parts = parts_of(&ct, &enc.body);
    let names: Vec<&str> = parts.iter().map(|p| p.0.as_str()).collect();
    assert_eq!(
        names,
        [
            "frame_duration_ms",
            "image",
            "markup",
            "model",
            "prompt",
            "svg[reasoning][effort]",
            "tags[]",
            "tags[]"
        ]
    );
    for bad in ["svg[model", "[x]", "svg[a]x"] {
        let mut body = Vec::new();
        body.extend_from_slice(
            format!("--b\r\nContent-Disposition: form-data; name=\"{bad}\"\r\n\r\nv\r\n--b--\r\n")
                .as_bytes(),
        );
        assert!(
            OpenAiImages::decode_call(ImageOperation::Edit, &body, &headers).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn uploads_get_a_file_name_from_their_media_type() {
    let mut r = ImageRequest::new("m", ImageOperation::Edit).with_prompt("hat");
    for media in [
        "image/jpeg",
        "image/webp",
        "image/gif",
        "image/svg+xml",
        "image/avif",
    ] {
        r.images
            .push(ImageInput::new(Source::base64(media, "AA==").unwrap()));
    }
    r.extras = kaeso_kpp::Extras::new(DialectId::OpenAiImages).with_field(
        "n_tags",
        kaeso_kpp::RawJson::from_string("[1, true, null]".into()).unwrap(),
    );
    let (enc, _) = encode(ImagesProfile::OPENAI, &r).unwrap();
    let ct = enc.headers.get("content-type").unwrap().to_owned();
    let boundary = form_boundary(&ct).unwrap();
    let parts = read_form(&enc.body, &boundary, FormLimits::new(64, 1 << 20)).unwrap();
    let files: Vec<&str> = parts.iter().filter_map(|p| p.filename.as_deref()).collect();
    assert_eq!(
        files,
        [
            "image.jpg",
            "image.webp",
            "image.gif",
            "image.svg",
            "image.bin"
        ]
    );
    let tags: Vec<&str> = parts
        .iter()
        .filter(|p| p.name == "n_tags[]")
        .filter_map(|p| p.text())
        .collect();
    assert_eq!(tags, ["1", "true"]);
    assert_eq!(
        Source::base64("image/png", "not base64!").unwrap_err().kind,
        kaeso_kpp::ErrorKind::InvalidRequest
    );
}

#[test]
fn profiles_refuse_what_their_providers_lack() {
    let mut r = generate("fox");
    r.format = Some(ImageFormat::Gif);
    assert_eq!(refusal(ImagesProfile::TOGETHER, &r).0, "output_format");
    let mut r = generate("fox");
    r.compression = Some(50);
    assert_eq!(refusal(ImagesProfile::XAI, &r).0, "output_compression");
    let mut r = generate("fox");
    r.background = Some(kaeso_kpp::ImageBackground::Transparent);
    assert_eq!(refusal(ImagesProfile::ZAI, &r).0, "background");
    let mut r = generate("fox");
    r.negative_prompt = Some("text".into());
    assert_eq!(refusal(ImagesProfile::OPENAI, &r).0, "negative_prompt");
    let compatible = ImagesProfile::compatible("Local images");
    assert_eq!(compatible.provider, "Local images");
    encode(compatible, &generate("fox")).unwrap();
    let foreign = {
        let mut r = generate("fox");
        r.extras = kaeso_kpp::Extras::new(DialectId::Gemini)
            .with_field("generationConfig", kaeso_kpp::RawJson::empty_object());
        r
    };
    let err = encode(ImagesProfile::OPENAI, &foreign).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Unsupported { .. }));
}

#[test]
fn streams_end_cleanly_or_report_what_broke() {
    let req = Request::Image(ImageRequest::new("m", ImageOperation::Generate));
    let mut dec = OpenAiImages::stream_decoder(&EgressCtx::new(&req));
    let events = dec.feed(b"data: {\"type\":\"image_generation.completed\"");
    assert!(events.is_empty());
    let end = dec.finish();
    assert!(matches!(end.as_slice(), [StreamEvent::Error(_)]), "{end:?}");
    assert!(dec.feed(b"data: x\n\n").is_empty());
    let mut dec = OpenAiImages::stream_decoder(&EgressCtx::new(&req));
    let events =
        dec.feed(b"data: {\"type\":\"image_generation.partial_image\"}\n\ndata: not json\n\n");
    assert!(
        events.iter().all(|e| matches!(e, StreamEvent::Extra(_))),
        "{events:?}"
    );
    let mut enc = OpenAiImages::stream_encoder(&stream_ctx(ImageOperation::Generate));
    for e in &events {
        assert!(!enc.encode(e).unwrap().is_empty());
    }
    let foreign = StreamEvent::Extra(kaeso_kpp::RawItem::new(
        DialectId::Gemini,
        kaeso_kpp::RawJson::empty_object(),
    ));
    assert!(enc.encode(&foreign).unwrap().is_empty());
    assert!(
        enc.encode(&StreamEvent::ResponseStop(kaeso_kpp::ResponseStop::new()))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn model_listings_use_openai_s_shape() {
    let list = br#"{"object":"list","data":[{"id":"gpt-image-2","object":"model","created":1,"owned_by":"openai"}]}"#;
    let models = OpenAiImages::decode_models(200, &HeaderList::new(), list).unwrap();
    assert_eq!(models.models[0].id, "gpt-image-2");
    OpenAiImages::encode_models(&models).unwrap();
    let one = OpenAiImages::decode_model(
        200,
        &HeaderList::new(),
        br#"{"id":"gpt-image-2","object":"model","created":1,"owned_by":"openai"}"#,
    )
    .unwrap();
    OpenAiImages::encode_model(&one).unwrap();
    assert!(OpenAiImages::decode_models(401, &HeaderList::new(), b"{}").is_err());
    assert!(OpenAiImages::decode_model(404, &HeaderList::new(), b"{}").is_err());
    assert_eq!(
        OpenAiImages::encode_model_request("gpt-image-2").path,
        "/v1/models/gpt-image-2"
    );
    OpenAiImages::encode_models_request(&OpenAiImages::decode_models_query("").unwrap()).unwrap();
}
