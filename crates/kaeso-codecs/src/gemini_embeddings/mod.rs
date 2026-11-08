#[cfg(test)]
mod tests;

use base64::Engine as _;
use kaeso_kpp::{
    DialectId, EmbedInput, EmbedRequest, EmbedResponse, Embedding, EmbeddingVector, Extras,
    KppError, ModelInfo, ModelList, ModelsQuery, Origin, RawJson, RawUsage, Reply, Request,
    RequestKind, StreamEvent, Usage,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::value::RawValue;

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};
use crate::cross::check_foreign;
use crate::gemini::{GeminiAction, decode_error_value, encode_google_error};
use crate::wire::{
    Ext, add_nested_extra, extra_for, extras_from, nested_extra_for, to_body, verbatim_from_form,
    verbatim_into_form, without_position,
};

const D: DialectId = DialectId::GeminiEmbeddings;
const FORM_ACTION: &str = "action";
const FORM_MODELS: &str = "models";
const FORM_CONFIG_DIMS: &str = "config_dimensions";
const FORM_NO_USAGE: &str = "no_usage";
const SINGLE: &str = "single";
const CONFIG: &str = "embedContentConfig";

/// Gemini embeddings: `embedContent` and `batchEmbedContents`. Requests name their model and
/// method in the path; apps' requests are decoded with
/// [`crate::dispatch::decode_gemini_request`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct GeminiEmbeddings;

