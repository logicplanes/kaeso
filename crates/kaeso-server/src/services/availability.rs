use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use kaeso_codecs::{EncodedRequest, HeaderList, dispatch};
use kaeso_core::catalogue::{Effective, Entry};
use kaeso_kpp::{DialectId, ErrorKind, KppError, ModelInfo, ModelList, ModelsQuery};
use kaeso_store::StoreError;
use kaeso_store::pg::catalogue::{self, Answer, Recorded};
use kaeso_store::pg::credentials::KeyInUse;
use kaeso_upstream::{Call, Outgoing, ResponseBody, Timeouts};
use moka::future::Cache;
use moka::ops::compute::{CompResult, Op};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::model_api::pipeline::{declared_key_refusal, without_key_fragments};
use crate::services::credentials::Resolved;
use crate::state::AppState;

const MAX_LISTING_PAGES: usize = 50;

pub const RECHECK_EVERY: time::Duration = time::Duration::minutes(5);

const REMEMBER_ANSWERS: Duration = Duration::from_secs(60);

const REMEMBERED_MAX: u64 = 100_000;

#[derive(Clone, Debug)]
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
    Failed(KppError),
}

type ListingKey = (&'static str, DialectId, KeyInUse);

type ProbeKey = (&'static str, DialectId, KeyInUse, String);

#[derive(Debug)]
pub struct Listings(Cache<ListingKey, Arc<Listing>>);

impl Default for Listings {
    fn default() -> Self {
        Listings(Cache::builder().max_capacity(REMEMBERED_MAX).build())
    }
}

impl Listings {
    fn expiring(after: Duration) -> Self {
        Listings(
            Cache::builder()
                .max_capacity(REMEMBERED_MAX)
                .time_to_live(after)
                .build(),
        )
    }

    async fn get(&self, key: ListingKey, asked: &Asked<'_>) -> Arc<Listing> {
        self.0
            .get_with(key, async { Arc::new(asked.listing().await) })
            .await
    }
}

#[derive(Debug)]
pub struct Recall {
    listings: Listings,
    probes: Cache<ProbeKey, Probe>,
    attempts: Cache<(KeyInUse, Uuid), OffsetDateTime>,
}

impl Default for Recall {
    fn default() -> Self {
        Recall {
            listings: Listings::expiring(REMEMBER_ANSWERS),
            probes: Cache::builder()
                .max_capacity(REMEMBERED_MAX)
                .time_to_live(REMEMBER_ANSWERS)
                .build(),
            attempts: Cache::builder()
                .max_capacity(REMEMBERED_MAX)
                .time_to_live(RECHECK_EVERY.unsigned_abs())
                .build(),
        }
    }
}

impl Recall {
    pub async fn probe(&self, state: &AppState, credential: &Resolved, eff: &Effective) -> Probe {
        let key = (
            eff.provider.id,
            eff.dialect,
            credential.key(),
            eff.model_id.clone(),
        );
        self.probes
            .get_with(key, probe(state, credential, eff, &self.listings))
            .await
    }

    pub async fn claim_recheck(&self, key: KeyInUse, entry: &Entry, now: OffsetDateTime) -> bool {
        let checked = entry.availability_checked_at;
        let claimed = self
            .attempts
            .entry((key, entry.id))
            .and_compute_with(|attempt| {
                let last = attempt.map(|a| a.into_value()).max(checked);
                std::future::ready(if recheck_due(last, now) {
                    Op::Put(now)
                } else {
                    Op::Nop
                })
            })
            .await;
        matches!(
            claimed,
            CompResult::Inserted(_) | CompResult::ReplacedWith(_)
        )
    }
}

pub fn recheck_due(last: Option<OffsetDateTime>, now: OffsetDateTime) -> bool {
    last.is_none_or(|at| now >= at + RECHECK_EVERY)
}

struct Asked<'a> {
    state: &'a AppState,
    credential: &'a Resolved,
    eff: &'a Effective,
}

impl Asked<'_> {
    fn account(&self) -> Option<&crate::services::provider_accounts::Hosts> {
        self.state.accounts.hosts(self.credential.method)
    }

    fn dialect(&self) -> DialectId {
        if self.account().is_some() {
            DialectId::OpenAiResponses
        } else {
            self.eff.dialect
        }
    }

    async fn send(&self, request: EncodedRequest) -> Result<(u16, HeaderList, Bytes), KppError> {
        let account_base = match self.account() {
            Some(hosts) => Some(
                hosts
                    .api_base()
                    .map_err(|e| KppError::provider(e.to_string()))?,
            ),
            None => None,
        };
        let Some(base) = account_base
            .as_ref()
            .or_else(|| crate::model_api::pipeline::base_url(self.state, self.eff.provider.id))
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
        let request = dispatch::encode_models_request(self.dialect(), query)?;
        let (status, headers, body) = self.send(request).await?;
        let mut page = dispatch::decode_models(self.dialect(), status, &headers, &body)
            .map_err(|e| without_key_fragments(declared_key_refusal(self.eff.provider, e)))?;
        if self.account().is_some() {
            page.models.retain(listed);
        }
        Ok(page)
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
                Listing::Failed(e)
            }
        }
    }
}

