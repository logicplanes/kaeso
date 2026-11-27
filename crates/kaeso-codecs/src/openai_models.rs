use kaeso_kpp::{
    DialectId, Extras, KppError, ModelFeature, ModelInfo, ModelList, ModelPrices, ModelsQuery,
    Origin,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::codec::{EgressCtx, EncodedRequest, HeaderList, under_api_root};
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
    // What the model is, in the fields OpenRouter's listing names (`GET /api/v1/models`,
    // https://openrouter.ai/docs/api-reference/models/get-models), which apps that read OpenAI-
    // format listings take a model's name, context window, modalities and prices from. Written
    // only: a listing that sends them keeps them as extras, so they are written back as sent.
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u64>,
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub architecture: Option<ArchitectureWire>,
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub pricing: Option<PricingWire>,
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub top_provider: Option<TopProviderWire>,
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub supported_parameters: Option<Vec<&'static str>>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct ArchitectureWire {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_modalities: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_modalities: Option<Vec<String>>,
}

// USD per token, as decimal strings.
#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct PricingWire {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_cache_read: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_cache_write: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_reasoning: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct TopProviderWire {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u64>,
}

fn architecture(m: &ModelInfo) -> Option<ArchitectureWire> {
    let (input, output) = (&m.input_modalities, &m.output_modalities);
    if input.is_none() && output.is_none() {
        return None;
    }
    Some(ArchitectureWire {
        modality: input
            .as_ref()
            .zip(output.as_ref())
            .filter(|(i, o)| !i.is_empty() && !o.is_empty())
            .map(|(i, o)| format!("{}->{}", i.join("+"), o.join("+"))),
        input_modalities: input.clone(),
        output_modalities: output.clone(),
    })
}

fn pricing(prices: &ModelPrices) -> Option<PricingWire> {
    let w = PricingWire {
        prompt: prices.input.as_deref().and_then(per_token),
        completion: prices.output.as_deref().and_then(per_token),
        input_cache_read: prices.cache_read.as_deref().and_then(per_token),
        input_cache_write: prices.cache_write.as_deref().and_then(per_token),
        internal_reasoning: prices.reasoning.as_deref().and_then(per_token),
    };
    let any = [
        &w.prompt,
        &w.completion,
        &w.input_cache_read,
        &w.input_cache_write,
        &w.internal_reasoning,
    ]
    .iter()
    .any(|p| p.is_some());
    any.then_some(w)
}

// A price per million tokens as a price per token, exactly: `"3.00"` is `"0.000003"`. A value
// that is not a plain non-negative decimal gives `None`.
pub(crate) fn per_token(per_million: &str) -> Option<String> {
    let (int, frac) = per_million.split_once('.').unwrap_or((per_million, ""));
    let plain = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if (int.is_empty() && frac.is_empty()) || !plain(int) || !plain(frac) {
        return None;
    }
    let mut digits = format!("{int}{frac}");
    let mut scale = frac.len() + 6;
    while scale > 0 && digits.ends_with('0') {
        digits.pop();
        scale -= 1;
    }
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some("0".to_owned());
    }
    if scale == 0 {
        return Some(digits.to_owned());
    }
    match digits.len().checked_sub(scale) {
        Some(0) | None => Some(format!("0.{}{digits}", "0".repeat(scale - digits.len()))),
        Some(n) => {
            let (whole, part) = digits.split_at_checked(n)?;
            Some(format!("{whole}.{part}"))
        }
    }
}

// The request parameters of Kaeso's Chat Completions endpoint each feature stands for, in the
// names OpenRouter's `supported_parameters` uses (`structured_outputs` means a `json_schema`
// `response_format` is honoured).
fn parameters(features: &[ModelFeature]) -> Vec<&'static str> {
    let mut out = Vec::new();
    for f in features {
        out.extend_from_slice(match f {
            ModelFeature::Tools => &["tools", "tool_choice"][..],
            ModelFeature::Reasoning => &["reasoning", "reasoning_effort"],
            ModelFeature::StructuredOutput => &["response_format", "structured_outputs"],
            ModelFeature::Temperature => &["temperature"],
            _ => &[],
        });
    }
    out
}

fn describe(w: &mut ModelWire, m: &ModelInfo) {
    w.name.clone_from(&m.display_name);
    w.context_length = m.context_window;
    w.architecture = architecture(m);
    w.pricing = m.prices.as_ref().and_then(pricing);
    if m.context_window.is_some() || m.max_output_tokens.is_some() {
        w.top_provider = Some(TopProviderWire {
            context_length: m.context_window,
            max_completion_tokens: m.max_output_tokens,
        });
    }
    w.supported_parameters = m.features.as_deref().map(parameters);
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
        let mut w = ModelWire {
            id: m.id.clone(),
            object: Maybe::Value("model".to_owned()),
            created: Maybe::Value(m.created_at.unwrap_or(0)),
            owned_by: Maybe::Value(
                m.owned_by
                    .clone()
                    .unwrap_or_else(|| DEFAULT_OWNED_BY.to_owned()),
            ),
            shutdown_date: Maybe::from_option(m.retires_at.and_then(format_date), Maybe::Null),
            ..ModelWire::default()
        };
        describe(&mut w, m);
        return Ext::new(w);
    };
    let mut w = ModelWire {
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
        ..ModelWire::default()
    };
    describe(&mut w, m);
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

/// The request for one model under the provider's API root ([`EgressCtx::api_root`]), such as
/// Volcengine Ark's `/api/v3/models/{model}` in place of `/v1/models/{model}`.
pub(crate) fn retrieve_request_in(model: &str, ctx: &EgressCtx) -> EncodedRequest {
    rooted(retrieve_request(model), ctx)
}

/// The listing request under the provider's API root ([`EgressCtx::api_root`]).
pub(crate) fn list_request_in(
    dialect: DialectId,
    query: &ModelsQuery,
    ctx: &EgressCtx,
) -> Result<EncodedRequest, KppError> {
    list_request(dialect, query).map(|r| rooted(r, ctx))
}

fn rooted(mut request: EncodedRequest, ctx: &EgressCtx) -> EncodedRequest {
    request.path = under_api_root(ctx, &request.path);
    request
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
