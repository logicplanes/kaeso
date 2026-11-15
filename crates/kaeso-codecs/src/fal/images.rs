//! Images through fal's queue: FLUX.1 [dev] and [schnell], FLUX1.1 [pro] and Seedream 5.5,
//! each with the input and output of its OpenAPI document
//! (`https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=<model>`, read 2026-11-14).

use kaeso_kpp::{
    GeneratedImage, ImageBackground, ImageDelivery, ImageFormat, ImageInput, ImageOperation,
    ImageQuality, ImageRequest, ImageResponse, ImageSize, KppError, Origin, Request, Source,
};
use serde_json::{Map, Value, json};

use super::{D, PROVIDER, error, queued, submit};
use crate::codec::{EgressCtx, EncodedRequest};
use crate::cross::check_extras;
use crate::video_jobs::{Fields, body, link_source, put, put_extras};

/// The image models Kaeso maps to fal's inputs.
pub const IMAGE_MODELS: &[&str] = &[
    "fal-ai/flux/dev",
    "fal-ai/flux/schnell",
    "fal-ai/flux-pro/v1.1",
    "bytedance/seedream/v5.5/text-to-image",
    "bytedance/seedream/v5.5/edit",
];

const SEEDREAM_EDIT: &str = "bytedance/seedream/v5.5/edit";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Family {
    /// FLUX.1 [dev] and [schnell]: `num_images` 1 to 4, `image_size` up to 14142 a side.
    Flux,
    /// FLUX1.1 [pro]: as FLUX.1, without the safety checker switch.
    FluxPro,
    /// Seedream 5.5: `num_images` 1 to 6, `background`, an `image_size` of 921,600 to
    /// 4,624,220 pixels at an aspect ratio between 1:16 and 16:1, no seed.
    Seedream,
    /// Seedream 5.5 edit: Seedream with up to 10 `image_urls`.
    SeedreamEdit,
}

impl Family {
    fn of(model: &str) -> Option<Family> {
        Some(match model {
            "fal-ai/flux/dev" | "fal-ai/flux/schnell" => Family::Flux,
            "fal-ai/flux-pro/v1.1" => Family::FluxPro,
            "bytedance/seedream/v5.5/text-to-image" => Family::Seedream,
            SEEDREAM_EDIT => Family::SeedreamEdit,
            _ => return None,
        })
    }

    fn max_images(self) -> u32 {
        match self {
            Family::Flux | Family::FluxPro => 4,
            Family::Seedream | Family::SeedreamEdit => 6,
        }
    }

    fn is_seedream(self) -> bool {
        matches!(self, Family::Seedream | Family::SeedreamEdit)
    }
}

// Seedream 5.5 edit takes up to 10 images; more are cut to the last 10 without a word.
const MAX_EDIT_IMAGES: usize = 10;
const FLUX_MAX_SIDE: u32 = 14142;
const SEEDREAM_MIN_AREA: u64 = 921_600;
const SEEDREAM_MAX_AREA: u64 = 4_624_220;

fn refused(feature: &str, param: &str, hint: &str) -> KppError {
    crate::video_jobs::refused(D, PROVIDER, feature, param, hint)
}

fn unknown_model(model: &str) -> KppError {
    refused(
        &format!("the image model `{model}` through Kaeso"),
        "model",
        &format!("use one of {}.", IMAGE_MODELS.join(", ")),
    )
}

