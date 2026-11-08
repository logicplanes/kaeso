use base64::Engine as _;
use indexmap::IndexMap;
use kaeso_kpp::{
    Extras, ImageBackground, ImageDelivery, ImageFormat, ImageInput, ImageOperation, ImageQuality,
    ImageRequest, ImageSize, KppError, Origin, RawJson, Request, Source, StreamMode,
};
use serde_json::{Map, Value, json};

use super::DIALECT;
use super::profile::{DeliveryForm, EditsForm, ImageField, ImagesProfile, SizeForm, UserField};
use super::wire::{ImageRefW, ImageRequestW, REQUEST_FIELDS};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList, is_forwarded_header};
use crate::cross::check_extras;
use crate::multipart::{FormError, FormField, FormLimits, FormPart, form_boundary, read_form};
use crate::wire::{Ext, extra_for, extras_from, parse_data_url, to_body, verbatim_from_form};

/// The most parts one edit upload may hold: OpenAI's 16 images, a mask, a markup image and
/// every field of `CreateImageEditRequest`, with room for fields Kaeso keeps as extras.
pub const MAX_FORM_PARTS: usize = 64;

/// The most bytes one uploaded image may hold: OpenAI's limit of 50 MB per image
/// (`CreateImageEditRequest.image` at openai-openapi 926a7b8).
pub const MAX_IMAGE_BYTES: usize = 50 * 1024 * 1024;

const FORM_BODY: &str = "body";
const FORM_USER: &str = "user";
const FORM_IMAGE_FIELD: &str = "image_field";
const FORM_STREAM_OFF: &str = "stream_off";
const MULTIPART: &str = "multipart";
const JSON: &str = "json";
const MARKUP: &str = "markup";
const INT_EXTRAS: &[&str] = &["frame_count", "frame_duration_ms"];

fn invalid(detail: impl std::fmt::Display) -> KppError {
    KppError::invalid_request(format!(
        "The request body is not a valid {} request: {detail}.",
        DIALECT.display_name()
    ))
}

fn refused(feature: &str, profile: &ImagesProfile, param: &str, hint: &str) -> KppError {
    KppError::unsupported_by(
        feature.to_owned(),
        DIALECT,
        format!("{} does not take {feature}; {hint}", profile.provider),
    )
    .with_param(param.to_owned())
}

/// Decodes an Images API request for `operation`: JSON for generations and JSON edits,
/// `multipart/form-data` for uploads.
pub(super) fn decode(
    operation: ImageOperation,
    body: &[u8],
    headers: &HeaderList,
) -> Result<Request, KppError> {
    let multipart = headers.get("content-type").is_some_and(|t| {
        t.trim_start()
            .to_ascii_lowercase()
            .starts_with("multipart/")
    });
    let mut req = if multipart {
        decode_form(
            operation,
            body,
            headers.get("content-type").unwrap_or_default(),
        )?
    } else {
        if operation == ImageOperation::Variation {
            return Err(invalid(
                "a variation request is a `multipart/form-data` upload",
            ));
        }
        decode_json(operation, body)?
    };
    for (name, value) in headers.iter() {
        if is_forwarded_header(DIALECT, name) {
            req.extras.insert_header(name, value);
        }
    }
    Ok(Request::Image(req))
}

/// The operation a body without a path asks for: an upload or a JSON body with `images` is an
/// edit, anything else a generation.
pub(super) fn guess_operation(body: &[u8], headers: &HeaderList) -> ImageOperation {
    let multipart = headers.get("content-type").is_some_and(|t| {
        t.trim_start()
            .to_ascii_lowercase()
            .starts_with("multipart/")
    });
    let has_images =
        serde_json::from_slice::<Map<String, Value>>(body).is_ok_and(|m| m.contains_key("images"));
    if multipart || has_images {
        ImageOperation::Edit
    } else {
        ImageOperation::Generate
    }
}

