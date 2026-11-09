//! Images on DashScope's synchronous multimodal generation API
//! (<https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image>,
//! `…/qwen-image-editing`, `…/wan27-image-gen-edit/synchronous`,
//! `…/wan26-image-gen-edit/synchronous`, `…/wan-text-to-image-v2/synchronous` and `…/z-image`,
//! read 2026-11-09): one user message with the images and the prompt, `parameters` beside it,
//! and the images back as links valid for 24 hours.
//!
//! Wan 2.5 and earlier text-to-image models answer only as tasks
//! (`…/wan-text-to-image-v2/create-task` and `…/query-result`): the request takes `input.prompt`
//! and `X-DashScope-Async: enable`, the reply names a task, and the task's result lists the
//! images in `output.results`. [`pending_task`] tells a caller which task to wait for.

use kaeso_kpp::{
    DialectId, Extras, GeneratedImage, ImageDelivery, ImageInput, ImageOperation, ImageRequest,
    ImageResponse, ImageSize, KppError, Origin, RawUsage, Reply, Request, Source, StreamMode,
    Usage,
};
use serde_json::{Map, Value};

use super::{D, PROVIDER};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList};
use crate::video_jobs::{Fields, body, provider_object, put, put_extras, refused};

/// The path of DashScope's synchronous multimodal generation, from the root of the host. Image
/// generation, Qwen3-TTS and Qwen-Audio-ASR share it.
pub const GENERATION_PATH: &str = "/api/v1/services/aigc/multimodal-generation/generation";

/// The path of DashScope's text-to-image tasks, from the root of the host.
pub const TEXT2IMAGE_PATH: &str = "/api/v1/services/aigc/text2image/image-synthesis";

// The resolution tiers Wan takes in `size` instead of pixels (the Wan 2.7 and 2.6 references).
const TIERS: &[&str] = &["1K", "2K", "4K"];

// The extension fields QwenCloud's OpenAI-compatible image mode documents; an OpenAI-format app
// sends them as extra body fields, and DashScope takes the same names in `parameters`.
const CARRIED: &[&str] = &[
    "prompt_extend",
    "prompt_extend_mode",
    "enable_thinking",
    "watermark",
];

const CHOICES: &str = "choices";
const RESULTS: &str = "results";
const TEXT_FIRST: &str = "text_first";
// The task form's `input.prompt`, kept so a native request is written back as it came.
const PROMPT_FORM: &str = "prompt_form";

/// How a QwenCloud image model takes a request on DashScope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImageFamily {
    /// Qwen-Image 2.x and 3.0: up to six images, edits of one to ten images, sizes in pixels.
    QwenImage,
    /// Wan 2.7 Image: zero to nine input images, a tier or pixels.
    Wan27,
    /// Wan 2.6 Image: edits of one to four images; it makes images from a prompt alone only in
    /// its streamed interleaved mode, which Kaeso doesn't use.
    Wan26,
    /// Z-Image: one image from a prompt.
    ZImage,
    /// Wan 2.6 text-to-image (`wan2.6-t2i`): one to four images from a prompt, synchronously.
    WanT2i,
    /// Wan 2.5 and earlier text-to-image, such as `wan2.2-t2i-flash`: one to four images from a
    /// prompt, made as a task.
    WanT2iTask,
}

/// The family of a QwenCloud image model, or `None` for one Kaeso doesn't send to DashScope.
pub fn image_family(model: &str) -> Option<ImageFamily> {
    let m = model.to_ascii_lowercase();
    if m.starts_with("wan2.6-t2i") {
        Some(ImageFamily::WanT2i)
    } else if m.starts_with("wan") && m.contains("-t2i") {
        Some(ImageFamily::WanT2iTask)
    } else if m.starts_with("qwen-image-") {
        Some(ImageFamily::QwenImage)
    } else if m.starts_with("wan2.7-image") {
        Some(ImageFamily::Wan27)
    } else if m.starts_with("wan2.6-image") {
        Some(ImageFamily::Wan26)
    } else if m.starts_with("z-image") {
        Some(ImageFamily::ZImage)
    } else {
        None
    }
}