fn check_size(f: Family, size: &ImageSize) -> Result<(), KppError> {
    let Some((w, h)) = size.dimensions() else {
        return match size {
            ImageSize::Auto => Ok(()),
            other => Err(KppError::invalid_request(format!(
                "`size` must be `auto` or WIDTHxHEIGHT, not `{}`.",
                other.as_wire()
            ))
            .with_param("size")),
        };
    };
    let fits = if f.is_seedream() {
        let area = u64::from(w) * u64::from(h);
        (SEEDREAM_MIN_AREA..=SEEDREAM_MAX_AREA).contains(&area)
            && w.max(h) <= w.min(h).saturating_mul(16)
    } else {
        (1..=FLUX_MAX_SIDE).contains(&w) && (1..=FLUX_MAX_SIDE).contains(&h)
    };
    if fits {
        return Ok(());
    }
    let hint = if f.is_seedream() {
        "use 921,600 to 4,624,220 pixels at most 16 times as long as wide, such as 1024x1024."
    } else {
        "use at most 14142 pixels a side."
    };
    Err(refused(
        &format!("the size `{}`", size.as_wire()),
        "size",
        hint,
    ))
}

/// Checks that fal can make `r`, before anything is sent.
pub(super) fn check(r: &ImageRequest) -> Result<Family, KppError> {
    check_extras(&r.extras, D, "", "request")?;
    let f = Family::of(&r.model).ok_or_else(|| unknown_model(&r.model))?;
    if r.operation == ImageOperation::Variation {
        return Err(refused(
            "image variations",
            "image",
            "send the image to `/v1/images/edits` with a prompt.",
        ));
    }
    if r.prompt.as_deref().is_none_or(str::is_empty) {
        return Err(KppError::invalid_request("An image needs a `prompt`.").with_param("prompt"));
    }
    if !r.stream.is_off() {
        return Err(refused(
            "streamed images",
            "stream",
            "ask without `stream`.",
        ));
    }
    match (f, r.images.is_empty()) {
        (Family::SeedreamEdit, true) => {
            return Err(KppError::invalid_request(format!(
                "`{SEEDREAM_EDIT}` edits images; send at least one `image`."
            ))
            .with_param("image"));
        }
        (Family::SeedreamEdit, false) | (_, true) => {}
        (_, false) => {
            return Err(refused(
                &format!("images to edit on `{}`", r.model),
                "image",
                &format!("use `{SEEDREAM_EDIT}`."),
            ));
        }
    }
    if r.images.len() > MAX_EDIT_IMAGES {
        return Err(refused(
            "more than 10 images to edit",
            "image",
            "send fewer images.",
        ));
    }
    for image in &r.images {
        check_extras(&image.extras, D, "image", "request")?;
        if matches!(image.source, Source::FileId(_)) {
            return Err(refused(
                "file ids",
                "image",
                "upload the image or send its link.",
            ));
        }
    }
    if r.mask.is_some() {
        return Err(refused(
            "masks",
            "mask",
            "describe the area to change in the prompt instead.",
        ));
    }
    if let Some(n) = r.count
        && !(1..=f.max_images()).contains(&n)
    {
        return Err(refused(
            &format!("`n` of {n} on `{}`", r.model),
            "n",
            &format!("send 1 to {}.", f.max_images()),
        ));
    }
    if let Some(size) = &r.size {
        check_size(f, size)?;
    }
    if let Some(q) = &r.quality
        && *q != ImageQuality::Auto
    {
        return Err(refused(
            &format!("the quality `{}`", q.as_wire()),
            "quality",
            "leave it out.",
        ));
    }
    if let Some(format) = &r.format
        && !matches!(format, ImageFormat::Png | ImageFormat::Jpeg)
    {
        return Err(refused(
            &format!("the output format `{}`", format.as_wire()),
            "output_format",
            "use png or jpeg.",
        ));
    }
    match (&r.background, f.is_seedream()) {
        (None | Some(ImageBackground::Auto), _) => {}
        (Some(ImageBackground::Transparent), true) if r.format == Some(ImageFormat::Jpeg) => {
            return Err(refused(
                "a transparent background in a jpeg",
                "background",
                "ask for png.",
            ));
        }
        (Some(ImageBackground::Transparent | ImageBackground::Opaque), true) => {}
        (Some(b), _) => {
            return Err(refused(
                &format!("the background `{}` on `{}`", b.as_wire(), r.model),
                "background",
                "leave it out.",
            ));
        }
    }
    if r.seed.is_some() && f.is_seedream() {
        return Err(refused(
            &format!("a seed on `{}`", r.model),
            "seed",
            "leave it out.",
        ));
    }
    let unsupported = [
        ("output_compression", r.compression.is_some()),
        ("negative_prompt", r.negative_prompt.is_some()),
        ("partial_images", r.partial_images.is_some()),
        ("aspect_ratio", r.aspect_ratio.is_some()),
        ("resolution", r.resolution.is_some()),
        ("user", r.user.is_some()),
    ];
    if let Some((name, _)) = unsupported.iter().find(|(_, set)| *set) {
        return Err(refused(&format!("`{name}`"), name, "leave it out."));
    }
    Ok(f)
}