fn decode_json(operation: ImageOperation, body: &[u8]) -> Result<ImageRequest, KppError> {
    let origin = Origin::new(DIALECT);
    let Ext {
        known: w,
        extra,
        verbatim,
    } = Ext::<ImageRequestW>::from_slice(body).map_err(|e| {
        let text = crate::wire::without_position(&e);
        let err = invalid(&text);
        match field_in_error(body, &text) {
            Some(f) => err.with_param(f),
            None => err,
        }
    })?;
    let mut req = ImageRequest::new(w.model.clone().unwrap_or_default(), operation);
    let mut extras = extras_from(&origin, extra, verbatim);
    if operation == ImageOperation::Edit {
        extras.form.insert(FORM_BODY.to_owned(), JSON.into());
    }
    if operation == ImageOperation::Generate && (w.images.is_some() || w.mask.is_some()) {
        return Err(invalid("a generation request takes no `images`").with_param("images"));
    }
    for (i, r) in w.images.iter().flatten().enumerate() {
        req.images
            .push(image_ref(r).map_err(|e| e.with_param(format!("images[{i}]")))?);
    }
    req.mask = w
        .mask
        .as_ref()
        .map(image_ref)
        .transpose()
        .map_err(|e| e.with_param("mask"))?;
    apply_common(&mut req, &mut extras, Common::from_wire(&w))?;
    req.extras = extras;
    Ok(req)
}

// serde names a missing or unknown field but not one of the wrong type, so each typed
// field is tried on its own.
fn field_in_error(body: &[u8], text: &str) -> Option<&'static str> {
    if let Some(f) = REQUEST_FIELDS
        .iter()
        .find(|f| text.contains(&format!("`{f}`")))
    {
        return Some(f);
    }
    let map: Map<String, Value> = serde_json::from_slice(body).ok()?;
    REQUEST_FIELDS.iter().copied().find(|f| {
        map.get(*f)
            .is_some_and(|v| serde_json::from_value::<ImageRequestW>(json!({ *f: v })).is_err())
    })
}

fn image_ref(r: &Ext<ImageRefW>) -> Result<ImageInput, KppError> {
    if !r.extra.is_empty() || !r.verbatim.is_empty() {
        return Err(invalid(
            "an image reference takes only `image_url` or `file_id`",
        ));
    }
    let source = match (&r.image_url, &r.file_id) {
        (Some(url), None) => url_source(url),
        (None, Some(id)) => Source::file_id(DIALECT, id.clone()),
        _ => {
            return Err(invalid(
                "an image reference needs exactly one of `image_url` and `file_id`",
            ));
        }
    };
    Ok(ImageInput::new(source))
}

fn url_source(url: &str) -> Source {
    match parse_data_url(url) {
        Some(d) if d.header.is_none() => Source::base64(d.media_type, d.data),
        _ => Source::Url(url.to_owned()),
    }
}

struct Common {
    prompt: Option<String>,
    n: Option<u32>,
    size: Option<String>,
    quality: Option<String>,
    response_format: Option<String>,
    output_format: Option<String>,
    output_compression: Option<u8>,
    background: Option<String>,
    stream: Option<bool>,
    partial_images: Option<u32>,
    safety_identifier: Option<String>,
    user: Option<String>,
    aspect_ratio: Option<String>,
    resolution: Option<String>,
    seed: Option<i64>,
    negative_prompt: Option<String>,
}

impl Common {
    fn from_wire(w: &ImageRequestW) -> Common {
        Common {
            prompt: w.prompt.clone(),
            n: w.n,
            size: w.size.clone(),
            quality: w.quality.clone(),
            response_format: w.response_format.clone(),
            output_format: w.output_format.clone(),
            output_compression: w.output_compression,
            background: w.background.clone(),
            stream: w.stream,
            partial_images: w.partial_images,
            safety_identifier: w.safety_identifier.clone(),
            user: w.user.clone(),
            aspect_ratio: w.aspect_ratio.clone(),
            resolution: w.resolution.clone(),
            seed: w.seed,
            negative_prompt: w.negative_prompt.clone(),
        }
    }
}

