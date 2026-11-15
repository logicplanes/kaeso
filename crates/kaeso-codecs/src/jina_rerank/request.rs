use kaeso_kpp::{
    Extras, KppError, Origin, RawJson, Request, RequestKind, RerankDocument, RerankRequest,
};
use serde_json::Value;

use super::wire::{DocumentWire, RerankRequestWire};
use super::{DIALECT, RERANK_PATH, not_rerank};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList};
use crate::cross::check_foreign;
use crate::wire::{Ext, extra_for, extras_from, to_body, verbatim_from_form};

/// The form key that marks a document the app wrote as an object.
pub(super) const OBJECT: &str = "object";

const FIELDS: [&str; 6] = [
    "model",
    "query",
    "documents",
    "top_n",
    "return_documents",
    "max_doc_length",
];

// `max_doc_length` exists for these models only (`RerankerV3Request`); the other rerankers'
// schemas have no counterpart of Cohere's `max_tokens_per_doc`.
const MAX_DOC_LENGTH_MODELS: [&str; 2] = ["jina-reranker-v3", "jina-reranker-v3.5"];

fn invalid(message: String, param: &str) -> KppError {
    KppError::invalid_request(message).with_param(param)
}

fn decode_document(raw: &RawJson, i: usize, origin: &Origin) -> Result<RerankDocument, KppError> {
    let param = format!("documents[{i}]");
    let shape = || {
        invalid(
            format!("`{param}` must be a string or an object with a string `text`."),
            &param,
        )
    };
    match raw.get().as_bytes().first() {
        Some(b'"') => raw
            .parse::<String>()
            .map(RerankDocument::new)
            .map_err(|_| shape()),
        Some(b'{') => {
            let w = Ext::<DocumentWire>::from_text(raw.get()).map_err(|_| shape())?;
            let mut doc = RerankDocument::new(w.known.text);
            doc.extras = extras_from(origin, w.extra, w.verbatim);
            doc.extras.form.insert(OBJECT.to_owned(), true.into());
            Ok(doc)
        }
        _ => Err(shape()),
    }
}

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
    let documents = w
        .documents
        .iter()
        .enumerate()
        .map(|(i, d)| decode_document(d, i, &origin))
        .collect::<Result<Vec<_>, _>>()?;
    let mut req = RerankRequest::new(w.model.clone(), w.query.clone(), documents);
    req.top_n = w.top_n;
    // Jina AI returns the documents unless told not to.
    req.return_documents = Some(w.return_documents.unwrap_or(true));
    req.max_tokens_per_doc = w.max_doc_length;
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
    // Jina AI's text rerankers take a document as a string or `{"text"}` (`TextDoc`), so a field
    // beside `text` has nowhere to go.
    for (i, d) in r.documents.iter().enumerate() {
        if let Some(key) = d.extras.fields.keys().next() {
            return Err(KppError::unsupported(
                "document fields",
                format!(
                    "Jina AI takes each document as text only; remove `{key}` from `documents[{i}]`."
                ),
            )
            .with_param(format!("documents[{i}]")));
        }
    }
    if r.max_tokens_per_doc.is_some() && !MAX_DOC_LENGTH_MODELS.contains(&r.model.as_str()) {
        return Err(KppError::unsupported(
            "max_tokens_per_doc",
            format!(
                "`max_tokens_per_doc` is supported by Jina AI's `jina-reranker-v3` and `jina-reranker-v3.5` only, not `{}`; remove it.",
                r.model
            ),
        )
        .with_param("max_tokens_per_doc"));
    }
    check_foreign(req, target)
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
    RawJson::from_serialize(&Value::String(d.text.clone())).map_err(ser)
}

pub(super) fn encode(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    check(req, &ctx.origin(DIALECT))?;
    let Request::Rerank(r) = req else {
        return Err(not_rerank(req.kind(), "request"));
    };
    let w = RerankRequestWire {
        model: r.model.clone(),
        query: r.query.clone(),
        documents: r
            .documents
            .iter()
            .map(document_value)
            .collect::<Result<_, _>>()?,
        top_n: r.top_n,
        // Always sent: Jina AI's default is true, an app's that doesn't say is false.
        return_documents: Some(r.returns_documents()),
        max_doc_length: r.max_tokens_per_doc,
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

/// Whether the app wrote a document as an object, so it is sent and answered as one.
pub(super) fn written_as_object(extras: &Extras) -> bool {
    extras.form_flag(DIALECT, OBJECT)
}
