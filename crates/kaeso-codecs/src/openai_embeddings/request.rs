use kaeso_kpp::{
    EmbedEncoding, EmbedInput, EmbedRequest, Extras, KppError, Origin, RawJson, Request,
    RequestKind,
};
use serde_json::Value;

use super::DIALECT;
use super::EmbeddingsProfile;
use super::wire::CreateEmbeddingRequest;
use crate::codec::{EgressCtx, EncodedRequest, HeaderList, is_forwarded_header};
use crate::cross::check_foreign;
use crate::wire::{Ext, extra_for, extras_from, to_body, verbatim_from_form};

const INPUT: &str = "input";
const ENCODING: &str = "encoding_format";

fn invalid(msg: &str) -> KppError {
    KppError::invalid_request(format!(
        "The request body is not a valid {} request: {msg}.",
        DIALECT.display_name()
    ))
    .with_param("input")
}

fn decode_input(raw: &RawJson) -> Result<(Vec<EmbedInput>, Option<&'static str>), KppError> {
    let value = raw.to_value().map_err(|_| invalid("`input` is not JSON"))?;
    let token = |v: &Value| v.as_u64().and_then(|n| u32::try_from(n).ok());
    let tokens = |v: &Value| -> Option<Vec<u32>> { v.as_array()?.iter().map(token).collect() };
    match &value {
        Value::String(s) => Ok((vec![EmbedInput::Text(s.clone())], None)),
        Value::Array(items) if items.is_empty() => Ok((Vec::new(), None)),
        Value::Array(items) if items.iter().all(Value::is_string) => {
            let inputs = items
                .iter()
                .filter_map(|v| v.as_str().map(|s| EmbedInput::Text(s.to_owned())))
                .collect::<Vec<_>>();
            let marker = (inputs.len() == 1).then_some("array");
            Ok((inputs, marker))
        }
        Value::Array(items) if items.iter().all(Value::is_number) => {
            let t =
                tokens(&value).ok_or_else(|| invalid("token ids must be non-negative integers"))?;
            Ok((vec![EmbedInput::Tokens(t)], None))
        }
        Value::Array(items) if items.iter().all(Value::is_array) => {
            let inputs = items
                .iter()
                .map(|v| tokens(v).map(EmbedInput::Tokens))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| invalid("token ids must be non-negative integers"))?;
            let marker = (inputs.len() == 1).then_some("nested");
            Ok((inputs, marker))
        }
        _ => Err(invalid(
            "`input` must be a string, an array of strings, an array of token ids or an array of token-id arrays",
        )),
    }
}

fn encode_input(inputs: &[EmbedInput], marker: Option<&str>) -> Result<RawJson, KppError> {
    let value = match inputs {
        [EmbedInput::Text(s)] if marker != Some("array") => Value::String(s.clone()),
        [EmbedInput::Tokens(t)] if marker != Some("nested") => Value::from(t.clone()),
        _ => Value::Array(
            inputs
                .iter()
                .map(|i| match i {
                    EmbedInput::Text(s) => Value::String(s.clone()),
                    EmbedInput::Tokens(t) => Value::from(t.clone()),
                    _ => Value::Null,
                })
                .collect(),
        ),
    };
    RawJson::from_serialize(&value)
        .map_err(|e| KppError::provider(format!("Kaeso could not serialize the input ({e}).")))
}

pub(super) fn decode(
    kind: RequestKind,
    body: &[u8],
    headers: &HeaderList,
) -> Result<Request, KppError> {
    if kind != RequestKind::Embed {
        return Err(KppError::unsupported_by(
            "generation request",
            DIALECT,
            "Send generation requests to a chat or responses endpoint.",
        ));
    }
    let origin = Origin::new(DIALECT);
    let w = Ext::<CreateEmbeddingRequest>::from_slice(body).map_err(|e| {
        let text = e.to_string();
        let text = text.split(" at line ").next().unwrap_or(&text).to_owned();
        let field = ["model", "dimensions", "user", "encoding_format", "input"]
            .into_iter()
            .find(|f| text.contains(&format!("`{f}`")));
        let err = KppError::invalid_request(format!(
            "The request body is not a valid {} request: {text}.",
            DIALECT.display_name()
        ));
        match field {
            Some(f) => err.with_param(f),
            None => err,
        }
    })?;
    let (input, marker) = decode_input(&w.input)?;
    let mut req = EmbedRequest::new(w.model.clone(), input);
    req.dimensions = w.dimensions;
    req.user = w.user.clone();
    req.encoding = match w.encoding_format.as_deref() {
        None | Some("float") => EmbedEncoding::Float,
        Some("base64") => EmbedEncoding::Base64,
        Some(other) => {
            return Err(KppError::invalid_request(format!(
                "`encoding_format` must be `float` or `base64`, not `{other}`."
            ))
            .with_param("encoding_format"));
        }
    };
    let mut extras: Extras = extras_from(&origin, w.extra.clone(), w.verbatim.clone());
    if let Some(m) = marker {
        extras.form.insert(INPUT.to_owned(), m.into());
    }
    if w.encoding_format.as_deref() == Some("float") {
        extras.form.insert(ENCODING.to_owned(), "float".into());
    }
    for (name, value) in headers.iter() {
        if is_forwarded_header(DIALECT, name) {
            extras.insert_header(name, value);
        }
    }
    req.extras = extras;
    Ok(Request::Embed(req))
}