fn no(feature: &str, param: &str, hint: &str) -> KppError {
    refused(D, PROVIDER, feature, param, hint)
}

fn image_link(input: &ImageInput) -> Option<String> {
    match &input.source {
        Source::Url(u) => Some(u.clone()),
        Source::Base64(b) => Some(format!("data:{};base64,{}", b.media_type, b.data)),
        _ => None,
    }
}

pub(super) fn decode_request(body: &[u8]) -> Result<Request, KppError> {
    let mut f = Fields::parse(body, D, "request")?;
    let mut extras = Extras::new(Origin::new(D));
    let mut r = ImageRequest::new(
        f.str("model")?.unwrap_or_default(),
        ImageOperation::Generate,
    );
    if let Some(mut input) = f.object("input")? {
        if let Some(prompt) = input.str("prompt")? {
            r.prompt = Some(prompt);
            r.negative_prompt = input.str("negative_prompt")?;
            extras.form.insert(PROMPT_FORM.into(), Value::Bool(true));
            input.rest_into(&mut extras, "input");
        } else {
            let messages = input.array("messages")?.unwrap_or_default();
            let [Value::Object(message)] = messages.as_slice() else {
                return Err(KppError::invalid_request(
                    "`input.messages` holds exactly one user message.",
                )
                .with_param("messages"));
            };
            let mut m = Fields::from_map(message.clone(), D, "request");
            if m.str("role")?.as_deref() != Some("user") {
                return Err(KppError::invalid_request("The message's `role` is `user`.")
                    .with_param("messages"));
            }
            for (i, item) in m
                .array("content")?
                .unwrap_or_default()
                .into_iter()
                .enumerate()
            {
                match item {
                    Value::Object(part) if part.len() == 1 && part.contains_key("image") => {
                        let link = part
                            .get("image")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        r.images
                            .push(ImageInput::new(crate::video_jobs::link_source(
                                link.to_owned(),
                            )));
                    }
                    Value::Object(part) if part.len() == 1 && part.contains_key("text") => {
                        if r.prompt.is_some() {
                            return Err(KppError::invalid_request(
                                "The message holds one `text` part only.",
                            )
                            .with_param("content"));
                        }
                        if i == 0 {
                            extras.form.insert(TEXT_FIRST.into(), Value::Bool(true));
                        }
                        r.prompt = part.get("text").and_then(Value::as_str).map(str::to_owned);
                    }
                    _ => {
                        return Err(KppError::unsupported_by(
                            "content other than images and one prompt",
                            D,
                            "Kaeso maps an image request's images and its prompt only.",
                        )
                        .with_param("content"));
                    }
                }
            }
            m.rest_into(&mut extras, "input.message");
            input.rest_into(&mut extras, "input");
        }
    }
    if r.images.is_empty() {
        extras.form.shift_remove(TEXT_FIRST);
    } else {
        r.operation = ImageOperation::Edit;
    }
    if let Some(mut p) = f.object("parameters")? {
        r.count = p.u32("n")?;
        match p.str("size")? {
            Some(s) if TIERS.contains(&s.to_ascii_uppercase().as_str()) => r.resolution = Some(s),
            Some(s) => r.size = Some(pixels(&s)?),
            None => {}
        }
        if let Some(n) = p.str("negative_prompt")? {
            r.negative_prompt = Some(n);
        }
        r.seed = p.i64("seed")?;
        p.rest_into(&mut extras, "parameters");
    }
    f.rest_into(&mut extras, "");
    r.extras = extras;
    Ok(Request::Image(r))
}

