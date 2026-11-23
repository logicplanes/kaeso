//! Responses a provider keeps for an app (OpenAI's `store` and `previous_response_id`): which
//! requests may ask for them, which project owns each one, and the record Kaeso keeps of it.

use kaeso_codecs::openai_responses::StoredState;
use kaeso_core::provider::AuthMethod;
use kaeso_core::providers::ProviderRef;
use kaeso_kpp::{DialectId, ErrorKind, KppError, Request};
use kaeso_store::pg::stored_responses::{self as store, StoredResponse};
use sha2::{Digest, Sha256};
use time::Duration;
use uuid::Uuid;

use crate::model_api::errors::error;
use crate::model_api::hooks::Caller;
use crate::services::credentials::Resolved;
use crate::state::AppState;

/// How long OpenAI keeps a stored response: "at least 30 days"
/// (https://developers.openai.com/api/docs/guides/migrate-to-responses).
pub const RETENTION: Duration = Duration::days(30);

const SETTING: &str = "Allow responses stored at the provider";

pub fn id_hash(id: &str) -> Vec<u8> {
    Sha256::digest(id.as_bytes()).to_vec()
}

/// What a request keeps at its provider, once it may.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keeping {
    /// The provider keeps the reply, so Kaeso records it for the project.
    pub records: bool,
    /// The stored response the request continues.
    pub continues: Option<StoredResponse>,
}

impl Keeping {
    /// The tokens of the conversation the request continues, which the provider bills as input
    /// although the request doesn't carry them.
    pub fn earlier_tokens(&self) -> u64 {
        self.continues
            .as_ref()
            .map_or(0, |r| u64::try_from(r.total_tokens).unwrap_or(0))
    }
}

pub struct Asked<'a> {
    pub caller: &'a Caller,
    pub ingress: DialectId,
    pub egress: DialectId,
    pub provider: &'a ProviderRef,
    pub model: &'a str,
    pub credential: &'a Resolved,
}

/// Why a request may not keep or continue a response: a model that can't (an alias moves on to
/// its next step) or a request that can't anywhere.
#[derive(Debug)]
pub enum Refusal {
    Unsupported(KppError),
    Final(KppError),
}

fn refused(field: &str, message: String) -> KppError {
    error(ErrorKind::InvalidRequest, 400, message).with_param(field)
}

/// Not found, as OpenAI answers a `previous_response_id` it doesn't know, so an id never tells
/// whether another project holds it.
pub fn unknown(id: &str, param: &str) -> KppError {
    error(
        ErrorKind::NotFound,
        404,
        format!("Previous response with id '{id}' not found."),
    )
    .with_param(param)
}

/// Whether the request may keep or continue a response at its provider, and what Kaeso records.
/// `None` when it asks for nothing and its reply isn't kept either.
pub async fn admit(
    state: &AppState,
    asked: &Asked<'_>,
    req: &Request,
) -> Result<Option<Keeping>, Refusal> {
    let (Request::Generate(g) | Request::CountTokens(g)) = req else {
        return Ok(None);
    };
    if asked.ingress != DialectId::OpenAiResponses {
        return Ok(None);
    }
    if kaeso_codecs::openai_responses::has_item_reference(g) {
        return Err(Refusal::Final(refused(
            "input",
            "`item_reference` items point to provider-stored state Kaeso cannot check for this project; send the items themselves in `input` instead.".to_owned(),
        )));
    }
    let wanted: StoredState = kaeso_codecs::openai_responses::stored_state(g);
    let field = wanted.field();
    let provider = asked.provider.name();
    let keeps =
        asked.egress == DialectId::OpenAiResponses && asked.provider.responses().stores_responses;
    if !keeps {
        if !wanted.asked() {
            return Ok(None);
        }
        return Err(Refusal::Unsupported(refused(
            field,
            format!(
                "`{field}` needs a provider that keeps responses, called in the Responses format; {provider} serves `{}` in another way. Send the whole conversation in `input` instead.",
                asked.model
            ),
        )));
    }
    if asked.credential.method != AuthMethod::ApiKey {
        if !wanted.asked() {
            return Ok(None);
        }
        return Err(Refusal::Unsupported(refused(
            field,
            format!(
                "`{field}` needs an API key of {provider}; this project's {provider} key is an account sign-in, which keeps no responses. Send the whole conversation in `input` instead."
            ),
        )));
    }
    if !crate::services::response_cache::stores_responses(state, asked.caller.project_id).await {
        if !wanted.asked() {
            return Ok(None);
        }
        return Err(Refusal::Final(refused(
            field,
            format!(
                "Responses stored at the provider are off for this project; an admin can turn on \"{SETTING}\" in the organization's and the project's settings, or send `store: false` and the whole conversation in `input`."
            ),
        )));
    }
    let continues = match &wanted.previous {
        Some(id) => {
            let found = store::find(
                &state.pg,
                asked.caller.org_id,
                asked.caller.project_id,
                &id_hash(id),
                state.clock.now(),
            )
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "cannot read a stored response");
                Refusal::Final(error(
                    ErrorKind::ProviderError,
                    503,
                    "Kaeso cannot check the stored response right now. Try again shortly.",
                ))
            })?
            .ok_or_else(|| Refusal::Final(unknown(id, "previous_response_id")))?;
            if found.provider != asked.provider.id() {
                return Err(Refusal::Unsupported(refused(
                    "previous_response_id",
                    format!(
                        "The response `{id}` is kept at another provider than {provider}; continue it with a model of that provider."
                    ),
                )));
            }
            if found.credential_id != asked.credential.id.uuid() {
                return Err(Refusal::Final(replaced(id)));
            }
            Some(found)
        }
        None => None,
    };
    Ok(Some(Keeping {
        records: matches!(req, Request::Generate(_)) && wanted.store != Some(false),
        continues,
    }))
}

pub fn replaced(id: &str) -> KppError {
    error(
        ErrorKind::InvalidRequest,
        409,
        format!(
            "The provider key that stored the response `{id}` was replaced, so Kaeso can't reach it."
        ),
    )
    .with_param("previous_response_id")
}

/// What a finished request that the provider keeps leaves to record.
#[derive(Clone, Debug)]
pub struct Kept {
    pub credential: Uuid,
    pub provider: String,
    pub model: String,
    pub catalogue_model_id: Uuid,
    pub response_id: Option<String>,
}

pub async fn record(state: &AppState, caller: &Caller, request_id: Uuid, kept: &Kept, tokens: u64) {
    let Some(id) = kept.response_id.as_deref().filter(|id| !id.is_empty()) else {
        return;
    };
    let row = StoredResponse {
        id_hash: id_hash(id),
        org_id: caller.org_id,
        project_id: caller.project_id,
        credential_id: kept.credential,
        provider: kept.provider.clone(),
        model: kept.model.clone(),
        catalogue_model_id: kept.catalogue_model_id,
        request_id,
        total_tokens: i64::try_from(tokens).unwrap_or(i64::MAX),
        expires_at: state.clock.now() + RETENTION,
    };
    if let Err(e) = store::insert(&state.pg, &row).await {
        tracing::error!(error = %e, "cannot record a stored response; continuing it will be refused");
    }
}
