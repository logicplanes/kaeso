use kaeso_kpp::{
    CacheWriteBreakdown, CountTokensResponse, KppError, Origin, RawJson, RawUsage, Reply, Response,
    StopReason, Usage,
};
use serde_json::value::RawValue;
use serde_json::{Map, Value, json};

use super::blocks::{BlockEncoder, Dec, Side, decode_block, own_ext};
use super::common::{
    D, record_spelling, request_kind_name, spelling, stop_from_wire, stop_to_wire, to_raw,
};
use super::error::decode_error;
use super::wire::{CountTokensRespW, MessageRespW, UsageW};
use crate::codec::{EgressCtx, EncodeCtx, HeaderList};
use crate::wire::without_position;
use crate::wire::{Ext, Maybe, extra_for, extras_from, to_body, verbatim_from_form};
use kaeso_kpp::{Extras, RequestKind};

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
            let w: Ext<CountTokensRespW> =
                Ext::from_slice(body).map_err(|e| provider_error(without_position(&e)))?;
            let mut r = CountTokensResponse::new(w.known.input_tokens);
            r.extras = extras_from(&origin, w.extra, w.verbatim);
            Ok(Reply::CountTokens(r))
        }
        RequestKind::Generate => {
            let w: Ext<MessageRespW> =
                Ext::from_slice(body).map_err(|e| provider_error(without_position(&e)))?;
            let Ext {
                known: w,
                extra,
                verbatim,
            } = w;
            let dec = Dec::new(&origin, Some(ctx));
            let blocks = w
                .content
                .iter()
                .map(|b| decode_block(b, dec))
                .collect::<Result<Vec<_>, _>>()
                .map_err(provider_error)?;
            let mut extras = extras_from(&origin, extra, verbatim);
            let stop = decode_stop(&mut extras, "", w.stop_reason, w.stop_sequence, false);
            let mut r = Response::new(w.id, w.model, blocks);
            r.stop = stop.unwrap_or_default();
            r.usage = decode_usage(&w.usage, &origin).map_err(provider_error)?;
            r.extras = extras;
            Ok(Reply::Generate(r))
        }
        other => Err(KppError::unsupported_by(
            request_kind_name(other).replace("request", "response"),
            D,
            "Anthropic Messages serves message generation (`/v1/messages`) and token counting (`/v1/messages/count_tokens`) only.",
        )),
    }
}

fn provider_error(e: String) -> KppError {
    KppError::provider(format!(
        "The provider sent a response that is not a valid Anthropic Messages response: {e}."
    ))
}

pub(crate) fn decode_stop(
    extras: &mut Extras,
    scope: &str,
    reason: Maybe<String>,
    sequence: Maybe<String>,
    nullable: bool,
) -> Option<StopReason> {
    let stop = reason
        .as_option()
        .map(|r| stop_from_wire(r, sequence.as_option().map(String::as_str)));
    match (&reason, nullable) {
        (Maybe::Value(r), _) => {
            if stop.as_ref().map(|s| stop_to_wire(s, true).0).as_deref() != Some(r.as_str()) {
                record_spelling(extras, scope, "stop_reason", Some(Value::String(r.clone())));
            }
        }
        (Maybe::Null, true) => {}
        (Maybe::Null, false) => record_spelling(extras, scope, "stop_reason", Some(Value::Null)),
        (Maybe::Absent, _) => record_spelling(extras, scope, "stop_reason", None),
    }
    let expected = match &stop {
        Some(StopReason::StopSequence(s)) => Maybe::Value(s.clone()),
        _ => Maybe::Null,
    };
    if sequence != expected {
        let actual = match sequence {
            Maybe::Absent => None,
            Maybe::Null => Some(Value::Null),
            Maybe::Value(s) => Some(Value::String(s)),
        };
        record_spelling(extras, scope, "stop_sequence", actual);
    }
    stop
}

pub(crate) fn encode_stop(
    stop: Option<&StopReason>,
    extras: &Extras,
    scope: &str,
) -> (Maybe<String>, Maybe<String>) {
    let own = extras.is_from(D);
    let wire = stop.map(|s| stop_to_wire(s, own));
    let reason = spelled(extras, scope, "stop_reason").unwrap_or_else(|| {
        wire.as_ref()
            .map_or(Maybe::Null, |(r, _)| Maybe::Value(r.clone()))
    });
    let sequence = spelled(extras, scope, "stop_sequence").unwrap_or_else(|| {
        match wire.and_then(|(_, s)| s) {
            Some(s) => Maybe::Value(s),
            None => Maybe::Null,
        }
    });
    (reason, sequence)
}

