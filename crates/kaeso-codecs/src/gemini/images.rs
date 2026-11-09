use kaeso_kpp::{
    Block, ErrorKind, GeneratedImage, ImageDelivery, ImageFormat, ImageOperation, ImageRequest,
    ImageResponse, ImageSize, KppError, Origin, Reply, Source, StopReason, StreamMode,
};
use serde_json::{Map, Value, json};

use super::D;
use super::path::GeminiAction;
use super::response::decode_generate;
use super::wire::ResponseW;
use crate::codec::{EgressCtx, EncodedRequest, HeaderList};
use crate::cross::check_extras;
use crate::wire::{Ext, to_body};

// Output sizes per aspect ratio and `imageSize`, from the tables of AI Studio's image
// generation guide (https://aistudio.google.com/docs/image-generation): one table for the
// Gemini 3 image models, which share their sizes, and one for Gemini 2.5 Flash Image.
const SIZES_3: &[(&str, &str, u32, u32)] = &[
    ("1:1", "512", 512, 512),
    ("1:1", "1K", 1024, 1024),
    ("1:1", "2K", 2048, 2048),
    ("1:1", "4K", 4096, 4096),
    ("1:4", "512", 256, 1024),
    ("1:4", "1K", 512, 2048),
    ("1:4", "2K", 1024, 4096),
    ("1:4", "4K", 2048, 8192),
    ("1:8", "512", 192, 1536),
    ("1:8", "1K", 384, 3072),
    ("1:8", "2K", 768, 6144),
    ("1:8", "4K", 1536, 12288),
    ("2:3", "512", 424, 632),
    ("2:3", "1K", 848, 1264),
    ("2:3", "2K", 1696, 2528),
    ("2:3", "4K", 3392, 5056),
    ("3:2", "512", 632, 424),
    ("3:2", "1K", 1264, 848),
    ("3:2", "2K", 2528, 1696),
    ("3:2", "4K", 5056, 3392),
    ("3:4", "512", 448, 600),
    ("3:4", "1K", 896, 1200),
    ("3:4", "2K", 1792, 2400),
    ("3:4", "4K", 3584, 4800),
    ("4:1", "512", 1024, 256),
    ("4:1", "1K", 2048, 512),
    ("4:1", "2K", 4096, 1024),
    ("4:1", "4K", 8192, 2048),
    ("4:3", "512", 600, 448),
    ("4:3", "1K", 1200, 896),
    ("4:3", "2K", 2400, 1792),
    ("4:3", "4K", 4800, 3584),
    ("4:5", "512", 464, 576),
    ("4:5", "1K", 928, 1152),
    ("4:5", "2K", 1856, 2304),
    ("4:5", "4K", 3712, 4608),
    ("5:4", "512", 576, 464),
    ("5:4", "1K", 1152, 928),
    ("5:4", "2K", 2304, 1856),
    ("5:4", "4K", 4608, 3712),
    ("8:1", "512", 1536, 192),
    ("8:1", "1K", 3072, 384),
    ("8:1", "2K", 6144, 768),
    ("8:1", "4K", 12288, 1536),
    ("9:16", "512", 384, 688),
    ("9:16", "1K", 768, 1376),
    ("9:16", "2K", 1536, 2752),
    ("9:16", "4K", 3072, 5504),
    ("16:9", "512", 688, 384),
    ("16:9", "1K", 1376, 768),
    ("16:9", "2K", 2752, 1536),
    ("16:9", "4K", 5504, 3072),
    ("21:9", "1K", 1584, 672),
    ("21:9", "2K", 3168, 1344),
    ("21:9", "4K", 6336, 2688),
];

const SIZES_2_5: &[(&str, u32, u32)] = &[
    ("1:1", 1024, 1024),
    ("2:3", 832, 1248),
    ("3:2", 1248, 832),
    ("3:4", 864, 1184),
    ("4:3", 1184, 864),
    ("4:5", 896, 1152),
    ("5:4", 1152, 896),
    ("9:16", 768, 1344),
    ("16:9", 1344, 768),
    ("21:9", 1536, 672),
];

fn refused(feature: &str, param: &str, hint: &str) -> KppError {
    KppError::unsupported_by(
        feature.to_owned(),
        D,
        format!("Gemini image models do not take {feature}; {hint}"),
    )
    .with_param(param.to_owned())
}

/// The aspect ratio and `imageSize` Gemini writes `size` as, for `model`.
fn gemini_size(
    model: &str,
    width: u32,
    height: u32,
) -> Option<(&'static str, Option<&'static str>)> {
    if model.contains("gemini-2.5-flash-image") {
        return SIZES_2_5
            .iter()
            .find(|(_, w, h)| (*w, *h) == (width, height))
            .map(|(ratio, _, _)| (*ratio, None));
    }
    SIZES_3
        .iter()
        .find(|(_, _, w, h)| (*w, *h) == (width, height))
        .map(|(ratio, size, _, _)| (*ratio, Some(*size)))
}

