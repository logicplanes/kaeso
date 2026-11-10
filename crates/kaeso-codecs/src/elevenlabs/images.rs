//! ElevenLabs' image generations (`POST /v1/flows/image`, `GET /v1/flows/image/{id}`), whose
//! body depends on `model_id`: OpenAI's GPT Image, Google's Gemini image models and ByteDance's
//! Seedream (the `ImageGenerationRequest` schema of ElevenLabs' OpenAPI document and its image
//! and video guide, read 2026-11-09). A generation is a task: the create call answers with an
//! id, and the generation's status call, asked no more than every two seconds, ends with a
//! link to the image. ElevenLabs-format apps send the same body, which is read back to KPP.

use kaeso_kpp::{
    GeneratedImage, ImageBackground, ImageInput, ImageOperation, ImageQuality, ImageRequest,
    ImageResponse, ImageSize, KppError, Origin, Reply, Request, Source,
};
use serde_json::{Map, Value};

use super::D;
use crate::codec::{EgressCtx, EncodedRequest, HeaderList};
use crate::cross::check_extras;
use crate::video_jobs::{Fields, body, put, put_extras};

/// The path of image generations; a generation's own path adds its id.
pub const IMAGE_PATH: &str = "/v1/flows/image";

/// How long ElevenLabs asks a caller to wait between two status calls of an image generation.
pub const POLL_GAP: std::time::Duration = std::time::Duration::from_secs(2);

const PROVIDER: &str = "ElevenLabs";

const GPT_RATIOS: &[&str] = &["1:1", "3:2", "2:3"];
const GPT2_RATIOS: &[&str] = &[
    "auto", "1:1", "4:5", "5:4", "3:4", "4:3", "2:3", "3:2", "1:2", "2:1", "9:16", "16:9", "21:9",
    "1:3", "3:1",
];
const GEMINI_RATIOS: &[&str] = &[
    "auto", "1:1", "2:3", "3:2", "3:4", "4:3", "4:5", "5:4", "9:16", "16:9", "21:9",
];
const GEMINI_FLASH_RATIOS: &[&str] = &[
    "auto", "1:1", "2:3", "3:2", "3:4", "4:3", "4:5", "5:4", "9:16", "16:9", "21:9", "1:4", "4:1",
    "1:8", "8:1",
];
const SEEDREAM_RATIOS: &[&str] = &["auto", "1:1", "3:4", "16:9", "4:3", "9:16"];

/// What one model takes, as its request schema lists it.
struct Model {
    images: usize,
    mask: bool,
    qualities: &'static [&'static str],
    background: bool,
    ratios: &'static [&'static str],
    resolutions: &'static [&'static str],
    seed: bool,
}

fn model(id: &str) -> Option<Model> {
    const GPT_QUALITIES: &[&str] = &["low", "medium", "high"];
    const GPT25_QUALITIES: &[&str] = &["low", "medium", "high", "xhigh", "max"];
    const SIZES: &[&str] = &["1K", "2K", "4K"];
    let none: &[&str] = &[];
    Some(match id {
        "gpt-image-1" | "gpt-image-1.5" => Model {
            images: 5,
            mask: true,
            qualities: GPT_QUALITIES,
            background: true,
            ratios: GPT_RATIOS,
            resolutions: none,
            seed: false,
        },
        "gpt-image-2" | "gpt-image-2.5-sunburst" | "gpt-image-2.5-flare" => Model {
            images: 10,
            mask: true,
            qualities: if id == "gpt-image-2" {
                GPT_QUALITIES
            } else {
                GPT25_QUALITIES
            },
            background: false,
            ratios: GPT2_RATIOS,
            resolutions: SIZES,
            seed: false,
        },
        "gemini-2.5-flash-image" => Model {
            images: 5,
            mask: false,
            qualities: none,
            background: false,
            ratios: GEMINI_RATIOS,
            resolutions: none,
            seed: false,
        },
        "gemini-3-pro-image" => Model {
            images: 10,
            mask: false,
            qualities: none,
            background: false,
            ratios: GEMINI_RATIOS,
            resolutions: SIZES,
            seed: false,
        },
        "gemini-3.1-flash-image" => Model {
            images: 14,
            mask: false,
            qualities: none,
            background: false,
            ratios: GEMINI_FLASH_RATIOS,
            resolutions: &["512", "1K", "2K", "4K"],
            seed: false,
        },
        "gemini-3.1-flash-lite-image" => Model {
            images: 14,
            mask: false,
            qualities: none,
            background: false,
            ratios: GEMINI_RATIOS,
            resolutions: &["1K"],
            seed: false,
        },
        "bytedance-seedream-5-lite" | "bytedance-seedream-5-pro" => Model {
            images: 10,
            mask: false,
            qualities: none,
            background: false,
            ratios: SEEDREAM_RATIOS,
            resolutions: if id.ends_with("lite") {
                &["2K", "3K"]
            } else {
                &["1K", "2K"]
            },
            seed: true,
        },
        _ => return None,
    })
}