fn pixels(s: &str) -> Result<ImageSize, KppError> {
    let parsed = s.split_once('*').and_then(|(w, h)| {
        Some(ImageSize::pixels(
            crate::video_jobs::digits(w.trim())?,
            crate::video_jobs::digits(h.trim())?,
        ))
    });
    parsed.ok_or_else(|| {
        KppError::invalid_request(format!(
            "`size` is `WIDTH*HEIGHT` or a tier such as `2K`, not `{s}`."
        ))
        .with_param("size")
    })
}

/// The fields of an OpenAI-format app that carry over as parameters, and the check that every
/// other foreign field is refused.
fn carried(r: &ImageRequest) -> Result<Vec<(String, Value)>, KppError> {
    if r.extras.is_from(D) {
        return Ok(Vec::new());
    }
    let from_openai = r
        .extras
        .origin
        .as_ref()
        .is_some_and(|o| o.is(DialectId::OpenAiImages));
    let mut rest = r.extras.clone();
    let mut out = Vec::new();
    if from_openai {
        for key in CARRIED {
            if let Some(v) = rest.fields.shift_remove(*key)
                && let Ok(value) = v.to_value()
            {
                out.push(((*key).to_owned(), value));
            }
        }
    }
    crate::cross::check_extras(&rest, D, "", "request")?;
    for image in r.images.iter().chain(r.mask.as_ref()) {
        crate::cross::check_extras(&image.extras, D, "image", "request")?;
    }
    Ok(out)
}

