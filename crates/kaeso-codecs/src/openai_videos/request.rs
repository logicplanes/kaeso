use base64::Engine as _;
use kaeso_kpp::{
    Extras, ImageSize, KppError, Origin, Request, Source, VideoImage, VideoImageRole,
    VideoOperation, VideoRequest,
};
use serde_json::{Map, Value, json};

use super::DIALECT;
use super::profile::{VideosBody, VideosProfile};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList, is_forwarded_header};
use crate::multipart::{FormError, FormField, FormLimits, FormPart, form_boundary, read_form};
use crate::video_jobs::{
    Fields, body, check_foreign_extras, check_roles, image_link, put, put_extras, refuse_unset,
    refused, video_request,
};

/// The most parts one create upload may hold: every field of `CreateVideoMultipartBody` and
/// one reference, with room for fields Kaeso keeps as extras.
pub const MAX_FORM_PARTS: usize = 32;

/// The most bytes the uploaded reference may hold: `ImageRefParam.image_url`'s 20 MiB limit
/// (`maxLength: 20971520` at openai-openapi 926a7b8).
pub const MAX_REFERENCE_BYTES: usize = 20 * 1024 * 1024;

const FORM_BODY: &str = "body";
const MULTIPART: &str = "multipart";
const SECONDS_NUMBER: &str = "seconds_number";
const REFERENCE_OBJECT: &str = "input_reference_object";

fn invalid(detail: impl std::fmt::Display) -> KppError {
    KppError::invalid_request(format!(
        "The request body is not a valid {} request: {detail}.",
        DIALECT.display_name()
    ))
}

fn is_multipart(headers: &HeaderList) -> Option<&str> {
    headers.get("content-type").filter(|t| {
        t.trim_start()
            .to_ascii_lowercase()
            .starts_with("multipart/")
    })
}

pub(super) fn decode_create(body: &[u8], headers: &HeaderList) -> Result<Request, KppError> {
    let r = match is_multipart(headers) {
        Some(ct) => decode_form(VideoOperation::Generate, body, ct)?,
        None => decode_json(VideoOperation::Generate, body)?,
    };
    Ok(Request::Video(r))
}

pub(super) fn decode_remix(body: &[u8], headers: &HeaderList) -> Result<Request, KppError> {
    let r = match is_multipart(headers) {
        Some(ct) => decode_form(VideoOperation::Remix, body, ct)?,
        None => decode_json(VideoOperation::Remix, body)?,
    };
    Ok(Request::Video(r))
}

fn decode_json(operation: VideoOperation, body: &[u8]) -> Result<VideoRequest, KppError> {
    let mut f = Fields::parse(body, DIALECT, "request")?;
    let origin = Origin::new(DIALECT);
    let mut extras = Extras::new(origin.clone());
    let mut r = VideoRequest::new(String::new(), operation);
    r.prompt = f.str("prompt")?;
    if operation == VideoOperation::Generate {
        r.model = f.str("model")?.unwrap_or_default();
        if let Some(reference) = f.object("input_reference")? {
            r.images.push(reference_image(reference, &origin)?);
        }
        if let Some((seconds, as_text)) = f.seconds("seconds")? {
            r.seconds = Some(seconds);
            if !as_text {
                extras
                    .form
                    .insert(SECONDS_NUMBER.to_owned(), Value::Bool(true));
            }
        }
        r.size = f.str("size")?.map(|s| size(&s)).transpose()?;
    }
    f.rest_into(&mut extras, "");
    r.extras = extras;
    Ok(r)
}

fn size(s: &str) -> Result<ImageSize, KppError> {
    let size = ImageSize::from_wire(s);
    if size.dimensions().is_none() {
        return Err(KppError::invalid_request(format!(
            "`size` must be WIDTHxHEIGHT, such as 1280x720, not `{s}`."
        ))
        .with_param("size"));
    }
    Ok(size)
}

