use kaeso_kpp::{
    Block, CacheHint, CacheTtl, Citation, DocumentBlock, ImageBlock, KppError, Opaque, OpaqueKind,
    Origin, RawItem, RawJson, ReasoningBlock, Source, TextBlock, ToolCallBlock, ToolResultBlock,
};
use serde_json::value::RawValue;

use super::common::{
    D, Rewrites, empty_object, is_data_url, nested_ext, parse_base64_data_url, scoped, to_raw,
};
use super::wire::{
    CacheControlW, CitationsConfigW, DataSourceW, DocumentW, FileSourceW, ImageW, Raw,
    RedactedThinkingW, TextW, ThinkingW, ToolResultW, ToolUseW, UrlSourceW,
};
use crate::codec::EgressCtx;
use crate::cross::{ReplyPolicy, reasoning_replayable, reasoning_reply_policy};
use crate::wire::without_position;
use crate::wire::{
    Ext, StringOr, add_nested_extra, extra_for, extras_from, type_tag, verbatim_from_form,
    verbatim_into_form,
};
use serde_json::Value;

pub(crate) const CONTENT_FORM: &str = "content";

const EMPTY_SIGNATURE_FORM: &str = "empty_signature";

const CITATIONS_FORM: &str = "citations";

const MAX_TOOL_RESULT_DEPTH: u8 = 4;

#[derive(Clone, Copy)]
pub(crate) struct Dec<'a> {
    origin: &'a Origin,
    ctx: Option<&'a EgressCtx>,
    depth: u8,
}

impl<'a> Dec<'a> {
    pub(crate) fn new(origin: &'a Origin, ctx: Option<&'a EgressCtx>) -> Self {
        Dec {
            origin,
            ctx,
            depth: 0,
        }
    }

    fn nested(self) -> Result<Self, String> {
        if self.depth >= MAX_TOOL_RESULT_DEPTH {
            return Err(format!(
                "`tool_result` blocks nest more than {MAX_TOOL_RESULT_DEPTH} levels deep"
            ));
        }
        Ok(Dec {
            depth: self.depth + 1,
            ..self
        })
    }

    fn tool_id(&self, sent: &str) -> String {
        self.ctx
            .map_or(sent, |c| c.original_tool_id(sent))
            .to_owned()
    }

    fn tool_name(&self, sent: &str) -> String {
        self.ctx
            .map_or(sent, |c| c.original_tool_name(sent))
            .to_owned()
    }
}

