use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};

use bytes::Bytes;
use kaeso_codecs::{EncodedRequest, HeaderList, dispatch};
use kaeso_core::catalogue::Effective;
use kaeso_kpp::{DialectId, ErrorKind, KppError, ModelList, ModelsQuery};
use kaeso_store::StoreError;
use kaeso_store::pg::catalogue::{self, Answer, Recorded};
use kaeso_store::pg::credentials::KeyInUse;
use kaeso_upstream::{Call, Outgoing, ResponseBody, Timeouts};
use tokio::sync::OnceCell;

use crate::services::credentials::Resolved;
use crate::state::AppState;

const MAX_LISTING_PAGES: usize = 50;

#[derive(Debug)]
pub enum Probe {
    Served,
    Unserved,
    Inconclusive(KppError),
    Failed(KppError),
}

#[derive(Debug)]
enum Listing {
    Complete(HashSet<String>),
    Unfinished,
    Failed,
}

type ListingKey = (&'static str, DialectId, KeyInUse);

#[derive(Debug, Default)]
pub struct Listings(Mutex<HashMap<ListingKey, Arc<OnceCell<Listing>>>>);

impl Listings {
    fn cell(&self, key: ListingKey) -> Arc<OnceCell<Listing>> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(key)
            .or_default()
            .clone()
    }
}

struct Asked<'a> {
    state: &'a AppState,
    credential: &'a Resolved,
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
                credential: &self.credential.secret,
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

    async fn every_page(&self) -> Result<Option<HashSet<String>>, KppError> {
        let mut ids = HashSet::new();
        let mut query = ModelsQuery::default();
        for _ in 0..MAX_LISTING_PAGES {
            let page = self.page(&query).await?;
            let last = page.models.last().map(|m| m.id.clone());
            ids.extend(page.models.into_iter().map(|m| m.id));
            if !page.has_more {
                return Ok(Some(ids));
            }
            if last.is_none() || last == query.after_id {
                return Ok(None);
            }
            query.after_id = last;
        }
        Ok(None)
    }

    async fn listing(&self) -> Listing {
        let provider = self.eff.provider.id;
        match self.every_page().await {
            Ok(Some(ids)) => Listing::Complete(ids),
            Ok(None) => {
                tracing::info!(
                    provider,
                    pages = MAX_LISTING_PAGES,
                    "the provider's model listing did not end"
                );
                Listing::Unfinished
            }
            Err(e) => {
                tracing::info!(provider, kind = e.kind.as_str(), error = %e.message, "the provider's model listing cannot confirm a model not found");
                Listing::Failed
            }
        }
    }
}

pub async fn probe(
    state: &AppState,
    credential: &Resolved,
    eff: &Effective,
    listings: &Listings,
) -> Probe {
    let asked = Asked {
        state,
        credential,
        eff,
    };
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
    let cell = listings.cell((eff.provider.id, eff.dialect, credential.key()));
    match cell.get_or_init(|| asked.listing()).await {
        Listing::Complete(ids) if ids.contains(&eff.model_id) => Probe::Served,
        Listing::Complete(_) => Probe::Unserved,
        Listing::Unfinished | Listing::Failed => Probe::Inconclusive(not_found),
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
