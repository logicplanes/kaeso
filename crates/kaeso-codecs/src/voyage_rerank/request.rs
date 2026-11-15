use kaeso_kpp::{KppError, Origin, Request, RequestKind, RerankDocument, RerankRequest};

use super::wire::RerankRequestWire;
use super::{DIALECT, RERANK_PATH, not_rerank};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList};
use crate::cross::check_foreign;
use crate::wire::{Ext, extra_for, extras_from, to_body, verbatim_from_form};

const FIELDS: [&str; 5] = ["model", "query", "documents", "top_k", "return_documents"];

pub(super) fn decode(
    kind: RequestKind,
    body: &[u8],
    _headers: &HeaderList,
) -> Result<Request, KppError> {
    if kind != RequestKind::Rerank {
        return Err(not_rerank(
            kind,
            &format!("{kind:?} request").to_lowercase(),
        ));
    }
    let origin = Origin::new(DIALECT);
    let w = Ext::<RerankRequestWire>::from_slice(body).map_err(|e| {
        let text = crate::wire::without_position(&e);
        let err = KppError::invalid_request(format!(
            "The request body is not a valid {} request: {text}.",
            DIALECT.display_name()
        ));
        match FIELDS
            .into_iter()
            .find(|f| text.contains(&format!("`{f}`")))
        {
            Some(f) => err.with_param(f),
            None => err,
        }
    })?;
    let documents = w.documents.iter().map(RerankDocument::new).collect();
    let mut req = RerankRequest::new(w.model.clone(), w.query.clone(), documents);
    req.top_n = w.top_k;
    req.return_documents = w.return_documents;
    req.extras = extras_from(&origin, w.extra.clone(), w.verbatim.clone());
    req.validate()?;
    Ok(Request::Rerank(req))
}

pub(super) fn check(req: &Request, target: &Origin) -> Result<(), KppError> {
    let Request::Rerank(r) = req else {
        return Err(not_rerank(
            req.kind(),
            &format!("{:?} request", req.kind()).to_lowercase(),
        ));
    };
    // Voyage AI takes documents as strings only and refuses every field it doesn't know
    // (live 400s, 2026-11-14): `top_n` and `max_tokens_per_doc` are "not supported by our API",
    // and an object document "is not a valid string".
    for (i, d) in r.documents.iter().enumerate() {
        if let Some(key) = d.extras.fields.keys().next() {
            return Err(KppError::unsupported(
                "document fields",
                format!(
                    "Voyage AI takes each document as text only; remove `{key}` from `documents[{i}]`."
                ),
            )
            .with_param(format!("documents[{i}]")));
        }
    }
    if r.max_tokens_per_doc.is_some() {
        return Err(KppError::unsupported(
            "max_tokens_per_doc",
            "`max_tokens_per_doc` is not supported by Voyage AI, which cuts documents at the model's limit; remove it.",
        )
        .with_param("max_tokens_per_doc"));
    }
    check_foreign(req, target)
}

pub(super) fn encode(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    check(req, &ctx.origin(DIALECT))?;
    let Request::Rerank(r) = req else {
        return Err(not_rerank(req.kind(), "request"));
    };
    let w = RerankRequestWire {
        query: r.query.clone(),
        documents: r.documents.iter().map(|d| d.text.clone()).collect(),
        model: r.model.clone(),
        top_k: r.top_n,
        return_documents: r.return_documents,
    };
    let body = to_body(&Ext::with(
        w,
        extra_for(&r.extras, DIALECT),
        verbatim_from_form(&r.extras, DIALECT, ""),
    ))?;
    Ok(EncodedRequest::post_json(
        crate::codec::under_api_root(ctx, RERANK_PATH),
        HeaderList::new().with("content-type", "application/json"),
        body,
    ))
}