pub(super) fn check(r: &ImageRequest) -> Result<ImageFamily, KppError> {
    carried(r)?;
    let Some(fam) = image_family(&r.model) else {
        return Err(KppError::unsupported_by(
            format!("the model `{}`", r.model),
            D,
            "Kaeso sends Qwen-Image, Wan Image, Wan text-to-image and Z-Image models to DashScope.",
        )
        .with_param("model"));
    };
    if r.operation == ImageOperation::Variation {
        return Err(no(
            "image variations",
            "image",
            "send the image and a prompt to `/v1/images/edits`.",
        ));
    }
    if r.mask.is_some() {
        return Err(no(
            "masks",
            "mask",
            "describe the change in the prompt instead.",
        ));
    }
    let unsupported = [
        ("quality", r.quality.is_some()),
        ("output_format", r.format.is_some()),
        ("output_compression", r.compression.is_some()),
        ("background", r.background.is_some()),
        ("aspect_ratio", r.aspect_ratio.is_some()),
    ];
    if let Some((field, _)) = unsupported.iter().find(|(_, set)| *set) {
        return Err(no(&format!("`{field}`"), field, "remove it."));
    }
    if r.delivery == Some(ImageDelivery::Base64) {
        return Err(no(
            "`b64_json`",
            "response_format",
            "it answers with links only; remove `response_format`.",
        ));
    }
    if !r.stream.is_off() && r.partial_images.is_some_and(|n| n > 0) {
        return Err(no(
            "partial images",
            "partial_images",
            "set `partial_images` to 0 to get the finished image as one event.",
        ));
    }
    if r.images.iter().any(|i| image_link(i).is_none()) {
        return Err(no(
            "file ids",
            "image",
            "upload the image or send it as a link.",
        ));
    }
    let edits = !r.images.is_empty();
    let tier = r.resolution.as_deref().map(str::to_ascii_uppercase);
    if let Some(t) = &tier
        && !TIERS.contains(&t.as_str())
    {
        return Err(no(
            &format!("the resolution `{t}`"),
            "resolution",
            "use 1K, 2K or 4K.",
        ));
    }
    let pixel_size = match &r.size {
        Some(ImageSize::Auto) | None => None,
        Some(s) => Some(s.dimensions().ok_or_else(|| {
            no(
                &format!("the size `{}`", s.as_wire()),
                "size",
                "send `WIDTHxHEIGHT` in pixels.",
            )
        })?),
    };
    if tier.is_some() && pixel_size.is_some() {
        return Err(no(
            "both a size and a resolution",
            "resolution",
            "send one of them.",
        ));
    }
    match fam {
        ImageFamily::QwenImage => {
            let two_one = r.model.starts_with("qwen-image-2.1");
            let most = if two_one { 10 } else { 3 };
            if r.images.len() > most {
                return Err(no(
                    &format!("more than {most} images on this model"),
                    "image",
                    "send fewer images.",
                ));
            }
            if two_one && r.negative_prompt.is_some() {
                return Err(no(
                    "`negative_prompt` on Qwen-Image 2.1",
                    "negative_prompt",
                    "remove it.",
                ));
            }
            if tier.is_some() {
                return Err(no(
                    "resolution tiers on Qwen-Image",
                    "resolution",
                    "send `size` in pixels.",
                ));
            }
            if r.count.is_some_and(|n| n > 6) {
                return Err(no("`n` above 6", "n", "ask for fewer images."));
            }
        }
        ImageFamily::Wan27 => {
            if r.images.len() > 9 {
                return Err(no("more than 9 images", "image", "send fewer images."));
            }
            if r.negative_prompt.is_some() {
                return Err(no(
                    "`negative_prompt` on Wan 2.7",
                    "negative_prompt",
                    "remove it.",
                ));
            }
        }
        ImageFamily::Wan26 => {
            if !edits {
                return Err(no(
                    "a prompt without images on Wan 2.6",
                    "image",
                    "send one to four images to `/v1/images/edits`, or use `wan2.7-image`.",
                ));
            }
            if r.images.len() > 4 {
                return Err(no("more than 4 images", "image", "send fewer images."));
            }
            if tier.as_deref() == Some("4K") {
                return Err(no(
                    "the resolution `4K` on Wan 2.6",
                    "resolution",
                    "use 1K or 2K.",
                ));
            }
            if r.count.is_some_and(|n| n > 4) {
                return Err(no("`n` above 4", "n", "ask for fewer images."));
            }
        }
        ImageFamily::WanT2i | ImageFamily::WanT2iTask => {
            if edits {
                return Err(no(
                    "images to edit on Wan text-to-image",
                    "image",
                    "send a prompt to `/v1/images/generations`, or use `wan2.7-image`.",
                ));
            }
            if r.count.is_some_and(|n| n > 4) {
                return Err(no("`n` above 4", "n", "ask for fewer images."));
            }
            if tier.is_some() {
                return Err(no(
                    "resolution tiers on Wan text-to-image",
                    "resolution",
                    "send `size` in pixels.",
                ));
            }
        }
        ImageFamily::ZImage => {
            if edits {
                return Err(no(
                    "images to edit on Z-Image",
                    "image",
                    "send a prompt to `/v1/images/generations`, or use `qwen-image-3.0`.",
                ));
            }
            if r.count.is_some_and(|n| n > 1) {
                return Err(no("`n` above 1", "n", "ask for one image."));
            }
            if r.negative_prompt.is_some() {
                return Err(no(
                    "`negative_prompt` on Z-Image",
                    "negative_prompt",
                    "remove it.",
                ));
            }
            if tier.is_some() {
                return Err(no(
                    "resolution tiers on Z-Image",
                    "resolution",
                    "send `size` in pixels.",
                ));
            }
        }
    }
    Ok(fam)
}