impl crate::codec::sealed::Sealed for GeminiEmbeddings {}
impl crate::codec::sealed::Sealed for NoStream {}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmbedW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    content: Box<RawValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    output_dimensionality: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    embed_content_config: Option<Ext<ConfigW>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigW {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    output_dimensionality: Option<u32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct BatchW {
    requests: Vec<Ext<EmbedW>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct ContentW {
    parts: Vec<Ext<TextPartW>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct TextPartW {
    text: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SingleRespW {
    embedding: Ext<VectorW>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    usage_metadata: Option<Box<RawValue>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchRespW {
    #[serde(default)]
    embeddings: Vec<Ext<VectorW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    usage_metadata: Option<Box<RawValue>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct VectorW {
    #[serde(default)]
    values: Vec<f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageW {
    #[serde(default)]
    prompt_token_count: u64,
}

fn invalid(detail: impl std::fmt::Display) -> KppError {
    KppError::invalid_request(format!(
        "The request body is not a valid Gemini Embeddings request: {detail}."
    ))
}

fn bad(detail: impl std::fmt::Display) -> KppError {
    KppError::provider(format!(
        "The provider sent a response that is not a valid Gemini Embeddings response: {detail}."
    ))
}

fn not_generation() -> KppError {
    KppError::unsupported_by(
        "generation request",
        D,
        "Send generation requests to `generateContent` instead.",
    )
}

fn parse<T: serde::de::DeserializeOwned + Serialize>(body: &[u8]) -> Result<Ext<T>, KppError> {
    let raw: &RawValue =
        serde_json::from_slice(body).map_err(|_| invalid("the body is not valid JSON"))?;
    Ext::from_raw(raw).map_err(|e| invalid(without_position(&e)))
}

fn text_of(content: &RawValue) -> Option<String> {
    let c: Ext<ContentW> = Ext::from_raw(content).ok()?;
    if !c.extra.is_empty() || !c.verbatim.is_empty() {
        return None;
    }
    match c.known.parts.as_slice() {
        [p] if p.extra.is_empty() && p.verbatim.is_empty() => Some(p.known.text.clone()),
        _ => None,
    }
}

struct Decoded {
    dims: Vec<Option<u32>>,
    config_dims: Vec<Value>,
    models: Vec<Value>,
}

fn decode_one(
    w: Ext<EmbedW>,
    prefix: &str,
    index: usize,
    model: &str,
    req: &mut EmbedRequest,
    seen: &mut Decoded,
) {
    let Ext {
        known,
        extra,
        verbatim,
    } = w;
    let field = |key: &str| {
        if prefix.is_empty() {
            key.to_owned()
        } else {
            format!("{prefix}.{key}")
        }
    };
    for (k, v) in extra {
        req.extras.fields.insert(field(&k), v);
    }
    verbatim_into_form(&mut req.extras, prefix, verbatim);
    match text_of(&known.content) {
        Some(text) => req.input.push(EmbedInput::Text(text)),
        None => {
            req.extras
                .fields
                .insert(field("content"), RawJson::from(known.content));
            req.input.push(EmbedInput::Text(String::new()));
        }
    }
    let path_model = format!("models/{model}");
    match known.model {
        Some(m) if m == path_model => {}
        Some(m) => seen.models.push(Value::from(vec![Value::from(index), Value::from(m)])),
        None => seen.models.push(Value::from(vec![Value::from(index), Value::Null])),
    }
    let mut dims = known.output_dimensionality;
    if let Some(config) = known.embed_content_config {
        let Ext {
            known: c,
            extra,
            verbatim,
        } = config;
        add_nested_extra(&mut req.extras, &field(CONFIG), extra);
        verbatim_into_form(&mut req.extras, &field(CONFIG), verbatim);
        if c.output_dimensionality.is_some() {
            seen.config_dims.push(Value::from(index));
            dims = c.output_dimensionality;
        }
        req.extras
            .form
            .insert(format!("objects:{index}"), true.into());
    }
    seen.dims.push(dims);
}

impl GeminiEmbeddings {
    pub(crate) fn decode_call(
        model: &str,
        action: GeminiAction,
        body: &[u8],
        _headers: &HeaderList,
    ) -> Result<Request, KppError> {
        let model = model.strip_prefix("models/").unwrap_or(model);
        let mut req = EmbedRequest::new(model, Vec::new());
        req.extras = Extras::new(Origin::new(D));
        let mut seen = Decoded {
            dims: Vec::new(),
            config_dims: Vec::new(),
            models: Vec::new(),
        };
        match action {
            GeminiAction::EmbedContent => {
                let w = parse::<EmbedW>(body)?;
                decode_one(w, "", 0, model, &mut req, &mut seen);
                req.extras.form.insert(FORM_ACTION.into(), SINGLE.into());
            }
            GeminiAction::BatchEmbedContents => {
                let Ext {
                    known,
                    extra,
                    verbatim,
                } = parse::<BatchW>(body)?;
                for (k, v) in extra {
                    req.extras.fields.insert(format!("batch:{k}"), v);
                }
                verbatim_into_form(&mut req.extras, "batch", verbatim);
                for (i, r) in known.requests.into_iter().enumerate() {
                    decode_one(r, &format!("requests[{i}]"), i, model, &mut req, &mut seen);
                }
            }
            _ => return Err(not_generation()),
        }
        let first = seen.dims.first().copied().flatten();
        if seen.dims.iter().all(|d| *d == first) {
            req.dimensions = first;
        } else {
            for (i, d) in seen.dims.iter().enumerate() {
                if let Some(d) = d {
                    let key = if action == GeminiAction::EmbedContent {
                        "outputDimensionality".to_owned()
                    } else {
                        format!("requests[{i}].outputDimensionality")
                    };
                    req.extras
                        .fields
                        .insert(key, RawJson::from_serialize(d).map_err(invalid)?);
                }
            }
        }
        if !seen.config_dims.is_empty() {
            req.extras
                .form
                .insert(FORM_CONFIG_DIMS.into(), Value::Array(seen.config_dims));
        }
        if !seen.models.is_empty() {
            req.extras
                .form
                .insert(FORM_MODELS.into(), Value::Array(seen.models));
        }
        Ok(Request::Embed(req))
    }
}

fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
    let Request::Embed(r) = req else {
        return Err(not_generation());
    };
    check_foreign(req, &ctx.origin(D))?;
    if r.input.iter().any(|i| !matches!(i, EmbedInput::Text(_))) {
        return Err(KppError::unsupported_by(
            "token-id inputs",
            D,
            "Gemini embeds text; send the inputs as strings.",
        )
        .with_param("input"));
    }
    Ok(())
}

fn ser(e: serde_json::Error) -> KppError {
    KppError::provider(format!(
        "Kaeso could not write a Gemini embeddings request ({e}). This is a bug in Kaeso; please report it."
    ))
}

fn encode_one(
    r: &EmbedRequest,
    index: usize,
    prefix: &str,
    text: &str,
) -> Result<Ext<EmbedW>, KppError> {
    let own = r.extras.is_from(D);
    let field = |key: &str| {
        if prefix.is_empty() {
            key.to_owned()
        } else {
            format!("{prefix}.{key}")
        }
    };
    let content = match own.then(|| r.extras.fields.get(&field("content"))).flatten() {
        Some(raw) => raw.clone().into_inner(),
        None => serde_json::value::to_raw_value(&serde_json::json!({"parts": [{"text": text}]}))
            .map_err(ser)?,
    };
    let recorded_model = r
        .extras
        .form_value(D, FORM_MODELS)
        .and_then(Value::as_array)
        .and_then(|m| {
            m.iter()
                .find(|e| e.get(0).and_then(Value::as_u64) == Some(index as u64))
        })
        .and_then(|e| e.get(1).cloned());
    let model = match recorded_model {
        Some(Value::String(m)) => Some(m),
        Some(_) => None,
        None => Some(format!(
            "models/{}",
            r.model.strip_prefix("models/").unwrap_or(&r.model)
        )),
    };
    let in_config = r
        .extras
        .form_value(D, FORM_CONFIG_DIMS)
        .and_then(Value::as_array)
        .is_some_and(|a| a.iter().any(|v| v.as_u64() == Some(index as u64)));
    let dims = r.dimensions.or_else(|| {
        r.extras
            .fields
            .get(&field("outputDimensionality"))
            .filter(|_| own)
            .and_then(|v| v.parse::<u32>().ok())
    });
    let config_present = own && r.extras.form_flag(D, &format!("objects:{index}"));
    let config = if in_config || config_present {
        Some(Ext::with(
            ConfigW {
                output_dimensionality: if in_config { dims } else { None },
            },
            nested_extra_for(&r.extras, D, &field(CONFIG)),
            verbatim_from_form(&r.extras, D, &field(CONFIG)),
        ))
    } else {
        None
    };
    let mut extra = indexmap::IndexMap::new();
    if own {
        for (k, v) in r.extras.body_fields() {
            let rest = if prefix.is_empty() {
                (!k.contains('.') && !k.contains(':')).then_some(k)
            } else {
                k.strip_prefix(prefix)
                    .and_then(|r| r.strip_prefix('.'))
                    .filter(|r| !r.contains('.'))
            };
            if let Some(rest) = rest
                && rest != "content"
                && rest != "outputDimensionality"
            {
                extra.insert(rest.to_owned(), v.clone());
            }
        }
    }
    Ok(Ext::with(
        EmbedW {
            model,
            content,
            output_dimensionality: if in_config { None } else { dims },
            embed_content_config: config,
        },
        extra,
        verbatim_from_form(&r.extras, D, prefix),
    ))
}

fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    check(req, ctx)?;
    let Request::Embed(r) = req else {
        return Err(not_generation());
    };
    let own = r.extras.is_from(D);
    let single = own && r.extras.form_str(D, FORM_ACTION) == Some(SINGLE);
    let texts: Vec<&str> = r
        .input
        .iter()
        .map(|i| match i {
            EmbedInput::Text(t) => t.as_str(),
            _ => "",
        })
        .collect();
    let (action, body) = if single {
        let one = encode_one(r, 0, "", texts.first().copied().unwrap_or_default())?;
        (GeminiAction::EmbedContent, to_body(&one)?)
    } else {
        let requests = texts
            .iter()
            .enumerate()
            .map(|(i, t)| encode_one(r, i, &format!("requests[{i}]"), t))
            .collect::<Result<Vec<_>, _>>()?;
        let extra = if own {
            r.extras
                .body_fields()
                .filter_map(|(k, v)| Some((k.strip_prefix("batch:")?.to_owned(), v.clone())))
                .collect()
        } else {
            Default::default()
        };
        (
            GeminiAction::BatchEmbedContents,
            to_body(&Ext::with(
                BatchW { requests },
                extra,
                verbatim_from_form(&r.extras, D, "batch"),
            ))?,
        )
    };
    Ok(EncodedRequest::post_json(
        ctx.gemini_profile.path(&r.model, action)?,
        HeaderList::new().with("content-type", "application/json"),
        body,
    ))
}

fn vector(w: Ext<VectorW>, index: u32, origin: &Origin) -> Embedding {
    let Ext {
        known,
        extra,
        verbatim,
    } = w;
    let mut e = Embedding::new(index, EmbeddingVector::Float(known.values));
    e.extras = extras_from(origin, extra, verbatim);
    e
}

fn usage(raw: Option<&RawValue>, origin: &Origin) -> Result<Option<Usage>, KppError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let w: UsageW = serde_json::from_str(raw.get()).map_err(|e| bad(without_position(&e)))?;
    let value: Value = serde_json::from_str(raw.get()).map_err(|e| bad(without_position(&e)))?;
    let mut u = Usage::new(w.prompt_token_count, 0);
    u.raw = Some(RawUsage::new(origin.clone(), value));
    Ok(Some(u))
}

fn decode_response(
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<Reply, KppError> {
    if !(200..300).contains(&status) {
        return Err(decode_error_value(status, headers, body, D));
    }
    if ctx.kind != RequestKind::Embed {
        return Err(KppError::unsupported_by("generation response", D, ""));
    }
    let origin = ctx.origin(D);
    let value: Value = serde_json::from_slice(body).map_err(|e| bad(without_position(&e)))?;
    let mut resp = EmbedResponse::new("", Vec::new());
    let (u, mut extras) = if value.get("embedding").is_some() {
        let Ext {
            known,
            extra,
            verbatim,
        } = Ext::<SingleRespW>::from_slice(body).map_err(|e| bad(without_position(&e)))?;
        resp.data.push(vector(known.embedding, 0, &origin));
        let mut extras = extras_from(&origin, extra, verbatim);
        extras.form.insert(FORM_ACTION.into(), SINGLE.into());
        (usage(known.usage_metadata.as_deref(), &origin)?, extras)
    } else {
        let Ext {
            known,
            extra,
            verbatim,
        } = Ext::<BatchRespW>::from_slice(body).map_err(|e| bad(without_position(&e)))?;
        for (i, e) in known.embeddings.into_iter().enumerate() {
            resp.data
                .push(vector(e, u32::try_from(i).unwrap_or(u32::MAX), &origin));
        }
        (
            usage(known.usage_metadata.as_deref(), &origin)?,
            extras_from(&origin, extra, verbatim),
        )
    };
    match u {
        Some(u) => resp.usage = u,
        None => {
            extras.form.insert(FORM_NO_USAGE.into(), true.into());
        }
    }
    resp.extras = extras;
    Ok(Reply::Embed(resp))
}

fn floats(v: &EmbeddingVector) -> Result<Vec<f64>, KppError> {
    match v {
        EmbeddingVector::Float(f) => Ok(f.clone()),
        EmbeddingVector::Base64(s) => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(s)
                .map_err(|_| bad("a base64 embedding is not valid base64"))?;
            let (chunks, rest) = bytes.as_chunks::<4>();
            if !rest.is_empty() {
                return Err(bad("a base64 embedding is not a list of 32-bit floats"));
            }
            chunks
                .iter()
                .map(|c| {
                    f32::from_le_bytes(*c)
                        .to_string()
                        .parse::<f64>()
                        .map_err(|_| bad("a base64 embedding holds a value that is not a number"))
                })
                .collect()
        }
        _ => Err(KppError::unsupported_by("embedding vector kind", D, "")),
    }
}

fn usage_value(u: &Usage) -> Value {
    if let Some(raw) = u.raw_for(D) {
        return raw.clone();
    }
    serde_json::json!({ "promptTokenCount": u.total_input_tokens() })
}

fn encode_response(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
    let Reply::Embed(r) = reply else {
        return Err(KppError::unsupported_by(
            "generation response",
            D,
            "Gemini embeddings apps can only receive embeddings.",
        ));
    };
    let own = r.extras.is_from(D);
    let single = if own {
        r.extras.form_str(D, FORM_ACTION) == Some(SINGLE)
    } else {
        matches!(ctx.request.as_deref(), Some(Request::Embed(q)) if q.extras.form_str(D, FORM_ACTION) == Some(SINGLE))
    };
    let vectors = r
        .data
        .iter()
        .map(|e| {
            Ok(Ext::with(
                VectorW {
                    values: floats(&e.vector)?,
                },
                extra_for(&e.extras, D),
                verbatim_from_form(&e.extras, D, ""),
            ))
        })
        .collect::<Result<Vec<_>, KppError>>()?;
    let usage = if r.extras.form_flag(D, FORM_NO_USAGE) {
        None
    } else {
        Some(serde_json::value::to_raw_value(&usage_value(&r.usage)).map_err(ser)?)
    };
    let extra = extra_for(&r.extras, D);
    let verbatim = verbatim_from_form(&r.extras, D, "");
    if single {
        let embedding = vectors.into_iter().next().unwrap_or_else(|| Ext::new(VectorW::default()));
        to_body(&Ext::with(
            SingleRespW {
                embedding,
                usage_metadata: usage,
            },
            extra,
            verbatim,
        ))
    } else {
        to_body(&Ext::with(
            BatchRespW {
                embeddings: vectors,
                usage_metadata: usage,
            },
            extra,
            verbatim,
        ))
    }
}

fn no_streaming() -> KppError {
    KppError::unsupported_by(
        "streaming",
        D,
        "The embeddings endpoints return one complete response; send the request without streaming.",
    )
}

struct NoStream {
    origin: Origin,
    done: bool,
}

impl StreamEncoder for NoStream {
    fn encode(&mut self, _event: &StreamEvent) -> Result<Vec<u8>, KppError> {
        Err(no_streaming())
    }

    fn finish(&mut self) -> Result<Vec<u8>, KppError> {
        Ok(Vec::new())
    }
}

impl StreamDecoder for NoStream {
    fn feed(&mut self, _bytes: &[u8]) -> Vec<StreamEvent> {
        self.error()
    }

    fn finish(&mut self) -> Vec<StreamEvent> {
        self.error()
    }
}

impl NoStream {
    fn error(&mut self) -> Vec<StreamEvent> {
        if std::mem::replace(&mut self.done, true) {
            return Vec::new();
        }
        vec![StreamEvent::Error(no_streaming().with_origin(self.origin.clone()))]
    }
}

impl Codec for GeminiEmbeddings {
    const DIALECT: DialectId = D;

    /// Decodes a body without its path as `embedContent` with an empty model. Apps' requests
    /// go through [`crate::dispatch::decode_gemini_request`].
    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        crate::limits::request_shape(D, body)?;
        if kind != RequestKind::Embed {
            return Err(not_generation());
        }
        let action = if serde_json::from_slice::<Value>(body)
            .ok()
            .is_some_and(|v| v.get("requests").is_some())
        {
            GeminiAction::BatchEmbedContents
        } else {
            GeminiAction::EmbedContent
        };
        Self::decode_call("", action, body, headers)
    }

    fn encode_response(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        encode_response(reply, ctx)
    }

    fn stream_encoder(ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        let _ = ctx;
        Box::new(NoStream {
            origin: Origin::new(D),
            done: false,
        })
    }

    fn encode_error(err: &KppError) -> EncodedError {
        encode_google_error(err, D)
    }

    fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
        check(req, ctx)
    }

    fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
        encode_request(req, ctx)
    }

