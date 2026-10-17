use bytes::Bytes;
use kaeso_codecs::{EncodedRequest, HeaderList, dispatch};
use kaeso_core::catalogue::Effective;
use kaeso_kpp::{ErrorKind, KppError, ModelList, ModelsQuery};
use kaeso_store::StoreError;
use kaeso_store::pg::catalogue::{self, Answer, Recorded};
use kaeso_upstream::{Call, Outgoing, ResponseBody, Timeouts};
use secrecy::SecretString;

use crate::state::AppState;

const MAX_LISTING_PAGES: usize = 50;

#[derive(Debug)]
pub enum Probe {
    Served,
    Unserved,
    Inconclusive(KppError),
    Failed(KppError),
}

enum Listed {
    Yes,
    No,
    Unfinished,
}

struct Asked<'a> {
    state: &'a AppState,
    secret: &'a SecretString,
    eff: &'a Effective,
}

impl Asked<'_> {
    async fn send(&self, request: EncodedRequest) -> Result<(u16, HeaderList, Bytes), KppError> {
        let Some(base) = crate::model_api::pipeline::base_url(self.state, self.eff.provider.id)
        else {
            return Err(KppError::provider(format!(
                "no base URL for `{}`",
                self.eff.provider.id
            )));
        };
        let wait = self.state.config.upstream.first_byte_timeout;
        let resp = self
            .state
            .upstream
            .send(Call {
                base_url: base,
                auth: self.eff.provider.auth,
                credential: self.secret,
                request: Outgoing::from(request),
                stream: false,
                timeouts: Timeouts {
                    request: wait,
                    first_byte: wait,
                    idle: wait,
                },
                max_body: self.state.config.body_limits.provider_response.as_usize(),
            })
            .await
            .map_err(|e| crate::model_api::pipeline::upstream_error(&e))?;
        let ResponseBody::Full(body) = resp.body else {
            return Err(KppError::provider("the model check arrived as a stream"));
        };
        Ok((resp.status, resp.headers, body))
    }

    async fn page(&self, query: &ModelsQuery) -> Result<ModelList, KppError> {
        let request = dispatch::encode_models_request(self.eff.dialect, query)?;
        let (status, headers, body) = self.send(request).await?;
        dispatch::decode_models(self.eff.dialect, status, &headers, &body)
    }

    async fn listed(&self) -> Result<Listed, KppError> {
        let mut query = ModelsQuery::default();
        for _ in 0..MAX_LISTING_PAGES {
            let page = self.page(&query).await?;
            if page.models.iter().any(|m| m.id == self.eff.model_id) {
                return Ok(Listed::Yes);
            }
            if !page.has_more {
                return Ok(Listed::No);
            }
            let last = page.models.last().map(|m| m.id.clone());
            if last.is_none() || last == query.after_id {
                return Ok(Listed::Unfinished);
            }
            query.after_id = last;
        }
        Ok(Listed::Unfinished)
    }
}

pub async fn probe(state: &AppState, secret: &SecretString, eff: &Effective) -> Probe {
    let asked = Asked { state, secret, eff };
    let request = match dispatch::encode_model_request(eff.dialect, &eff.model_id) {
        Ok(r) => r,
        Err(e) => return Probe::Failed(e),
    };
    let not_found = match asked.send(request).await {
        Ok((status, headers, body)) => {
            match dispatch::decode_model(eff.dialect, status, &headers, &body) {
                Ok(_) => return Probe::Served,
                Err(e) if e.kind == ErrorKind::NotFound => e,
                Err(e) => return Probe::Failed(e),
            }
        }
        Err(e) => return Probe::Failed(e),
    };
    match asked.listed().await {
        Ok(Listed::Yes) => Probe::Served,
        Ok(Listed::No) => Probe::Unserved,
        Ok(Listed::Unfinished) => {
            tracing::info!(provider = eff.provider.id, model = %eff.model_id, pages = MAX_LISTING_PAGES, "the provider's model listing did not end");
            Probe::Inconclusive(not_found)
        }
        Err(e) => {
            tracing::info!(provider = eff.provider.id, model = %eff.model_id, kind = e.kind.as_str(), error = %e.message, "the provider's model listing cannot confirm a model not found");
            Probe::Inconclusive(not_found)
        }
    }
}

pub async fn record(state: &AppState, answer: &Answer<'_>) {
    match write(state, answer).await {
        Ok(Recorded::Yes) => crate::services::catalogue::forget(state, answer.project_id).await,
        Ok(Recorded::KeyChanged) => {
            tracing::info!(
                provider = answer.provider,
                "the provider key changed while Kaeso asked the provider about a model; the answer is not recorded"
            );
        }
        Ok(Recorded::EntryGone) => {}
        Err(e) => tracing::warn!(error = %e, "cannot record whether the provider serves a model"),
    }
}

async fn write(state: &AppState, answer: &Answer<'_>) -> Result<Recorded, StoreError> {
    let mut tx = state.pg.begin().await?;
    let recorded = catalogue::record_availability(&mut tx, answer).await?;
    tx.commit().await?;
    Ok(recorded)
}