fn apply_common(req: &mut ImageRequest, extras: &mut Extras, c: Common) -> Result<(), KppError> {
    req.prompt = c.prompt;
    req.count = c.n;
    req.size = c.size.as_deref().map(ImageSize::from_wire);
    req.quality = c.quality.as_deref().map(ImageQuality::from_wire);
    req.format = c.output_format.as_deref().map(ImageFormat::from_wire);
    req.compression = c.output_compression;
    req.background = c.background.as_deref().map(ImageBackground::from_wire);
    req.delivery = match c.response_format.as_deref() {
        None => None,
        Some("url") => Some(ImageDelivery::Url),
        Some("b64_json") => Some(ImageDelivery::Base64),
        Some(other) => {
            return Err(KppError::invalid_request(format!(
                "`response_format` must be `url` or `b64_json`, not `{other}`."
            ))
            .with_param("response_format"));
        }
    };
    match c.stream {
        Some(true) => req.stream = StreamMode::on(true),
        Some(false) => {
            extras.form.insert(FORM_STREAM_OFF.to_owned(), true.into());
        }
        None => {}
    }
    req.partial_images = c.partial_images;
    req.aspect_ratio = c.aspect_ratio;
    req.resolution = c.resolution;
    req.seed = c.seed;
    req.negative_prompt = c.negative_prompt;
    match (c.safety_identifier, c.user) {
        (Some(id), user) => {
            req.user = Some(id);
            if let Some(user) = user {
                let raw = RawJson::from_serialize(&user)
                    .map_err(|_| invalid("`user` is not a string"))?;
                extras.fields.insert("user".to_owned(), raw);
            }
        }
        (None, Some(user)) => {
            req.user = Some(user);
            extras.form.insert(FORM_USER.to_owned(), "user".into());
        }
        (None, None) => {}
    }
    Ok(())
}

fn form_error(e: FormError) -> KppError {
    match e {
        FormError::PartTooLarge { name, limit } => {
            KppError::invalid_request(format!("The upload `{name}` is larger than {limit} bytes."))
                .with_status(413)
                .with_param(name)
        }
        FormError::TooManyParts(n) => {
            KppError::invalid_request(format!("The upload has more than {n} parts."))
        }
        other => invalid(other),
    }
}

fn decode_form(
    operation: ImageOperation,
    body: &[u8],
    content_type: &str,
) -> Result<ImageRequest, KppError> {
    let boundary = form_boundary(content_type).map_err(form_error)?;
    let parts = read_form(
        body,
        &boundary,
        FormLimits::new(MAX_FORM_PARTS, MAX_IMAGE_BYTES),
    )
    .map_err(form_error)?;
    let origin = Origin::new(DIALECT);
    let mut extras = Extras::new(origin);
    extras.form.insert(FORM_BODY.to_owned(), MULTIPART.into());
    let mut text: IndexMap<String, String> = IndexMap::new();
    let mut req = ImageRequest::new(String::new(), operation);
    let mut nested = Map::new();
    for part in &parts {
        let name = part.name.as_str();
        match name {
            "image" | "image[]" => {
                if part.filename.is_none() {
                    return Err(invalid(format!("`{name}` must be a file")).with_param("image"));
                }
                if !extras.form.contains_key(FORM_IMAGE_FIELD) {
                    extras.form.insert(FORM_IMAGE_FIELD.to_owned(), name.into());
                }
                req.images.push(uploaded(part));
            }
            "mask" if part.filename.is_some() => req.mask = Some(uploaded(part)),
            MARKUP if part.filename.is_some() => {
                let url = data_url(&uploaded(part).source).unwrap_or_default();
                let raw = RawJson::from_serialize(&json!({ "image_url": url }))
                    .map_err(|_| invalid("`markup` cannot be read"))?;
                extras.fields.insert(MARKUP.to_owned(), raw);
            }
            _ if part.filename.is_some() => {
                return Err(KppError::invalid_request(format!(
                    "Kaeso does not know the file field `{name}`; send images as `image[]`."
                ))
                .with_param(name.to_owned()));
            }
            _ => {
                let value = part.text().ok_or_else(|| {
                    invalid(format!("`{name}` is not UTF-8 text")).with_param(name)
                })?;
                if REQUEST_FIELDS.contains(&name) {
                    if text.insert(name.to_owned(), value.to_owned()).is_some() {
                        return Err(
                            invalid(format!("`{name}` is sent more than once")).with_param(name)
                        );
                    }
                } else {
                    insert_nested(&mut nested, name, value)?;
                }
            }
        }
    }
    let number = |key: &str| -> Result<Option<u64>, KppError> {
        text.get(key)
            .map(|v| {
                v.trim().parse::<u64>().map_err(|_| {
                    KppError::invalid_request(format!("`{key}` must be a whole number, not `{v}`."))
                        .with_param(key.to_owned())
                })
            })
            .transpose()
    };
    let small = |key: &str, n: Option<u64>| -> Result<Option<u32>, KppError> {
        n.map(|n| {
            u32::try_from(n).map_err(|_| {
                KppError::invalid_request(format!("`{key}` is too large."))
                    .with_param(key.to_owned())
            })
        })
        .transpose()
    };
    let stream = match text.get("stream").map(|s| s.trim()) {
        None => None,
        Some("true") => Some(true),
        Some("false") => Some(false),
        Some(other) => {
            return Err(KppError::invalid_request(format!(
                "`stream` must be `true` or `false`, not `{other}`."
            ))
            .with_param("stream"));
        }
    };
    let seed = match text.get("seed") {
        None => None,
        Some(v) => Some(v.trim().parse::<i64>().map_err(|_| {
            KppError::invalid_request(format!("`seed` must be a whole number, not `{v}`."))
                .with_param("seed")
        })?),
    };
    let compression = small("output_compression", number("output_compression")?)?
        .map(|c| {
            u8::try_from(c).map_err(|_| {
                KppError::invalid_request("`output_compression` must be between 0 and 100.")
                    .with_param("output_compression")
            })
        })
        .transpose()?;
    let common = Common {
        prompt: text.get("prompt").cloned(),
        n: small("n", number("n")?)?,
        size: text.get("size").cloned(),
        quality: text.get("quality").cloned(),
        response_format: text.get("response_format").cloned(),
        output_format: text.get("output_format").cloned(),
        output_compression: compression,
        background: text.get("background").cloned(),
        stream,
        partial_images: small("partial_images", number("partial_images")?)?,
        safety_identifier: text.get("safety_identifier").cloned(),
        user: text.get("user").cloned(),
        aspect_ratio: text.get("aspect_ratio").cloned(),
        resolution: text.get("resolution").cloned(),
        seed,
        negative_prompt: text.get("negative_prompt").cloned(),
    };
    req.model = text.get("model").cloned().unwrap_or_default();
    for (key, value) in nested {
        let raw = RawJson::from_serialize(&value).map_err(|_| invalid("a field cannot be read"))?;
        extras.fields.insert(key, raw);
    }
    apply_common(&mut req, &mut extras, common)?;
    req.extras = extras;
    Ok(req)
}