// SIWC "Models and inference": an account's catalogue lists every model it may name, and
// only those with `visibility: "list"` are meant to be offered; it has no per-model route.
fn listed(model: &ModelInfo) -> bool {
    model
        .extras
        .fields
        .get("visibility")
        .is_some_and(|v| v.get() == r#""list""#)
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
    if asked.account().is_some() {
        let listing = listings
            .get((eff.provider.id, asked.dialect(), credential.key()), &asked)
            .await;
        return match listing.as_ref() {
            Listing::Complete(ids) if ids.contains(&eff.model_id) => Probe::Served,
            Listing::Complete(_) => Probe::Unserved,
            Listing::Failed(e) if e.kind == ErrorKind::Authentication => Probe::Failed(e.clone()),
            Listing::Failed(e) => Probe::Inconclusive(e.clone()),
            Listing::Unfinished => Probe::Inconclusive(KppError::provider(
                "the account's model listing did not end",
            )),
        };
    }
    let request = match dispatch::encode_model_request(eff.dialect, &eff.model_id) {
        Ok(r) => r,
        Err(e) => return Probe::Failed(e),
    };
    let not_found = match asked.send(request).await {
        Ok((status, headers, body)) => {
            match dispatch::decode_model(eff.dialect, status, &headers, &body)
                .map_err(|e| without_key_fragments(declared_key_refusal(eff.provider, e)))
            {
                Ok(_) => return Probe::Served,
                Err(e) if e.kind == ErrorKind::NotFound => e,
                Err(e) => return Probe::Failed(e),
            }
        }
        Err(e) => return Probe::Failed(e),
    };
    let listing = listings
        .get((eff.provider.id, eff.dialect, credential.key()), &asked)
        .await;
    match listing.as_ref() {
        Listing::Complete(ids) if ids.contains(&eff.model_id) => Probe::Served,
        Listing::Complete(_) => Probe::Unserved,
        Listing::Failed(e) if e.kind == ErrorKind::Authentication => Probe::Failed(e.clone()),
        Listing::Unfinished | Listing::Failed(_) => Probe::Inconclusive(not_found),
    }
}

pub async fn record(state: &AppState, answer: &Answer<'_>) {
    match write(state, answer).await {
        Ok(Recorded::Changed) => {
            crate::services::catalogue::forget(state, answer.project_id).await;
        }
        Ok(Recorded::Unchanged | Recorded::EntryGone) => {}
        Ok(Recorded::KeyChanged) => {
            tracing::info!(
                provider = answer.provider,
                "the provider key changed while Kaeso asked the provider about a model; the answer is not recorded"
            );
        }
        Err(e) => tracing::warn!(error = %e, "cannot record whether the provider serves a model"),
    }
}

async fn write(state: &AppState, answer: &Answer<'_>) -> Result<Recorded, StoreError> {
    let mut tx = state.pg.begin().await?;
    let recorded = catalogue::record_availability(&mut tx, answer).await?;
    tx.commit().await?;
    Ok(recorded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeso_core::catalogue::{Availability, Fields};
    use time::macros::datetime;

    fn unavailable(checked: Option<OffsetDateTime>) -> Entry {
        Entry {
            id: Uuid::now_v7(),
            provider: "anthropic".into(),
            model_id: "claude-retired".into(),
            overrides: Fields::default(),
            custom: false,
            delisted: false,
            last_known: None,
            availability: Some(Availability::Unavailable),
            availability_checked_at: checked,
        }
    }

    fn key(revision: i64) -> KeyInUse {
        KeyInUse {
            id: Uuid::from_u128(7),
            revision,
        }
    }

    #[tokio::test]
    async fn one_request_rechecks_a_model_once_its_answer_is_old() {
        let checked = datetime!(2026-10-19 12:00 UTC);
        let entry = unavailable(Some(checked));
        let recall = Recall::default();
        let soon = checked + RECHECK_EVERY - time::Duration::seconds(1);
        assert!(!recall.claim_recheck(key(1), &entry, soon).await);
        let due = checked + RECHECK_EVERY;
        assert!(recall.claim_recheck(key(1), &entry, due).await);
        assert!(!recall.claim_recheck(key(1), &entry, due).await);
        assert!(
            recall.claim_recheck(key(2), &entry, due).await,
            "another key asks on its own"
        );
        let later = due + RECHECK_EVERY - time::Duration::seconds(1);
        assert!(!recall.claim_recheck(key(1), &entry, later).await);
        assert!(
            recall
                .claim_recheck(key(1), &entry, due + RECHECK_EVERY)
                .await
        );
    }

    #[tokio::test]
    async fn concurrent_requests_claim_a_due_recheck_once() {
        let entry = unavailable(None);
        let recall = Recall::default();
        let now = datetime!(2026-10-19 12:00 UTC);
        let claims =
            futures::future::join_all((0..32).map(|_| recall.claim_recheck(key(1), &entry, now)))
                .await;
        assert_eq!(claims.iter().filter(|c| **c).count(), 1);
    }

    #[test]
    fn a_recheck_is_due_without_an_answer_or_after_the_interval() {
        let at = datetime!(2026-10-19 12:00 UTC);
        assert!(recheck_due(None, at));
        assert!(!recheck_due(Some(at), at));
        assert!(recheck_due(Some(at), at + RECHECK_EVERY));
    }
}
