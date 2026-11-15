//! Deepgram's model listing, `GET /v1/models`: `{"stt": [...], "tts": [...]}`, one entry per
//! model version and language, each named by `canonical_name` (`nova-3-general`,
//! `aura-2-thalia-en`), plus a `languages` map. `GET /v1/models/{model_id}` takes the entry's
//! `uuid` and answers one entry.

use kaeso_kpp::{Extras, KppError, ModelInfo, ModelList, Origin, RawJson};
use serde_json::{Map, Value};

use super::D;

const LIST: &str = "list";
const LISTS: [&str; 2] = ["stt", "tts"];

fn bad(detail: &str) -> KppError {
    KppError::provider(format!(
        "Deepgram sent a model listing Kaeso cannot read: {detail}."
    ))
}

fn info_of(entry: Value, list: Option<&str>) -> Result<ModelInfo, KppError> {
    let Value::Object(mut m) = entry else {
        return Err(bad("a model is not an object"));
    };
    let Some(Value::String(id)) = m.remove("canonical_name") else {
        return Err(bad("a model has no `canonical_name`"));
    };
    let mut info = ModelInfo::new(id);
    let mut extras = Extras::new(Origin::new(D));
    extras.fields.extend(
        m.into_iter()
            .filter_map(|(k, v)| RawJson::from_serialize(&v).ok().map(|raw| (k, raw))),
    );
    if let Some(list) = list {
        extras.form.insert(LIST.to_owned(), list.into());
    }
    info.extras = extras;
    Ok(info)
}

pub(super) fn decode(body: &[u8]) -> Result<ModelList, KppError> {
    let value: Value = serde_json::from_slice(body).map_err(|_| bad("it is not JSON"))?;
    let Value::Object(mut m) = value else {
        return Err(bad("it is not an object"));
    };
    let mut models = Vec::new();
    for list in LISTS {
        match m.remove(list) {
            Some(Value::Array(entries)) => {
                for entry in entries {
                    models.push(info_of(entry, Some(list))?);
                }
            }
            None | Some(Value::Null) => {}
            Some(_) => return Err(bad("a list of models is not a list")),
        }
    }
    let mut out = ModelList::new(models);
    let mut extras = Extras::new(Origin::new(D));
    extras.fields.extend(
        m.into_iter()
            .filter_map(|(k, v)| RawJson::from_serialize(&v).ok().map(|raw| (k, raw))),
    );
    out.extras = extras;
    Ok(out)
}

pub(super) fn decode_one(body: &[u8]) -> Result<ModelInfo, KppError> {
    let value: Value = serde_json::from_slice(body).map_err(|_| bad("it is not JSON"))?;
    info_of(value, None)
}

fn entry_of(model: &ModelInfo) -> Map<String, Value> {
    let mut m = Map::new();
    if model.extras.is_from(D) {
        m.extend(
            model
                .extras
                .fields
                .iter()
                .filter_map(|(k, v)| v.to_value().ok().map(|value| (k.clone(), value))),
        );
    }
    if let Some(name) = &model.display_name {
        m.entry("name").or_insert_with(|| name.clone().into());
    }
    m.insert("canonical_name".to_owned(), model.id.clone().into());
    m
}

fn list_of(model: &ModelInfo) -> &str {
    match model.extras.form_str(D, LIST) {
        Some("tts") => "tts",
        _ => "stt",
    }
}

pub(super) fn encode(list: &ModelList) -> Result<Vec<u8>, KppError> {
    let mut out = Map::new();
    for name in LISTS {
        let entries: Vec<Value> = list
            .models
            .iter()
            .filter(|m| list_of(m) == name)
            .map(|m| Value::Object(entry_of(m)))
            .collect();
        out.insert(name.to_owned(), Value::Array(entries));
    }
    if list.extras.is_from(D) {
        for (k, value) in list
            .extras
            .fields
            .iter()
            .filter_map(|(k, v)| v.to_value().ok().map(|value| (k, value)))
        {
            out.entry(k.clone()).or_insert(value);
        }
    }
    crate::wire::to_body(&out)
}

pub(super) fn encode_one(model: &ModelInfo) -> Result<Vec<u8>, KppError> {
    crate::wire::to_body(&entry_of(model))
}