fn uploaded(part: &FormPart<'_>) -> ImageInput {
    let media_type = part
        .content_type
        .clone()
        .unwrap_or_else(|| "application/octet-stream".to_owned());
    let data = base64::engine::general_purpose::STANDARD.encode(part.body);
    let mut input = ImageInput::new(Source::base64(media_type, data));
    input.filename = part.filename.clone();
    input
}

// Multipart names nest with brackets, as OpenAI's SDKs write `svg[model]` and
// `svg[reasoning][effort]`; JSON bodies carry the same fields as objects.
fn insert_nested(out: &mut Map<String, Value>, name: &str, value: &str) -> Result<(), KppError> {
    let (head, rest) = match name.find('[') {
        Some(i) => name.split_at(i),
        None => (name, ""),
    };
    let mut keys = Vec::new();
    let mut tail = rest;
    while let Some(after) = tail.strip_prefix('[') {
        let end = after
            .find(']')
            .ok_or_else(|| invalid(format!("the field name `{name}` is not closed")))?;
        let (key, more) = after.split_at(end);
        keys.push(key);
        tail = more.get(1..).unwrap_or_default();
    }
    if !tail.is_empty() || head.is_empty() {
        return Err(invalid(format!("the field name `{name}` cannot be read")));
    }
    let leaf = if keys.is_empty() && INT_EXTRAS.contains(&head) {
        value
            .trim()
            .parse::<u64>()
            .map(Value::from)
            .unwrap_or_else(|_| Value::String(value.to_owned()))
    } else {
        Value::String(value.to_owned())
    };
    let mut slot = out.entry(head.to_owned()).or_insert(Value::Null);
    for key in keys {
        if key.is_empty() {
            if !slot.is_array() {
                *slot = Value::Array(Vec::new());
            }
            let Value::Array(items) = slot else {
                return Err(invalid(format!("the field name `{name}` cannot be read")));
            };
            items.push(Value::Null);
            slot = items
                .last_mut()
                .ok_or_else(|| invalid(format!("the field name `{name}` cannot be read")))?;
        } else {
            if !slot.is_object() {
                *slot = Value::Object(Map::new());
            }
            let Value::Object(map) = slot else {
                return Err(invalid(format!("the field name `{name}` cannot be read")));
            };
            slot = map.entry(key.to_owned()).or_insert(Value::Null);
        }
    }
    *slot = leaf;
    Ok(())
}

