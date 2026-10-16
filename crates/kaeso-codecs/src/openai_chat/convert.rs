use indexmap::IndexMap;
use kaeso_kpp::{
    Block, CacheHint, DocumentBlock, Extras, ImageBlock, KppError, Origin, RawItem, RawJson,
    RefusalBlock, Source, TextBlock, ToolCallBlock,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use serde_json::value::RawValue;

use super::DIALECT;
use super::wire::{
    CacheBreakpoint, FilePart, FileRef, FunctionCall, ImagePart, ImageUrl, RefusalPart, TextPart,
    ToolCall,
};
use crate::wire::{
    Ext, add_nested_extra, extra_for, extras_from, nested_extra_for, parse_data_url, type_tag,
    verbatim_from_form, verbatim_into_form, without_position,
};

pub(crate) const FORM_BREAKPOINT_MODE: &str = "mode";
pub(crate) const FORM_ARGUMENTS: &str = "arguments";
pub(crate) const FORM_REFUSAL_PART: &str = "part";
pub(crate) const FORM_BARE_FILE_DATA: &str = "bare_file_data";

pub(crate) fn nulls_to_verbatim(
    extra: &mut IndexMap<String, RawJson>,
    verbatim: &mut IndexMap<String, RawJson>,
) {
    let nulls: Vec<String> = extra
        .iter()
        .filter(|(_, v)| v.get().trim() == "null")
        .map(|(k, _)| k.clone())
        .collect();
    for key in nulls {
        if let Some(v) = extra.shift_remove(&key) {
            verbatim.insert(key, v);
        }
    }
}

pub(crate) fn extras_of(
    origin: &Origin,
    mut extra: IndexMap<String, RawJson>,
    mut verbatim: IndexMap<String, RawJson>,
) -> Extras {
    nulls_to_verbatim(&mut extra, &mut verbatim);
    extras_from(origin, extra, verbatim)
}

pub(crate) fn add_nested(
    extras: &mut Extras,
    prefix: &str,
    mut extra: IndexMap<String, RawJson>,
    mut verbatim: IndexMap<String, RawJson>,
) {
    nulls_to_verbatim(&mut extra, &mut verbatim);
    add_nested_extra(extras, prefix, extra);
    verbatim_into_form(extras, prefix, verbatim);
}

pub(crate) fn own_keys(extras: &Extras) -> (IndexMap<String, RawJson>, IndexMap<String, RawJson>) {
    (
        extra_for(extras, DIALECT),
        verbatim_from_form(extras, DIALECT, ""),
    )
}

pub(crate) fn nested_keys(
    extras: &Extras,
    prefix: &str,
) -> (IndexMap<String, RawJson>, IndexMap<String, RawJson>) {
    (
        nested_extra_for(extras, DIALECT, prefix),
        verbatim_from_form(extras, DIALECT, prefix),
    )
}

pub(crate) fn ext_with<T>(known: T, extras: &Extras) -> Ext<T> {
    let (extra, verbatim) = own_keys(extras);
    Ext::with(known, extra, verbatim)
}

pub(crate) fn ext_nested<T>(known: T, extras: &Extras, prefix: &str) -> Ext<T> {
    let (extra, verbatim) = nested_keys(extras, prefix);
    Ext::with(known, extra, verbatim)
}

pub(crate) fn parse_ext<T: DeserializeOwned + Serialize>(
    raw: &RawValue,
    what: &str,
    from_provider: bool,
) -> Result<Ext<T>, KppError> {
    Ext::from_raw(raw).map_err(|e| {
        let e = without_position(&e);
        if from_provider {
            KppError::provider(format!(
                "The provider sent an OpenAI Chat Completions {what} that Kaeso cannot read: {e}."
            ))
        } else {
            KppError::invalid_request(format!(
                "The request contains an invalid OpenAI Chat Completions {what}: {e}."
            ))
        }
    })
}

pub(crate) fn to_raw<T: Serialize + ?Sized>(value: &T) -> Result<RawJson, KppError> {
    RawJson::from_serialize(value).map_err(|e| {
        KppError::provider(format!(
            "Kaeso could not serialize an OpenAI Chat Completions object ({e}). This is a bug in Kaeso; please report it."
        ))
    })
}

pub(crate) fn form_str(value: &str) -> Value {
    Value::String(value.to_owned())
}

pub(crate) const FORM_DATA_URL: &str = "data_url";

pub(crate) fn data_url_for(extras: &Extras, media_type: &str, data: &str) -> String {
    if let Some(header) = extras.form_str(DIALECT, FORM_DATA_URL) {
        let probe = format!("{header},");
        if parse_data_url(&probe).is_some_and(|d| d.media_type == media_type) {
            return format!("{header},{data}");
        }
    }
    data_url(media_type, data)
}

fn mark_header(extras: &mut Extras, header: Option<&str>) {
    if let Some(header) = header {
        extras
            .form
            .insert(FORM_DATA_URL.to_owned(), form_str(header));
    }
}

pub(crate) fn data_url(media_type: &str, data: &str) -> String {
    format!("data:{media_type};base64,{data}")
}

pub(crate) fn decode_breakpoint(bp: Ext<CacheBreakpoint>, origin: &Origin) -> CacheHint {
    let Ext {
        known,
        extra,
        verbatim,
        ..
    } = bp;
    let mut hint = CacheHint::new(None);
    hint.extras = extras_of(origin, extra, verbatim);
    match known.mode.as_deref() {
        Some("explicit") => {}
        Some(other) => {
            hint.extras
                .form
                .insert(FORM_BREAKPOINT_MODE.to_owned(), form_str(other));
        }
        None => {
            hint.extras
                .form
                .insert(FORM_BREAKPOINT_MODE.to_owned(), Value::Null);
        }
    }
    hint
}

pub(crate) fn encode_breakpoint(cache: Option<&CacheHint>) -> Option<Ext<CacheBreakpoint>> {
    let cache = cache?;
    if !cache.extras.is_from(DIALECT) {
        return None;
    }
    let mode = match cache.extras.form_value(DIALECT, FORM_BREAKPOINT_MODE) {
        None => Some("explicit".to_owned()),
        Some(Value::String(s)) => Some(s.clone()),
        Some(_) => None,
    };
    Some(ext_with(CacheBreakpoint { mode }, &cache.extras))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PartContext {
    TextOnly,
    User,
    Assistant,
}

pub(crate) fn decode_part(
    raw: &RawJson,
    ctx: PartContext,
    origin: &Origin,
) -> Result<Block, KppError> {
    let ty = type_tag(raw.as_raw()).map_err(|e| {
        KppError::invalid_request(format!(
            "The request contains an invalid OpenAI Chat Completions content part: {}.",
            without_position(&e)
        ))
    })?;
    match (ty.as_deref(), ctx) {
        (Some("text"), _) => decode_text_part(raw, origin),
        (Some("refusal"), PartContext::Assistant) => {
            let part: Ext<RefusalPart> = parse_ext(raw.as_raw(), "refusal part", false)?;
            let mut block = RefusalBlock::new(part.known.refusal);
            block.extras = extras_of(origin, part.extra, part.verbatim);
            block
                .extras
                .form
                .insert(FORM_REFUSAL_PART.to_owned(), Value::Bool(true));
            Ok(Block::Refusal(block))
        }
        (Some("image_url"), PartContext::User) => decode_image_part(raw, origin),
        (Some("file"), PartContext::User) => decode_file_part(raw, origin),
        (Some("input_audio"), _) => Err(KppError::unsupported(
            "input_audio",
            "Audio input (`input_audio` content parts) is not supported by Kaeso. Send the audio transcript as text instead.",
        )),
        _ => Ok(Block::Unknown(RawItem::new(origin.clone(), raw.clone()))),
    }
}

fn decode_text_part(raw: &RawJson, origin: &Origin) -> Result<Block, KppError> {
    let part: Ext<TextPart> = parse_ext(raw.as_raw(), "text part", false)?;
    let Ext {
        known,
        extra,
        verbatim,
        ..
    } = part;
    let mut block = TextBlock::new(known.text);
    block.cache = known
        .prompt_cache_breakpoint
        .map(|bp| decode_breakpoint(bp, origin));
    block.extras = extras_of(origin, extra, verbatim);
    Ok(Block::Text(block))
}

fn decode_image_part(raw: &RawJson, origin: &Origin) -> Result<Block, KppError> {
    let part: Ext<ImagePart> = parse_ext(raw.as_raw(), "image_url part", false)?;
    let Ext {
        known,
        extra,
        verbatim,
        ..
    } = part;
    let Ext {
        known: image_url,
        extra: url_extra,
        verbatim: url_verbatim,
        ..
    } = known.image_url;
    let parsed = parse_data_url(&image_url.url);
    let header = parsed.as_ref().and_then(|d| d.header.map(str::to_owned));
    let source = match parsed {
        Some(d) => Source::base64(d.media_type, d.data),
        None => Source::Url(image_url.url.clone()),
    };
    let mut block = ImageBlock::new(source);
    block.cache = known
        .prompt_cache_breakpoint
        .map(|bp| decode_breakpoint(bp, origin));
    block.extras = extras_of(origin, extra, verbatim);
    add_nested(&mut block.extras, "image_url", url_extra, url_verbatim);
    mark_header(&mut block.extras, header.as_deref());
    Ok(Block::Image(block))
}

fn decode_file_part(raw: &RawJson, origin: &Origin) -> Result<Block, KppError> {
    let part: Ext<FilePart> = parse_ext(raw.as_raw(), "file part", false)?;
    let Ext {
        known,
        extra,
        verbatim,
        ..
    } = part;
    let Ext {
        known: file,
        extra: mut file_extra,
        verbatim: file_verbatim,
        ..
    } = known.file;
    let mut bare = false;
    let mut header = None;
    let source = match (file.file_data, file.file_id) {
        (Some(file_data), file_id) => {
            if let Some(id) = file_id {
                file_extra.insert("file_id".to_owned(), to_raw(&id)?);
            }
            match parse_data_url(&file_data) {
                Some(d) => {
                    header = d.header.map(str::to_owned);
                    Source::base64(d.media_type, d.data)
                }
                None => {
                    bare = true;
                    Source::base64("application/pdf", file_data)
                }
            }
        }
        (None, Some(id)) => Source::file_id(origin.clone(), id),
        (None, None) => {
            return Err(KppError::invalid_request(
                "A `file` content part needs `file.file_data` or `file.file_id`.",
            ));
        }
    };
    let mut block = DocumentBlock::new(source);
    block.cache = known
        .prompt_cache_breakpoint
        .map(|bp| decode_breakpoint(bp, origin));
    block.extras = extras_of(origin, extra, verbatim);
    add_nested(&mut block.extras, "file", file_extra, file_verbatim);
    mark_header(&mut block.extras, header.as_deref());
    if bare {
        block
            .extras
            .form
            .insert(FORM_BARE_FILE_DATA.to_owned(), Value::Bool(true));
    }
    Ok(Block::Document(block))
}

pub(crate) fn encode_text_part(block: &TextBlock) -> Result<RawJson, KppError> {
    let part = TextPart {
        ty: "text".to_owned(),
        text: block.text.clone(),
        prompt_cache_breakpoint: encode_breakpoint(block.cache.as_ref()),
    };
    to_raw(&ext_with(part, &block.extras))
}

pub(crate) fn encode_refusal_part(block: &RefusalBlock) -> Result<RawJson, KppError> {
    let part = RefusalPart {
        ty: "refusal".to_owned(),
        refusal: block.text.clone(),
    };
    to_raw(&ext_with(part, &block.extras))
}

pub(crate) fn encode_image_part(block: &ImageBlock) -> Result<RawJson, KppError> {
    let url = match &block.source {
        Source::Url(url) => url.clone(),
        Source::Base64(b) => data_url_for(&block.extras, &b.media_type, &b.data),
        other => {
            return Err(source_unsupported("image", other));
        }
    };
    let part = ImagePart {
        ty: "image_url".to_owned(),
        image_url: ext_nested(ImageUrl { url }, &block.extras, "image_url"),
        prompt_cache_breakpoint: encode_breakpoint(block.cache.as_ref()),
    };
    to_raw(&ext_with(part, &block.extras))
}

pub(crate) fn encode_file_part(block: &DocumentBlock) -> Result<RawJson, KppError> {
    let mut file = FileRef::default();
    match &block.source {
        Source::Base64(b) => {
            let bare = block.extras.form_flag(DIALECT, FORM_BARE_FILE_DATA);
            file.file_data = Some(if bare {
                b.data.clone()
            } else {
                data_url_for(&block.extras, &b.media_type, &b.data)
            });
        }
        Source::FileId(f) => file.file_id = Some(f.id.clone()),
        other => return Err(source_unsupported("document", other)),
    }
    let part = FilePart {
        ty: "file".to_owned(),
        file: ext_nested(file, &block.extras, "file"),
        prompt_cache_breakpoint: encode_breakpoint(block.cache.as_ref()),
    };
    to_raw(&ext_with(part, &block.extras))
}

fn source_unsupported(kind: &str, source: &Source) -> KppError {
    let what = match source {
        Source::Url(_) => "a URL",
        Source::Text(_) => "inline text",
        Source::FileId(_) => "a file id",
        _ => "this source",
    };
    KppError::unsupported(
        format!("{kind} source"),
        format!(
            "OpenAI Chat Completions cannot take {kind} content given as {what}. Send it as base64 data instead."
        ),
    )
}

pub(crate) fn arguments_to_raw(text: &str) -> (RawJson, Option<Value>) {
    match RawJson::from_string(text.to_owned()) {
        Ok(raw) if raw.get() == text => (raw, None),
        Ok(raw) => (raw, Some(form_str(text))),
        Err(_) => {
            let raw = if text.trim().is_empty() {
                RawJson::empty_object()
            } else {
                RawJson::from_serialize(text).unwrap_or_else(|_| RawJson::empty_object())
            };
            (raw, Some(form_str(text)))
        }
    }
}

pub(crate) fn arguments_text(call: &ToolCallBlock) -> String {
    if call.extras.is_from(DIALECT) {
        return match call.extras.form_str(DIALECT, FORM_ARGUMENTS) {
            Some(literal) => literal.to_owned(),
            None => call.arguments.get().to_owned(),
        };
    }
    match call.arguments.parse::<String>() {
        Ok(text) => text,
        Err(_) => call.arguments.get().to_owned(),
    }
}

pub(crate) fn decode_tool_call(
    raw: &RawJson,
    origin: &Origin,
    from_provider: bool,
    name_back: &dyn Fn(&str) -> String,
) -> Result<Block, KppError> {
    let ty = type_tag(raw.as_raw()).ok().flatten();
    if ty.as_deref() != Some("function") {
        return Ok(Block::Unknown(RawItem::new(origin.clone(), raw.clone())));
    }
    let call: Ext<ToolCall> = parse_ext(raw.as_raw(), "tool call", from_provider)?;
    let Ext {
        known,
        extra,
        verbatim,
        ..
    } = call;
    let Ext {
        known: function,
        extra: fn_extra,
        verbatim: fn_verbatim,
        ..
    } = known.function;
    let (arguments, literal) = arguments_to_raw(&function.arguments);
    let mut block = ToolCallBlock::new(known.id, name_back(&function.name), arguments);
    block.extras = extras_of(origin, extra, verbatim);
    add_nested(&mut block.extras, "function", fn_extra, fn_verbatim);
    if let Some(literal) = literal {
        block.extras.form.insert(FORM_ARGUMENTS.to_owned(), literal);
    }
    Ok(Block::ToolCall(block))
}

pub(crate) fn encode_tool_call(call: &ToolCallBlock, name: &str) -> Result<RawJson, KppError> {
    let function = FunctionCall {
        name: name.to_owned(),
        arguments: arguments_text(call),
    };
    let wire = ToolCall {
        id: call.id.clone(),
        ty: "function".to_owned(),
        function: ext_nested(function, &call.extras, "function"),
    };
    to_raw(&ext_with(wire, &call.extras))
}

pub(crate) fn is_tool_call_item(item: &RawItem) -> bool {
    matches!(
        type_tag(item.raw.as_raw()).ok().flatten().as_deref(),
        Some("function" | "custom")
    )
}

pub(crate) const MAX_TOOL_NAME: usize = 64;

pub(crate) fn valid_tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_TOOL_NAME
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub(crate) fn safe_tool_name(
    name: &str,
    sent_to_original: &mut IndexMap<String, String>,
) -> String {
    crate::wire::rewrite_name(name, valid_tool_name, MAX_TOOL_NAME, sent_to_original)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_urls_split_with_params_and_any_case() {
        let plain = parse_data_url("data:image/png;base64,AAAA").unwrap();
        assert_eq!(
            (plain.media_type, plain.data, plain.header),
            ("image/png", "AAAA", None)
        );
        let named = parse_data_url("data:image/png;name=a.png;base64,AAAA").unwrap();
        assert_eq!(named.media_type, "image/png");
        assert_eq!(named.header, Some("data:image/png;name=a.png;base64"));
        let upper = parse_data_url("DATA:image/png;BASE64,AAAA").unwrap();
        assert_eq!(upper.header, Some("DATA:image/png;BASE64"));
        assert_eq!(parse_data_url("https://example.com/a.png"), None);
        assert_eq!(parse_data_url("data:;base64,AAAA"), None);
        assert_eq!(parse_data_url("data:text/plain,hello"), None);
        assert_eq!(
            data_url("application/pdf", "JVBE"),
            "data:application/pdf;base64,JVBE"
        );
    }

    #[test]
    fn tool_names_are_rewritten_deterministically() {
        let mut names = IndexMap::new();
        let mut send = |name: &str| safe_tool_name(name, &mut names);
        assert_eq!(send("get_weather"), "get_weather");
        let long = "x".repeat(100);
        let a = send(&long);
        assert!(valid_tool_name(&a), "{a}");
        assert_eq!(a, send(&long));
        let b = send("mcp.server/tool name");
        assert!(valid_tool_name(&b), "{b}");
        assert!(b.starts_with("mcp_server_tool_name_"));
        assert_ne!(send("a.b"), send("a/b"));
        assert!(valid_tool_name(&send("")));
        assert_eq!(
            names.get(&b).map(String::as_str),
            Some("mcp.server/tool name")
        );
        assert!(!names.contains_key("get_weather"));
    }

    #[test]
    fn rewritten_tool_names_never_collide() {
        let mut names = IndexMap::new();
        let first = safe_tool_name("a.b", &mut names);
        names.insert(first.clone(), "someone else".to_owned());
        let second = safe_tool_name("a.b", &mut names);
        assert_ne!(first, second);
        assert!(valid_tool_name(&second), "{second}");
        assert_eq!(names.get(&second).map(String::as_str), Some("a.b"));
    }

    #[test]
    fn invalid_arguments_are_kept_as_literal() {
        let (raw, form) = arguments_to_raw("{\"a\": 1}");
        assert_eq!(raw.get(), "{\"a\": 1}");
        assert!(form.is_none());
        let (raw, form) = arguments_to_raw("");
        assert_eq!(raw.get(), "{}");
        assert_eq!(form, Some(Value::String(String::new())));
        let (raw, form) = arguments_to_raw("{\"a\":");
        assert_eq!(raw.get(), "\"{\\\"a\\\":\"");
        assert_eq!(form, Some(Value::String("{\"a\":".into())));
    }
}
