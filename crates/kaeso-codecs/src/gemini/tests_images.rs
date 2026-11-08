use kaeso_kpp::{
    ErrorKind, ImageDelivery, ImageFormat, ImageInput, ImageOperation, ImageQuality, ImageRequest,
    ImageSize, Reply, Request, Source, StreamMode,
};
use serde_json::{Value, json};

use super::*;
use crate::test_support::fixtures;

const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";

fn encode(
    r: ImageRequest,
    profile: GeminiProfile,
) -> Result<(EncodedRequest, EgressCtx), KppError> {
    let req = Request::Image(r);
    let mut ctx = EgressCtx::new(&req)
        .with_provider("google")
        .with_gemini_profile(profile);
    let enc = Gemini::encode_request(&req, &mut ctx)?;
    Ok((enc, ctx))
}

fn body(r: ImageRequest) -> Value {
    let (enc, _) = encode(r, GeminiProfile::GOOGLE_AI_STUDIO).unwrap();
    serde_json::from_slice(&enc.body).unwrap()
}

fn param(r: ImageRequest, profile: GeminiProfile) -> String {
    let err = encode(r, profile).unwrap_err();
    err.param.unwrap_or_default()
}

fn fox(model: &str) -> ImageRequest {
    ImageRequest::new(model, ImageOperation::Generate).with_prompt("A red fox")
}

#[test]
fn an_openai_image_request_asks_gemini_for_images_only() {
    let mut r = fox("gemini-3.1-flash-image");
    r.size = Some(ImageSize::pixels(1376, 768));
    r.seed = Some(3);
    r.user = Some("u".into());
    r.stream = StreamMode::on(true);
    let (enc, ctx) = encode(r, GeminiProfile::GOOGLE_AI_STUDIO).unwrap();
    assert_eq!(
        enc.path,
        "/v1beta/models/gemini-3.1-flash-image:generateContent"
    );
    assert!(ctx.stream.is_off());
    assert_eq!(ctx.left_out, ["user"]);
    assert_eq!(
        serde_json::from_slice::<Value>(&enc.body).unwrap(),
        json!({
            "contents": [{"role": "user", "parts": [{"text": "A red fox"}]}],
            "generationConfig": {
                "responseModalities": ["IMAGE"],
                "imageConfig": {"aspectRatio": "16:9", "imageSize": "1K"},
                "seed": 3
            }
        })
    );
    let mut legacy = fox("gemini-2.5-flash-image");
    legacy.size = Some(ImageSize::pixels(1344, 768));
    assert_eq!(
        body(legacy)["generationConfig"]["imageConfig"],
        json!({"aspectRatio": "16:9"})
    );
    let mut tier = fox("gemini-nano-banana-2.1");
    tier.aspect_ratio = Some("21:9".into());
    tier.resolution = Some("4K".into());
    assert_eq!(
        body(tier)["generationConfig"]["imageConfig"],
        json!({"aspectRatio": "21:9", "imageSize": "4K"})
    );
}

#[test]
fn an_edit_sends_its_images_inline_after_the_prompt() {
    let edit = ImageRequest::new("gemini-3-pro-image", ImageOperation::Edit)
        .with_prompt("Add a hat")
        .with_image(ImageInput::new(Source::base64("image/png", PNG)));
    assert_eq!(
        body(edit)["contents"][0]["parts"],
        json!([{"text": "Add a hat"}, {"inlineData": {"mimeType": "image/png", "data": PNG}}])
    );
}

#[test]
fn the_agent_platform_takes_an_output_format_and_ai_studio_does_not() {
    let mut r = fox("gemini-3.1-flash-image");
    r.format = Some(ImageFormat::Jpeg);
    r.compression = Some(80);
    assert_eq!(
        param(r.clone(), GeminiProfile::GOOGLE_AI_STUDIO),
        "output_format"
    );
    let (enc, _) = encode(r, GeminiProfile::AGENT_PLATFORM).unwrap();
    assert_eq!(
        enc.path,
        "/v1/publishers/google/models/gemini-3.1-flash-image:generateContent"
    );
    let b: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        b["generationConfig"]["imageConfig"]["imageOutputOptions"],
        json!({"mimeType": "image/jpeg", "compressionQuality": 80})
    );
    let mut webp = fox("gemini-3.1-flash-image");
    webp.format = Some(ImageFormat::Webp);
    assert_eq!(param(webp, GeminiProfile::AGENT_PLATFORM), "output_format");
}