pub(super) fn data_url(source: &Source) -> Option<String> {
    match source {
        Source::Base64(b) => Some(format!("data:{};base64,{}", b.media_type, b.data)),
        Source::Url(u) => Some(u.clone()),
        _ => None,
    }
}

/// Checks that the provider `ctx` describes can take `req`, before anything is sent.
pub(super) fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
    let Request::Image(r) = req else {
        return Err(KppError::unsupported_by(
            format!("{:?} request", req.kind()),
            DIALECT,
            "Send image requests to `/v1/images/generations` or `/v1/images/edits`.",
        ));
    };
    plan(r, &ctx.images_profile).map(|_| ())
}

struct Plan {
    path: &'static str,
    multipart: bool,
    streams: bool,
}

fn plan(r: &ImageRequest, p: &ImagesProfile) -> Result<Plan, KppError> {
    check_extras(&r.extras, DIALECT, "", "request")?;
    if r.extras.is_from(DIALECT) {
        for (key, _) in r.extras.body_fields() {
            let top = key.split(['.', '[']).next().unwrap_or(key);
            if p.refuses(top) {
                return Err(refused(&format!("`{top}`"), p, top, "remove it."));
            }
        }
    }
    let (path, multipart) = match r.operation {
        ImageOperation::Variation => {
            return Err(KppError::unsupported_by(
                "image variations",
                DIALECT,
                format!(
                    "{} no longer makes image variations; send the image to `/v1/images/edits` with a prompt.",
                    p.provider
                ),
            )
            .with_param("image"));
        }
        ImageOperation::Edit => (
            p.edits_path().ok_or_else(|| {
                KppError::unsupported_by(
                    "image edits",
                    DIALECT,
                    format!("{} only generates images from a prompt; send it to `/v1/images/generations`.", p.provider),
                )
                .with_param("image")
            })?,
            edits_multipart(r, p)?,
        ),
        _ => (p.generations_path, false),
    };
    if r.mask.is_some() && !p.mask {
        return Err(refused(
            "a mask",
            p,
            "mask",
            "describe the area to change in the prompt instead.",
        ));
    }
    let max_inputs = match p.edits {
        EditsForm::UrlObjects { max, .. } | EditsForm::OnGenerations { max, .. } => Some(max),
        _ => None,
    };
    if let Some(max) = max_inputs
        && r.images.len() > max as usize
    {
        return Err(refused(
            &format!("more than {max} images"),
            p,
            "image",
            "send fewer images.",
        ));
    }
    if let EditsForm::OnGenerations {
        data_urls: false, ..
    } = p.edits
        && r.images.iter().any(|i| !matches!(i.source, Source::Url(_)))
    {
        return Err(refused(
            "uploaded images",
            p,
            "image",
            "send the image as a link in `images[].image_url`.",
        ));
    }
    if let Some(n) = r.count {
        if !p.count && n > 1 {
            return Err(refused(
                "`n` above 1",
                p,
                "n",
                "send one request per image.",
            ));
        }
        if let Some(max) = p.max_count
            && n > max
        {
            return Err(refused(
                &format!("`n` above {max}"),
                p,
                "n",
                "ask for fewer images.",
            ));
        }
    }
    if let Some(size) = &r.size {
        match p.sizes {
            SizeForm::Size => {}
            SizeForm::WidthHeight if *size == ImageSize::Auto || size.dimensions().is_some() => {}
            SizeForm::WidthHeight => {
                return Err(refused(
                    &format!("the size `{}`", size.as_wire()),
                    p,
                    "size",
                    "send `WIDTHxHEIGHT` in pixels.",
                ));
            }
            SizeForm::AspectRatio if *size == ImageSize::Auto => {}
            SizeForm::AspectRatio => {
                return Err(refused(
                    "pixel sizes",
                    p,
                    "size",
                    "send `aspect_ratio` and `resolution` instead.",
                ));
            }
        }
    }
    for (field, set) in [
        ("aspect_ratio", r.aspect_ratio.is_some()),
        ("resolution", r.resolution.is_some()),
        ("seed", r.seed.is_some()),
        ("negative_prompt", r.negative_prompt.is_some()),
    ] {
        if set && !p.extends(field) {
            return Err(refused(&format!("`{field}`"), p, field, "remove it."));
        }
    }
    if let Some(q) = &r.quality
        && !p.takes_quality(q.as_wire())
    {
        return Err(refused(
            &format!("the quality `{}`", q.as_wire()),
            p,
            "quality",
            "remove it or pick another value.",
        ));
    }
    if let Some(f) = &r.format
        && !p.takes_format(f.as_wire())
    {
        return Err(refused(
            &format!("the output format `{}`", f.as_wire()),
            p,
            "output_format",
            "remove it or pick another format.",
        ));
    }
    if r.compression.is_some() && p.refuses("output_compression") {
        return Err(refused(
            "`output_compression`",
            p,
            "output_compression",
            "remove it.",
        ));
    }
    if r.background.is_some() && p.refuses("background") {
        return Err(refused("`background`", p, "background", "remove it."));
    }
    match (r.delivery, p.delivery, p.links_only) {
        (Some(ImageDelivery::Base64), DeliveryForm::Fixed, true) => {
            return Err(refused(
                "`b64_json`",
                p,
                "response_format",
                "it answers with links only; remove `response_format`.",
            ));
        }
        (Some(ImageDelivery::Url), DeliveryForm::Fixed, false) => {
            return Err(refused(
                "`url`",
                p,
                "response_format",
                "it answers with the images' bytes only; remove `response_format`.",
            ));
        }
        _ => {}
    }
    let wants_stream = !r.stream.is_off();
    let streams = wants_stream
        && match r.operation {
            ImageOperation::Edit => p.edits_stream,
            _ => p.streams,
        };
    if !streams && r.partial_images.is_some_and(|n| n > 0) {
        return Err(refused(
            "partial images",
            p,
            "partial_images",
            "set `partial_images` to 0 to get the finished image as one event.",
        ));
    }
    Ok(Plan {
        path,
        multipart,
        streams,
    })
}

