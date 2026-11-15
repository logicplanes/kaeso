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
    "max_tokens_per_doc",
    "return_documents",
];

fn invalid(message: String, param: &str) -> KppError {
    KppError::invalid_request(message).with_param(param)
}

fn decode_document(raw: &RawJson, i: usize, origin: &Origin) -> Result<RerankDocument, KppError> {
    let param = format!("documents[{i}]");
    match raw.get().as_bytes().first() {
        Some(b'"') => raw
            .parse::<String>()
            .map(RerankDocument::new)
            .map_err(|e| invalid(format!("`{param}` is not a valid string ({e})."), &param)),
        Some(b'{') => {
            let w = Ext::<DocumentWire>::from_text(raw.get()).map_err(|_| {
                invalid(
                    format!("`{param}` must be a string or an object with a string `text`."),
                    &param,
                )
            })?;
            let mut doc = RerankDocument::new(w.known.text);
            doc.extras = extras_from(origin, w.extra, w.verbatim);
            doc.extras.form.insert(OBJECT.to_owned(), true.into());
            Ok(doc)
        }
        _ => Err(invalid(
            format!("`{param}` must be a string or an object with a string `text`."),
            &param,
        )),
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
    req.max_tokens_per_doc = w.max_tokens_per_doc;
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
    for (i, d) in r.documents.iter().enumerate() {
        // Cohere's v2 API takes each document as a string, so a field beside `text` has
        // nowhere to go (https://docs.cohere.com/reference/rerank).
        if let Some(key) = d.extras.fields.keys().next() {
            return Err(KppError::unsupported(
                "document fields",
                format!(
                    "Cohere's Rerank API takes each document as text only; remove `{key}` from `documents[{i}]`."
                ),
            )
            .with_param(format!("documents[{i}]")));
        }
    }
    check_foreign(req, target)
}

fn body(r: &RerankRequest) -> Result<Vec<u8>, KppError> {
    let documents = r
        .documents
        .iter()
        .map(|d| RawJson::from_serialize(&Value::String(d.text.clone())))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| KppError::provider(format!("Kaeso could not serialize a document ({e}).")))?;
    let w = RerankRequestWire {
        model: r.model.clone(),
        query: r.query.clone(),
        documents,
        top_n: r.top_n,
        max_tokens_per_doc: r.max_tokens_per_doc,
        // Cohere's v2 API has no `return_documents`; the documents an app asks for are its
        // own, written into the reply by the app's codec.
        return_documents: None,
    };
    let mut verbatim = verbatim_from_form(&r.extras, DIALECT, "");
    verbatim.shift_remove("return_documents");
    to_body(&Ext::with(w, extra_for(&r.extras, DIALECT), verbatim))
}

pub(super) fn encode(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    check(req, &ctx.origin(DIALECT))?;
    let Request::Rerank(r) = req else {
        return Err(not_rerank(req.kind(), "request"));
    };
    let headers = HeaderList::new().with("content-type", "application/json");
    Ok(EncodedRequest::post_json(
        crate::codec::under_api_root(ctx, RERANK_PATH),
        headers,
        body(r)?,
    ))
}

/// Whether the app wrote a document as an object, so a reply writes it back as one.
pub(super) fn written_as_object(extras: &Extras) -> bool {
    extras.form_flag(DIALECT, OBJECT)
}
