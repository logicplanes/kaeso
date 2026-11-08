//! ElevenLabs' listings: `GET /v1/models` (a JSON array of `ModelResponseModel`) and the voice
//! listings `GET /v1/voices`, `GET /v2/voices` and `GET /v1/voices/{voice_id}`.

use kaeso_kpp::{KppError, ModelInfo, ModelList, ModelsQuery};
use serde_json::{Map, Value};

use crate::codec::{EncodedRequest, HeaderList};

pub(super) const MODELS_PATH: &str = "/v1/models";

/// The voice listing paths an ElevenLabs app may call through Kaeso.
pub const VOICES_PATHS: &[&str] = &["/v1/voices", "/v2/voices"];

fn bad(detail: &str) -> KppError {
    KppError::provider(format!(
        "ElevenLabs sent a listing Kaeso cannot read: {detail}."
    ))
}

pub(super) fn decode_query(query: &str) -> Result<ModelsQuery, KppError> {
    if query.is_empty() {
        return Ok(ModelsQuery::default());
    }
    Err(
        KppError::invalid_request("ElevenLabs lists every model at once; send no query.")
            .with_param("query"),
    )
}

fn model_json(m: &ModelInfo) -> Value {
    let mut o = Map::new();
    o.insert("model_id".into(), m.id.clone().into());
    o.insert(
        "name".into(),
        m.display_name
            .clone()
            .unwrap_or_else(|| m.id.clone())
            .into(),
    );
    for (k, v) in &m.extras.fields {
        if let Ok(v) = v.to_value() {
            o.entry(k.clone()).or_insert(v);
        }
    }
    Value::Object(o)
}

pub(super) fn encode_models(list: &ModelList) -> Result<Vec<u8>, KppError> {
    crate::wire::to_body(&list.models.iter().map(model_json).collect::<Vec<_>>())
}

pub(super) fn encode_model(model: &ModelInfo) -> Result<Vec<u8>, KppError> {
    crate::wire::to_body(&model_json(model))
}

fn model_of(v: &Value) -> Option<ModelInfo> {
    let id = v.get("model_id")?.as_str()?;
    let mut m = ModelInfo::new(id);
    m.display_name = v.get("name").and_then(Value::as_str).map(str::to_owned);
    let mut extras = kaeso_kpp::Extras::new(super::D);
    if let Value::Object(o) = v {
        for (k, val) in o {
            if k != "model_id"
                && k != "name"
                && let Ok(raw) = kaeso_kpp::RawJson::from_serialize(val)
            {
                extras.fields.insert(k.clone(), raw);
            }
        }
    }
    m.extras = extras;
    Some(m)
}

pub(super) fn decode_models(body: &[u8]) -> Result<ModelList, KppError> {
    let v: Value = serde_json::from_slice(body).map_err(|_| bad("not JSON"))?;
    let items = v.as_array().ok_or_else(|| bad("not a list"))?;
    Ok(ModelList::new(items.iter().filter_map(model_of).collect()))
}

pub(super) fn decode_model(body: &[u8]) -> Result<ModelInfo, KppError> {
    let v: Value = serde_json::from_slice(body).map_err(|_| bad("not JSON"))?;
    model_of(&v).ok_or_else(|| bad("a model has no `model_id`"))
}

/// The request for a voice listing at ElevenLabs: `path` is `/v1/voices`, `/v2/voices` or
/// `/v1/voices/{voice_id}`, `query` its query string without `?`. Voices are the account's own
/// at ElevenLabs, so Kaeso asks ElevenLabs with the project's key.
pub fn voice_listing_request(path: &str, query: &str) -> Result<EncodedRequest, KppError> {
    let one = path
        .strip_prefix("/v1/voices/")
        .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric()));
    if !VOICES_PATHS.contains(&path) && !one {
        return Err(
            KppError::invalid_request(format!("Kaeso does not list `{path}`.")).with_status(404),
        );
    }
    let safe = query
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"=&_-.%+,".contains(&b));
    if !safe {
        return Err(
            KppError::invalid_request("The query has characters Kaeso does not pass on.")
                .with_param("query"),
        );
    }
    let full = if query.is_empty() {
        path.to_owned()
    } else {
        format!("{path}?{query}")
    };
    Ok(EncodedRequest::get(full, HeaderList::new()))
}

/// The body of ElevenLabs' answer to a voice listing, checked to be one: a list of voices or
/// one voice.
pub fn voices_reply(status: u16, headers: &HeaderList, body: &[u8]) -> Result<Vec<u8>, KppError> {
    if !(200..300).contains(&status) {
        return Err(super::error::decode(status, headers, body));
    }
    let v: Value = serde_json::from_slice(body).map_err(|_| bad("not JSON"))?;
    if v.get("voices").is_some_and(Value::is_array) || v.get("voice_id").is_some() {
        return Ok(body.to_vec());
    }
    Err(bad("neither voices nor a voice"))
}