fn edits_multipart(r: &ImageRequest, p: &ImagesProfile) -> Result<bool, KppError> {
    let EditsForm::Endpoint { file_ids, .. } = p.edits else {
        return Ok(false);
    };
    let inputs = r.images.iter().chain(r.mask.as_ref());
    let mut linked = false;
    for input in inputs {
        match &input.source {
            Source::Base64(_) => {}
            Source::Url(_) => linked = true,
            Source::FileId(f) if f.origin.is(DIALECT) && file_ids => linked = true,
            Source::FileId(_) => {
                return Err(refused(
                    "file ids",
                    p,
                    "image",
                    "upload the image or send it as a link.",
                ));
            }
            _ => {
                return Err(
                    invalid("an image must be bytes, a link or a file id").with_param("image")
                );
            }
        }
    }
    let json = r.extras.form_str(DIALECT, FORM_BODY) == Some(JSON);
    Ok(!linked && !json)
}

/// Encodes `req` for the provider `ctx` describes; a stream it can't send is asked for whole,
/// and `ctx.stream` turns off so that the reply is read as one.
pub(super) fn encode(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    let Request::Image(r) = req else {
        return Err(KppError::unsupported_by("generation request", DIALECT, ""));
    };
    let p = ctx.images_profile;
    let plan = plan(r, &p)?;
    if !plan.streams {
        ctx.stream = StreamMode::Off;
    }
    if r.user.is_some() && p.user == UserField::None {
        ctx.left_out.push("user".to_owned());
    }
    let mut headers = HeaderList::new();
    if r.extras.is_from(DIALECT) {
        for (name, value) in r.extras.headers() {
            if is_forwarded_header(DIALECT, name) {
                headers.push(name, value);
            }
        }
    }
    let path = match &ctx.base_path {
        Some(base) => format!("{base}{}", plan.path),
        None => plan.path.to_owned(),
    };
    let body = json_body(r, &p, plan.streams)?;
    if plan.multipart {
        let (content_type, bytes) = form_body(r, body)?;
        let mut headers = headers;
        headers.push("content-type", content_type);
        return Ok(EncodedRequest::post_json(path, headers, bytes));
    }
    headers.push("content-type", "application/json");
    let mut w = Map::new();
    for (k, v) in body {
        w.insert(k, v);
    }
    Ok(EncodedRequest::post_json(path, headers, to_body(&w)?))
}

fn put(out: &mut IndexMap<String, Value>, key: &str, value: impl Into<Value>) {
    out.insert(key.to_owned(), value.into());
}