fn image_link(image: &ImageInput) -> Option<String> {
    match &image.source {
        Source::Url(u) => Some(u.clone()),
        Source::Base64(b) => Some(b.data_url()),
        _ => None,
    }
}

/// The input of the model `r` names.
pub(super) fn encode_request(r: &ImageRequest) -> Result<EncodedRequest, KppError> {
    let f = check(r)?;
    let mut out = Map::new();
    out.insert("prompt".into(), r.prompt.clone().unwrap_or_default().into());
    if f == Family::SeedreamEdit {
        out.insert(
            "image_urls".into(),
            Value::Array(
                r.images
                    .iter()
                    .filter_map(image_link)
                    .map(Value::from)
                    .collect(),
            ),
        );
    }
    if let Some((w, h)) = r.size.as_ref().and_then(ImageSize::dimensions) {
        out.insert("image_size".into(), json!({ "width": w, "height": h }));
    }
    put(&mut out, "num_images", r.count);
    put(&mut out, "seed", r.seed);
    put(
        &mut out,
        "output_format",
        r.format.as_ref().map(|f| f.as_wire().to_owned()),
    );
    put(
        &mut out,
        "background",
        r.background
            .as_ref()
            .filter(|b| **b != ImageBackground::Auto)
            .map(|b| b.as_wire().to_owned()),
    );
    // `sync_mode` puts each image into the result as a data URI, so an app that asks for the
    // bytes gets them without a download from fal's storage.
    if r.delivery == Some(ImageDelivery::Base64) {
        out.insert("sync_mode".into(), Value::Bool(true));
    }
    put_extras(&mut out, &r.extras, D, "");
    submit(&r.model, body(&out)?)
}

/// A fal-format image input, as the dialect's fixtures write it; the model is in the URL.
pub(super) fn decode_request(body: &[u8]) -> Result<Request, KppError> {
    let mut f = Fields::parse(body, D, "request")?;
    let mut r = ImageRequest::new(String::new(), ImageOperation::Generate);
    r.prompt = f.str("prompt")?;
    for url in f.array("image_urls")?.unwrap_or_default() {
        let Value::String(url) = url else {
            return Err(
                KppError::invalid_request("`image_urls` lists links.").with_param("image_urls")
            );
        };
        r.images.push(ImageInput::new(link_source(url)));
        r.operation = ImageOperation::Edit;
    }
    if let Some(Value::Object(size)) = f.peek("image_size") {
        let side = |k: &str| {
            size.get(k)
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
        };
        if let (Some(w), Some(h), 2) = (side("width"), side("height"), size.len()) {
            r.size = Some(ImageSize::pixels(w, h));
            f.take("image_size");
        }
    }
    r.count = f.u32("num_images")?;
    r.seed = f.i64("seed")?;
    r.format = f.str("output_format")?.map(|s| ImageFormat::from_wire(&s));
    r.background = f.str("background")?.map(|s| ImageBackground::from_wire(&s));
    if f.peek("sync_mode") == Some(&Value::Bool(true)) {
        f.take("sync_mode");
        r.delivery = Some(ImageDelivery::Base64);
    }
    r.extras = f.rest(&Origin::new(D));
    Ok(Request::Image(r))
}

