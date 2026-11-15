//! Images through Replicate's official models: FLUX.1 [schnell] and [dev], FLUX1.1 [pro],
//! Seedream 4.5 and Nano Banana Pro, each with the input of its Cog OpenAPI schema
//! (`https://replicate.com/{owner}/{name}/api/schema`, read 2026-11-14).

use kaeso_kpp::{
    GeneratedImage, ImageBackground, ImageFormat, ImageInput, ImageOperation, ImageQuality,
    ImageRequest, ImageResponse, ImageSize, KppError, Origin, Request, Source,
};
use serde_json::{Map, Value};

use super::{D, PROVIDER, Prediction, error, prediction_of, submit};
use crate::codec::{EgressCtx, EncodedRequest};
use crate::cross::check_extras;
use crate::video_jobs::{Fields, body, link_source, put, put_extras};

/// The image models Kaeso maps to Replicate's inputs.
pub const IMAGE_MODELS: &[&str] = &[
    "black-forest-labs/flux-schnell",
    "black-forest-labs/flux-dev",
    "black-forest-labs/flux-1.1-pro",
    "bytedance/seedream-4.5",
    "google/nano-banana-pro",
];

const FLUX_RATIOS: &[&str] = &[
    "1:1", "16:9", "21:9", "3:2", "2:3", "4:5", "5:4", "3:4", "4:3", "9:16", "9:21",
];
const FLUX_PRO_RATIOS: &[&str] = &[
    "1:1", "16:9", "3:2", "2:3", "4:5", "5:4", "9:16", "3:4", "4:3",
];
const SEEDREAM_RATIOS: &[&str] = &[
    "match_input_image",
    "1:1",
    "4:3",
    "3:4",
    "16:9",
    "9:16",
    "3:2",
    "2:3",
    "21:9",
];
const NANO_RATIOS: &[&str] = &[
    "match_input_image",
    "1:1",
    "2:3",
    "3:2",
    "3:4",
    "4:3",
    "4:5",
    "5:4",
    "9:16",
    "16:9",
    "21:9",
];
// Seedream 4.5 and Nano Banana Pro take 1 to 14 input images (their `image_input`).
const MAX_INPUT_IMAGES: usize = 14;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Family {
    /// FLUX.1 [schnell] and [dev]: `num_outputs` 1 to 4, an `aspect_ratio`, no pixel size.
    Flux,
    /// FLUX1.1 [pro]: one image, an `aspect_ratio` or `custom` with a width and height of 256
    /// to 1440 in steps of 32.
    FluxPro,
    /// Seedream 4.5: up to 15 images as a group (`max_images`), `size` 2K, 4K or `custom` with
    /// 1024 to 4096 a side, up to 14 input images; no seed or output format.
    Seedream,
    /// Nano Banana Pro: one image at `resolution` 1K, 2K or 4K and an `aspect_ratio`, up to 14
    /// input images, JPEG or PNG.
    NanoBanana,
}

impl Family {
    pub(super) fn of(model: &str) -> Option<Family> {
        Some(match model {
            "black-forest-labs/flux-schnell" | "black-forest-labs/flux-dev" => Family::Flux,
            "black-forest-labs/flux-1.1-pro" => Family::FluxPro,
            "bytedance/seedream-4.5" => Family::Seedream,
            "google/nano-banana-pro" => Family::NanoBanana,
            _ => return None,
        })
    }

    fn ratios(self) -> &'static [&'static str] {
        match self {
            Family::Flux => FLUX_RATIOS,
            Family::FluxPro => FLUX_PRO_RATIOS,
            Family::Seedream => SEEDREAM_RATIOS,
            Family::NanoBanana => NANO_RATIOS,
        }
    }

    fn max_images(self) -> u32 {
        match self {
            Family::Flux => 4,
            Family::Seedream => 15,
            Family::FluxPro | Family::NanoBanana => 1,
        }
    }

    fn resolutions(self) -> &'static [&'static str] {
        match self {
            Family::Seedream => &["2K", "4K"],
            Family::NanoBanana => &["1K", "2K", "4K"],
            Family::Flux | Family::FluxPro => &[],
        }
    }

    fn formats(self) -> &'static [ImageFormat] {
        match self {
            Family::Flux | Family::FluxPro => {
                &[ImageFormat::Webp, ImageFormat::Jpeg, ImageFormat::Png]
            }
            Family::NanoBanana => &[ImageFormat::Jpeg, ImageFormat::Png],
            Family::Seedream => &[],
        }
    }

    /// Whether the model's output is one link rather than a list of them.
    fn single_output(self) -> bool {
        matches!(self, Family::FluxPro | Family::NanoBanana)
    }

    fn takes_images(self) -> bool {
        matches!(self, Family::Seedream | Family::NanoBanana)
    }

    fn takes_seed(self) -> bool {
        matches!(self, Family::Flux | Family::FluxPro)
    }
}