pub(super) fn check(req: &Request, target: &Origin) -> Result<(), KppError> {
    let Request::Embed(r) = req else {
        return Err(KppError::unsupported_by(
            "generation request",
            DIALECT,
            "Send generation requests to a chat or responses endpoint.",
        ));
    };
    let texts = r.input.iter().any(|i| matches!(i, EmbedInput::Text(_)));
    let tokens = r.input.iter().any(|i| matches!(i, EmbedInput::Tokens(_)));
    if texts && tokens {
        return Err(KppError::unsupported_by(
            "mixed text and token inputs",
            DIALECT,
            "Send either strings or token arrays in one request.",
        )
        .with_param("input"));
    }
    if r.input
        .iter()
        .any(|i| !matches!(i, EmbedInput::Text(_) | EmbedInput::Tokens(_)))
    {
        return Err(
            KppError::unsupported_by("embedding input kind", DIALECT, "").with_param("input"),
        );
    }
    check_foreign(req, target)
}

fn body(r: &EmbedRequest, profile: &EmbeddingsProfile) -> Result<Vec<u8>, KppError> {
    let marker = r.extras.form_str(DIALECT, INPUT);
    let renamed = profile.dimensions_field != "dimensions";
    let mut w = CreateEmbeddingRequest {
        input: encode_input(&r.input, marker)?,
        model: r.model.clone(),
        encoding_format: None,
        dimensions: r.dimensions.filter(|_| !renamed),
        user: r.user.clone(),
    };
    w.encoding_format = match r.encoding {
        EmbedEncoding::Base64 => Some("base64".to_owned()),
        _ if r.extras.form_str(DIALECT, ENCODING) == Some("float") && profile.float_encoding => {
            Some("float".to_owned())
        }
        _ => None,
    };
    let mut extra = extra_for(&r.extras, DIALECT);
    let mut verbatim = verbatim_from_form(&r.extras, DIALECT, "");
    if renamed {
        // A `null` an app wrote goes under the provider's name too.
        if let Some(v) = verbatim.shift_remove("dimensions") {
            verbatim.insert(profile.dimensions_field.to_owned(), v);
        }
        if let Some(d) = r.dimensions {
            let value = RawJson::from_serialize(&d).map_err(|e| {
                KppError::provider(format!("Kaeso could not serialize `dimensions` ({e})."))
            })?;
            extra.insert(profile.dimensions_field.to_owned(), value);
        }
    }
    if !profile.user {
        // An app's `"user": null` says nothing; a provider without `user` refuses the key.
        verbatim.shift_remove("user");
    }
    if profile.encoding_field != ENCODING {
        if let Some(v) = verbatim.shift_remove(ENCODING) {
            verbatim.insert(profile.encoding_field.to_owned(), v);
        }
        if let Some(format) = w.encoding_format.take() {
            let value = RawJson::from_serialize(&format).map_err(|e| {
                KppError::provider(format!("Kaeso could not serialize `{ENCODING}` ({e})."))
            })?;
            extra.insert(profile.encoding_field.to_owned(), value);
        }
    }
    to_body(&Ext::with(w, extra, verbatim))
}

// What the provider can't take of OpenAI's fields is refused here, before anything is sent.
fn check_profile(r: &EmbedRequest, profile: &EmbeddingsProfile) -> Result<(), KppError> {
    let provider = profile.provider;
    if let Some(d) = r.dimensions {
        if !profile.dimensions {
            return Err(KppError::unsupported(
                "dimensions",
                format!("`dimensions` is not supported by {provider}; remove it from the request."),
            )
            .with_param("dimensions"));
        }
        if !profile.dimension_sizes.is_empty() && !profile.dimension_sizes.contains(&d) {
            let sizes = profile
                .dimension_sizes
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(KppError::unsupported(
                "dimensions",
                format!("{provider} takes `dimensions` of {sizes} only, not {d}."),
            )
            .with_param("dimensions"));
        }
    }
    if r.user.is_some() && !profile.user {
        return Err(KppError::unsupported(
            "user",
            format!("`user` is not supported by {provider}; remove it from the request."),
        )
        .with_param("user"));
    }
    let field = profile.encoding_field;
    let encoding_set =
        r.encoding == EmbedEncoding::Base64 || r.extras.form_str(DIALECT, ENCODING).is_some();
    if field != ENCODING && encoding_set && r.extras.fields.contains_key(field) {
        return Err(KppError::invalid_request(format!(
            "`encoding_format` reaches {provider} as `{field}`, which the request sets too; send one of them."
        ))
        .with_param(field));
    }
    if !profile.token_inputs && r.input.iter().any(|i| matches!(i, EmbedInput::Tokens(_))) {
        return Err(KppError::unsupported(
            "token inputs",
            format!("{provider} takes `input` as text only; send strings instead of token ids."),
        )
        .with_param("input"));
    }
    Ok(())
}

pub(super) fn encode(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    check(req, &ctx.origin(DIALECT))?;
    let Request::Embed(r) = req else {
        return Err(KppError::unsupported_by("generation request", DIALECT, ""));
    };
    let profile = ctx.embeddings_profile;
    check_profile(r, &profile)?;
    let mut headers = HeaderList::new().with("content-type", "application/json");
    if r.extras.is_from(DIALECT) {
        for (name, value) in r.extras.headers() {
            if is_forwarded_header(DIALECT, name) {
                headers.push(name, value);
            }
        }
    }
    Ok(EncodedRequest::post_json(
        crate::codec::under_api_root(ctx, super::EMBEDDINGS_PATH),
        headers,
        body(r, &profile)?,
    ))
}
