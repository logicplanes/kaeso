use kaeso_kpp::{
    Extras, KppError, Origin, RawJson, RawUsage, Reply, Request, RequestKind, RerankDocument,
    RerankResponse, RerankResult, Usage,
};
use serde_json::{Value, json};

use super::request::{OBJECT, written_as_object};
use super::wire::{DocumentWire, MetaWire, RerankResponseWire, ResultWire};
use super::{DIALECT, not_rerank};
use crate::codec::{EgressCtx, EncodeCtx, HeaderList};
use crate::wire::{Ext, extra_for, extras_from, parse_provider, to_body, verbatim_from_form};

/// The form key that marks a reply without `meta`.
const NO_META: &str = "no_meta";

fn bad(msg: &str) -> KppError {
    KppError::provider(format!(
        "The provider sent a response that is not a valid {} response: {msg}.",
        DIALECT.display_name()
    ))
}

// Cohere documents its billed units as numbers that may have a fraction; whole units are billed.
fn units(n: Option<f64>) -> Option<u64> {
    n.filter(|n| n.is_finite() && *n >= 0.0)
        .map(|n| n.ceil() as u64)
}

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
    let mut resp = RerankResponse::new(String::new(), results);
    resp.id = w.id;
    if let Some(meta) = &w.meta {
        let read: MetaWire = meta.parse().map_err(|_| bad("`meta` is not an object"))?;
        let billed = read.billed_units.unwrap_or_default();
        let mut usage = Usage::new(units(billed.input_tokens).unwrap_or(0), 0);
        usage.raw = meta
            .to_value()
            .ok()
            .map(|v| RawUsage::new(origin.clone(), v));
        resp.usage = usage;
        resp.search_units = units(billed.search_units);
    }
    let no_meta = w.meta.is_none() && !verbatim.contains_key("meta");
    let mut extras: Extras = extras_from(&origin, extra, verbatim);
    if no_meta {
        extras.form.insert(NO_META.to_owned(), true.into());
    }
    resp.extras = extras;
    Ok(Reply::Rerank(resp))
}

// `meta` as Cohere writes it: the provider's own when it was Cohere, else the units the reply
// reports, under the API version this endpoint answers as.
fn meta_value(r: &RerankResponse) -> Value {
    if let Some(raw) = r.usage.raw_for(DIALECT) {
        return raw.clone();
    }
    let mut billed = serde_json::Map::new();
    let tokens = r.usage.total_input_tokens();
    if tokens > 0 || r.search_units.is_none() {
        billed.insert("input_tokens".to_owned(), tokens.into());
    }
    if let Some(units) = r.search_units {
        billed.insert("search_units".to_owned(), units.into());
    }
    json!({"api_version": {"version": "2"}, "billed_units": billed})
}

fn document_value(d: &RerankDocument) -> Result<RawJson, KppError> {
    let ser = |e: serde_json::Error| {
        KppError::provider(format!("Kaeso could not serialize a document ({e})."))
    };
    if d.extras.is_from(DIALECT) && !written_as_object(&d.extras) {
        return RawJson::from_serialize(&d.text).map_err(ser);
    }
    let w = Ext::with(
        DocumentWire {
            text: d.text.clone(),
        },
        extra_for(&d.extras, DIALECT),
        verbatim_from_form(&d.extras, DIALECT, ""),
    );
    RawJson::from_serialize(&w).map_err(ser)
}

pub(super) fn encode(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
    let Reply::Rerank(r) = reply else {
        return Err(not_rerank(
            reply.kind(),
            &format!("{:?} response", reply.kind()).to_lowercase(),
        ));
    };
    let asked = match ctx.request.as_deref() {
        Some(Request::Rerank(req)) if req.returns_documents() => Some(&req.documents),
        _ => None,
    };
    let results = r
        .results
        .iter()
        .map(|res| {
            // A provider that answers without the documents an app asked for, as Cohere's v2
            // API always does, gets them from the app's own request.
            let document = match (&res.document, asked) {
                (Some(d), _) => Some(document_value(d)?),
                (None, Some(docs)) => match docs.get(res.index as usize) {
                    Some(d) => Some(document_value(&RerankDocument::new(d.text.clone()))?),
                    None => None,
                },
                (None, None) => None,
            };
            let w = ResultWire {
                index: res.index,
                relevance_score: res.relevance_score,
                document,
            };
            Ok(Ext::with(
                w,
                extra_for(&res.extras, DIALECT),
                verbatim_from_form(&res.extras, DIALECT, ""),
            ))
        })
        .collect::<Result<Vec<_>, KppError>>()?;
    let verbatim = verbatim_from_form(&r.extras, DIALECT, "");
    let meta = if verbatim.contains_key("meta") || r.extras.form_flag(DIALECT, NO_META) {
        None
    } else {
        Some(RawJson::from_serialize(&meta_value(r)).map_err(|e| {
            KppError::provider(format!("Kaeso could not serialize the usage ({e})."))
        })?)
    };
    let w = RerankResponseWire {
        results,
        id: r.id.clone(),
        meta,
    };
    to_body(&Ext::with(w, extra_for(&r.extras, DIALECT), verbatim))
}