fn image_of(v: Value, origin: &Origin) -> Result<(GeneratedImage, Option<(u32, u32)>), KppError> {
    let Value::Object(map) = v else {
        return Err(KppError::provider(format!(
            "{PROVIDER} listed an image that is not an object."
        )));
    };
    let mut f = Fields::from_map(map, D, "reply");
    let url = f
        .str("url")?
        .filter(|u| !u.is_empty())
        .ok_or_else(|| KppError::provider(format!("{PROVIDER} sent an image without its link.")))?;
    let side = |k: &str| {
        f.peek(k)
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
    };
    let size = side("width").zip(side("height"));
    let mut image = GeneratedImage::new(link_source(url));
    image.extras = f.rest(origin);
    Ok((image, size))
}

// FLUX.1's safety checker replaces a flagged image with a black one and says so only in
// `has_nsfw_concepts` (the FLUX.1 input schema's `enable_safety_checker`).
fn flagged(v: &Value) -> Option<usize> {
    v.get("has_nsfw_concepts")?
        .as_array()?
        .iter()
        .position(|b| b.as_bool() == Some(true))
}

/// A finished image request: the model's output, or the failure the queue reports.
pub(super) fn decode_response(body: &[u8], ctx: &EgressCtx) -> Result<ImageResponse, KppError> {
    let v: Value = serde_json::from_slice(body).map_err(|_| {
        KppError::provider(format!("{PROVIDER} sent an image result that is not JSON."))
    })?;
    if let Some(q) = queued(&v) {
        if q.failed {
            return Err(error::failed(&v));
        }
        return Err(KppError::provider(format!(
            "{PROVIDER} answered before the image was made."
        )));
    }
    if let Some(i) = flagged(&v) {
        return Err(KppError::new(
            kaeso_kpp::ErrorKind::ContentFiltered,
            format!(
                "{PROVIDER}'s safety checker flagged image {} and sent a black image instead; change the prompt.",
                i + 1
            ),
        )
        .with_status(400)
        .with_origin(Origin::new(D))
        .with_raw(v));
    }
    let origin = ctx.origin(D);
    let mut f = crate::video_jobs::provider_object(body, D)?;
    let list = f
        .array("images")?
        .ok_or_else(|| KppError::provider(format!("{PROVIDER} sent a result without `images`.")))?;
    let mut images = Vec::with_capacity(list.len());
    let mut sizes = Vec::with_capacity(list.len());
    for item in list {
        let (image, size) = image_of(item, &origin)?;
        images.push(image);
        sizes.push(size);
    }
    let mut reply = ImageResponse::new(String::new(), images);
    if let Some(Some((w, h))) = sizes.first().copied()
        && sizes.iter().all(|s| *s == Some((w, h)))
    {
        reply.settings.size = Some(ImageSize::pixels(w, h));
    }
    reply.extras = f.rest(&origin);
    Ok(reply)
}

/// The model's output, as fal wrote it.
pub(super) fn encode_response(r: &ImageResponse) -> Result<Vec<u8>, KppError> {
    let mut images = Vec::with_capacity(r.images.len());
    for image in &r.images {
        let url = match &image.source {
            Source::Url(u) => u.clone(),
            Source::Base64(b) => b.data_url(),
            _ => {
                return Err(KppError::unsupported_by(
                    "images by file id",
                    D,
                    "fal answers with links or data URIs.",
                ));
            }
        };
        let mut out = Map::new();
        out.insert("url".into(), url.into());
        put_extras(&mut out, &image.extras, D, "");
        images.push(Value::Object(out));
    }
    let mut out = Map::new();
    out.insert("images".into(), Value::Array(images));
    put_extras(&mut out, &r.extras, D, "");
    body(&out)
}