    fn decode_response(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<Reply, KppError> {
        decode_response(status, headers, body, ctx)
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        Box::new(NoStream {
            origin: ctx.origin(D),
            done: false,
        })
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        decode_error_value(status, headers, body, D)
    }

    fn decode_models_query(query: &str) -> Result<ModelsQuery, KppError> {
        crate::gemini::Gemini::decode_models_query(query)
    }

    fn encode_models(list: &ModelList) -> Result<Vec<u8>, KppError> {
        crate::gemini::Gemini::encode_models(list)
    }

    fn encode_model(model: &ModelInfo) -> Result<Vec<u8>, KppError> {
        crate::gemini::Gemini::encode_model(model)
    }

    fn decode_models(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
    ) -> Result<ModelList, KppError> {
        crate::gemini::Gemini::decode_models(status, headers, body)
    }

    fn decode_model(status: u16, headers: &HeaderList, body: &[u8]) -> Result<ModelInfo, KppError> {
        crate::gemini::Gemini::decode_model(status, headers, body)
    }

    fn encode_model_request(model: &str) -> EncodedRequest {
        crate::gemini::Gemini::encode_model_request(model)
    }

    fn encode_models_request(query: &ModelsQuery) -> Result<EncodedRequest, KppError> {
        crate::gemini::Gemini::encode_models_request(query)
    }

    fn encode_model_request_with(model: &str, ctx: &EgressCtx) -> Result<EncodedRequest, KppError> {
        crate::gemini::Gemini::encode_model_request_with(model, ctx)
    }

    fn encode_models_request_with(
        query: &ModelsQuery,
        ctx: &EgressCtx,
    ) -> Result<EncodedRequest, KppError> {
        crate::gemini::Gemini::encode_models_request_with(query, ctx)
    }
}