fn refused(feature: &str, param: &str, hint: &str) -> KppError {
    crate::video_jobs::refused(D, PROVIDER, feature, param, hint)
}

// OpenAI's GPT Image sizes, which say an aspect ratio; ElevenLabs takes the ratio and picks the
// pixels from its own resolution setting.
fn ratio_of(size: &ImageSize) -> Option<&'static str> {
    match size {
        ImageSize::Auto => Some("auto"),
        _ => match size.dimensions()? {
            (w, h) if w == h => Some("1:1"),
            (1536, 1024) => Some("3:2"),
            (1024, 1536) => Some("2:3"),
            _ => None,
        },
    }
}

/// Checks that ElevenLabs can make `r`, before anything is sent.
pub(super) fn check(r: &ImageRequest) -> Result<(), KppError> {
    check_extras(&r.extras, D, "", "request")?;
    let Some(m) = model(&r.model) else {
        return Err(refused(
            &format!("the image model `{}`", r.model),
            "model",
            "use one of the models its image API lists.",
        ));
    };
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
    if r.images.len() > m.images {
        return Err(refused(
            &format!("more than {} images for `{}`", m.images, r.model),
            "image",
            "send fewer images.",
        ));
    }
    for (i, image) in r.images.iter().chain(&r.mask).enumerate() {
        check_extras(&image.extras, D, "image", "request")?;
        if !matches!(image.source, Source::Base64(_) | Source::FileId(_)) {
            let param = if i < r.images.len() { "image" } else { "mask" };
            return Err(refused(
                "images by link",
                param,
                "upload the image instead.",
            ));
        }
    }
    if r.mask.is_some() && !m.mask {
        return Err(refused(
            &format!("a mask on `{}`", r.model),
            "mask",
            "describe the area to change in the prompt instead.",
        ));
    }
    if r.mask.is_some() && r.images.is_empty() {
        return Err(
            KppError::invalid_request("A `mask` needs an image to edit.").with_param("mask"),
        );
    }
    if r.count.is_some_and(|n| n > 1) {
        return Err(refused("`n` above 1", "n", "send one request per image."));
    }
    if let Some(q) = &r.quality
        && *q != ImageQuality::Auto
        && !m.qualities.contains(&q.as_wire())
    {
        return Err(refused(
            &format!("the quality `{}` on `{}`", q.as_wire(), r.model),
            "quality",
            "use one the model lists, or leave it out.",
        ));
    }
    if r.background.is_some() && !m.background {
        return Err(refused(
            &format!("a background on `{}`", r.model),
            "background",
            "remove it.",
        ));
    }
    let ratio = match (&r.aspect_ratio, &r.size) {
        (Some(ratio), _) => Some(ratio.as_str()),
        (None, Some(size)) => Some(ratio_of(size).ok_or_else(|| {
            refused(
                &format!("the size `{}`", size.as_wire()),
                "size",
                "use 1024x1024, 1536x1024, 1024x1536 or auto, or send `aspect_ratio`.",
            )
        })?),
        (None, None) => None,
    };
    if let Some(ratio) = ratio
        && !m.ratios.contains(&ratio)
    {
        let param = if r.aspect_ratio.is_some() {
            "aspect_ratio"
        } else {
            "size"
        };
        return Err(refused(
            &format!("the aspect ratio `{ratio}` on `{}`", r.model),
            param,
            "use one the model lists.",
        ));
    }
    if let Some(res) = &r.resolution
        && !m.resolutions.contains(&res.as_str())
    {
        return Err(refused(
            &format!("the resolution `{res}` on `{}`", r.model),
            "resolution",
            "use one the model lists, or leave it out.",
        ));
    }
    if r.seed.is_some() && !m.seed {
        return Err(refused(
            &format!("a seed on `{}`", r.model),
            "seed",
            "remove it.",
        ));
    }
    let unsupported = [
        ("output_format", "output_format", r.format.is_some()),
        (
            "output_compression",
            "output_compression",
            r.compression.is_some(),
        ),
        (
            "negative_prompt",
            "negative_prompt",
            r.negative_prompt.is_some(),
        ),
        (
            "partial_images",
            "partial_images",
            r.partial_images.is_some(),
        ),
    ];
    if let Some((name, param, _)) = unsupported.iter().find(|(.., set)| *set) {
        return Err(refused(&format!("`{name}`"), param, "remove it."));
    }
    Ok(())
}

