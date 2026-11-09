use kaeso_kpp::{
    Extras, GeneratedImage, ImageBackground, ImageEvent, ImageFormat, ImageOperation, ImageQuality,
    ImageResponse, ImageSettings, ImageSize, KppError, Origin, RawJson, RawUsage, Reply, Request,
    RequestKind, Source, Usage,
};
use serde_json::{Map, Value, json};

use super::DIALECT;
use super::wire::{ImageW, ImagesResponseW, UsageW};
use crate::codec::{EgressCtx, EncodeCtx, HeaderList};
use crate::wire::{Ext, extra_for, extras_from, parse_provider, to_body, verbatim_from_form};

const MEDIA_TYPE_KEYS: [&str; 2] = ["mime_type", "media_type"];

pub(super) fn decode(
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<Reply, KppError> {
    if !(200..300).contains(&status) {
        return Err(super::decode_error(status, headers, body));
    }
    if ctx.kind != RequestKind::Image {
        return Err(KppError::unsupported_by("generation response", DIALECT, ""));
    }
    let origin = ctx.origin(DIALECT);
    let Ext {
        known: w,
        extra,
        verbatim,
    } = parse_provider::<ImagesResponseW>(body, DIALECT)?;
    let settings = settings_of(
        w.size.as_deref(),
        w.quality.as_deref(),
        w.output_format.as_deref(),
        w.background.as_deref(),
    );
    let fallback = settings
        .format
        .as_ref()
        .and_then(ImageFormat::media_type)
        .unwrap_or("image/png");
    let images = w
        .data
        .iter()
        .map(|item| image_of(item, fallback, &origin))
        .collect::<Result<Vec<_>, _>>()?;
    let mut resp = ImageResponse::new(String::new(), images);
    resp.created = w.created;
    resp.settings = settings;
    if let Some(raw) = &w.usage {
        resp.usage = usage_of(raw, &origin);
        resp.settings.resolution = billed_tier(raw);
    }
    let mut extras = extras_from(&origin, extra, verbatim);
    if let Some(Value::String(model)) = extras.fields.get("model").and_then(|m| m.to_value().ok()) {
        resp.model = model;
    }
    if w.usage.is_none() {
        extras.form.insert("no_usage".to_owned(), true.into());
    }
    resp.extras = extras;
    Ok(Reply::Image(resp))
}

pub(super) fn settings_of(
    size: Option<&str>,
    quality: Option<&str>,
    format: Option<&str>,
    background: Option<&str>,
) -> ImageSettings {
    let mut s = ImageSettings::default();
    s.size = size.map(ImageSize::from_wire);
    s.quality = quality.map(ImageQuality::from_wire);
    s.format = format.map(ImageFormat::from_wire);
    s.background = background.map(ImageBackground::from_wire);
    s
}

fn bad(detail: &str) -> KppError {
    KppError::provider(format!(
        "The provider sent a response that is not a valid {} response: {detail}.",
        DIALECT.display_name()
    ))
}

fn image_of(
    item: &Ext<ImageW>,
    fallback: &str,
    origin: &Origin,
) -> Result<GeneratedImage, KppError> {
    let media_type = MEDIA_TYPE_KEYS
        .iter()
        .find_map(|k| item.extra.get(*k)?.parse::<String>().ok())
        .unwrap_or_else(|| fallback.to_owned());
    let source = match (&item.b64_json, &item.url) {
        (Some(data), _) => Source::base64(media_type, data)
            .map_err(|_| bad("an image's `b64_json` is not valid base64"))?,
        (None, Some(url)) => Source::Url(url.clone()),
        (None, None) => return Err(bad("an image has neither `b64_json` nor `url`")),
    };
    let mut image = GeneratedImage::new(source);
    image.revised_prompt = item.revised_prompt.clone();
    let mut extras = extras_from(origin, item.extra.clone(), item.verbatim.clone());
    if item.b64_json.is_some()
        && let Some(url) = &item.url
        && let Ok(raw) = RawJson::from_serialize(url)
    {
        extras.fields.insert("url".to_owned(), raw);
    }
    image.extras = extras;
    Ok(image)
}

/// The tier QwenCloud's OpenAI-compatible image mode reports it billed, such as
/// `"output_image_type": "qima_output_2k"`
/// (<https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image-openai>).
fn billed_tier(raw: &RawJson) -> Option<String> {
    let usage = raw.to_value().ok()?;
    let kind = usage.get("output_image_type")?.as_str()?;
    let tier = kind.rsplit('_').next()?.to_ascii_uppercase();
    let digits = tier.strip_suffix('K')?;
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())).then_some(tier)
}