pub(crate) fn decode_block(raw: &RawValue, dec: Dec<'_>) -> Result<Block, String> {
    let origin = dec.origin;
    let unknown = || Block::Unknown(RawItem::new(origin.clone(), RawJson::from(raw)));
    let ty = type_tag(raw).map_err(|e| without_position(&e))?;
    let block = match ty.as_deref() {
        Some("text") => {
            let w: Ext<TextW> = parse(raw, "text")?;
            let mut b = TextBlock::new(w.known.text);
            b.citations = w
                .known
                .citations
                .unwrap_or_default()
                .into_iter()
                .map(|c| Citation::new(origin.clone(), RawJson::from(c)))
                .collect();
            b.cache = w.known.cache_control.map(|c| decode_cache(c, origin));
            b.extras = extras_from(origin, w.extra, w.verbatim);
            Block::Text(b)
        }
        Some("image") => {
            let w: Ext<ImageW> = parse(raw, "image")?;
            let mut extras = extras_from(origin, w.extra, w.verbatim);
            let Some(source) = decode_source(&w.known.source, origin, &mut extras, false)? else {
                return Ok(unknown());
            };
            let mut b = ImageBlock::new(source);
            b.cache = w.known.cache_control.map(|c| decode_cache(c, origin));
            b.extras = extras;
            Block::Image(b)
        }
        Some("document") => {
            let w: Ext<DocumentW> = parse(raw, "document")?;
            let mut extras = extras_from(origin, w.extra, w.verbatim);
            let Some(source) = decode_source(&w.known.source, origin, &mut extras, true)? else {
                return Ok(unknown());
            };
            let mut b = DocumentBlock::new(source);
            b.title = w.known.title;
            b.context = w.known.context;
            if let Some(c) = w.known.citations {
                b.citations = c.known.enabled == Some(true);
                match c.known.enabled {
                    Some(true) => {}
                    Some(false) => {
                        extras.form.insert(CITATIONS_FORM.into(), "disabled".into());
                    }
                    None => {
                        extras.form.insert(CITATIONS_FORM.into(), "empty".into());
                    }
                }
                add_nested_extra(&mut extras, "citations", c.extra);
                verbatim_into_form(&mut extras, "citations", c.verbatim);
            }
            b.cache = w.known.cache_control.map(|c| decode_cache(c, origin));
            b.extras = extras;
            Block::Document(b)
        }
        Some("tool_use") => {
            let mut w: Ext<ToolUseW> = parse(raw, "tool_use")?;
            if let Some(caller) = w.extra.get("caller")
                && caller.to_value().ok() == Some(serde_json::json!({"type": "direct"}))
                && let Some(caller) = w.extra.shift_remove("caller")
            {
                w.verbatim.insert("caller".to_owned(), caller);
            }
            let mut b = ToolCallBlock::new(
                dec.tool_id(&w.known.id),
                dec.tool_name(&w.known.name),
                RawJson::from(w.known.input),
            );
            b.cache = w.known.cache_control.map(|c| decode_cache(c, origin));
            b.extras = extras_from(origin, w.extra, w.verbatim);
            Block::ToolCall(b)
        }
        Some("tool_result") => {
            let w: Ext<ToolResultW> = parse(raw, "tool_result")?;
            let mut extras = extras_from(origin, w.extra, w.verbatim);
            let content = match w.known.content {
                None => Vec::new(),
                Some(c) => decode_content(c, dec.nested()?, &mut extras, "")?,
            };
            let mut b = ToolResultBlock::new(w.known.tool_use_id, content);
            b.is_error = w.known.is_error == Some(true);
            b.cache = w.known.cache_control.map(|c| decode_cache(c, origin));
            b.extras = extras;
            Block::ToolResult(b)
        }
        Some("thinking") => {
            let w: Ext<ThinkingW> = parse(raw, "thinking")?;
            if let Some(block) = w
                .known
                .signature
                .as_deref()
                .and_then(crate::continuity::unwrap_reasoning)
            {
                return Ok(Block::Reasoning(block));
            }
            let mut b = ReasoningBlock::new(origin.clone());
            b.text.push(w.known.thinking);
            b.extras = extras_from(origin, w.extra, w.verbatim);
            match w.known.signature {
                Some(sig) if sig.is_empty() => {
                    b.extras
                        .form
                        .insert(EMPTY_SIGNATURE_FORM.into(), Value::Bool(true));
                }
                Some(sig) => b.opaque.push(Opaque::new(
                    origin.clone(),
                    OpaqueKind::AnthropicThinkingSignature,
                    sig,
                )),
                None => {}
            }
            Block::Reasoning(b)
        }
        Some("redacted_thinking") => {
            let w: Ext<RedactedThinkingW> = parse(raw, "redacted_thinking")?;
            let mut b = ReasoningBlock::new(origin.clone());
            b.opaque.push(Opaque::new(
                origin.clone(),
                OpaqueKind::AnthropicRedactedThinking,
                w.known.data,
            ));
            b.extras = extras_from(origin, w.extra, w.verbatim);
            Block::Reasoning(b)
        }
        _ => unknown(),
    };
    Ok(block)
}

pub(crate) fn decode_content(
    content: StringOr<Raw>,
    dec: Dec<'_>,
    extras: &mut kaeso_kpp::Extras,
    scope: &str,
) -> Result<Vec<Block>, String> {
    match content {
        StringOr::String(s) => {
            extras
                .form
                .insert(scoped(scope, CONTENT_FORM), "string".into());
            Ok(vec![Block::text(s)])
        }
        StringOr::List(list) => {
            if list.is_empty() {
                extras
                    .form
                    .insert(scoped(scope, CONTENT_FORM), "array".into());
            }
            list.iter().map(|b| decode_block(b, dec)).collect()
        }
    }
}

fn parse<T: serde::de::DeserializeOwned + serde::Serialize>(
    raw: &RawValue,
    what: &str,
) -> Result<Ext<T>, String> {
    Ext::from_raw(raw).map_err(|e| format!("invalid `{what}` block: {}", without_position(&e)))
}