pub(super) fn encode_request(
    r: &ImageRequest,
    ctx: &mut EgressCtx,
) -> Result<EncodedRequest, KppError> {
    let fam = check(r)?;
    let carried = carried(r)?;
    ctx.stream = StreamMode::Off;
    if r.user.is_some() {
        ctx.left_out.push("user".to_owned());
    }
    let own = r.extras.is_from(D);
    let mut content: Vec<Value> = r
        .images
        .iter()
        .filter_map(image_link)
        .map(|link| {
            let mut part = Map::new();
            part.insert("image".into(), link.into());
            Value::Object(part)
        })
        .collect();
    let mut text = Map::new();
    text.insert("text".into(), r.prompt.clone().unwrap_or_default().into());
    if r.extras.form_flag(D, TEXT_FIRST) {
        content.insert(0, Value::Object(text));
    } else {
        content.push(Value::Object(text));
    }
    let task = fam == ImageFamily::WanT2iTask;
    let mut input = Map::new();
    if task {
        input.insert("prompt".into(), r.prompt.clone().unwrap_or_default().into());
        put(&mut input, "negative_prompt", r.negative_prompt.clone());
    } else {
        let mut message = Map::new();
        message.insert("role".into(), "user".into());
        message.insert("content".into(), Value::Array(content));
        put_extras(&mut message, &r.extras, D, "input.message");
        input.insert(
            "messages".into(),
            Value::Array(vec![Value::Object(message)]),
        );
    }
    put_extras(&mut input, &r.extras, D, "input");
    let mut params = Map::new();
    let count = match (fam, r.count) {
        (ImageFamily::ZImage, _) => None,
        // Wan 2.6 and Wan text-to-image make four images when `n` is left out, not the one an
        // OpenAI app expects.
        (ImageFamily::Wan26 | ImageFamily::WanT2i | ImageFamily::WanT2iTask, None) if !own => {
            Some(1)
        }
        (_, n) => n,
    };
    put(&mut params, "n", count);
    let size = match (&r.resolution, &r.size) {
        (Some(t), _) if own => Some(t.clone()),
        (Some(t), _) => Some(t.to_ascii_uppercase()),
        (None, Some(s)) => s.dimensions().map(|(w, h)| format!("{w}*{h}")),
        (None, None) => None,
    };
    put(&mut params, "size", size);
    if !task {
        put(&mut params, "negative_prompt", r.negative_prompt.clone());
    }
    put(&mut params, "seed", r.seed);
    for (key, value) in carried {
        params.insert(key, value);
    }
    put_extras(&mut params, &r.extras, D, "parameters");
    let mut out = Map::new();
    out.insert("model".into(), r.model.clone().into());
    out.insert("input".into(), Value::Object(input));
    if !params.is_empty() {
        out.insert("parameters".into(), Value::Object(params));
    }
    put_extras(&mut out, &r.extras, D, "");
    let headers = HeaderList::new().with("content-type", crate::codec::JSON_MEDIA_TYPE);
    if task {
        return Ok(EncodedRequest::post_json(
            TEXT2IMAGE_PATH,
            headers.with("x-dashscope-async", "enable"),
            body(&out)?,
        ));
    }
    Ok(EncodedRequest::post_json(
        GENERATION_PATH,
        headers,
        body(&out)?,
    ))
}

/// Whether the request asks QwenCloud to rewrite its prompt (`prompt_extend: true`), from an
/// OpenAI-format app's extra field or DashScope's own `parameters`.
pub fn extends_prompt(r: &ImageRequest) -> bool {
    ["prompt_extend", "parameters.prompt_extend"]
        .iter()
        .filter_map(|k| r.extras.fields.get(*k))
        .any(|v| v.to_value().ok() == Some(Value::Bool(true)))
}

/// The task a reply names while its images are still being made, from DashScope's answer to
/// a text-to-image task or to `GET /api/v1/tasks/{task_id}`.
pub fn pending_task(body: &[u8]) -> Option<String> {
    let v: Value = serde_json::from_slice(body).ok()?;
    let output = v.get("output")?;
    let status = output.get("task_status")?.as_str()?;
    if !matches!(status, "PENDING" | "RUNNING") {
        return None;
    }
    output
        .get("task_id")?
        .as_str()
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
}

fn number(v: &Map<String, Value>, key: &str) -> Option<u64> {
    v.get(key).and_then(Value::as_u64)
}