fn reference(image: &ImageInput) -> Value {
    let mut out = Map::new();
    match &image.source {
        Source::Base64(b) => {
            out.insert("type".into(), "inline_base64".into());
            out.insert("content_base64".into(), b.to_base64().into());
            out.insert("mime_type".into(), b.media_type.clone().into());
        }
        Source::FileId(f) => {
            out.insert("type".into(), "asset".into());
            out.insert("asset_id".into(), f.id.clone().into());
        }
        _ => {}
    }
    put_extras(&mut out, &image.extras, D, "");
    Value::Object(out)
}

// An `ImageReference`: an image sent inline or an asset already in the ElevenLabs workspace.
fn reference_from(v: Value, param: &str) -> Result<ImageInput, KppError> {
    let Value::Object(map) = v else {
        return Err(
            KppError::invalid_request(format!("`{param}` takes image reference objects."))
                .with_param(param),
        );
    };
    let mut f = Fields::from_map(map, D, "request");
    let source = match f.str("type")?.as_deref() {
        Some("inline_base64") => crate::media::from_base64(
            f.str("mime_type")?.unwrap_or_default(),
            f.str("content_base64")?.unwrap_or_default(),
            param,
        )?,
        Some("asset") => Source::file_id(Origin::new(D), f.str("asset_id")?.unwrap_or_default()),
        _ => {
            return Err(KppError::unsupported_by(
                "an image reference other than an upload or an asset",
                D,
                "Send the image inline.",
            )
            .with_param(param));
        }
    };
    let mut image = ImageInput::new(source);
    image.extras = f.rest(&Origin::new(D));
    Ok(image)
}

/// An ElevenLabs-format app's `POST /v1/flows/image` body.
pub(super) fn decode_request(body: &[u8]) -> Result<Request, KppError> {
    let mut f = Fields::parse(body, D, "request")?;
    let model = f.str("model_id")?.unwrap_or_default();
    if model.is_empty() {
        return Err(
            KppError::invalid_request("An image generation needs a `model_id`.")
                .with_param("model_id"),
        );
    }
    // A webhook would report ElevenLabs' own generation id, which the app can't ask Kaeso about.
    if f.take("webhook").is_some() {
        return Err(KppError::unsupported_by(
            "webhooks",
            D,
            "Ask `GET /v1/flows/image/{generation_id}` until the generation ends instead.",
        )
        .with_param("webhook"));
    }
    let mut r = ImageRequest::new(model, ImageOperation::Generate);
    r.prompt = f.str("prompt")?;
    for item in f.array("images")?.unwrap_or_default() {
        r.images.push(reference_from(item, "images")?);
    }
    if let Some(mask) = f.take("mask") {
        r.mask = Some(reference_from(mask, "mask")?);
    }
    if !r.images.is_empty() || r.mask.is_some() {
        r.operation = ImageOperation::Edit;
    }
    r.quality = f.str("quality")?.map(|q| ImageQuality::from_wire(&q));
    r.background = f.str("background")?.map(|b| ImageBackground::from_wire(&b));
    r.aspect_ratio = f.str("aspect_ratio")?;
    r.resolution = f.str("resolution")?;
    r.seed = f.i64("seed")?;
    r.extras = f.rest(&Origin::new(D));
    Ok(Request::Image(r))
}

