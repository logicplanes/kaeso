use kaeso_kpp::{
    Extras, KppError, RawJson, RawUsage, Reply, RequestKind, RerankDocument, RerankResponse,
    RerankResult, Usage,
};
use serde_json::{Value, json};

use super::wire::{RerankResponseWire, ResultWire, UsageWire};
use super::{DIALECT, not_rerank};
use crate::codec::{EgressCtx, EncodeCtx, HeaderList};
use crate::wire::{Ext, extra_for, extras_from, parse_provider, to_body, verbatim_from_form};

const OBJECT: &str = "object";
const NO_USAGE: &str = "no_usage";

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
    let mut results = Vec::with_capacity(w.data.len());
    for item in w.data {
        let Ext {
            known: item,
            extra: item_extra,
            verbatim: item_verbatim,
        } = item;
        let mut r = RerankResult::new(item.index, item.relevance_score);
        r.document = item.document.map(RerankDocument::new);
        r.extras = extras_from(&origin, item_extra, item_verbatim);
        results.push(r);
    }
    let mut resp = RerankResponse::new(w.model, results);
    if let Some(u) = &w.usage {
        let parsed: UsageWire = u
            .parse()
            .map_err(|_| bad("`usage.total_tokens` is missing"))?;
        // Voyage AI bills the query once per document plus every document, and reports that
        // sum as `total_tokens` (https://www.mongodb.com/docs/voyageai/management/billing/).
        let mut usage = Usage::new(parsed.total_tokens, 0);
        usage.raw = u.to_value().ok().map(|v| RawUsage::new(origin.clone(), v));
        resp.usage = usage;
    }
    let no_usage = w.usage.is_none() && !verbatim.contains_key("usage");
    let mut extras: Extras = extras_from(&origin, extra, verbatim);
    if no_usage {
        extras.form.insert(NO_USAGE.to_owned(), true.into());
    }
    if w.object != "list" {
        extras.form.insert(OBJECT.to_owned(), w.object.into());
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

pub(super) fn encode(reply: &Reply, _ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
    let Reply::Rerank(r) = reply else {
        return Err(not_rerank(
            reply.kind(),
            &format!("{:?} response", reply.kind()).to_lowercase(),
        ));
    };
    let data = r
        .results
        .iter()
        .map(|res| {
            Ext::with(
                ResultWire {
                    index: res.index,
                    relevance_score: res.relevance_score,
                    document: res.document.as_ref().map(|d| d.text.clone()),
                },
                extra_for(&res.extras, DIALECT),
                verbatim_from_form(&res.extras, DIALECT, ""),
            )
        })
        .collect();
    let verbatim = verbatim_from_form(&r.extras, DIALECT, "");
    let usage = if verbatim.contains_key("usage") || r.extras.form_flag(DIALECT, NO_USAGE) {
        None
    } else {
        Some(RawJson::from_serialize(&usage_value(r)).map_err(|e| {
            KppError::provider(format!("Kaeso could not serialize the usage ({e})."))
        })?)
    };
    let w = RerankResponseWire {
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