fn size_of(usage: &Map<String, Value>) -> Option<ImageSize> {
    if let (Some(w), Some(h)) = (
        number(usage, "output_width").or_else(|| number(usage, "width")),
        number(usage, "output_height").or_else(|| number(usage, "height")),
    ) {
        return Some(ImageSize::pixels(
            u32::try_from(w).ok()?,
            u32::try_from(h).ok()?,
        ));
    }
    let (w, h) = usage.get("size")?.as_str()?.split_once('*')?;
    Some(ImageSize::pixels(w.parse().ok()?, h.parse().ok()?))
}

// A task that ended without its images, or that is still running when its result is read.
fn failed_task(status: &str, output: &Fields) -> KppError {
    let text = |k: &str| match output.peek(k) {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    };
    match (status, text("code")) {
        ("FAILED", Some(code)) => {
            let mut err = serde_json::Map::new();
            err.insert("code".into(), code.into());
            if let Some(m) = text("message") {
                err.insert("message".into(), m.into());
            }
            super::decode_error(
                400,
                &HeaderList::new(),
                &serde_json::to_vec(&err).unwrap_or_default(),
            )
        }
        ("PENDING" | "RUNNING", _) => KppError::provider(
            "QwenCloud's image task had not finished when Kaeso read it; try again.",
        ),
        _ => KppError::provider(format!(
            "QwenCloud's image task ended as `{status}` without an image."
        )),
    }
}

// Each finished image's link in a task's `results` becomes its index among the reply's
// images; results that failed alone keep their code and message and make no image.
fn task_skeleton(results: &mut Value, images: &mut Vec<GeneratedImage>) {
    let Some(list) = results.as_array_mut() else {
        return;
    };
    for item in list {
        let Some(part) = item.as_object_mut() else {
            continue;
        };
        let Some(Value::String(link)) = part.get("url") else {
            continue;
        };
        let mut image = GeneratedImage::new(Source::Url(link.clone()));
        image.revised_prompt = part
            .get("actual_prompt")
            .and_then(Value::as_str)
            .map(str::to_owned);
        images.push(image);
        part.insert("url".into(), Value::from(images.len() - 1));
    }
}

// Each image link in `choices` becomes its index among the reply's images, so the reply's
// layout comes back around the images it carries.
fn skeleton(choices: &mut Value, images: &mut Vec<GeneratedImage>) {
    let Some(list) = choices.as_array_mut() else {
        return;
    };
    for choice in list {
        let Some(content) = choice
            .pointer_mut("/message/content")
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        let first = images.len();
        let mut revised = None;
        for item in content.iter_mut() {
            let Some(part) = item.as_object_mut() else {
                continue;
            };
            if let Some(Value::String(link)) = part.get("image") {
                images.push(GeneratedImage::new(Source::Url(link.clone())));
                part.insert("image".into(), Value::from(images.len() - 1));
            } else if let Some(Value::String(text)) = part.get("text") {
                revised = Some(text.clone());
            }
        }
        if let Some(text) = revised {
            for image in images.iter_mut().skip(first) {
                image.revised_prompt = Some(text.clone());
            }
        }
    }
}

pub(super) fn decode_response(body: &[u8], ctx: &EgressCtx) -> Result<Reply, KppError> {
    let mut f = provider_object(body, D)?;
    let mut extras = Extras::new(ctx.origin(D));
    let mut output = f.object("output")?.ok_or_else(|| {
        KppError::provider("QwenCloud answered an image request without its `output`.")
    })?;
    if let Some(Value::String(status)) = output.peek("task_status")
        && !matches!(status.as_str(), "SUCCEEDED")
    {
        return Err(failed_task(status, &output));
    }
    let mut images = Vec::new();
    match output.take(RESULTS) {
        Some(mut results) => {
            task_skeleton(&mut results, &mut images);
            extras.form.insert(RESULTS.into(), results);
        }
        None => {
            let mut choices = output.take(CHOICES).unwrap_or(Value::Array(Vec::new()));
            skeleton(&mut choices, &mut images);
            extras.form.insert(CHOICES.into(), choices);
        }
    }
    if images.is_empty() {
        return Err(KppError::provider(
            "QwenCloud answered an image request without an image.",
        ));
    }
    output.rest_into(&mut extras, "output");
    let mut resp = ImageResponse::new(String::new(), images);
    if let Some(Value::Object(usage)) = f.take("usage") {
        let n = |k: &str| number(&usage, k).unwrap_or(0);
        resp.settings.size = size_of(&usage);
        let mut u = Usage::new(n("input_tokens"), n("output_tokens"));
        u.raw = Some(RawUsage::new(ctx.origin(D), Value::Object(usage)));
        resp.usage = u;
    }
    f.rest_into(&mut extras, "");
    resp.extras = extras;
    Ok(Reply::Image(resp))
}

