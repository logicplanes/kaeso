use kaeso_kpp::{
    Extras, KppError, Origin, RawJson, RawUsage, Reply, RequestKind, RerankDocument,
    RerankResponse, RerankResult, Usage,
};
use serde_json::{Value, json};

use super::request::{OBJECT, written_as_object};
use super::wire::{DocumentWire, RerankResponseWire, ResultWire, UsageWire};
use super::{DIALECT, not_rerank};
use crate::codec::{EgressCtx, EncodeCtx, HeaderList};
use crate::wire::{Ext, extra_for, extras_from, parse_provider, to_body, verbatim_from_form};

/// The form key holding a reply's `object` when it isn't `"list"`.
const OBJECT_NAME: &str = "object_name";
/// The form keys that mark a reply without `object` or `usage`.
const NO_OBJECT: &str = "no_object";
const NO_USAGE: &str = "no_usage";

fn bad(msg: &str) -> KppError {
    KppError::provider(format!(
        "The provider sent a response that is not a valid {} response: {msg}.",
        DIALECT.display_name()
    ))
}

// Jina AI echoes a document in the form it was sent: a string or `{"text"}`. An `{"image"}` comes
// back only for an image Kaeso never sends.
fn decode_document(raw: &RawJson, origin: &Origin) -> Result<RerankDocument, KppError> {
    match raw.get().as_bytes().first() {
        Some(b'"') => raw
            .parse::<String>()
            .map(RerankDocument::new)
            .map_err(|e| bad(&e.to_string())),
        Some(b'{') => {
            let w = Ext::<DocumentWire>::from_text(raw.get())
                .map_err(|_| bad("a `document` has no string `text`"))?;
            let mut doc = RerankDocument::new(w.known.text);
            doc.extras = extras_from(origin, w.extra, w.verbatim);
            doc.extras.form.insert(OBJECT.to_owned(), true.into());
            Ok(doc)
        }
        _ => Err(bad(
            "a `document` must be a string or an object with `text`",
        )),
    }
}

pub(super) fn decode(
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<Reply, KppError> {
    if !(200..300).contains(&status) {
        return Err(super::error::decode(status, headers, body));
    }
    if ctx.kind != RequestKind::Rerank {
        return Err(not_rerank(ctx.kind, "response"));
    }
    let origin = ctx.origin(DIALECT);
    let Ext {
        known: w,
        extra,
        verbatim,
    } = parse_provider::<RerankResponseWire>(body, DIALECT)?;
    let mut results = Vec::with_capacity(w.results.len());
    for item in w.results {
        let Ext {
            known: item,
            extra: item_extra,
            verbatim: item_verbatim,
        } = item;
        let mut r = RerankResult::new(item.index, item.relevance_score);
        if let Some(doc) = &item.document {
            r.document = Some(decode_document(doc, &origin)?);
        }
        r.extras = extras_from(&origin, item_extra, item_verbatim);
        results.push(r);
    }
    let mut resp = RerankResponse::new(w.model, results);
    if let Some(u) = &w.usage {
        let parsed: UsageWire = u
            .parse()
            .map_err(|_| bad("`usage.total_tokens` is missing"))?;
        let mut usage = Usage::new(parsed.total_tokens, 0);
        usage.raw = u.to_value().ok().map(|v| RawUsage::new(origin.clone(), v));
        resp.usage = usage;
    }
    let no_usage = w.usage.is_none() && !verbatim.contains_key("usage");
    let no_object = w.object.is_none() && !verbatim.contains_key("object");
    let mut extras: Extras = extras_from(&origin, extra, verbatim);
    if no_usage {
        extras.form.insert(NO_USAGE.to_owned(), true.into());
    }
    if no_object {
        extras.form.insert(NO_OBJECT.to_owned(), true.into());
    }
    if let Some(o) = w.object.filter(|o| o != "list") {
        extras.form.insert(OBJECT_NAME.to_owned(), o.into());
    }
    resp.extras = extras;
    Ok(Reply::Rerank(resp))
}

fn usage_value(r: &RerankResponse) -> Value {
    if let Some(raw) = r.usage.raw_for(DIALECT) {
        return raw.clone();
    }
    json!({"total_tokens": r.usage.total_input_tokens()})
}

fn document_value(d: &RerankDocument) -> Result<RawJson, KppError> {
    let ser = |e: serde_json::Error| {
        KppError::provider(format!("Kaeso could not serialize a document ({e})."))
    };
    if d.extras.is_from(DIALECT) && written_as_object(&d.extras) {
        let w = Ext::with(
            DocumentWire {
                text: d.text.clone(),
            },
            extra_for(&d.extras, DIALECT),
            verbatim_from_form(&d.extras, DIALECT, ""),
        );
        return RawJson::from_serialize(&w).map_err(ser);
    }
    RawJson::from_serialize(&d.text).map_err(ser)
}

pub(super) fn encode(reply: &Reply, _ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
    let Reply::Rerank(r) = reply else {
        return Err(not_rerank(
            reply.kind(),
            &format!("{:?} response", reply.kind()).to_lowercase(),
        ));
    };
    let results = r
        .results
        .iter()
        .map(|res| {
            Ok(Ext::with(
                ResultWire {
                    index: res.index,
                    relevance_score: res.relevance_score,
                    document: res.document.as_ref().map(document_value).transpose()?,
                },
                extra_for(&res.extras, DIALECT),
                verbatim_from_form(&res.extras, DIALECT, ""),
            ))
        })
        .collect::<Result<Vec<_>, KppError>>()?;
    let verbatim = verbatim_from_form(&r.extras, DIALECT, "");
    let usage = if verbatim.contains_key("usage") || r.extras.form_flag(DIALECT, NO_USAGE) {
        None
    } else {
        Some(RawJson::from_serialize(&usage_value(r)).map_err(|e| {
            KppError::provider(format!("Kaeso could not serialize the usage ({e})."))
        })?)
    };
    let object = if verbatim.contains_key("object") || r.extras.form_flag(DIALECT, NO_OBJECT) {
        None
    } else {
        Some(
            r.extras
                .form_str(DIALECT, OBJECT_NAME)
                .unwrap_or("list")
                .to_owned(),
        )
    };
    let w = RerankResponseWire {
        model: r.model.clone(),
        object,
        usage,
        results,
    };
    to_body(&Ext::with(w, extra_for(&r.extras, DIALECT), verbatim))
}
