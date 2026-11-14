use kaeso_kpp::{
    Block, CountTokensResponse, Extras, KppError, Origin, RawJson, RawUsage, Reply, RequestKind,
    Response, StopReason, Usage,
};
use serde_json::{Map, Value};

use super::D;
use super::error::decode_error;
use super::parts::{Calls, PartDecoder, PartEncoder};
use super::wire::{CandidateW, ContentW, CountTokensResponseW, Raw, ResponseW, UsageW};
use crate::codec::{EgressCtx, EncodeCtx, HeaderList};
use crate::wire::{
    Ext, add_nested_extra, extra_for, extras_from, nested_extra_for, to_body, verbatim_from_form,
    verbatim_into_form, without_position,
};

pub(crate) const CANDIDATE: &str = "candidate";
pub(crate) const CONTENT: &str = "candidate.content";
const FORM_NO_CANDIDATES: &str = "no_candidates";
const FORM_FINISH: &str = "finish";
const FORM_NO_ID: &str = "no_response_id";
const FORM_NO_MODEL: &str = "no_model_version";
const FORM_NO_USAGE: &str = "no_usage";
const FORM_NO_CONTENT: &str = "no_content";
const FORM_NO_ROLE: &str = "no_role";

pub(crate) fn provider_error(e: impl std::fmt::Display) -> KppError {
    KppError::provider(format!(
        "The provider sent a response that is not a valid Gemini API response: {e}."
    ))
}

const FILTERED: &[&str] = &[
    "SAFETY",
    "BLOCKLIST",
    "PROHIBITED_CONTENT",
    "SPII",
    "IMAGE_SAFETY",
    "IMAGE_PROHIBITED_CONTENT",
    "RECITATION",
    "IMAGE_RECITATION",
];

/// The finish reasons of `Candidate.finishReason` at revision 20261106.
const FINISH_REASONS: &[&str] = &[
    "FINISH_REASON_UNSPECIFIED",
    "STOP",
    "MAX_TOKENS",
    "SAFETY",
    "RECITATION",
    "LANGUAGE",
    "OTHER",
    "BLOCKLIST",
    "PROHIBITED_CONTENT",
    "SPII",
    "MALFORMED_FUNCTION_CALL",
    "IMAGE_SAFETY",
    "IMAGE_PROHIBITED_CONTENT",
    "IMAGE_OTHER",
    "NO_IMAGE",
    "IMAGE_RECITATION",
    "UNEXPECTED_TOOL_CALL",
    "TOO_MANY_TOOL_CALLS",
    "MISSING_THOUGHT_SIGNATURE",
    "MALFORMED_RESPONSE",
    "ESCALATION",
    "PUP_LIMITED_DISABLED",
    "CONTINUATION",
];

pub(crate) fn stop_from_wire(reason: &str, calls: bool, extras: &mut Extras) -> StopReason {
    let stop = match reason {
        "STOP" if calls => StopReason::ToolUse,
        "STOP" => StopReason::EndTurn,
        "MAX_TOKENS" => StopReason::MaxTokens,
        r if FILTERED.contains(&r) => StopReason::ContentFilter,
        other => StopReason::Other(other.to_owned()),
    };
    if stop_to_wire(&stop) != reason {
        extras
            .form
            .insert(FORM_FINISH.to_owned(), reason.to_owned().into());
    }
    stop
}

pub(crate) fn stop_to_wire(stop: &StopReason) -> &str {
    match stop {
        StopReason::EndTurn | StopReason::ToolUse | StopReason::StopSequence(_) => "STOP",
        StopReason::MaxTokens => "MAX_TOKENS",
        StopReason::ContentFilter | StopReason::Refusal => "SAFETY",
        StopReason::Other(o) if FINISH_REASONS.contains(&o.as_str()) => o,
        _ => "OTHER",
    }
}

pub(crate) fn finish_reason(stop: &StopReason, extras: &Extras) -> String {
    extras
        .form_str(D, FORM_FINISH)
        .map_or_else(|| stop_to_wire(stop).to_owned(), str::to_owned)
}

pub(crate) fn decode_usage(raw: &Raw, origin: &Origin) -> Result<Usage, KppError> {
    let w: UsageW = serde_json::from_str(raw.get()).map_err(|e| {
        provider_error(format!("invalid `usageMetadata`: {}", without_position(&e)))
    })?;
    let value: Value = serde_json::from_str(raw.get()).map_err(provider_error)?;
    let input = w
        .prompt_token_count
        .saturating_add(w.tool_use_prompt_token_count)
        .saturating_sub(w.cached_content_token_count);
    let mut u = Usage::new(
        input,
        w.candidates_token_count
            .saturating_add(w.thoughts_token_count),
    );
    u.cache_read_tokens = w.cached_content_token_count;
    u.reasoning_tokens = (w.thoughts_token_count > 0).then_some(w.thoughts_token_count);
    // `promptTokensDetails` counts the cached tokens too, like `promptTokenCount`; KPP's
    // `input_*_tokens` are shares of `input_tokens`, which leaves the cache out.
    let uncached = |modality| {
        modality_tokens(&value, "promptTokensDetails", modality).map(|n| {
            n.saturating_sub(modality_tokens(&value, "cacheTokensDetails", modality).unwrap_or(0))
        })
    };
    u.input_image_tokens = uncached("IMAGE");
    u.cache_read_image_tokens = modality_tokens(&value, "cacheTokensDetails", "IMAGE");
    u.output_image_tokens = modality_tokens(&value, "candidatesTokensDetails", "IMAGE");
    u.input_audio_tokens = uncached("AUDIO");
    u.output_audio_tokens = modality_tokens(&value, "candidatesTokensDetails", "AUDIO");
    u.raw = Some(RawUsage::new(origin.clone(), value));
    Ok(u)
}