fn json_body(
    r: &ImageRequest,
    p: &ImagesProfile,
    streams: bool,
) -> Result<IndexMap<String, Value>, KppError> {
    let mut out = IndexMap::new();
    if !r.model.is_empty() {
        put(&mut out, "model", r.model.clone());
    }
    if let Some(prompt) = &r.prompt {
        put(&mut out, "prompt", prompt.clone());
    }
    if r.operation == ImageOperation::Edit {
        edit_inputs(&mut out, r, p)?;
    }
    if let Some(n) = r.count
        && p.count
    {
        put(&mut out, "n", n);
    }
    if let Some(size) = &r.size {
        match p.sizes {
            SizeForm::Size => put(&mut out, "size", size.as_wire()),
            SizeForm::WidthHeight => {
                if let Some((w, h)) = size.dimensions() {
                    put(&mut out, "width", w);
                    put(&mut out, "height", h);
                }
            }
            SizeForm::AspectRatio => {}
        }
    }
    if let Some(q) = &r.quality {
        put(&mut out, "quality", q.as_wire());
    }
    if let Some(d) = r.delivery {
        let word = match (d, p.delivery) {
            (_, DeliveryForm::Fixed) => None,
            (ImageDelivery::Url, _) => Some("url"),
            (ImageDelivery::Base64, DeliveryForm::Base64Word) => Some("base64"),
            (ImageDelivery::Base64, _) => Some("b64_json"),
            _ => None,
        };
        if let Some(word) = word {
            put(&mut out, "response_format", word);
        }
    }
    if let Some(f) = &r.format {
        put(&mut out, "output_format", f.as_wire());
    }
    if let Some(c) = r.compression {
        put(&mut out, "output_compression", c);
    }
    if let Some(b) = &r.background {
        put(&mut out, "background", b.as_wire());
    }
    if streams {
        put(&mut out, "stream", true);
        if let Some(n) = r.partial_images {
            put(&mut out, "partial_images", n);
        }
    } else if r.extras.form_flag(DIALECT, FORM_STREAM_OFF) {
        put(&mut out, "stream", false);
    }
    if let Some(a) = &r.aspect_ratio {
        put(&mut out, "aspect_ratio", a.clone());
    }
    if let Some(res) = &r.resolution {
        put(&mut out, "resolution", res.clone());
    }
    if let Some(seed) = r.seed {
        put(&mut out, "seed", seed);
    }
    if let Some(neg) = &r.negative_prompt {
        put(&mut out, "negative_prompt", neg.clone());
    }
    if let Some(user) = &r.user {
        let field = match p.user {
            UserField::SafetyIdentifier
                if r.extras.form_str(DIALECT, FORM_USER) == Some("user") =>
            {
                Some("user")
            }
            UserField::SafetyIdentifier => Some("safety_identifier"),
            UserField::User => Some("user"),
            UserField::UserId => Some("user_id"),
            _ => None,
        };
        if let Some(field) = field {
            put(&mut out, field, user.clone());
        }
    }
    for (k, v) in extra_for(&r.extras, DIALECT) {
        let value = v
            .to_value()
            .map_err(|_| invalid(format!("`{k}` cannot be written")))?;
        out.entry(k).or_insert(value);
    }
    for (k, v) in verbatim_from_form(&r.extras, DIALECT, "") {
        if let Ok(value) = v.to_value() {
            out.insert(k, value);
        }
    }
    Ok(out)
}

