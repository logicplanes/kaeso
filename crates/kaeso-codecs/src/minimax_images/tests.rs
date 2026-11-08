use kaeso_kpp::{
    ErrorKind, ImageDelivery, ImageInput, ImageOperation, ImageQuality, ImageRequest, ImageSize,
    Reply, Request, RequestKind, Source, StreamMode,
};
use serde_json::{Value, json};

use super::*;
use crate::test_support::{assert_json_eq, fixtures, sidecar};

const DIR: &str = "minimax_images";

fn ctx(req: &Request) -> EgressCtx {
    EgressCtx::new(req).with_provider("minimax")
}

fn encode(req: ImageRequest) -> Result<(Value, EgressCtx), KppError> {
    let req = Request::Image(req);
    let mut c = ctx(&req);
    let enc = MinimaxImages::encode_request(&req, &mut c)?;
    assert_eq!(enc.path, IMAGE_GENERATION_PATH);
    Ok((serde_json::from_slice(&enc.body).unwrap(), c))
}

fn param(req: ImageRequest) -> String {
    let err = encode(req).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
    err.param.unwrap()
}

#[test]
fn every_request_and_response_fixture_round_trips() {
    for (name, body) in fixtures(DIR, "requests", "json") {
        let req =
            MinimaxImages::decode_request(RequestKind::Image, &body, &HeaderList::new()).unwrap();
        let mut c = ctx(&req);
        let enc = MinimaxImages::encode_request(&req, &mut c).unwrap();
        assert_json_eq(&name, &body, &enc.body);
    }
    for (name, body) in fixtures(DIR, "responses", "json") {
        let req = Request::Image(ImageRequest::new("image-01", ImageOperation::Generate));
        let reply =
            MinimaxImages::decode_response(200, &HeaderList::new(), &body, &ctx(&req)).unwrap();
        let out = MinimaxImages::encode_response(&reply, &EncodeCtx::for_request(req)).unwrap();
        assert_json_eq(&name, &body, &out);
    }
}

#[test]
fn replies_carry_links_or_bytes_with_their_media_type() {
    let reply = |name: &str| {
        let body = fixtures(DIR, "responses", "json")
            .into_iter()
            .find(|(n, _)| n == name)
            .unwrap()
            .1;
        let req = Request::Image(ImageRequest::new("image-01", ImageOperation::Generate));
        match MinimaxImages::decode_response(200, &HeaderList::new(), &body, &ctx(&req)).unwrap() {
            Reply::Image(r) => r,
            other => panic!("{other:?}"),
        }
    };
    let r = reply("generate-urls");
    assert_eq!(r.images.len(), 3);
    assert!(matches!(&r.images[0].source, Source::Url(u) if u.ends_with("a.jpeg")));
    let r = reply("generate-base64");
    let Source::Base64(b) = &r.images[0].source else {
        panic!()
    };
    assert_eq!(b.media_type, "image/png");
}

#[test]
fn errors_in_base_resp_reach_apps_with_their_kind() {
    for (name, body) in fixtures(DIR, "errors", "json") {
        let status = sidecar(DIR, "errors", &name, "meta").unwrap()["status"]
            .as_u64()
            .unwrap() as u16;
        let req = Request::Image(ImageRequest::new("image-01", ImageOperation::Generate));
        let err = MinimaxImages::decode_response(status, &HeaderList::new(), &body, &ctx(&req))
            .unwrap_err();
        let kind = match name.as_str() {
            "sensitive-prompt-200" => ErrorKind::ContentFiltered,
            _ => ErrorKind::Authentication,
        };
        assert_eq!(err.kind, kind, "{name}");
    }
    let err = MinimaxImages::decode_error(500, &HeaderList::new(), b"oops");
    assert_eq!(err.status, Some(500));
    let enc = MinimaxImages::encode_error(&KppError::invalid_request("bad").with_status(400));
    assert_eq!(enc.status, 400);
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(body["base_resp"]["status_code"], 2013);
}

#[test]
fn an_openai_request_maps_to_minimax_s_fields() {
    let mut r = ImageRequest::new("image-01", ImageOperation::Generate).with_prompt("fox");
    r.size = Some(ImageSize::pixels(1024, 768));
    r.count = Some(4);
    r.seed = Some(7);
    r.delivery = Some(ImageDelivery::Base64);
    r.user = Some("u".into());
    r.stream = StreamMode::on(true);
    let (body, c) = encode(r).unwrap();
    assert_eq!(
        body,
        json!({"model": "image-01", "prompt": "fox", "width": 1024, "height": 768, "response_format": "base64", "seed": 7, "n": 4})
    );
    assert!(c.stream.is_off());
    assert_eq!(c.left_out, ["user"]);
    let mut ratio = ImageRequest::new("image-01", ImageOperation::Generate).with_prompt("fox");
    ratio.aspect_ratio = Some("16:9".into());
    ratio.size = Some(ImageSize::Auto);
    assert_eq!(encode(ratio).unwrap().0["aspect_ratio"], "16:9");
}