fn reference_image(mut f: Fields, origin: &Origin) -> Result<VideoImage, KppError> {
    let url = f.str("image_url")?;
    let file = f.str("file_id")?;
    let source = match (url, file) {
        (Some(url), None) => crate::video_jobs::link_source(url),
        (None, Some(id)) => Source::file_id(origin.clone(), id),
        _ => {
            return Err(invalid(
                "`input_reference` takes exactly one of `image_url` and `file_id`",
            )
            .with_param("input_reference"));
        }
    };
    let mut image = VideoImage::new(source, VideoImageRole::FirstFrame);
    let mut extras = Extras::new(origin.clone());
    extras
        .form
        .insert(REFERENCE_OBJECT.to_owned(), Value::Bool(true));
    f.rest_into(&mut extras, "");
    image.extras = extras;
    Ok(image)
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
    operation: VideoOperation,
    body: &[u8],
    content_type: &str,
) -> Result<VideoRequest, KppError> {
    let boundary = form_boundary(content_type).map_err(form_error)?;
    let parts = read_form(
        body,
        &boundary,
        FormLimits::new(MAX_FORM_PARTS, MAX_REFERENCE_BYTES),
    )
    .map_err(form_error)?;
    let mut nested = Map::new();
    let mut upload: Option<VideoImage> = None;
    for part in &parts {
        let name = part.name.as_str();
        if part.filename.is_some() {
            if name != "input_reference" || operation == VideoOperation::Remix {
                return Err(KppError::invalid_request(format!(
                    "Kaeso does not know the file field `{name}`; send the reference as `input_reference`."
                ))
                .with_param(name.to_owned()));
            }
            if upload.is_some() {
                return Err(invalid("`input_reference` is sent more than once")
                    .with_param("input_reference"));
            }
            upload = Some(uploaded(part));
            continue;
        }
        let value = part
            .text()
            .ok_or_else(|| invalid(format!("`{name}` is not UTF-8 text")).with_param(name))?;
        if !name.contains('[') && nested.contains_key(name) {
            return Err(invalid(format!("`{name}` is sent more than once")).with_param(name));
        }
        crate::multipart::insert_nested(&mut nested, name, value, &[]).map_err(invalid)?;
    }
    let mut r = decode_json(
        operation,
        &serde_json::to_vec(&Value::Object(nested)).map_err(invalid)?,
    )?;
    if let Some(image) = upload {
        if !r.images.is_empty() {
            return Err(
                invalid("`input_reference` is sent more than once").with_param("input_reference")
            );
        }
        r.images.push(image);
    }
    r.extras.form.shift_remove(SECONDS_NUMBER);
    r.extras.form.insert(FORM_BODY.to_owned(), MULTIPART.into());
    Ok(r)
}

fn uploaded(part: &FormPart<'_>) -> VideoImage {
    let media_type = part
        .content_type
        .clone()
        .unwrap_or_else(|| "application/octet-stream".to_owned());
    let data = base64::engine::general_purpose::STANDARD.encode(part.body);
    let mut image = VideoImage::new(Source::base64(media_type, data), VideoImageRole::FirstFrame);
    image.filename = part.filename.clone();
    image
}

pub(super) fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
    let r = video_request(req, DIALECT)?;
    check_foreign_extras(r, DIALECT)?;
    let p = &ctx.videos_profile;
    match p.body {
        VideosBody::OpenAi => check_openai(r, p),
        VideosBody::Together => check_together(r, p),
    }
}

fn check_openai(r: &VideoRequest, p: &VideosProfile) -> Result<(), KppError> {
    if r.operation == VideoOperation::Remix {
        return Ok(());
    }
    refuse_unset(r, DIALECT, p.provider, &["seconds", "size"])?;
    check_roles(r, DIALECT, p.provider, &[VideoImageRole::FirstFrame])?;
    if r.images.len() > 1 {
        return Err(refused(
            DIALECT,
            p.provider,
            "more than one reference",
            "input_reference",
            "send one image.",
        ));
    }
    Ok(())
}

fn check_together(r: &VideoRequest, p: &VideosProfile) -> Result<(), KppError> {
    if r.operation == VideoOperation::Remix {
        return Err(refused(
            DIALECT,
            p.provider,
            "remixes",
            "video_id",
            "create a new video instead.",
        ));
    }
    refuse_unset(
        r,
        DIALECT,
        p.provider,
        &[
            "seconds",
            "size",
            "aspect_ratio",
            "resolution",
            "seed",
            "negative_prompt",
            "audio",
        ],
    )?;
    for image in &r.images {
        if !matches!(image.source, Source::Url(_)) {
            return Err(refused(
                DIALECT,
                p.provider,
                "uploaded images",
                "input_reference",
                "send `input_reference` with an `image_url` link.",
            ));
        }
        if !image.extras.is_empty() {
            return Err(refused(
                DIALECT,
                p.provider,
                "fields of `input_reference` other than `image_url`",
                "input_reference",
                "send `input_reference` with only an `image_url` link.",
            ));
        }
    }
    Ok(())
}

