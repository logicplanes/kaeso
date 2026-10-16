use kaeso_kpp::{
    Extras, KppError, ModelInfo, ModelLifecycle, ModelList, ModelsQuery, Origin, RawJson,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::common::D as DIALECT;
use crate::models::{
    EPOCH_RFC3339, add_query_extra, bad_listing, decode_date, encode_date, format_rfc3339, none_or,
    parse_rfc3339, query_pairs, record_none,
};
use crate::wire::{
    Ext, Maybe, extra_for, extras_from, to_body, verbatim_from_form, without_position,
};

const LIMIT_RANGE: std::ops::RangeInclusive<u32> = 1..=1000;
const MAX_LIFECYCLES: usize = 3;
const EXTRA_KEYS: [&str; 2] = ["capabilities", "line"];

const TYPE: &str = "type";
const FIRST_ID: &str = "first_id";
const LAST_ID: &str = "last_id";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct ModelInfoWire {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub display_name: Maybe<String>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub created_at: Maybe<String>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub lifecycle: Maybe<String>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub deprecated_at: Maybe<String>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub retires_at: Maybe<String>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub max_input_tokens: Maybe<u64>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub max_tokens: Maybe<u64>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub capabilities: Maybe<RawJson>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub line: Maybe<RawJson>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct ListResponseWire {
    pub data: Vec<Ext<ModelInfoWire>>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub first_id: Maybe<String>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub has_more: Maybe<bool>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub last_id: Maybe<String>,
}

fn invalid(param: &str, message: String) -> KppError {
    KppError::invalid_request(message).with_param(param)
}

fn set_once(slot: &mut Option<String>, name: &str, value: String) -> Result<(), KppError> {
    if slot.is_some() {
        return Err(invalid(
            name,
            format!("The query parameter `{name}` was given more than once; send it once."),
        ));
    }
    if value.is_empty() {
        return Err(invalid(
            name,
            format!("The query parameter `{name}` is empty; send a model id or leave it out."),
        ));
    }
    *slot = Some(value);
    Ok(())
}

pub(super) fn decode_query(query: &str) -> Result<ModelsQuery, KppError> {
    let mut q = ModelsQuery::default();
    q.extras = Extras::new(Origin::new(DIALECT));
    let mut limit_seen = false;
    for (name, value) in query_pairs(query)? {
        match name.as_str() {
            "before_id" => set_once(&mut q.before_id, &name, value)?,
            "after_id" => set_once(&mut q.after_id, &name, value)?,
            "limit" => {
                if limit_seen {
                    return Err(invalid(
                        "limit",
                        "The query parameter `limit` was given more than once; send it once."
                            .to_owned(),
                    ));
                }
                limit_seen = true;
                let n = Some(value.as_str())
                    .filter(|v| v.bytes().all(|b| b.is_ascii_digit()))
                    .and_then(|v| v.parse::<u32>().ok())
                    .filter(|n| LIMIT_RANGE.contains(n))
                    .ok_or_else(|| {
                        invalid(
                            "limit",
                            format!(
                                "The query parameter `limit` must be a whole number from 1 to 1000, got `{value}`."
                            ),
                        )
                    })?;
                q.limit = Some(n);
            }
            "lifecycle" | "lifecycle[]" => {
                let stage = ModelLifecycle::from_wire(&value);
                if matches!(stage, ModelLifecycle::Other(_)) {
                    return Err(invalid(
                        "lifecycle",
                        format!(
                            "The query parameter `lifecycle` must be `active`, `deprecated` or `retired`, got `{value}`."
                        ),
                    ));
                }
                q.lifecycle.push(stage);
                if q.lifecycle.len() > MAX_LIFECYCLES {
                    return Err(invalid(
                        "lifecycle",
                        "The query parameter `lifecycle` takes up to 3 values.".to_owned(),
                    ));
                }
            }
            _ => add_query_extra(&mut q.extras, &name, &value),
        }
    }
    if q.before_id.is_some() && q.after_id.is_some() {
        return Err(invalid(
            "before_id",
            "Send either `before_id` or `after_id`, not both.".to_owned(),
        ));
    }
    Ok(q)
}

fn date_error(key: &str, text: &str) -> KppError {
    bad_listing(
        DIALECT,
        &format!("`{key}` `{text}` is not an RFC 3339 date-time"),
    )
}

fn decode_time(
    extras: &mut Extras,
    key: &str,
    value: &Maybe<String>,
) -> Result<Option<u64>, KppError> {
    match value {
        Maybe::Value(text) => decode_date(extras, key, text, parse_rfc3339, format_rfc3339)
            .map(Some)
            .ok_or_else(|| date_error(key, text)),
        Maybe::Absent | Maybe::Null => Ok(None),
    }
}

fn decode_info(w: Ext<ModelInfoWire>) -> Result<ModelInfo, KppError> {
    let Ext {
        known: w,
        extra,
        verbatim,
    } = w;
    let mut m = ModelInfo::new(w.id);
    let mut extras = extras_from(&Origin::new(DIALECT), extra, verbatim);
    if w.ty != "model" {
        extras.form.insert(TYPE.to_owned(), Value::from(w.ty));
    }
    record_none(&mut extras, "display_name", &w.display_name, false);
    record_none(&mut extras, "created_at", &w.created_at, false);
    record_none(&mut extras, "lifecycle", &w.lifecycle, false);
    record_none(&mut extras, "deprecated_at", &w.deprecated_at, true);
    record_none(&mut extras, "retires_at", &w.retires_at, true);
    record_none(&mut extras, "max_input_tokens", &w.max_input_tokens, true);
    record_none(&mut extras, "max_tokens", &w.max_tokens, true);
    m.display_name = w.display_name.into_option();
    m.created_at = decode_time(&mut extras, "created_at", &w.created_at)?;
    m.lifecycle = w
        .lifecycle
        .as_option()
        .map(|s| ModelLifecycle::from_wire(s));
    m.deprecated_at = decode_time(&mut extras, "deprecated_at", &w.deprecated_at)?;
    m.retires_at = decode_time(&mut extras, "retires_at", &w.retires_at)?;
    m.context_window = w.max_input_tokens.into_option();
    m.max_output_tokens = w.max_tokens.into_option();
    for (key, value) in EXTRA_KEYS.into_iter().zip([w.capabilities, w.line]) {
        match value {
            Maybe::Value(raw) => {
                extras.fields.insert(key.to_owned(), raw);
            }
            Maybe::Null => {
                if let Ok(raw) = RawJson::from_serialize(&Value::Null) {
                    extras.fields.insert(key.to_owned(), raw);
                }
            }
            Maybe::Absent => {}
        }
    }
    m.extras = extras;
    Ok(m)
}

fn encode_info(m: &ModelInfo) -> Ext<ModelInfoWire> {
    let ex = &m.extras;
    let own = ex.is_from(DIALECT);
    let time = |key: &str, value: Option<u64>| match value {
        Some(s) => encode_date(ex, DIALECT, key, s, parse_rfc3339, format_rfc3339)
            .map_or(Maybe::Null, Maybe::Value),
        None => none_or(ex, DIALECT, key, Maybe::Null),
    };
    let extra_field = |key: &str| match ex.fields_for(DIALECT).map(|f| f.get(key)) {
        Some(Some(raw)) => Maybe::Value(raw.clone()),
        Some(None) => Maybe::Absent,
        None => Maybe::Null,
    };
    let w = ModelInfoWire {
        id: m.id.clone(),
        ty: ex.form_str(DIALECT, TYPE).unwrap_or("model").to_owned(),
        display_name: Maybe::from_option(
            m.display_name.clone(),
            none_or(ex, DIALECT, "display_name", Maybe::Value(m.id.clone())),
        ),
        created_at: match m.created_at {
            Some(s) => Maybe::Value(
                encode_date(ex, DIALECT, "created_at", s, parse_rfc3339, format_rfc3339)
                    .unwrap_or_else(|| EPOCH_RFC3339.to_owned()),
            ),
            None => none_or(
                ex,
                DIALECT,
                "created_at",
                Maybe::Value(EPOCH_RFC3339.to_owned()),
            ),
        },
        lifecycle: Maybe::from_option(
            m.lifecycle.as_ref().map(|l| l.as_wire().to_owned()),
            none_or(ex, DIALECT, "lifecycle", Maybe::Value("active".to_owned())),
        ),
        deprecated_at: time("deprecated_at", m.deprecated_at),
        retires_at: time("retires_at", m.retires_at),
        max_input_tokens: Maybe::from_option(
            m.context_window,
            none_or(ex, DIALECT, "max_input_tokens", Maybe::Null),
        ),
        max_tokens: Maybe::from_option(
            m.max_output_tokens,
            none_or(ex, DIALECT, "max_tokens", Maybe::Null),
        ),
        capabilities: extra_field("capabilities"),
        line: extra_field("line"),
    };
    if !own {
        return Ext::new(w);
    }
    let mut extra = extra_for(ex, DIALECT);
    extra.retain(|k, _| !EXTRA_KEYS.contains(&k.as_str()));
    Ext::with(w, extra, verbatim_from_form(ex, DIALECT, ""))
}

pub(super) fn encode_model(m: &ModelInfo) -> Result<Vec<u8>, KppError> {
    to_body(&encode_info(m))
}

fn derived_id(list_ids: Option<&str>) -> Value {
    list_ids.map_or(Value::Null, Value::from)
}

fn encode_edge(ex: &Extras, key: &str, derived: Option<&str>) -> Maybe<String> {
    let kept = ex
        .form_value(DIALECT, key)
        .filter(|v| v.get("for") == Some(&derived_id(derived)));
    match kept {
        Some(v) => match v.get("sent") {
            Some(Value::String(s)) => Maybe::Value(s.clone()),
            Some(_) => Maybe::Null,
            None => Maybe::Absent,
        },
        None => Maybe::from_option(derived.map(str::to_owned), Maybe::Null),
    }
}

fn decode_edge(extras: &mut Extras, key: &str, received: &Maybe<String>, derived: Option<&str>) {
    let canonical = Maybe::from_option(derived.map(str::to_owned), Maybe::Null);
    if *received == canonical {
        return;
    }
    let mut marker = serde_json::Map::new();
    marker.insert("for".to_owned(), derived_id(derived));
    match received {
        Maybe::Value(s) => {
            marker.insert("sent".to_owned(), Value::from(s.as_str()));
        }
        Maybe::Null => {
            marker.insert("sent".to_owned(), Value::Null);
        }
        Maybe::Absent => {}
    }
    extras.form.insert(key.to_owned(), Value::Object(marker));
}

pub(super) fn encode_models(list: &ModelList) -> Result<Vec<u8>, KppError> {
    let ex = &list.extras;
    let first = list.models.first().map(|m| m.id.as_str());
    let last = list.models.last().map(|m| m.id.as_str());
    let w = ListResponseWire {
        data: list.models.iter().map(encode_info).collect(),
        first_id: encode_edge(ex, FIRST_ID, first),
        has_more: if list.has_more {
            Maybe::Value(true)
        } else {
            none_or(ex, DIALECT, "has_more", Maybe::Value(false))
        },
        last_id: encode_edge(ex, LAST_ID, last),
    };
    to_body(&Ext::with(
        w,
        extra_for(ex, DIALECT),
        verbatim_from_form(ex, DIALECT, ""),
    ))
}

fn parse<T: serde::de::DeserializeOwned + Serialize>(body: &[u8]) -> Result<Ext<T>, KppError> {
    Ext::from_slice(body).map_err(|e| bad_listing(DIALECT, &without_position(&e)))
}

pub(super) fn decode_model(body: &[u8]) -> Result<ModelInfo, KppError> {
    decode_info(parse(body)?)
}

pub(super) fn decode_models(body: &[u8]) -> Result<ModelList, KppError> {
    let Ext {
        known: w,
        extra,
        verbatim,
    } = parse::<ListResponseWire>(body)?;
    let models = w
        .data
        .into_iter()
        .map(decode_info)
        .collect::<Result<Vec<_>, _>>()?;
    let mut extras = extras_from(&Origin::new(DIALECT), extra, verbatim);
    record_none(&mut extras, "has_more", &w.has_more, false);
    let first = models.first().map(|m| m.id.as_str());
    let last = models.last().map(|m| m.id.as_str());
    decode_edge(&mut extras, FIRST_ID, &w.first_id, first);
    decode_edge(&mut extras, LAST_ID, &w.last_id, last);
    let mut list = ModelList::new(models);
    list.has_more = w.has_more.into_option().unwrap_or(false);
    list.extras = extras;
    Ok(list)
}