// `ModalityTokenCount` lists in `UsageMetadata`, discovery revision 20261106.
fn modality_tokens(usage: &Value, list: &str, modality: &str) -> Option<u64> {
    let counts = usage.get(list)?.as_array()?;
    let mut total = None;
    for c in counts {
        if c.get("modality").and_then(Value::as_str) == Some(modality) {
            let n = c.get("tokenCount").and_then(Value::as_u64).unwrap_or(0);
            total = Some(total.unwrap_or(0u64).saturating_add(n));
        }
    }
    total
}

pub(crate) fn encode_usage(u: &Usage) -> Value {
    if let Some(raw) = u.raw_for(D) {
        return raw.clone();
    }
    let thoughts = u.reasoning_tokens.unwrap_or(0).min(u.output_tokens);
    let prompt = u
        .input_tokens
        .saturating_add(u.cache_read_tokens)
        .saturating_add(u.cache_write_tokens);
    let mut m = Map::new();
    m.insert("promptTokenCount".into(), prompt.into());
    m.insert(
        "candidatesTokenCount".into(),
        u.output_tokens.saturating_sub(thoughts).into(),
    );
    m.insert(
        "totalTokenCount".into(),
        prompt.saturating_add(u.output_tokens).into(),
    );
    if u.cache_read_tokens > 0 {
        m.insert("cachedContentTokenCount".into(), u.cache_read_tokens.into());
    }
    if thoughts > 0 {
        m.insert("thoughtsTokenCount".into(), thoughts.into());
    }
    Value::Object(m)
}