fn refused(feature: &str, param: &str, hint: &str) -> KppError {
    crate::video_jobs::refused(D, PROVIDER, feature, param, hint)
}

fn listed(items: &[&str]) -> String {
    match items.split_last() {
        Some((last, [])) => (*last).to_owned(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
        None => String::new(),
    }
}

/// The pixel size `size` asks for on a model that takes one, checked against its limits.
fn custom_size(f: Family, model: &str, size: &ImageSize) -> Result<Option<(u32, u32)>, KppError> {
    if *size == ImageSize::Auto {
        return Ok(None);
    }
    let Some((w, h)) = size.dimensions() else {
        return Err(KppError::invalid_request(format!(
            "`size` must be `auto` or WIDTHxHEIGHT, not `{}`.",
            size.as_wire()
        ))
        .with_param("size"));
    };
    let fits = match f {
        Family::FluxPro => [w, h]
            .iter()
            .all(|s| (256..=1440).contains(s) && s % 32 == 0),
        Family::Seedream => [w, h].iter().all(|s| (1024..=4096).contains(s)),
        Family::Flux | Family::NanoBanana => {
            return Err(refused(
                &format!("a pixel size on `{model}`"),
                "size",
                "send `aspect_ratio` (and `resolution`) instead.",
            ));
        }
    };
    if !fits {
        let hint = if f == Family::FluxPro {
            "use 256 to 1440 pixels a side in steps of 32, such as 1024x768."
        } else {
            "use 1024 to 4096 pixels a side, such as 2048x2048."
        };
        return Err(refused(
            &format!("the size `{}`", size.as_wire()),
            "size",
            hint,
        ));
    }
    Ok(Some((w, h)))
}

/// Checks that Replicate can make `r`, before anything is sent.
pub(super) fn check(r: &ImageRequest) -> Result<Family, KppError> {
    check_extras(&r.extras, D, "", "request")?;
    let f = Family::of(&r.model).ok_or_else(|| {
        refused(
            &format!("the image model `{}` through Kaeso", r.model),
            "model",
            &format!("use one of {}.", IMAGE_MODELS.join(", ")),
        )
    })?;
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
    if !r.images.is_empty() && !f.takes_images() {
        return Err(refused(
            &format!("images to edit on `{}`", r.model),
            "image",
            "use `bytedance/seedream-4.5` or `google/nano-banana-pro`.",
        ));
    }
    if r.images.len() > MAX_INPUT_IMAGES {
        return Err(refused(
            "more than 14 input images",
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
        let hint = match f.max_images() {
            1 => "send 1 and make more requests for more images.".to_owned(),
            max => format!("send 1 to {max}."),
        };
        return Err(refused(&format!("`n` of {n} on `{}`", r.model), "n", &hint));
    }
    if let Some(size) = &r.size {
        let custom = custom_size(f, &r.model, size)?;
        if custom.is_some() && (r.aspect_ratio.is_some() || r.resolution.is_some()) {
            return Err(KppError::invalid_request(
                "Send either a pixel `size` or `aspect_ratio` and `resolution`, not both.",
            )
            .with_param("size"));
        }
    }
    if let Some(ratio) = &r.aspect_ratio
        && !f.ratios().contains(&ratio.as_str())
    {
        return Err(refused(
            &format!("the aspect ratio `{ratio}` on `{}`", r.model),
            "aspect_ratio",
            &format!("use {}.", listed(f.ratios())),
        ));
    }
    if r.aspect_ratio.as_deref() == Some("match_input_image") && r.images.is_empty() {
        return Err(KppError::invalid_request(
            "`match_input_image` needs an image to match; send one or another aspect ratio.",
        )
        .with_param("aspect_ratio"));
    }
    if let Some(res) = &r.resolution
        && resolution_of(f, res).is_none()
    {
        let hint = match f.resolutions() {
            [] => "leave it out.".to_owned(),
            some => format!("use {}.", listed(some)),
        };
        return Err(refused(
            &format!("the resolution `{res}` on `{}`", r.model),
            "resolution",
            &hint,
        ));
    }
    if let Some(format) = &r.format
        && !f.formats().contains(format)
    {
        let hint = if f.formats().is_empty() {
            "leave it out.".to_owned()
        } else {
            let names: Vec<&str> = f.formats().iter().map(ImageFormat::as_wire).collect();
            format!("use {}.", listed(&names))
        };
        return Err(refused(
            &format!("the output format `{}` on `{}`", format.as_wire(), r.model),
            "output_format",
            &hint,
        ));
    }
    if r.compression.is_some() && !matches!(f, Family::Flux | Family::FluxPro) {
        return Err(refused(
            &format!("`output_compression` on `{}`", r.model),
            "output_compression",
            "leave it out.",
        ));
    }
    if r.seed.is_some() && !f.takes_seed() {
        return Err(refused(
            &format!("a seed on `{}`", r.model),
            "seed",
            "leave it out.",
        ));
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
    if let Some(b) = &r.background
        && *b != ImageBackground::Auto
    {
        return Err(refused(
            &format!("the background `{}`", b.as_wire()),
            "background",
            "leave it out.",
        ));
    }
    let unsupported = [
        ("negative_prompt", r.negative_prompt.is_some()),
        ("partial_images", r.partial_images.is_some()),
        ("user", r.user.is_some()),
    ];
    if let Some((name, _)) = unsupported.iter().find(|(_, set)| *set) {
        return Err(refused(&format!("`{name}`"), name, "leave it out."));
    }
    Ok(f)
}

/// The resolution as `f` spells it, matched without case.
fn resolution_of(f: Family, res: &str) -> Option<&'static str> {
    f.resolutions()
        .iter()
        .copied()
        .find(|r| r.eq_ignore_ascii_case(res))
}

fn image_link(image: &ImageInput) -> Option<String> {
    match &image.source {
        Source::Url(u) => Some(u.clone()),
        Source::Base64(b) => Some(b.data_url()),
        _ => None,
    }
}

fn format_wire(format: &ImageFormat) -> String {
    match format {
        ImageFormat::Jpeg => "jpg".to_owned(),
        other => other.as_wire().to_owned(),
    }
}

/// The input of the model `r` names. Replicate answers with the images' links, so an app that
/// asks for the bytes gets them through Kaeso's fetch of `replicate.delivery`.
pub(super) fn encode_request(r: &ImageRequest) -> Result<EncodedRequest, KppError> {
    let f = check(r)?;
    let mut input = Map::new();
    input.insert("prompt".into(), r.prompt.clone().unwrap_or_default().into());
    if !r.images.is_empty() {
        input.insert(
            "image_input".into(),
            Value::Array(
                r.images
                    .iter()
                    .filter_map(image_link)
                    .map(Value::from)
                    .collect(),
            ),
        );
    }
    let custom = match &r.size {
        Some(size) => custom_size(f, &r.model, size)?,
        None => None,
    };
    match (f, custom) {
        (Family::FluxPro, Some((w, h))) => {
            input.insert("aspect_ratio".into(), "custom".into());
            input.insert("width".into(), w.into());
            input.insert("height".into(), h.into());
        }
        (Family::Seedream, Some((w, h))) => {
            input.insert("size".into(), "custom".into());
            input.insert("width".into(), w.into());
            input.insert("height".into(), h.into());
        }
        _ => {}
    }
    put(&mut input, "aspect_ratio", r.aspect_ratio.clone());
    if let Some(res) = r
        .resolution
        .as_deref()
        .and_then(|res| resolution_of(f, res))
    {
        let key = if f == Family::Seedream {
            "size"
        } else {
            "resolution"
        };
        input.insert(key.into(), res.into());
    }
    match f {
        Family::Flux => put(&mut input, "num_outputs", r.count),
        Family::Seedream => {
            if let Some(n) = r.count.filter(|n| *n > 1) {
                input.insert("sequential_image_generation".into(), "auto".into());
                input.insert("max_images".into(), n.into());
            }
        }
        Family::FluxPro | Family::NanoBanana => {}
    }
    put(&mut input, "seed", r.seed);
    put(
        &mut input,
        "output_format",
        r.format.as_ref().map(format_wire),
    );
    put(&mut input, "output_quality", r.compression);
    put_extras(&mut input, &r.extras, D, "");
    submit(&r.model, input, true)
}

/// A Replicate image model's input, as the dialect's fixtures write it; the model is in the
/// URL, so each field is read by its name alone.
pub(super) fn decode_request(mut f: Fields) -> Result<Request, KppError> {
    let mut r = ImageRequest::new(String::new(), ImageOperation::Generate);
    r.prompt = f.str("prompt")?;
    for url in f.array("image_input")?.unwrap_or_default() {
        let Value::String(url) = url else {
            return Err(
                KppError::invalid_request("`image_input` lists links.").with_param("image_input")
            );
        };
        r.images.push(ImageInput::new(link_source(url)));
        r.operation = ImageOperation::Edit;
    }
    let side = |f: &Fields, k: &str| {
        f.peek(k)
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
    };
    let custom = |f: &Fields, k: &str| f.peek(k).and_then(Value::as_str) == Some("custom");
    if (custom(&f, "aspect_ratio") || custom(&f, "size"))
        && let (Some(w), Some(h)) = (side(&f, "width"), side(&f, "height"))
    {
        r.size = Some(ImageSize::pixels(w, h));
        for k in ["width", "height"] {
            f.take(k);
        }
        if custom(&f, "aspect_ratio") {
            f.take("aspect_ratio");
        } else {
            f.take("size");
        }
    }
    r.aspect_ratio = f.str("aspect_ratio")?;
    r.resolution = match f.str("resolution")? {
        Some(res) => Some(res),
        None => f.str("size")?,
    };
    r.count = f.u32("num_outputs")?;
    if f.peek("sequential_image_generation")
        .and_then(Value::as_str)
        == Some("auto")
    {
        f.take("sequential_image_generation");
        r.count = f.u32("max_images")?;
    }
    r.seed = f.i64("seed")?;
    r.format = f.str("output_format")?.map(|s| match s.as_str() {
        "jpg" => ImageFormat::Jpeg,
        other => ImageFormat::from_wire(other),
    });
    r.compression = f
        .u32("output_quality")?
        .map(|q| u8::try_from(q.min(100)).unwrap_or(100));
    r.extras = f.rest(&Origin::new(D));
    Ok(Request::Image(r))
}

/// A finished image prediction: its output's links, or the failure Replicate reports.
pub(super) fn decode_response(body: &[u8], ctx: &EgressCtx) -> Result<ImageResponse, KppError> {
    let (v, p) = prediction_of(body)?;
    if p.ended_badly() {
        return Err(error::failed(&v));
    }
    if p.running() {
        return Err(KppError::provider(format!(
            "{PROVIDER} answered before the image was made."
        )));
    }
    let links = links_of(&p, &v)?;
    let origin = ctx.origin(D);
    let mut f = crate::video_jobs::provider_object(body, D)?;
    f.take("output");
    let model = f.str("model")?.unwrap_or_default();
    let images = links
        .into_iter()
        .map(|l| GeneratedImage::new(link_source(l)))
        .collect();
    let mut reply = ImageResponse::new(model, images);
    if let Some(Value::Object(input)) = f.peek("input") {
        let text = |k: &str| input.get(k).and_then(Value::as_str);
        let side = |k: &str| {
            input
                .get(k)
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
        };
        let custom = text("size") == Some("custom") || text("aspect_ratio") == Some("custom");
        if custom && let (Some(w), Some(h)) = (side("width"), side("height")) {
            reply.settings.size = Some(ImageSize::pixels(w, h));
        }
        reply.settings.resolution = text("resolution")
            .or_else(|| text("size").filter(|s| *s != "custom"))
            .map(str::to_owned);
    }
    reply.extras = f.rest(&origin);
    Ok(reply)
}

fn links_of(p: &Prediction, v: &Value) -> Result<Vec<String>, KppError> {
    p.links().ok_or_else(|| {
        if v.get("data_removed").and_then(Value::as_bool) == Some(true) {
            KppError::provider(format!(
                "{PROVIDER} removed the prediction's output; it keeps files for an hour."
            ))
        } else {
            KppError::provider(format!("{PROVIDER} finished without naming an image."))
        }
    })
}

/// The prediction, as Replicate writes it: one link for a model whose output is one, a list
/// otherwise.
pub(super) fn encode_response(r: &ImageResponse) -> Result<Vec<u8>, KppError> {
    let mut links = Vec::with_capacity(r.images.len());
    for image in &r.images {
        let url = match &image.source {
            Source::Url(u) => u.clone(),
            Source::Base64(b) => b.data_url(),
            _ => {
                return Err(KppError::unsupported_by(
                    "images by file id",
                    D,
                    "Replicate answers with links.",
                ));
            }
        };
        links.push(Value::from(url));
    }
    let single = Family::of(&r.model).is_some_and(Family::single_output) && links.len() == 1;
    let mut out = Map::new();
    if !r.model.is_empty() {
        out.insert("model".into(), r.model.clone().into());
    }
    out.insert(
        "output".into(),
        if single {
            links.pop().unwrap_or(Value::Null)
        } else {
            Value::Array(links)
        },
    );
    if !out.contains_key("status") {
        out.insert("status".into(), super::SUCCEEDED.into());
    }
    put_extras(&mut out, &r.extras, D, "");
    body(&out)
}