fn decode_source(
    raw: &RawValue,
    origin: &Origin,
    extras: &mut kaeso_kpp::Extras,
    document: bool,
) -> Result<Option<Source>, String> {
    let ty = type_tag(raw).map_err(|e| without_position(&e))?;
    let err = |e: serde_json::Error| format!("invalid `source`: {}", without_position(&e));
    let (source, extra, verbatim) = match ty.as_deref() {
        Some("base64") => {
            let w: Ext<DataSourceW> = Ext::from_raw(raw).map_err(err)?;
            (
                Source::base64(w.known.media_type, w.known.data),
                w.extra,
                w.verbatim,
            )
        }
        Some("text") if document => {
            let w: Ext<DataSourceW> = Ext::from_raw(raw).map_err(err)?;
            (
                Source::text(w.known.media_type, w.known.data),
                w.extra,
                w.verbatim,
            )
        }
        Some("url") => {
            let w: Ext<UrlSourceW> = Ext::from_raw(raw).map_err(err)?;
            (Source::Url(w.known.url), w.extra, w.verbatim)
        }
        Some("file") => {
            let w: Ext<FileSourceW> = Ext::from_raw(raw).map_err(err)?;
            (
                Source::file_id(origin.clone(), w.known.file_id),
                w.extra,
                w.verbatim,
            )
        }
        _ => return Ok(None),
    };
    add_nested_extra(extras, "source", extra);
    verbatim_into_form(extras, "source", verbatim);
    Ok(Some(source))
}

pub(crate) fn decode_cache(w: Ext<CacheControlW>, origin: &Origin) -> CacheHint {
    let ttl = w.known.ttl.map(|t| match t.as_str() {
        "5m" => CacheTtl::FiveMinutes,
        "1h" => CacheTtl::OneHour,
        _ => CacheTtl::Other(t),
    });
    let mut extras = extras_from(origin, w.extra, w.verbatim);
    if w.known.ty != "ephemeral"
        && let Ok(ty) = RawJson::from_serialize(&w.known.ty)
    {
        extras.fields.insert("type".into(), ty);
    }
    let mut hint = CacheHint::new(ttl);
    hint.extras = extras;
    hint
}