pub(crate) fn decode_response(
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<Reply, KppError> {
    if !(200..300).contains(&status) {
        return Err(decode_error(status, headers, body));
    }
    let origin = ctx.origin(D);
    match ctx.kind {
        RequestKind::CountTokens => {
            let w: Ext<CountTokensResponseW> =
                Ext::from_slice(body).map_err(|e| provider_error(without_position(&e)))?;
            let mut r = CountTokensResponse::new(w.known.total_tokens);
            r.extras = extras_from(&origin, w.extra, w.verbatim);
            Ok(Reply::CountTokens(r))
        }
        RequestKind::Generate => {
            let w: Ext<ResponseW> =
                Ext::from_slice(body).map_err(|e| provider_error(without_position(&e)))?;
            decode_generate(w, &origin).map(Reply::Generate)
        }
        _ => Err(KppError::unsupported_by(
            "embedding response",
            D,
            "Read embeddings with the Gemini embeddings codec.",
        )),
    }
}

pub(super) fn decode_generate(w: Ext<ResponseW>, origin: &Origin) -> Result<Response, KppError> {
    let Ext {
        known: w,
        extra,
        verbatim,
    } = w;
    let mut extras = extras_from(origin, extra, verbatim);
    let id = match w.response_id {
        Some(id) => id,
        None => {
            extras.form.insert(FORM_NO_ID.into(), true.into());
            String::new()
        }
    };
    let model = match w.model_version {
        Some(m) => m,
        None => {
            extras.form.insert(FORM_NO_MODEL.into(), true.into());
            String::new()
        }
    };
    let mut blocks = Vec::new();
    let mut stop = None;
    match w.candidates {
        None => {
            extras.form.insert(FORM_NO_CANDIDATES.into(), true.into());
            stop = Some(StopReason::ContentFilter);
        }
        Some(candidates) => {
            if candidates.len() > 1 {
                return Err(KppError::unsupported_by(
                    "more than one candidate",
                    D,
                    "Kaeso reads one candidate per reply; leave `candidateCount` out or set it to 1.",
                ));
            }
            if candidates.is_empty() {
                extras
                    .form
                    .insert(FORM_NO_CANDIDATES.into(), "empty".into());
            }
            for c in candidates {
                let finish = decode_candidate(c, origin, &id, &mut blocks, &mut extras)?;
                let calls = blocks.iter().any(|b| matches!(b, Block::ToolCall(_)));
                stop = finish.map(|f| stop_from_wire(&f, calls, &mut extras));
            }
        }
    }
    let mut r = Response::new(id, model, blocks);
    r.stop = stop.unwrap_or_else(|| {
        extras.form.insert(FORM_FINISH.into(), Value::Null);
        StopReason::EndTurn
    });
    match &w.usage_metadata {
        Some(u) => r.usage = decode_usage(u, origin)?,
        None => {
            extras.form.insert(FORM_NO_USAGE.into(), true.into());
        }
    }
    r.extras = extras;
    Ok(r)
}

pub(crate) fn id_seed(response_id: &str) -> String {
    response_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

fn decode_candidate(
    c: Ext<CandidateW>,
    origin: &Origin,
    response_id: &str,
    blocks: &mut Vec<Block>,
    extras: &mut Extras,
) -> Result<Option<String>, KppError> {
    let Ext {
        known,
        extra,
        verbatim,
    } = c;
    add_nested_extra(extras, CANDIDATE, extra);
    verbatim_into_form(extras, CANDIDATE, verbatim);
    match known.content {
        None => {
            extras.form.insert(FORM_NO_CONTENT.into(), true.into());
        }
        Some(content) => {
            let Ext {
                known,
                extra,
                verbatim,
            } = content;
            add_nested_extra(extras, CONTENT, extra);
            verbatim_into_form(extras, CONTENT, verbatim);
            match known.role.as_deref() {
                Some("model") => {}
                Some(other) => {
                    extras
                        .form
                        .insert(FORM_NO_ROLE.into(), other.to_owned().into());
                }
                None => {
                    extras.form.insert(FORM_NO_ROLE.into(), true.into());
                }
            }
            let mut calls = Calls::default();
            let mut dec = PartDecoder {
                origin,
                calls: &mut calls,
                id_seed: id_seed(response_id),
                generated: 0,
            };
            for part in &known.parts {
                dec.part(part, blocks).map_err(provider_error)?;
            }
        }
    }
    Ok(known.finish_reason)
}

pub(crate) fn encode_response(reply: &Reply, _ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
    match reply {
        Reply::Generate(r) => to_body(&encode_generate(r)?),
        Reply::CountTokens(c) => {
            let w = CountTokensResponseW {
                total_tokens: c.input_tokens,
            };
            let own = c.extras.is_from(D);
            let ext = if own {
                Ext::with(
                    w,
                    extra_for(&c.extras, D),
                    verbatim_from_form(&c.extras, D, ""),
                )
            } else {
                Ext::new(w)
            };
            to_body(&ext)
        }
        _ => Err(KppError::unsupported_by(
            "embedding response",
            D,
            "Gemini apps receive embeddings from the Gemini embeddings endpoints.",
        )),
    }
}

pub(crate) fn content_parts(blocks: &[Block]) -> Result<Vec<Raw>, KppError> {
    let target = Origin::new(D);
    let mut enc = PartEncoder::new(&target);
    enc.reply = true;
    for (i, b) in blocks.iter().enumerate() {
        enc.block(b, &format!("blocks[{i}]"))?;
    }
    enc.finish()
        .into_iter()
        .map(|v| serde_json::value::to_raw_value(&v))
        .collect::<Result<Vec<_>, _>>()
        .map_err(provider_error)
}

pub(crate) fn content(extras: &Extras, parts: Vec<Raw>) -> Ext<ContentW> {
    let role = match extras.form_value(D, FORM_NO_ROLE) {
        Some(Value::Bool(true)) => None,
        Some(Value::String(s)) => Some(s.clone()),
        _ => Some("model".to_owned()),
    };
    Ext::with(
        ContentW { role, parts },
        nested_extra_for(extras, D, CONTENT),
        verbatim_from_form(extras, D, CONTENT),
    )
}

fn encode_generate(r: &Response) -> Result<Ext<ResponseW>, KppError> {
    let own = r.extras.is_from(D);
    let candidates = match r.extras.form_value(D, FORM_NO_CANDIDATES) {
        Some(Value::Bool(true)) => None,
        Some(_) => Some(Vec::new()),
        None => {
            let content = if r.extras.form_flag(D, FORM_NO_CONTENT) {
                None
            } else {
                Some(content(&r.extras, content_parts(&r.blocks)?))
            };
            let finish = match r.extras.form_value(D, FORM_FINISH) {
                Some(Value::Null) => None,
                _ => Some(finish_reason(&r.stop, &r.extras)),
            };
            let mut extra = nested_extra_for(&r.extras, D, CANDIDATE);
            if !own {
                extra.insert(
                    "index".into(),
                    RawJson::from_string("0".into()).map_err(provider_error)?,
                );
            }
            Some(vec![Ext::with(
                CandidateW {
                    content,
                    finish_reason: finish,
                },
                extra,
                verbatim_from_form(&r.extras, D, CANDIDATE),
            )])
        }
    };
    let usage = if r.extras.form_flag(D, FORM_NO_USAGE) {
        None
    } else {
        Some(serde_json::value::to_raw_value(&encode_usage(&r.usage)).map_err(provider_error)?)
    };
    Ok(Ext::with(
        ResponseW {
            candidates,
            usage_metadata: usage,
            model_version: (!r.extras.form_flag(D, FORM_NO_MODEL)).then(|| r.model.clone()),
            response_id: (!r.extras.form_flag(D, FORM_NO_ID)).then(|| r.id.clone()),
        },
        extra_for(&r.extras, D),
        verbatim_from_form(&r.extras, D, ""),
    ))
}