pub(super) fn check(r: &ImageRequest, image_output_options: bool) -> Result<(), KppError> {
    check_extras(&r.extras, D, "", "request")?;
    if r.operation == ImageOperation::Variation {
        return Err(refused(
            "image variations",
            "image",
            "send the image to `/v1/images/edits` with a prompt.",
        ));
    }
    if r.mask.is_some() {
        return Err(refused(
            "a mask",
            "mask",
            "describe the area to change in the prompt instead.",
        ));
    }
    for (i, image) in r.images.iter().enumerate() {
        if !matches!(image.source, Source::Base64(_)) {
            return Err(refused(
                "images by link or file id",
                &format!("images[{i}]"),
                "upload the image instead.",
            ));
        }
    }
    if r.count.is_some_and(|n| n > 1) {
        return Err(refused("`n` above 1", "n", "send one request per image."));
    }
    if let Some(size) = &r.size
        && *size != ImageSize::Auto
    {
        let mapped = size
            .dimensions()
            .and_then(|(w, h)| gemini_size(&r.model, w, h));
        if mapped.is_none() {
            return Err(refused(
                &format!("the size `{}`", size.as_wire()),
                "size",
                "pick one of the sizes in Google's image guide, or send `aspect_ratio` and `resolution`.",
            ));
        }
        if r.aspect_ratio.is_some() || r.resolution.is_some() {
            return Err(KppError::invalid_request(
                "Send either `size` or `aspect_ratio` and `resolution`, not both.",
            )
            .with_param("size"));
        }
    }
    let output_options = r.format.is_some() || r.compression.is_some();
    if output_options && !image_output_options {
        let field = if r.format.is_some() {
            "output_format"
        } else {
            "output_compression"
        };
        return Err(refused(&format!("`{field}` here"), field, "remove it."));
    }
    if let Some(f) = &r.format
        && !matches!(f, ImageFormat::Png | ImageFormat::Jpeg)
    {
        return Err(refused(
            &format!("the output format `{}`", f.as_wire()),
            "output_format",
            "ask for `png` or `jpeg`.",
        ));
    }
    let unsupported = [
        ("quality", r.quality.is_some()),
        ("background", r.background.is_some()),
        ("negative_prompt", r.negative_prompt.is_some()),
    ];
    if let Some((field, _)) = unsupported.iter().find(|(_, set)| *set) {
        return Err(refused(&format!("`{field}`"), field, "remove it."));
    }
    if r.delivery == Some(ImageDelivery::Url) {
        return Err(refused(
            "`url`",
            "response_format",
            "they answer with the images' bytes; remove `response_format`.",
        ));
    }
    if !r.stream.is_off() && r.partial_images.is_some_and(|n| n > 0) {
        return Err(refused(
            "partial images",
            "partial_images",
            "set `partial_images` to 0 to get the finished image as one event.",
        ));
    }
    Ok(())
}

/// An image request as a `generateContent` call that asks only for images, as Google's image
/// guide does with `responseModalities: ["IMAGE"]`.
pub(super) fn encode(r: &ImageRequest, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    check(r, ctx.gemini_profile.image_output_options)?;
    ctx.stream = StreamMode::Off;
    if r.user.is_some() {
        ctx.left_out.push("user".to_owned());
    }
    let mut parts = vec![json!({ "text": r.prompt.clone().unwrap_or_default() })];
    for image in &r.images {
        if let Source::Base64(b) = &image.source {
            parts.push(json!({"inlineData": {"mimeType": b.media_type, "data": b.base64()}}));
        }
    }
    let mut image_config = Map::new();
    let sized = r
        .size
        .as_ref()
        .and_then(ImageSize::dimensions)
        .and_then(|(w, h)| gemini_size(&r.model, w, h));
    if let Some((ratio, size)) = sized {
        image_config.insert("aspectRatio".into(), ratio.into());
        if let Some(size) = size {
            image_config.insert("imageSize".into(), size.into());
        }
    }
    if let Some(ratio) = &r.aspect_ratio {
        image_config.insert("aspectRatio".into(), ratio.clone().into());
    }
    if let Some(size) = &r.resolution {
        image_config.insert("imageSize".into(), size.clone().into());
    }
    if r.format.is_some() || r.compression.is_some() {
        let mut options = Map::new();
        if let Some(media) = r.format.as_ref().and_then(ImageFormat::media_type) {
            options.insert("mimeType".into(), media.into());
        }
        if let Some(c) = r.compression {
            options.insert("compressionQuality".into(), c.into());
        }
        image_config.insert("imageOutputOptions".into(), Value::Object(options));
    }
    let mut config = Map::new();
    config.insert("responseModalities".into(), json!(["IMAGE"]));
    if !image_config.is_empty() {
        config.insert("imageConfig".into(), Value::Object(image_config));
    }
    if let Some(seed) = r.seed {
        config.insert("seed".into(), seed.into());
    }
    let body = json!({
        "contents": [{"role": "user", "parts": parts}],
        "generationConfig": config,
    });
    Ok(EncodedRequest::post_json(
        ctx.gemini_profile
            .path(&r.model, GeminiAction::GenerateContent)?,
        HeaderList::new().with("content-type", "application/json"),
        to_body(&body)?,
    ))
}

/// The images of a `generateContent` reply. Text the model wrote beside them has no place in
/// an image reply and is left out, as other formats leave out what their replies can't hold.
pub(super) fn decode(body: &[u8], origin: &Origin) -> Result<Reply, KppError> {
    let w: Ext<ResponseW> = Ext::from_slice(body).map_err(|e| {
        KppError::provider(format!(
            "The provider sent a response that is not a valid Gemini API response: {}.",
            crate::wire::without_position(&e)
        ))
    })?;
    let reply = decode_generate(w, origin)?;
    let images: Vec<GeneratedImage> = reply
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Image(i) => Some(GeneratedImage::new(i.source.clone())),
            _ => None,
        })
        .collect();
    if images.is_empty() {
        let (kind, why) = match &reply.stop {
            StopReason::ContentFilter | StopReason::Refusal => (
                ErrorKind::ContentFiltered,
                "Google's safety filters blocked the image".to_owned(),
            ),
            other => (
                ErrorKind::ProviderError,
                format!("the model answered without an image ({other:?})"),
            ),
        };
        return Err(
            KppError::new(kind, format!("No image came back: {why}.")).with_origin(origin.clone())
        );
    }
    let mut resp = ImageResponse::new(reply.model, images);
    resp.usage = reply.usage;
    Ok(Reply::Image(resp))
}
