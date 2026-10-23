use kaeso_kpp::{DialectId, Extras, KppError, ModelInfo, ModelList, ModelsQuery, Origin};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::codec::{EncodedRequest, HeaderList};
use crate::models::{
    MODELS_PATH, add_query_extra, bad_listing, decode_date, encode_date, format_date, model_path,
    none_or, parse_date, query_pairs, record_none,
};
use crate::wire::{
    Ext, Maybe, extra_for, extras_from, to_body, verbatim_from_form, without_position,
};

pub(crate) const DEFAULT_OWNED_BY: &str = "kaeso";

const OPENAI: [DialectId; 3] = [
    DialectId::OpenAiChat,
    DialectId::OpenAiResponses,
    DialectId::OpenAiEmbeddings,
];

const OBJECT: &str = "object";

const BARE_LIST: &str = "bare_list";

const PLAN_LIST: &str = "plan_list";

fn decode_object(extras: &mut Extras, object: Maybe<String>, usual: &str) {
    record_none(extras, OBJECT, &object, false);
    if let Maybe::Value(object) = object
        && object != usual
    {
        extras.form.insert(OBJECT.to_owned(), Value::from(object));
    }
}

fn encode_object(extras: &Extras, dialect: DialectId, usual: &str) -> Maybe<String> {
    match extras.form_str(dialect, OBJECT) {
        Some(object) => Maybe::Value(object.to_owned()),
        None => none_or(extras, dialect, OBJECT, Maybe::Value(usual.to_owned())),
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ModelWire {
    pub id: String,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub object: Maybe<String>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub created: Maybe<u64>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub owned_by: Maybe<String>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub shutdown_date: Maybe<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ListModelsResponse {
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub object: Maybe<String>,
    pub data: Vec<Ext<ModelWire>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct PlanModelWire {
    pub slug: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct PlanListWire {
    pub models: Vec<Ext<PlanModelWire>>,
}

fn source(extras: &Extras) -> Option<DialectId> {
    OPENAI.into_iter().find(|d| extras.is_from(*d))
}

pub(crate) fn decode_query(dialect: DialectId, query: &str) -> Result<ModelsQuery, KppError> {
    let mut q = ModelsQuery::default();
    q.extras = Extras::new(Origin::new(dialect));
    for (name, value) in query_pairs(query)? {
        add_query_extra(&mut q.extras, &name, &value);
    }
    Ok(q)
}

fn decode_info(dialect: DialectId, w: Ext<ModelWire>) -> Result<ModelInfo, KppError> {
    let Ext {
        known: w,
        extra,
        verbatim,
    } = w;
    let mut m = ModelInfo::new(w.id);
    let mut extras = extras_from(&Origin::new(dialect), extra, verbatim);
    decode_object(&mut extras, w.object, "model");
    record_none(&mut extras, "created", &w.created, false);
    record_none(&mut extras, "owned_by", &w.owned_by, false);
    record_none(&mut extras, "shutdown_date", &w.shutdown_date, true);
    m.created_at = w.created.into_option();
    m.owned_by = w.owned_by.into_option();
    if let Maybe::Value(date) = &w.shutdown_date {
        m.retires_at = Some(
            decode_date(&mut extras, "shutdown_date", date, parse_date, format_date).ok_or_else(
                || {
                    bad_listing(
                        dialect,
                        &format!("`shutdown_date` `{date}` is not a date (YYYY-MM-DD)"),
                    )
                },
            )?,
        );
    }
    m.extras = extras;
    Ok(m)
}

fn encode_info(m: &ModelInfo) -> Ext<ModelWire> {
    let ex = &m.extras;
    let Some(d) = source(ex) else {
        return Ext::new(ModelWire {
            id: m.id.clone(),
            object: Maybe::Value("model".to_owned()),
            created: Maybe::Value(m.created_at.unwrap_or(0)),
            owned_by: Maybe::Value(
                m.owned_by
                    .clone()
                    .unwrap_or_else(|| DEFAULT_OWNED_BY.to_owned()),
            ),
            shutdown_date: Maybe::from_option(m.retires_at.and_then(format_date), Maybe::Null),
        });
    };
    let w = ModelWire {
        id: m.id.clone(),
        object: encode_object(ex, d, "model"),
        created: Maybe::from_option(m.created_at, none_or(ex, d, "created", Maybe::Value(0))),
        owned_by: Maybe::from_option(
            m.owned_by.clone(),
            none_or(ex, d, "owned_by", Maybe::Value(DEFAULT_OWNED_BY.to_owned())),
        ),
        shutdown_date: match m.retires_at {
            Some(s) => encode_date(ex, d, "shutdown_date", s, parse_date, format_date)
                .map_or(Maybe::Null, Maybe::Value),
            None => none_or(ex, d, "shutdown_date", Maybe::Null),
        },
    };
    Ext::with(w, extra_for(ex, d), verbatim_from_form(ex, d, ""))
}

pub(crate) fn encode_model(m: &ModelInfo) -> Result<Vec<u8>, KppError> {
    to_body(&encode_info(m))
}

pub(crate) fn encode_models(list: &ModelList) -> Result<Vec<u8>, KppError> {
    let ex = &list.extras;
    if source(ex).is_some_and(|d| ex.form_flag(d, BARE_LIST)) {
        return to_body(&list.models.iter().map(encode_info).collect::<Vec<_>>());
    }
    if let Some(d) = source(ex).filter(|d| ex.form_flag(*d, PLAN_LIST)) {
        let w = PlanListWire {
            models: list.models.iter().map(encode_plan_info).collect(),
        };
        return to_body(&Ext::with(
            w,
            extra_for(ex, d),
            verbatim_from_form(ex, d, ""),
        ));
    }
    let w = ListModelsResponse {
        object: source(ex).map_or(Maybe::Value("list".to_owned()), |d| {
            encode_object(ex, d, "list")
        }),
        data: list.models.iter().map(encode_info).collect(),
    };
    match source(ex) {
        Some(d) => to_body(&Ext::with(
            w,
            extra_for(ex, d),
            verbatim_from_form(ex, d, ""),
        )),
        None => to_body(&w),
    }
}

pub(crate) fn decode_model(dialect: DialectId, body: &[u8]) -> Result<ModelInfo, KppError> {
    let w = Ext::<ModelWire>::from_slice(body)
        .map_err(|e| bad_listing(dialect, &without_position(&e)))?;
    decode_info(dialect, w)
}

fn encode_plan_info(m: &ModelInfo) -> Ext<PlanModelWire> {
    let w = PlanModelWire { slug: m.id.clone() };
    match source(&m.extras) {
        Some(d) => Ext::with(
            w,
            extra_for(&m.extras, d),
            verbatim_from_form(&m.extras, d, ""),
        ),
        None => Ext::new(w),
    }
}

pub(crate) fn decode_models(dialect: DialectId, body: &[u8]) -> Result<ModelList, KppError> {
    if body.trim_ascii_start().starts_with(b"[") {
        return decode_bare_list(dialect, body);
    }
    let Ext {
        known: w,
        extra,
        verbatim,
    } = match Ext::<ListModelsResponse>::from_slice(body) {
        Ok(listing) => listing,
        Err(e) => {
            return Ext::<PlanListWire>::from_slice(body)
                .map(|plan| decode_plan_list(dialect, plan))
                .map_err(|_| bad_listing(dialect, &without_position(&e)));
        }
    };
    let models = w
        .data
        .into_iter()
        .map(|m| decode_info(dialect, m))
        .collect::<Result<Vec<_>, _>>()?;
    let mut list = ModelList::new(models);
    let mut extras = extras_from(&Origin::new(dialect), extra, verbatim);
    decode_object(&mut extras, w.object, "list");
    list.extras = extras;
    Ok(list)
}

fn decode_plan_list(dialect: DialectId, plan: Ext<PlanListWire>) -> ModelList {
    let origin = Origin::new(dialect);
    let Ext {
        known: w,
        extra,
        verbatim,
    } = plan;
    let models = w
        .models
        .into_iter()
        .map(|m| {
            let Ext {
                known,
                extra,
                verbatim,
            } = m;
            let mut info = ModelInfo::new(known.slug);
            info.extras = extras_from(&origin, extra, verbatim);
            info
        })
        .collect();
    let mut list = ModelList::new(models);
    list.extras = extras_from(&origin, extra, verbatim);
    list.extras.form.insert(PLAN_LIST.to_owned(), true.into());
    list
}

fn decode_bare_list(dialect: DialectId, body: &[u8]) -> Result<ModelList, KppError> {
    let data: Vec<Ext<ModelWire>> =
        serde_json::from_slice(body).map_err(|e| bad_listing(dialect, &without_position(&e)))?;
    let models = data
        .into_iter()
        .map(|m| decode_info(dialect, m))
        .collect::<Result<Vec<_>, _>>()?;
    let mut list = ModelList::new(models);
    list.extras = Extras::new(Origin::new(dialect)).with_form(BARE_LIST, true.into());
    Ok(list)
}

pub(crate) fn retrieve_request(model: &str) -> EncodedRequest {
    EncodedRequest::get(model_path(model), HeaderList::new())
}

fn first_query_param(query: &ModelsQuery) -> Option<&'static str> {
    [
        ("limit", query.limit.is_some()),
        ("after_id", query.after_id.is_some()),
        ("before_id", query.before_id.is_some()),
        ("lifecycle", !query.lifecycle.is_empty()),
        ("query parameters", !query.extras.is_empty()),
    ]
    .into_iter()
    .find_map(|(name, set)| set.then_some(name))
}

pub(crate) fn list_request(
    dialect: DialectId,
    query: &ModelsQuery,
) -> Result<EncodedRequest, KppError> {
    if let Some(param) = first_query_param(query) {
        return Err(KppError::unsupported_by(
            param,
            dialect,
            "OpenAI lists every model in one page; send the listing without it.",
        ));
    }
    Ok(EncodedRequest::get(MODELS_PATH, HeaderList::new()))
}