/// The create call of an image generation.
pub(super) fn encode(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    let Request::Image(r) = req else {
        return Err(KppError::unsupported_by(
            format!("{:?} request", req.kind()),
            D,
            "",
        ));
    };
    check(r)?;
    if r.user.is_some() {
        ctx.left_out.push("user".to_owned());
    }
    let mut out = Map::new();
    out.insert("model_id".into(), r.model.clone().into());
    out.insert("prompt".into(), r.prompt.clone().unwrap_or_default().into());
    if !r.images.is_empty() {
        out.insert(
            "images".into(),
            Value::Array(r.images.iter().map(reference).collect()),
        );
    }
    put(&mut out, "mask", r.mask.as_ref().map(reference));
    put(
        &mut out,
        "quality",
        r.quality
            .as_ref()
            .filter(|q| **q != ImageQuality::Auto)
            .map(|q| q.as_wire().to_owned()),
    );
    put(
        &mut out,
        "background",
        r.background.as_ref().map(|b| match b {
            ImageBackground::Other(s) => s.clone(),
            other => other.as_wire().to_owned(),
        }),
    );
    let ratio = r
        .aspect_ratio
        .clone()
        .or_else(|| r.size.as_ref().and_then(ratio_of).map(str::to_owned));
    put(&mut out, "aspect_ratio", ratio);
    put(&mut out, "resolution", r.resolution.clone());
    put(&mut out, "seed", r.seed);
    put_extras(&mut out, &r.extras, D, "");
    Ok(EncodedRequest::post_json(
        IMAGE_PATH,
        HeaderList::new().with("content-type", crate::codec::JSON_MEDIA_TYPE),
        body(&out)?,
    ))
}

fn status_of(body: &[u8]) -> Option<(String, String)> {
    let v: Value = serde_json::from_slice(body).ok()?;
    let id = v.get("id")?.as_str()?.to_owned();
    let status = v.get("status")?.as_str()?.to_owned();
    Some((id, status))
}

/// The generation an answer names while its image is still being made.
pub(crate) fn pending_task(body: &[u8]) -> Option<String> {
    let (id, status) = status_of(body)?;
    matches!(status.as_str(), "pending" | "generating").then_some(id)
}

/// The status call of the generation `id`.
pub(crate) fn poll_request(id: &str) -> EncodedRequest {
    EncodedRequest::get(
        format!("{IMAGE_PATH}/{}", crate::video_jobs::path_segment(id)),
        HeaderList::new(),
    )
}

/// A finished generation: its image as a link, or the failure ElevenLabs reports.
pub(super) fn decode(body: &[u8]) -> Result<Reply, KppError> {
    let v: Value = serde_json::from_slice(body)
        .map_err(|_| KppError::provider("ElevenLabs sent an image generation that is not JSON."))?;
    let text = |k: &str| v.get(k).and_then(Value::as_str);
    match text("status") {
        Some("completed") => {
            let url = text("content_url").ok_or_else(|| {
                KppError::provider("ElevenLabs finished an image generation without its link.")
            })?;
            let mut image = GeneratedImage::new(Source::Url(url.to_owned()));
            let mut extras = kaeso_kpp::Extras::new(kaeso_kpp::Origin::new(D));
            for key in ["id", "content_mime_type"] {
                if let Some(value) = v.get(key)
                    && let Ok(raw) = kaeso_kpp::RawJson::from_serialize(value)
                {
                    extras.fields.insert(key.to_owned(), raw);
                }
            }
            image.extras = extras;
            Ok(Reply::Image(ImageResponse::new(String::new(), vec![image])))
        }
        Some("failed") => {
            let message = text("error_message")
                .unwrap_or("ElevenLabs could not make the image.")
                .to_owned();
            let kind = match text("failure_reason") {
                Some("moderated") => kaeso_kpp::ErrorKind::ContentFiltered,
                Some("invalid_parameters") => kaeso_kpp::ErrorKind::InvalidRequest,
                Some("timeout") => kaeso_kpp::ErrorKind::Timeout,
                Some("charging_failed") => kaeso_kpp::ErrorKind::Billing,
                _ => kaeso_kpp::ErrorKind::ProviderError,
            };
            let status = match kind {
                kaeso_kpp::ErrorKind::ContentFiltered | kaeso_kpp::ErrorKind::InvalidRequest => 400,
                kaeso_kpp::ErrorKind::Billing => 402,
                kaeso_kpp::ErrorKind::Timeout => 504,
                _ => 502,
            };
            Err(KppError::new(kind, message)
                .with_status(status)
                .with_origin(kaeso_kpp::Origin::new(D)))
        }
        _ => Err(KppError::provider(
            "ElevenLabs answered an image generation that has not finished.",
        )),
    }
}
