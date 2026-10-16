use kaeso_kpp::{
    EmbedEncoding, EmbedResponse, Embedding, EmbeddingVector, Extras, KppError, RawJson, RawUsage,
    Reply, Request, RequestKind, Usage,
};
use serde_json::Value;

use super::DIALECT;
use super::wire::{CreateEmbeddingResponse, EmbeddingWire, UsageWire};
use crate::codec::{EgressCtx, EncodeCtx, HeaderList};
use crate::wire::{Ext, extra_for, extras_from, parse_provider, to_body, verbatim_from_form};

const OBJECT: &str = "object";
const NO_USAGE: &str = "no_usage";

fn to_base64(values: &[f64]) -> String {
    use base64::Engine as _;
    let bytes: Vec<u8> = values
        .iter()
        .flat_map(|v| (*v as f32).to_le_bytes())
        .collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn from_base64(data: &str) -> Option<Vec<f64>> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .ok()?;
    let (chunks, rest) = bytes.as_chunks::<4>();
    if !rest.is_empty() {
        return None;
    }
    chunks
        .iter()
        .map(|c| f32::from_le_bytes(*c).to_string().parse::<f64>().ok())
        .collect()
}

fn convert(
    vector: &EmbeddingVector,
    want: Option<EmbedEncoding>,
) -> Result<EmbeddingVector, KppError> {
    Ok(match (vector, want) {
        (EmbeddingVector::Float(v), Some(EmbedEncoding::Base64)) => {
            EmbeddingVector::Base64(to_base64(v))
        }
        (EmbeddingVector::Base64(s), Some(EmbedEncoding::Float)) => EmbeddingVector::Float(
            from_base64(s)
                .ok_or_else(|| bad("a base64 `embedding` is not a list of 32-bit floats"))?,
        ),
        (other, _) => other.clone(),
    })
}

fn bad(msg: &str) -> KppError {
    KppError::provider(format!(
        "The provider sent a response that is not a valid {} response: {msg}.",
        DIALECT.display_name()
    ))
}

pub(super) fn decode(
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<Reply, KppError> {
    if !(200..300).contains(&status) {
        return Err(crate::openai_responses::error::decode(
            DIALECT, status, headers, body,
        ));
    }
    if ctx.kind != RequestKind::Embed {
        return Err(KppError::unsupported_by("generation response", DIALECT, ""));
    }
    let origin = ctx.origin(DIALECT);
    let w = parse_provider::<CreateEmbeddingResponse>(body, DIALECT)?;
    let mut data = Vec::with_capacity(w.data.len());
    for item in &w.data {
        let vector = match item.embedding.to_value().map_err(|e| bad(&e.to_string()))? {
            Value::String(s) => EmbeddingVector::Base64(s),
            Value::Array(_) => EmbeddingVector::Float(
                item.embedding
                    .parse::<Vec<f64>>()
                    .map_err(|_| bad("`embedding` must hold numbers"))?,
            ),
            _ => return Err(bad("`embedding` must be an array or a base64 string")),
        };
        let mut e = Embedding::new(item.index, vector);
        e.extras = extras_from(&origin, item.extra.clone(), item.verbatim.clone());
        if item.object != "embedding" {
            e.extras
                .form
                .insert(OBJECT.to_owned(), item.object.clone().into());
        }
        data.push(e);
    }
    let mut resp = EmbedResponse::new(w.model.clone(), data);
    if let Some(u) = &w.usage {
        let parsed: UsageWire = u
            .parse()
            .map_err(|_| bad("`usage.prompt_tokens` is missing"))?;
        let mut usage = Usage::new(parsed.prompt_tokens, 0);
        usage.raw = u.to_value().ok().map(|v| RawUsage::new(origin.clone(), v));
        resp.usage = usage;
    }
    let mut extras: Extras = extras_from(&origin, w.extra.clone(), w.verbatim.clone());
    if w.usage.is_none() && !w.verbatim.contains_key("usage") {
        extras.form.insert(NO_USAGE.to_owned(), true.into());
    }
    if w.object != "list" {
        extras
            .form
            .insert(OBJECT.to_owned(), w.object.clone().into());
    }
    resp.extras = extras;
    Ok(Reply::Embed(resp))
}

fn usage_value(u: &Usage) -> Value {
    let total = u.total_input_tokens();
    let mut v = serde_json::json!({"prompt_tokens": total, "total_tokens": total});
    if let (Some(Value::Object(raw)), Value::Object(map)) = (u.raw_for(DIALECT), &mut v) {
        for (k, x) in raw {
            map.insert(k.clone(), x.clone());
        }
    }
    v
}

pub(super) fn encode(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
    let want = match ctx.request.as_deref() {
        Some(Request::Embed(r)) => Some(r.encoding),
        _ => None,
    };
    let Reply::Embed(r) = reply else {
        return Err(KppError::unsupported_by(
            "generation response",
            DIALECT,
            "Embeddings apps can only receive embeddings.",
        ));
    };
    let ser = |e: serde_json::Error| {
        KppError::provider(format!("Kaeso could not serialize the embeddings ({e})."))
    };
    let data = r
        .data
        .iter()
        .map(|e| {
            let embedding = match &convert(&e.vector, want)? {
                EmbeddingVector::Float(v) => RawJson::from_serialize(v).map_err(ser)?,
                EmbeddingVector::Base64(s) => RawJson::from_serialize(s).map_err(ser)?,
                _ => {
                    return Err(KppError::unsupported_by(
                        "embedding vector kind",
                        DIALECT,
                        "",
                    ));
                }
            };
            let w = EmbeddingWire {
                object: e
                    .extras
                    .form_str(DIALECT, OBJECT)
                    .unwrap_or("embedding")
                    .to_owned(),
                index: e.index,
                embedding,
            };
            Ok(Ext::with(
                w,
                extra_for(&e.extras, DIALECT),
                verbatim_from_form(&e.extras, DIALECT, ""),
            ))
        })
        .collect::<Result<Vec<_>, KppError>>()?;
    let verbatim = verbatim_from_form(&r.extras, DIALECT, "");
    let usage = if verbatim.contains_key("usage") || r.extras.form_flag(DIALECT, NO_USAGE) {
        None
    } else {
        Some(RawJson::from_serialize(&usage_value(&r.usage)).map_err(ser)?)
    };
    let w = CreateEmbeddingResponse {
        object: r
            .extras
            .form_str(DIALECT, OBJECT)
            .unwrap_or("list")
            .to_owned(),
        data,
        model: r.model.clone(),
        usage,
    };
    to_body(&Ext::with(w, extra_for(&r.extras, DIALECT), verbatim))
}