fn link_of(image: &GeneratedImage) -> Value {
    match &image.source {
        Source::Url(u) => u.clone().into(),
        Source::Base64(b) => format!("data:{};base64,{}", b.media_type, b.data).into(),
        _ => Value::Null,
    }
}

fn fill(value: &mut Value, images: &[GeneratedImage]) {
    match value {
        Value::Object(map) => {
            if let Some(Value::Number(n)) = map.get("image")
                && let Some(image) = n
                    .as_u64()
                    .and_then(|i| usize::try_from(i).ok())
                    .and_then(|i| images.get(i))
            {
                map.insert("image".into(), link_of(image));
                return;
            }
            map.values_mut().for_each(|v| fill(v, images));
        }
        Value::Array(list) => list.iter_mut().for_each(|v| fill(v, images)),
        _ => {}
    }
}

pub(super) fn encode_response(r: &ImageResponse) -> Result<Vec<u8>, KppError> {
    let mut output = Map::new();
    if let Some(skeleton) = r.extras.form_value(D, RESULTS) {
        let mut results = skeleton.clone();
        fill_results(&mut results, &r.images);
        output.insert(RESULTS.into(), results);
        return finish_response(r, output);
    }
    let choices = match r.extras.form_value(D, CHOICES) {
        Some(skeleton) => {
            let mut choices = skeleton.clone();
            fill(&mut choices, &r.images);
            choices
        }
        None => {
            let mut content: Vec<Value> = r
                .images
                .iter()
                .map(|i| {
                    let mut part = Map::new();
                    part.insert("image".into(), link_of(i));
                    Value::Object(part)
                })
                .collect();
            if let Some(p) = r.images.iter().find_map(|i| i.revised_prompt.clone()) {
                let mut part = Map::new();
                part.insert("text".into(), p.into());
                content.push(Value::Object(part));
            }
            serde_json::json!([{
                "finish_reason": "stop",
                "message": {"role": "assistant", "content": content},
            }])
        }
    };
    output.insert(CHOICES.into(), choices);
    finish_response(r, output)
}

fn fill_results(results: &mut Value, images: &[GeneratedImage]) {
    for item in results.as_array_mut().into_iter().flatten() {
        if let Some(part) = item.as_object_mut()
            && let Some(Value::Number(n)) = part.get("url")
            && let Some(image) = n
                .as_u64()
                .and_then(|i| usize::try_from(i).ok())
                .and_then(|i| images.get(i))
        {
            part.insert("url".into(), link_of(image));
        }
    }
}

fn finish_response(r: &ImageResponse, mut output: Map<String, Value>) -> Result<Vec<u8>, KppError> {
    put_extras(&mut output, &r.extras, D, "output");
    let mut out = Map::new();
    out.insert("output".into(), Value::Object(output));
    let usage = match &r.usage.raw {
        Some(raw) if raw.origin.is(D) => Some(raw.value.clone()),
        _ => None,
    };
    put(&mut out, "usage", usage);
    put_extras(&mut out, &r.extras, D, "");
    body(&out)
}