fn spelled(extras: &Extras, scope: &str, field: &str) -> Option<Maybe<String>> {
    Some(match spelling(extras, scope, field)? {
        None => Maybe::Absent,
        Some(Value::String(s)) => Maybe::Value(s),
        Some(_) => Maybe::Null,
    })
}

pub(crate) fn decode_usage(raw: &RawValue, origin: &Origin) -> Result<Usage, String> {
    decode_usage_with(raw, origin, None)
}

pub(crate) fn decode_usage_with(
    raw: &RawValue,
    origin: &Origin,
    base: Option<&Usage>,
) -> Result<Usage, String> {
    let w: UsageW = serde_json::from_str(raw.get())
        .map_err(|e| format!("invalid `usage`: {}", without_position(&e)))?;
    let value: Value = serde_json::from_str(raw.get()).map_err(|e| without_position(&e))?;
    let or_base = |v: Option<u64>, f: fn(&Usage) -> u64| v.or(base.map(f)).unwrap_or(0);
    let mut u = Usage::new(
        or_base(w.input_tokens, |b| b.input_tokens),
        or_base(w.output_tokens, |b| b.output_tokens),
    );
    u.cache_read_tokens = or_base(w.cache_read_input_tokens, |b| b.cache_read_tokens);
    u.cache_write_tokens = or_base(w.cache_creation_input_tokens, |b| b.cache_write_tokens);
    u.cache_write_breakdown = w
        .cache_creation
        .map(|c| CacheWriteBreakdown::new(c.ephemeral_5m_input_tokens, c.ephemeral_1h_input_tokens))
        .or_else(|| base.and_then(|b| b.cache_write_breakdown));
    u.reasoning_tokens = w.output_tokens_details.and_then(|d| d.thinking_tokens);
    u.raw = Some(RawUsage::new(origin.clone(), value));
    Ok(u)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum UsageShape {
    Full,
    Delta,
}

pub(crate) fn encode_usage(u: &Usage, shape: UsageShape) -> Value {
    if let Some(raw) = u.raw_for(D) {
        return raw.clone();
    }
    let mut m = Map::new();
    m.insert("input_tokens".into(), u.input_tokens.into());
    m.insert(
        "cache_creation_input_tokens".into(),
        u.cache_write_tokens.into(),
    );
    m.insert("cache_read_input_tokens".into(), u.cache_read_tokens.into());
    if shape == UsageShape::Full {
        m.insert(
            "cache_creation".into(),
            u.cache_write_breakdown.map_or(Value::Null, |b| {
                json!({
                    "ephemeral_5m_input_tokens": b.ephemeral_5m,
                    "ephemeral_1h_input_tokens": b.ephemeral_1h,
                })
            }),
        );
    }
    m.insert("output_tokens".into(), u.output_tokens.into());
    if let Some(r) = u.reasoning_tokens {
        m.insert(
            "output_tokens_details".into(),
            json!({ "thinking_tokens": r }),
        );
    }
    Value::Object(m)
}

pub(crate) fn encode_response(reply: &Reply, _ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
    match reply {
        Reply::Generate(r) => to_body(&encode_message(r)?),
        Reply::CountTokens(c) => to_body(&own_ext(
            CountTokensRespW {
                input_tokens: c.input_tokens,
            },
            &c.extras,
        )),
        other => Err(KppError::unsupported_by(
            request_kind_name(other.kind()).replace("request", "response"),
            D,
            "Anthropic Messages serves message generation (`/v1/messages`) and token counting (`/v1/messages/count_tokens`) only.",
        )),
    }
}

fn encode_message(r: &Response) -> Result<Ext<MessageRespW>, KppError> {
    let own = r.extras.is_from(D);
    let mut enc = BlockEncoder::new(Origin::new(D), Side::Reply);
    let content = enc.blocks(&r.blocks)?;
    let (stop_reason, stop_sequence) = encode_stop(Some(&r.stop), &r.extras, "");
    let usage = to_raw(&encode_usage(&r.usage, UsageShape::Full))?;
    let mut extra = extra_for(&r.extras, D);
    if !own {
        add_default(&mut extra, "type", "\"message\"");
        add_default(&mut extra, "role", "\"assistant\"");
        add_default(&mut extra, "stop_details", "null");
    }
    Ok(Ext::with(
        MessageRespW {
            id: r.id.clone(),
            model: r.model.clone(),
            content,
            stop_reason,
            stop_sequence,
            usage,
        },
        extra,
        verbatim_from_form(&r.extras, D, ""),
    ))
}

pub(crate) fn add_default(extra: &mut indexmap::IndexMap<String, RawJson>, key: &str, json: &str) {
    if !extra.contains_key(key)
        && let Ok(v) = RawJson::from_string(json.to_owned())
    {
        extra.insert(key.to_owned(), v);
    }
}