pub(super) fn encode(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    check(req, ctx)?;
    let r = video_request(req, DIALECT)?;
    let p = ctx.videos_profile;
    let mut headers = HeaderList::new();
    if r.extras.is_from(DIALECT) {
        for (name, value) in r.extras.headers() {
            if is_forwarded_header(DIALECT, name) {
                headers.push(name, value);
            }
        }
    }
    let path = match r.operation {
        VideoOperation::Remix => format!(
            "{}/{}/remix",
            p.path,
            crate::video_jobs::path_segment(r.remix_of.as_deref().unwrap_or_default())
        ),
        _ => p.path.to_owned(),
    };
    let out = match p.body {
        VideosBody::OpenAi => openai_body(r),
        VideosBody::Together => together_body(r),
    };
    let multipart = p.body == VideosBody::OpenAi
        && (r.extras.form_str(DIALECT, FORM_BODY) == Some(MULTIPART)
            || r.images
                .iter()
                .any(|i| i.filename.is_some() && matches!(i.source, Source::Base64(_))));
    if multipart {
        let (content_type, bytes) = form_body(r, out)?;
        headers.push("content-type", content_type);
        return Ok(EncodedRequest {
            method: "POST",
            path,
            headers,
            body: bytes,
        });
    }
    headers.push("content-type", crate::codec::JSON_MEDIA_TYPE);
    Ok(EncodedRequest::post_json(path, headers, body(&out)?))
}

fn openai_body(r: &VideoRequest) -> Map<String, Value> {
    let mut out = Map::new();
    if r.operation == VideoOperation::Generate && !r.model.is_empty() {
        out.insert("model".into(), r.model.clone().into());
    }
    put(&mut out, "prompt", r.prompt.clone());
    if let Some(image) = r.images.first()
        && image.filename.is_none()
    {
        let mut reference = Map::new();
        match &image.source {
            Source::FileId(f) => {
                reference.insert("file_id".into(), f.id.clone().into());
            }
            _ => put(&mut reference, "image_url", image_link(image)),
        }
        put_extras(&mut reference, &image.extras, DIALECT, "");
        out.insert("input_reference".into(), Value::Object(reference));
    }
    if let Some(s) = r.seconds {
        let v = if r.extras.form_flag(DIALECT, SECONDS_NUMBER) {
            json!(s)
        } else {
            json!(s.to_string())
        };
        out.insert("seconds".into(), v);
    }
    put(
        &mut out,
        "size",
        r.size.as_ref().map(|s| s.as_wire().to_owned()),
    );
    put_extras(&mut out, &r.extras, DIALECT, "");
    out
}

fn together_body(r: &VideoRequest) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("model".into(), r.model.clone().into());
    put(&mut out, "prompt", r.prompt.clone());
    if let Some((w, h)) = r.size.as_ref().and_then(ImageSize::dimensions) {
        out.insert("width".into(), w.into());
        out.insert("height".into(), h.into());
    }
    put(&mut out, "resolution", r.resolution.clone());
    put(&mut out, "ratio", r.aspect_ratio.clone());
    put(&mut out, "seconds", r.seconds.map(|s| s.to_string()));
    put(&mut out, "seed", r.seed);
    put(&mut out, "negative_prompt", r.negative_prompt.clone());
    put(&mut out, "generate_audio", r.audio);
    let link = |i: &VideoImage| match &i.source {
        Source::Url(u) => u.clone(),
        _ => String::new(),
    };
    let frames: Vec<Value> = r
        .images
        .iter()
        .filter(|i| i.role != VideoImageRole::Reference)
        .map(|i| {
            let frame = if i.role == VideoImageRole::LastFrame {
                "last"
            } else {
                "first"
            };
            json!({ "input_image": link(i), "frame": frame })
        })
        .collect();
    let references: Vec<Value> = r
        .images
        .iter()
        .filter(|i| i.role == VideoImageRole::Reference)
        .map(|i| Value::String(link(i)))
        .collect();
    if !frames.is_empty() || !references.is_empty() {
        let mut media = Map::new();
        if !frames.is_empty() {
            media.insert("frame_images".into(), Value::Array(frames));
        }
        if !references.is_empty() {
            media.insert("reference_images".into(), Value::Array(references));
        }
        out.insert("media".into(), Value::Object(media));
    }
    put_extras(&mut out, &r.extras, DIALECT, "");
    out
}

fn form_body(r: &VideoRequest, body: Map<String, Value>) -> Result<(String, Vec<u8>), KppError> {
    let mut texts: Vec<(String, String)> = Vec::new();
    for (k, v) in &body {
        crate::multipart::flatten(k, v, &mut texts);
    }
    let upload = match r.images.first() {
        Some(VideoImage {
            source: Source::Base64(b),
            filename: Some(name),
            ..
        }) => Some((
            name.clone(),
            base64::engine::general_purpose::STANDARD
                .decode(&b.data)
                .map_err(|_| {
                    invalid("the reference is not valid base64").with_param("input_reference")
                })?,
            b.media_type.clone(),
        )),
        _ => None,
    };
    let mut fields: Vec<FormField<'_>> = texts.iter().map(|(k, v)| FormField::Text(k, v)).collect();
    if let Some((name, bytes, media)) = &upload {
        fields.push(FormField::File("input_reference", name, media, bytes));
    }
    let w = crate::multipart::FormWriter::write(&fields);
    Ok((w.content_type(), w.into_body()))
}