pub(super) fn usage_of(raw: &RawJson, origin: &Origin) -> Usage {
    let w: UsageW = raw.parse().unwrap_or_default();
    let cached = w
        .input_tokens_details
        .as_ref()
        .and_then(|d| d.cached_tokens)
        .unwrap_or(0);
    let input = w.input_tokens.or(w.prompt_tokens).unwrap_or(0);
    let mut u = Usage::new(
        input.saturating_sub(cached),
        w.output_tokens.or(w.completion_tokens).unwrap_or(0),
    );
    u.cache_read_tokens = cached;
    u.input_image_tokens = w.input_tokens_details.as_ref().and_then(|d| d.image_tokens);
    u.output_image_tokens = w
        .output_tokens_details
        .as_ref()
        .or(w.completion_tokens_details.as_ref())
        .and_then(|d| d.image_tokens);
    if let Ok(value) = raw.to_value() {
        u.raw = Some(RawUsage::new(origin.clone(), value));
    }
    u
}

pub(super) fn wire_usage(u: &Usage) -> Option<Value> {
    if let Some(raw) = u.raw_for(DIALECT) {
        return Some(raw.clone());
    }
    let input = u.total_input_tokens();
    if input == 0 && u.output_tokens == 0 {
        return None;
    }
    let image = u
        .input_image_tokens
        .unwrap_or(0)
        .saturating_add(u.cache_read_image_tokens.unwrap_or(0));
    let mut usage = json!({
        "input_tokens": input,
        "output_tokens": u.output_tokens,
        "total_tokens": input.saturating_add(u.output_tokens),
        "input_tokens_details": {
            "text_tokens": input.saturating_sub(image),
            "image_tokens": image,
        },
    });
    if let (Some(image_out), Value::Object(map)) = (u.output_image_tokens, &mut usage) {
        map.insert(
            "output_tokens_details".to_owned(),
            json!({
                "image_tokens": image_out,
                "text_tokens": u.output_tokens.saturating_sub(image_out),
            }),
        );
    }
    Some(usage)
}

fn operation(ctx: &EncodeCtx) -> ImageOperation {
    match ctx.request.as_deref() {
        Some(Request::Image(r)) => r.operation,
        _ => ImageOperation::Generate,
    }
}

pub(super) fn encode(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
    let Reply::Image(r) = reply else {
        return Err(KppError::unsupported_by(
            format!("{:?} response", reply.kind()),
            DIALECT,
            "",
        ));
    };
    if !ctx.stream.is_off() {
        return Ok(as_stream(r, ctx));
    }
    let mut out = Map::new();
    out.insert(
        "created".to_owned(),
        r.created.or(ctx.created_at).unwrap_or(0).into(),
    );
    let data: Vec<Value> = r.images.iter().map(wire_image).collect();
    out.insert("data".to_owned(), data.into());
    let s = &r.settings;
    let pairs = [
        (
            "background",
            s.background.as_ref().map(ImageBackground::as_wire),
        ),
        ("output_format", s.format.as_ref().map(ImageFormat::as_wire)),
        ("size", s.size.as_ref().map(ImageSize::as_wire)),
        ("quality", s.quality.as_ref().map(ImageQuality::as_wire)),
    ];
    for (k, v) in pairs {
        if let Some(v) = v {
            out.insert(k.to_owned(), v.into());
        }
    }
    if !r.extras.form_flag(DIALECT, "no_usage")
        && let Some(usage) = wire_usage(&r.usage)
    {
        out.insert("usage".to_owned(), usage);
    }
    write_extras(&mut out, &r.extras);
    to_body(&out)
}

fn write_extras(out: &mut Map<String, Value>, extras: &Extras) {
    for (k, v) in extra_for(extras, DIALECT) {
        if let Ok(value) = v.to_value() {
            out.entry(k).or_insert(value);
        }
    }
    for (k, v) in verbatim_from_form(extras, DIALECT, "") {
        if let Ok(value) = v.to_value() {
            out.insert(k, value);
        }
    }
}

fn wire_image(image: &GeneratedImage) -> Value {
    let mut item = Map::new();
    match &image.source {
        Source::Base64(b) => {
            item.insert("b64_json".to_owned(), b.to_base64().into());
        }
        Source::Url(u) => {
            item.insert("url".to_owned(), u.clone().into());
        }
        _ => {}
    }
    if let Some(p) = &image.revised_prompt {
        item.insert("revised_prompt".to_owned(), p.clone().into());
    }
    write_extras(&mut item, &image.extras);
    Value::Object(item)
}

/// The events of a reply that was asked for as a stream from a provider that answers whole:
/// one finished image each, as OpenAI sends with `partial_images` 0.
pub(super) fn as_stream(r: &ImageResponse, ctx: &EncodeCtx) -> Vec<u8> {
    let mut enc = super::stream::Encoder::new(operation(ctx), ctx.created_at);
    let mut out = Vec::new();
    let last = r.images.len().saturating_sub(1);
    for (i, image) in r.images.iter().enumerate() {
        let mut event = ImageEvent::finished(image.clone(), (i == last).then(|| r.usage.clone()));
        event.created = r.created;
        event.settings = r.settings.clone();
        out.extend(enc.event_bytes(&event));
    }
    out
}