#[test]
fn what_minimax_lacks_is_refused_with_the_field() {
    let base = || ImageRequest::new("image-01", ImageOperation::Generate).with_prompt("fox");
    let edit = ImageRequest::new("image-01", ImageOperation::Edit)
        .with_prompt("hat")
        .with_image(ImageInput::new(Source::Url(
            "https://example.com/a.png".into(),
        )));
    assert_eq!(param(edit), "image");
    let vary = ImageRequest::new("image-01", ImageOperation::Variation).with_image(
        ImageInput::new(Source::Url("https://example.com/a.png".into())),
    );
    assert_eq!(param(vary), "image");
    let mut r = base();
    r.quality = Some(ImageQuality::High);
    assert_eq!(param(r), "quality");
    let mut r = base();
    r.count = Some(10);
    assert_eq!(param(r), "n");
    let mut r = base();
    r.size = Some(ImageSize::from_wire("2K"));
    assert_eq!(param(r), "size");
    let mut r = base();
    r.negative_prompt = Some("text".into());
    assert_eq!(param(r), "negative_prompt");
    let mut r = base();
    r.stream = StreamMode::on(true);
    r.partial_images = Some(1);
    assert_eq!(param(r), "partial_images");
    let embed = Request::Embed(kaeso_kpp::EmbedRequest::new("m", vec![]));
    assert!(MinimaxImages::check(&embed, &ctx(&embed)).is_err());
}

#[test]
fn minimax_requests_that_break_its_rules_are_refused() {
    for (body, field) in [
        (
            r#"{"model":"image-01","prompt":"p","width":1024}"#,
            Some("width"),
        ),
        (
            r#"{"model":"image-01","prompt":"p","response_format":"b64_json"}"#,
            Some("response_format"),
        ),
        (r#"{"model":"image-01"}"#, None),
    ] {
        let err =
            MinimaxImages::decode_request(RequestKind::Image, body.as_bytes(), &HeaderList::new())
                .unwrap_err();
        assert_eq!(err.param.as_deref(), field, "{body}");
    }
    assert!(
        MinimaxImages::decode_request(RequestKind::Generate, b"{}", &HeaderList::new()).is_err()
    );
    let req = Request::Image(ImageRequest::new("image-01", ImageOperation::Generate));
    let mut dec = MinimaxImages::stream_decoder(&ctx(&req));
    assert!(matches!(dec.feed(b"x").as_slice(), [StreamEvent::Error(_)]));
    assert!(dec.finish().is_empty());
}

#[test]
fn model_listings_and_streams_say_what_minimax_serves() {
    let list = br#"{"object":"list","data":[{"id":"image-01","object":"model","created":1,"owned_by":"minimax"}]}"#;
    let models = MinimaxImages::decode_models(200, &HeaderList::new(), list).unwrap();
    MinimaxImages::encode_models(&models).unwrap();
    let one = MinimaxImages::decode_model(
        200,
        &HeaderList::new(),
        br#"{"id":"image-01","object":"model","created":1,"owned_by":"minimax"}"#,
    )
    .unwrap();
    MinimaxImages::encode_model(&one).unwrap();
    assert!(MinimaxImages::decode_models(401, &HeaderList::new(), b"{}").is_err());
    assert!(MinimaxImages::decode_model(404, &HeaderList::new(), b"{}").is_err());
    let _ = MinimaxImages::encode_model_request("image-01");
    MinimaxImages::encode_models_request(&MinimaxImages::decode_models_query("").unwrap()).unwrap();
    let req = Request::Image(ImageRequest::new("image-01", ImageOperation::Generate));
    let mut enc = MinimaxImages::stream_encoder(&EncodeCtx::for_request(req.clone()));
    assert!(
        enc.encode(&StreamEvent::ResponseStop(kaeso_kpp::ResponseStop::new()))
            .is_err()
    );
    assert!(enc.finish().unwrap().is_empty());
    for (kind, code) in [
        (ErrorKind::RateLimited, 1002),
        (ErrorKind::Authentication, 1004),
        (ErrorKind::Billing, 1008),
        (ErrorKind::ContentFiltered, 1026),
        (ErrorKind::Overloaded, 1000),
    ] {
        let enc = MinimaxImages::encode_error(&KppError::new(kind, "x"));
        let body: Value = serde_json::from_slice(&enc.body).unwrap();
        assert_eq!(body["base_resp"]["status_code"], code);
    }
    let err = MinimaxImages::decode_error(
        200,
        &HeaderList::new(),
        br#"{"base_resp":{"status_code":1039,"status_msg":""}}"#,
    );
    assert_eq!(err.kind, ErrorKind::RateLimited);
    assert!(err.message.contains("1039"));
    let err = MinimaxImages::decode_error(
        400,
        &HeaderList::new(),
        br#"{"base_resp":{"status_code":2013,"status_msg":"bad n"}}"#,
    );
    assert_eq!(
        (err.kind, err.status),
        (ErrorKind::InvalidRequest, Some(400))
    );
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    assert!(MinimaxImages::decode_response(200, &HeaderList::new(), b"{}", &ctx).is_err());
    let reply = Reply::Embed(kaeso_kpp::EmbedResponse::new("m", vec![]));
    assert!(MinimaxImages::encode_response(&reply, &EncodeCtx::for_request(req)).is_err());
}
