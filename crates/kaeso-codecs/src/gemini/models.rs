use kaeso_kpp::{Extras, KppError, ModelInfo, ModelList, ModelsQuery, Origin, RawJson};
use serde::{Deserialize, Serialize};

use super::D;
use super::path::GeminiProfile;
use crate::codec::{EncodedRequest, HeaderList};
use crate::models::{add_query_extra, bad_listing, percent_encode, query_pairs};
use crate::wire::{Ext, extra_for, extras_from, to_body, verbatim_from_form, without_position};

const MODEL_PREFIX: &str = "models/";
const FORM_BARE_NAME: &str = "bare_name";
const NEXT_PAGE_TOKEN: &str = "nextPageToken";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelW {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    input_token_limit: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    output_token_limit: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListW {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    models: Vec<Ext<ModelW>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    next_page_token: Option<String>,
}

/// `pageSize` is the page's limit and `pageToken` the id after which it starts: a Gemini app
/// treats the token as opaque, so Kaeso's listings use the last id of the page before.
pub(crate) fn decode_query(query: &str) -> Result<ModelsQuery, KppError> {
    let mut q = ModelsQuery::default();
    let mut extras = Extras::new(Origin::new(D));
    for (name, value) in query_pairs(query)? {
        match name.as_str() {
            "pageSize" => {
                let limit = value.parse::<u32>().map_err(|_| {
                    KppError::invalid_request(format!(
                        "The query parameter `pageSize` must be a whole number, not `{value}`."
                    ))
                    .with_param("pageSize")
                })?;
                q.limit = Some(limit);
            }
            "pageToken" if !value.is_empty() => q.after_id = Some(value),
            _ => add_query_extra(&mut extras, &name, &value),
        }
    }
    q.extras = extras;
    Ok(q)
}

fn model_from(w: Ext<ModelW>, origin: &Origin) -> ModelInfo {
    let Ext {
        known,
        extra,
        verbatim,
    } = w;
    let mut extras = extras_from(origin, extra, verbatim);
    let id = match known.name.strip_prefix(MODEL_PREFIX) {
        Some(id) => id.to_owned(),
        None => {
            extras.form.insert(FORM_BARE_NAME.into(), true.into());
            known.name
        }
    };
    let mut m = ModelInfo::new(id);
    m.display_name = known.display_name;
    m.context_window = known.input_token_limit;
    m.max_output_tokens = known.output_token_limit;
    m.extras = extras;
    m
}

fn model_to(m: &ModelInfo) -> Ext<ModelW> {
    let own = m.extras.is_from(D);
    let name = if own && m.extras.form_flag(D, FORM_BARE_NAME) {
        m.id.clone()
    } else {
        format!("{MODEL_PREFIX}{}", m.id)
    };
    Ext::with(
        ModelW {
            name,
            display_name: m.display_name.clone(),
            input_token_limit: m.context_window,
            output_token_limit: m.max_output_tokens,
        },
        extra_for(&m.extras, D),
        verbatim_from_form(&m.extras, D, ""),
    )
}

pub(crate) fn encode_models(list: &ModelList) -> Result<Vec<u8>, KppError> {
    let own = list.extras.is_from(D);
    let token = own
        .then(|| list.extras.fields.get(NEXT_PAGE_TOKEN))
        .flatten()
        .and_then(|t| t.parse::<String>().ok());
    let next_page_token = match token {
        Some(t) => Some(t),
        None if list.has_more => list.models.last().map(|m| m.id.clone()),
        None => None,
    };
    let mut extra = extra_for(&list.extras, D);
    extra.shift_remove(NEXT_PAGE_TOKEN);
    let w = ListW {
        models: list.models.iter().map(model_to).collect(),
        next_page_token,
    };
    to_body(&Ext::with(
        w,
        extra,
        verbatim_from_form(&list.extras, D, ""),
    ))
}

pub(crate) fn encode_model(model: &ModelInfo) -> Result<Vec<u8>, KppError> {
    to_body(&model_to(model))
}

pub(crate) fn decode_models(body: &[u8]) -> Result<ModelList, KppError> {
    let origin = Origin::new(D);
    let w: Ext<ListW> = Ext::from_slice(body).map_err(|e| bad_listing(D, &without_position(&e)))?;
    let Ext {
        known,
        extra,
        verbatim,
    } = w;
    let mut list = ModelList::new(
        known
            .models
            .into_iter()
            .map(|m| model_from(m, &origin))
            .collect(),
    );
    list.extras = extras_from(&origin, extra, verbatim);
    if let Some(token) = known.next_page_token {
        list.has_more = true;
        list.extras.fields.insert(
            NEXT_PAGE_TOKEN.into(),
            RawJson::from_serialize(&token).map_err(|e| bad_listing(D, &e.to_string()))?,
        );
    }
    Ok(list)
}

/// The `nextPageToken` a decoded Gemini listing gave, `None` on its last page.
pub(crate) fn next_page_token(list: &ModelList) -> Option<String> {
    list.extras
        .is_from(D)
        .then(|| list.extras.fields.get(NEXT_PAGE_TOKEN))
        .flatten()
        .and_then(|t| t.parse::<String>().ok())
}

pub(crate) fn decode_model(body: &[u8]) -> Result<ModelInfo, KppError> {
    let w: Ext<ModelW> =
        Ext::from_slice(body).map_err(|e| bad_listing(D, &without_position(&e)))?;
    Ok(model_from(w, &Origin::new(D)))
}

pub(crate) fn retrieve_request(
    model: &str,
    profile: &GeminiProfile,
) -> Result<EncodedRequest, KppError> {
    Ok(EncodedRequest::get(
        profile.model_path(model)?,
        HeaderList::new(),
    ))
}

/// `limit` becomes `pageSize` and `after_id` the `pageToken` a listing's `nextPageToken`
/// gave; Gemini pages only forward and filters nothing.
pub(crate) fn list_request(
    query: &ModelsQuery,
    profile: &GeminiProfile,
) -> Result<EncodedRequest, KppError> {
    let refused = [
        ("before_id", query.before_id.is_some()),
        ("lifecycle", !query.lifecycle.is_empty()),
        ("query parameters", !query.extras.fields.is_empty()),
    ]
    .into_iter()
    .find(|(_, set)| *set);
    if let Some((name, _)) = refused {
        return Err(KppError::unsupported_by(
            name,
            D,
            "Gemini lists models a page at a time with `pageSize` and `pageToken` only.",
        )
        .with_param(name));
    }
    let mut path = profile.list_path().to_owned();
    let mut sep = '?';
    if let Some(limit) = query.limit {
        path.push_str(&format!("{sep}pageSize={limit}"));
        sep = '&';
    }
    if let Some(token) = &query.after_id {
        path.push(sep);
        path.push_str("pageToken=");
        percent_encode(&mut path, token);
    }
    Ok(EncodedRequest::get(path, HeaderList::new()))
}