#[test]
fn what_gemini_lacks_is_refused_with_the_field() {
    let p = GeminiProfile::GOOGLE_AI_STUDIO;
    let mut r = fox("gemini-3.1-flash-image");
    r.size = Some(ImageSize::pixels(1536, 1024));
    assert_eq!(param(r, p.clone()), "size");
    let mut r = fox("gemini-3.1-flash-image");
    r.size = Some(ImageSize::pixels(1024, 1024));
    r.aspect_ratio = Some("1:1".into());
    assert_eq!(param(r, p.clone()), "size");
    let mut r = fox("gemini-3.1-flash-image");
    r.count = Some(2);
    assert_eq!(param(r, p.clone()), "n");
    let mut r = fox("gemini-3.1-flash-image");
    r.quality = Some(ImageQuality::High);
    assert_eq!(param(r, p.clone()), "quality");
    let mut r = fox("gemini-3.1-flash-image");
    r.delivery = Some(ImageDelivery::Url);
    assert_eq!(param(r, p.clone()), "response_format");
    let mut r = fox("gemini-3.1-flash-image");
    r.stream = StreamMode::on(true);
    r.partial_images = Some(1);
    assert_eq!(param(r, p.clone()), "partial_images");
    let mut masked = ImageRequest::new("gemini-3.1-flash-image", ImageOperation::Edit)
        .with_prompt("hat")
        .with_image(ImageInput::new(Source::base64("image/png", PNG)));
    masked.mask = Some(ImageInput::new(Source::base64("image/png", PNG)));
    assert_eq!(param(masked, p.clone()), "mask");
    let linked = ImageRequest::new("gemini-3.1-flash-image", ImageOperation::Edit)
        .with_prompt("hat")
        .with_image(ImageInput::new(Source::Url(
            "https://example.com/a.png".into(),
        )));
    assert_eq!(param(linked, p.clone()), "images[0]");
    let vary = ImageRequest::new("gemini-3.1-flash-image", ImageOperation::Variation)
        .with_image(ImageInput::new(Source::base64("image/png", PNG)));
    assert_eq!(param(vary, p), "image");
}

#[test]
fn a_gemini_reply_becomes_an_image_reply_with_image_tokens() {
    let body = fixtures("gemini", "responses", "json")
        .into_iter()
        .find(|(n, _)| n == "image-output")
        .unwrap()
        .1;
    let req = Request::Image(fox("gemini-3.1-flash-image"));
    let ctx = EgressCtx::new(&req);
    let Reply::Image(r) = Gemini::decode_response(200, &HeaderList::new(), &body, &ctx).unwrap()
    else {
        panic!()
    };
    assert_eq!(r.model, "gemini-3.1-flash-image");
    assert_eq!(r.images.len(), 1);
    assert!(matches!(&r.images[0].source, Source::Base64(b) if b.media_type == "image/png"));
    assert_eq!(r.usage.output_tokens, 1296);
    assert_eq!(r.usage.output_image_tokens, Some(1290));
    let blocked = br#"{"candidates":[{"finishReason":"IMAGE_SAFETY","content":{"parts":[]}}],"usageMetadata":{"promptTokenCount":5,"totalTokenCount":5}}"#;
    let err = Gemini::decode_response(200, &HeaderList::new(), blocked, &ctx).unwrap_err();
    assert!(
        err.message.starts_with("No image came back"),
        "{}",
        err.message
    );
    let text_only =
        br#"{"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":"no"}]}}]}"#;
    let err = Gemini::decode_response(200, &HeaderList::new(), text_only, &ctx).unwrap_err();
    assert_eq!(err.kind, ErrorKind::ProviderError);
    let err = Gemini::decode_response(
        400,
        &HeaderList::new(),
        br#"{"error":{"code":400,"message":"bad","status":"INVALID_ARGUMENT"}}"#,
        &ctx,
    )
    .unwrap_err();
    assert_eq!(err.status, Some(400));
}
