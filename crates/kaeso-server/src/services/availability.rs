use kaeso_codecs::dispatch;
use kaeso_core::catalogue::{Availability, Effective};
use kaeso_kpp::{ErrorKind, KppError};
use kaeso_store::pg::catalogue;
use kaeso_upstream::{Call, Outgoing, ResponseBody, Timeouts};
use secrecy::SecretString;
use uuid::Uuid;

use crate::state::AppState;

#[derive(Debug)]
pub enum Probe {
    Served,
    Unserved,
    Failed(KppError),
}

impl Probe {
    pub fn availability(&self) -> Option<Availability> {
        match self {
            Probe::Served => Some(Availability::Available),
            Probe::Unserved => Some(Availability::Unavailable),
            Probe::Failed(_) => None,
        }
    }
}

pub async fn probe(state: &AppState, secret: &SecretString, eff: &Effective) -> Probe {
    let request = match dispatch::encode_model_request(eff.dialect, &eff.model_id) {
        Ok(r) => r,
        Err(e) => return Probe::Failed(e),
    };
    let Some(base) = crate::model_api::pipeline::base_url(state, eff.provider.id) else {
        return Probe::Failed(KppError::provider(format!(
            "no base URL for `{}`",
            eff.provider.id
        )));
    };
    let wait = state.config.upstream.first_byte_timeout;
    let sent = state
        .upstream
        .send(Call {
            base_url: base,
            auth: eff.provider.auth,
            credential: secret,
            request: Outgoing::from(request),
            stream: false,
            timeouts: Timeouts {
                request: wait,
                first_byte: wait,
                idle: wait,
            },
            max_body: state.config.body_limits.provider_response.as_usize(),
        })
        .await;
    let resp = match sent {
        Ok(r) => r,
        Err(e) => return Probe::Failed(crate::model_api::pipeline::upstream_error(&e)),
    };
    let ResponseBody::Full(body) = resp.body else {
        return Probe::Failed(KppError::provider("the model check arrived as a stream"));
    };
    match dispatch::decode_model(eff.dialect, resp.status, &resp.headers, &body) {
        Ok(_) => Probe::Served,
        Err(e) if e.kind == ErrorKind::NotFound => Probe::Unserved,
        Err(e) => Probe::Failed(e),
    }
}

pub async fn record(state: &AppState, project: Uuid, entry: Uuid, availability: Availability) {
    let at = state.clock.now();
    match catalogue::set_availability(&state.pg, project, entry, availability, at).await {
        Ok(_) => crate::services::catalogue::forget(state, project).await,
        Err(e) => tracing::warn!(error = %e, "cannot record whether the provider serves a model"),
    }
}