fn edit_inputs(
    out: &mut IndexMap<String, Value>,
    r: &ImageRequest,
    p: &ImagesProfile,
) -> Result<(), KppError> {
    let reference = |input: &ImageInput| -> Result<Value, KppError> {
        Ok(match &input.source {
            Source::FileId(f) => json!({ "file_id": f.id }),
            other => {
                json!({ "image_url": data_url(other).ok_or_else(|| invalid("an image must be bytes, a link or a file id"))? })
            }
        })
    };
    let url = |input: &ImageInput| -> Result<String, KppError> {
        data_url(&input.source).ok_or_else(|| {
            refused(
                "file ids",
                p,
                "image",
                "upload the image or send it as a link.",
            )
        })
    };
    match p.edits {
        EditsForm::Endpoint { .. } => {
            let images = r
                .images
                .iter()
                .map(reference)
                .collect::<Result<Vec<_>, _>>()?;
            put(out, "images", images);
            if let Some(mask) = &r.mask {
                put(out, "mask", reference(mask)?);
            }
        }
        EditsForm::UrlObjects { .. } => {
            let objects = r
                .images
                .iter()
                .map(|i| Ok(json!({ "type": "image_url", "url": url(i)? })))
                .collect::<Result<Vec<_>, KppError>>()?;
            match <[Value; 1]>::try_from(objects) {
                Ok([one]) => put(out, "image", one),
                Err(many) => put(out, "images", many),
            }
        }
        EditsForm::OnGenerations { field, .. } => {
            let urls = r.images.iter().map(url).collect::<Result<Vec<_>, _>>()?;
            match field {
                ImageField::Image => match <[String; 1]>::try_from(urls) {
                    Ok([one]) => put(out, "image", one),
                    Err(many) => put(out, "image", many),
                },
                ImageField::ImageUrl => {
                    if let Some(one) = urls.into_iter().next() {
                        put(out, "image_url", one);
                    }
                }
                ImageField::InputReferences => {
                    let refs: Vec<Value> = urls
                        .into_iter()
                        .map(|u| json!({"type": "image_url", "image_url": {"url": u}}))
                        .collect();
                    put(out, "input_references", refs);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn form_body(
    r: &ImageRequest,
    body: IndexMap<String, Value>,
) -> Result<(String, Vec<u8>), KppError> {
    let decode = |input: &ImageInput| -> Result<(String, Vec<u8>, String), KppError> {
        let Source::Base64(b) = &input.source else {
            return Err(invalid("an uploaded image must be bytes"));
        };
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&b.data)
            .map_err(|_| invalid("an image is not valid base64").with_param("image"))?;
        let name = input
            .filename
            .clone()
            .unwrap_or_else(|| default_name(&b.media_type));
        Ok((name, bytes, b.media_type.clone()))
    };
    let images = r.images.iter().map(decode).collect::<Result<Vec<_>, _>>()?;
    let mask = r.mask.as_ref().map(decode).transpose()?;
    let markup = match body
        .get(MARKUP)
        .and_then(|m| m.get("image_url"))
        .and_then(Value::as_str)
    {
        Some(url) => {
            let d = parse_data_url(url).ok_or_else(|| {
                invalid("`markup` must be an upload or a data URL").with_param(MARKUP)
            })?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(d.data)
                .map_err(|_| invalid("`markup` is not valid base64").with_param(MARKUP))?;
            Some((default_name(d.media_type), bytes, d.media_type.to_owned()))
        }
        None => None,
    };
    let image_field = match r.extras.form_str(DIALECT, FORM_IMAGE_FIELD) {
        Some("image") if images.len() == 1 => "image",
        _ => "image[]",
    };
    let mut texts: Vec<(String, String)> = Vec::new();
    for (k, v) in &body {
        if matches!(k.as_str(), "images" | "mask" | MARKUP) {
            continue;
        }
        flatten(k, v, &mut texts);
    }
    let mut fields: Vec<FormField<'_>> = Vec::new();
    for (k, v) in &texts {
        fields.push(FormField::Text(k, v));
    }
    for (name, bytes, media) in &images {
        fields.push(FormField::File(image_field, name, media, bytes));
    }
    if let Some((name, bytes, media)) = &mask {
        fields.push(FormField::File("mask", name, media, bytes));
    }
    if let Some((name, bytes, media)) = &markup {
        fields.push(FormField::File(MARKUP, name, media, bytes));
    }
    let w = crate::multipart::FormWriter::write(&fields);
    Ok((w.content_type(), w.into_body()))
}

fn default_name(media_type: &str) -> String {
    let ext = ImageFormat::from_media_type(media_type).map_or("bin", |f| match f {
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Png => "png",
        ImageFormat::Webp => "webp",
        ImageFormat::Gif => "gif",
        ImageFormat::Svg => "svg",
        _ => "bin",
    });
    format!("image.{ext}")
}

fn flatten(key: &str, value: &Value, out: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                flatten(&format!("{key}[{k}]"), v, out);
            }
        }
        Value::Array(items) => {
            for v in items {
                flatten(&format!("{key}[]"), v, out);
            }
        }
        Value::String(s) => out.push((key.to_owned(), s.clone())),
        Value::Null => {}
        other => out.push((key.to_owned(), other.to_string())),
    }
}