pub(crate) fn encode_cache(hint: &CacheHint) -> Ext<CacheControlW> {
    let mut extra = extra_for(&hint.extras, D);
    let ty = extra
        .shift_remove("type")
        .and_then(|t| t.parse::<String>().ok())
        .unwrap_or_else(|| "ephemeral".to_owned());
    let ttl = hint.ttl.as_ref().and_then(|t| match t {
        CacheTtl::FiveMinutes => Some("5m".to_owned()),
        CacheTtl::OneHour => Some("1h".to_owned()),
        CacheTtl::Other(s) => Some(s.clone()),
        _ => None,
    });
    Ext::with(
        CacheControlW { ty, ttl },
        extra,
        verbatim_from_form(&hint.extras, D, ""),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Side {
    Request,
    Reply,
}

pub(crate) struct BlockEncoder {
    pub(crate) target: Origin,
    pub(crate) side: Side,
    pub(crate) rewrites: Rewrites,
}

impl BlockEncoder {
    pub(crate) fn new(target: Origin, side: Side) -> Self {
        BlockEncoder {
            target,
            side,
            rewrites: Rewrites::default(),
        }
    }

    fn id(&mut self, id: &str) -> String {
        match self.side {
            Side::Request => self.rewrites.id(id),
            Side::Reply => id.to_owned(),
        }
    }

    pub(crate) fn name(&mut self, name: &str) -> String {
        match self.side {
            Side::Request => self.rewrites.name(name),
            Side::Reply => name.to_owned(),
        }
    }

    pub(crate) fn blocks(&mut self, blocks: &[Block]) -> Result<Vec<Raw>, KppError> {
        let mut out = Vec::with_capacity(blocks.len());
        for b in blocks {
            if let Some(raw) = self.block(b)? {
                out.push(raw);
            }
        }
        Ok(out)
    }

    pub(crate) fn content(
        &mut self,
        blocks: &[Block],
        extras: &kaeso_kpp::Extras,
        scope: &str,
    ) -> Result<Option<StringOr<Raw>>, KppError> {
        let form = extras.form_str(D, &scoped(scope, CONTENT_FORM));
        if form == Some("string")
            && let [Block::Text(t)] = blocks
            && t.cache.is_none()
            && t.citations.is_empty()
            && t.extras.is_empty()
        {
            return Ok(Some(StringOr::String(t.text.clone())));
        }
        let list = self.blocks(blocks)?;
        if list.is_empty() && form != Some("array") {
            return Ok(None);
        }
        Ok(Some(StringOr::List(list)))
    }

    pub(crate) fn block(&mut self, block: &Block) -> Result<Option<Raw>, KppError> {
        let own = block.extras().is_some_and(|e| e.is_from(D));
        if self.side == Side::Reply
            && !own
            && matches!(
                block,
                Block::Image(_) | Block::Document(_) | Block::ToolResult(_)
            )
        {
            return Ok(None);
        }
        if let Block::Text(t) = block
            && self.side == Side::Request
            && t.text.is_empty()
            && t.cache.is_none()
            && t.citations.is_empty()
            && !t.extras.is_from(D)
        {
            return Ok(None);
        }
        let raw = match block {
            Block::Text(t) => {
                let citations: Vec<Raw> = t
                    .citations
                    .iter()
                    .filter(|c| c.origin.is(D))
                    .map(|c| c.raw.clone().into_inner())
                    .collect();
                let w = TextW {
                    ty: "text".into(),
                    text: t.text.clone(),
                    cache_control: t.cache.as_ref().map(encode_cache),
                    citations: (!citations.is_empty()).then_some(citations),
                };
                to_raw(&own_ext(w, &t.extras))?
            }
            Block::Image(b) => {
                let source = self.source(&b.source, &b.extras, false)?;
                let w = ImageW {
                    ty: "image".into(),
                    source,
                    cache_control: b.cache.as_ref().map(encode_cache),
                };
                to_raw(&own_ext(w, &b.extras))?
            }
            Block::Document(b) => {
                let source = self.source(&b.source, &b.extras, true)?;
                let form = b.extras.form_str(D, CITATIONS_FORM);
                let enabled = match (b.citations, form) {
                    (true, _) => Some(Some(true)),
                    (false, Some("disabled")) => Some(Some(false)),
                    (false, Some("empty")) => Some(None),
                    (false, _) => None,
                };
                let w = DocumentW {
                    ty: "document".into(),
                    source,
                    title: b.title.clone(),
                    context: b.context.clone(),
                    citations: enabled.map(|enabled| {
                        nested_ext(CitationsConfigW { enabled }, &b.extras, "citations")
                    }),
                    cache_control: b.cache.as_ref().map(encode_cache),
                };
                to_raw(&own_ext(w, &b.extras))?
            }
            Block::ToolCall(c) => {
                if self.side == Side::Reply && !own && !c.arguments.is_object() {
                    return Err(KppError::provider(format!(
                        "The provider returned arguments for the tool call `{}` that are not a JSON object, which an Anthropic Messages reply cannot carry; send the request again, or raise `max_tokens` if the reply was cut off.",
                        c.name
                    )));
                }
                let w = ToolUseW {
                    ty: "tool_use".into(),
                    id: self.id(&c.id),
                    name: self.name(&c.name),
                    input: c.arguments.clone().into_inner(),
                    cache_control: c.cache.as_ref().map(encode_cache),
                };
                to_raw(&own_ext(w, &c.extras))?
            }
            Block::ToolResult(r) => {
                let content = self.content(&r.content, &r.extras, "")?;
                let w = ToolResultW {
                    ty: "tool_result".into(),
                    tool_use_id: self.id(&r.call_id),
                    content,
                    is_error: r.is_error.then_some(true),
                    cache_control: r.cache.as_ref().map(encode_cache),
                };
                to_raw(&own_ext(w, &r.extras))?
            }
            Block::Reasoning(r) => return self.reasoning(r),
            Block::Refusal(r) => match self.side {
                Side::Reply => to_raw(&TextW {
                    ty: "text".into(),
                    text: r.text.clone(),
                    cache_control: None,
                    citations: None,
                })?,
                Side::Request => {
                    return Err(KppError::unsupported_by(
                        "refusal block",
                        D,
                        "Anthropic has no refusal content block. Remove the refusal from the conversation history or replace it with text.",
                    ));
                }
            },
            Block::Unknown(item) => {
                if item.origin.is(D) {
                    item.raw.clone().into_inner()
                } else if self.side == Side::Reply {
                    return Ok(None);
                } else {
                    return Err(KppError::unsupported_by(
                        "unknown content block",
                        D,
                        "It comes from another API format.",
                    ));
                }
            }
            other => {
                return Err(KppError::unsupported_by(
                    format!("{} block", other.kind()),
                    D,
                    "This version of Kaeso cannot write this kind of content as Anthropic Messages content.",
                ));
            }
        };
        Ok(Some(raw))
    }

    fn reasoning(&mut self, r: &ReasoningBlock) -> Result<Option<Raw>, KppError> {
        let keep = match self.side {
            Side::Request => reasoning_replayable(r, &self.target),
            Side::Reply => match reasoning_reply_policy(r.origin.as_ref(), D) {
                ReplyPolicy::Keep => true,
                ReplyPolicy::Carry => {
                    let w = ThinkingW {
                        ty: "thinking".into(),
                        thinking: carried_thinking(r),
                        signature: Some(crate::continuity::wrap_reasoning(r)),
                    };
                    return Ok(Some(to_raw(&w)?));
                }
                _ => false,
            },
        };
        if !keep {
            return Ok(None);
        }
        if let Some(redacted) = r.opaque_of(OpaqueKind::AnthropicRedactedThinking) {
            let w = RedactedThinkingW {
                ty: "redacted_thinking".into(),
                data: redacted.data.clone(),
            };
            return Ok(Some(to_raw(&own_ext(w, &r.extras))?));
        }
        let signature = match r
            .opaque_of(OpaqueKind::AnthropicThinkingSignature)
            .filter(|o| !o.data.is_empty())
        {
            Some(o) => Some(o.data.clone()),
            None if r.extras.form_flag(D, EMPTY_SIGNATURE_FORM) => Some(String::new()),
            None if r.extras.is_from(D) => None,
            None => return Ok(None),
        };
        let w = ThinkingW {
            ty: "thinking".into(),
            thinking: r.text.concat(),
            signature,
        };
        Ok(Some(to_raw(&own_ext(w, &r.extras))?))
    }

    fn source(
        &self,
        source: &Source,
        extras: &kaeso_kpp::Extras,
        document: bool,
    ) -> Result<Raw, KppError> {
        let raw = match source {
            Source::Base64(s) => to_raw(&nested_ext(
                DataSourceW {
                    ty: "base64".into(),
                    media_type: s.media_type.clone(),
                    data: s.data.clone(),
                },
                extras,
                "source",
            ))?,
            Source::Text(s) if document => to_raw(&nested_ext(
                DataSourceW {
                    ty: "text".into(),
                    media_type: s.media_type.clone(),
                    data: s.data.clone(),
                },
                extras,
                "source",
            ))?,
            Source::Url(url) if is_data_url(url) && !extras.is_from(D) => {
                let Some((media_type, data)) = parse_base64_data_url(url) else {
                    return Err(KppError::unsupported_by(
                        "data URL without base64 encoding",
                        D,
                        "Send the file as a base64 data URL or as base64 data.",
                    ));
                };
                to_raw(&DataSourceW {
                    ty: "base64".into(),
                    media_type,
                    data,
                })?
            }
            Source::Url(url) => to_raw(&nested_ext(
                UrlSourceW {
                    ty: "url".into(),
                    url: url.clone(),
                },
                extras,
                "source",
            ))?,
            Source::FileId(f) if f.origin.is(D) || self.side == Side::Request => {
                to_raw(&nested_ext(
                    FileSourceW {
                        ty: "file".into(),
                        file_id: f.id.clone(),
                    },
                    extras,
                    "source",
                ))?
            }
            Source::Text(_) => {
                return Err(KppError::unsupported_by(
                    "text image source",
                    D,
                    "Send the image as base64 data, a URL or an Anthropic file id.",
                ));
            }
            Source::FileId(_) => {
                return Err(KppError::unsupported_by(
                    "file id from another provider",
                    D,
                    "Provider file ids only work with the provider that issued them.",
                ));
            }
            _ => {
                return Err(KppError::unsupported_by(
                    "content source",
                    D,
                    "This version of Kaeso cannot write this kind of source as Anthropic Messages content.",
                ));
            }
        };
        Ok(raw)
    }
}

pub(crate) fn own_ext<T>(known: T, extras: &kaeso_kpp::Extras) -> Ext<T> {
    Ext::with(
        known,
        extra_for(extras, D),
        verbatim_from_form(extras, D, ""),
    )
}

pub(crate) fn empty_parameters() -> Raw {
    RawValue::from_string(r#"{"type":"object","properties":{}}"#.to_owned())
        .unwrap_or_else(|_| empty_object())
}

pub(crate) fn carried_thinking(r: &ReasoningBlock) -> String {
    r.summary
        .iter()
        .chain(r.text.iter())
        .filter(|t| !t.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n\n")
}
