use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use bytes::Bytes;
use futures::StreamExt;
use kaeso_codecs::continuity::reasoning_for_tool_calls;
use kaeso_codecs::gemini::GeminiAction;
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_core::catalogue::Entry;
use kaeso_core::catalogue::resolve::{Access, Alias, ResolveError, Resolved};
use kaeso_core::catalogue::route::{self, Failure as RouteFailure};
use kaeso_core::catalogue::{Availability, Effective, capability};
use kaeso_core::estimate::{self, Tally};
use kaeso_core::id::{
    ApiKeyId, CatalogueModelId, OrgId, ProjectId, RequestId, ServiceAccountId, UserId,
};
use kaeso_core::limits::WorstCase;
use kaeso_core::provider::AuthMethod;
use kaeso_core::providers::ProviderRef;
use kaeso_kpp::{
    DialectId, ErrorKind, ImageOperation, KppError, Reply, Request, RequestKind, StreamMode,
    TranscriptionOperation,
};
use kaeso_store::pg::catalogue::{Answer, ProjectModels};
use kaeso_upstream::{Call, Outgoing, ResponseBody, Timeouts, UpstreamError};
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

use super::auth;
use super::errors::{
    Served, TIMING_HEADER, Tried, Walked, error, respond, server_timing, stamp, stamp_route,
};
use super::hooks::{
    Admission, CLIENT_DISCONNECTED, CLIENT_STALLED, Caller, FellBack, Outcome, Reservation,
    SHUT_DOWN, Sink, Source,
};
use super::input_count::InputCounter;
use super::listing::project_models;
use super::stream;
use crate::http::decompress::{self, Encoding, Failed, Refused};
use crate::services::availability::{self, Probe};
use crate::services::continuity;
use crate::services::cooldowns;
use crate::services::credentials::{self, ResolveError as CredentialError};
use crate::services::log_media::Keep;
use crate::services::response_cache::{self, Verdict};
use crate::state::AppState;

const MAX_ERROR_BODY: usize = 1 << 20;

pub const OFF_THREAD_BYTES: usize = 256 * 1024;

#[derive(Debug)]
pub struct Incoming {
    pub ingress: DialectId,
    pub kind: RequestKind,
    pub headers: HeaderMap,
    pub body: Body,
    pub request_id: RequestId,
    pub client_ip: Option<IpAddr>,
    pub caller: Option<Caller>,
    pub source: Source,
    pub stream_only: bool,
    pub max_body: usize,
    pub alias: Option<Alias>,
    pub gemini: Option<GeminiCall>,
    pub image: Option<ImageOperation>,
    pub transcription: Option<TranscriptionOperation>,
    pub elevenlabs: Option<kaeso_codecs::elevenlabs::ElevenLabsCall>,
    pub video: Option<super::videos::VideoIncoming>,
    pub generation: Option<super::hooks::GenerationCall>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeminiCall {
    pub model: String,
    pub action: GeminiAction,
}

pub(super) struct Recording {
    state: AppState,
    sink: Arc<dyn Sink>,
    outcome: Option<Outcome>,
    started: Instant,
    // When the current attempt sent its request to its provider.
    sent_at: Option<Instant>,
    // How long the alias steps that failed waited for their providers, each from sending its
    // request until it failed. It is the providers' time, which their steps' durations show, so
    // Kaeso's overhead leaves it out (`own_time`).
    waited: Duration,
    reservation: Reservation,
    sent_input: Option<u64>,
    held: Option<super::memory::Held>,
    keep_in_cache: Option<response_cache::Slot>,
    // A response the provider keeps for the project, recorded once the request succeeded.
    pub(super) kept: Option<crate::services::stored_responses::Kept>,
    logging: Option<tokio::task::JoinHandle<(Bytes, Keep)>>,
    // Moves into a stream's pump, so a stream counts until it ends.
    serving: Option<crate::metrics::Serving>,
    // How long the current step may take, from its first try: its retries share it.
    step_wait: Option<Duration>,
    // `kaeso-limits` for the attempt that answered, or for the limit that refused it.
    limits: Option<HeaderValue>,
}

impl Recording {
    fn stamp_limits(&self, resp: &mut Response) {
        if let Some(v) = &self.limits {
            resp.headers_mut()
                .insert(super::errors::LIMITS_HEADER, v.clone());
        }
    }

    fn found_limits(&mut self, left: &[super::hooks::Left]) {
        let o = self.outcome();
        if o.sees_limits {
            let with_spend = o.sees_cost;
            self.limits = super::errors::limits_header(left, with_spend);
        }
    }

    pub(super) fn outcome(&mut self) -> &mut Outcome {
        self.outcome
            .get_or_insert_with(|| blank_outcome(RequestId::new()))
    }

    pub(super) fn state(&self) -> &AppState {
        &self.state
    }

    pub(super) fn arrived(&self) -> Instant {
        self.started
    }

    pub(super) fn reservation(&self) -> &Reservation {
        &self.reservation
    }

    // Kaeso's own time from the request's arrival until `at`: without the request upload and
    // without the failed steps' waits for their providers. Everything else stays in, as it is
    // Kaeso's work: routing, the cooldown and limit bookkeeping between steps, and each step's
    // own decoding, encoding and checks. Restarting the clock for each step would hide that.
    fn own_time(&mut self, at: Instant) -> Duration {
        let waited = self.waited;
        at.saturating_duration_since(self.started)
            .saturating_sub(self.outcome().upload.unwrap_or_default())
            .saturating_sub(waited)
    }

    // An attempt failed at `at`: the time since it sent its request was its provider's. An
    // attempt that failed before sending waited for no provider. Cooldowns only hold a step for
    // later requests and never wait, so no other wait of a step is the provider's.
    fn attempt_failed(&mut self, at: Instant) {
        if let Some(sent) = self.sent_at.take() {
            self.waited = self
                .waited
                .saturating_add(at.saturating_duration_since(sent));
        }
    }

    fn timing(&mut self) -> HeaderValue {
        let own = self.own_time(Instant::now());
        let o = self.outcome();
        server_timing(o.upload, o.overhead.unwrap_or(own), o.ttfb)
    }

    fn time(&mut self, resp: &mut Response) {
        let timing = self.timing();
        resp.headers_mut().insert(TIMING_HEADER, timing);
    }

    pub(super) async fn finish(mut self, status: u16) {
        let elapsed = self.started.elapsed();
        if let Some(mut o) = self.outcome.take() {
            o.status = status;
            if let (Some(kept), Some(caller)) = (self.kept.take(), o.caller.as_ref())
                && (200..300).contains(&status)
            {
                let tokens = o.usage.as_ref().map_or(0, kaeso_core::cost::total_tokens);
                crate::services::stored_responses::record(
                    &self.state,
                    caller,
                    o.request_id.uuid(),
                    &kept,
                    tokens,
                )
                .await;
            }
            if o.duration.is_zero() {
                o.duration = elapsed;
            }
            let charged = self.hand_over(o);
            if self.reservation.is_held() {
                let _ = charged.await;
            }
        }
    }

    fn hand_over(&mut self, mut o: Outcome) -> tokio::sync::oneshot::Receiver<()> {
        let (done, charged) = tokio::sync::oneshot::channel();
        let (sink, state) = (self.sink.clone(), self.state.clone());
        let logging = self.logging.take();
        self.state.in_flight.finish(async move {
            if let Some(logging) = logging {
                match logging.await {
                    Ok((body, left)) => {
                        o.request_body = Some(body);
                        o.request_logged = Some(left);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, request_id = %o.request_id, "cannot prepare a request's body for the log; it is logged without it");
                    }
                }
            }
            crate::services::guardrails::rules::mask_logged(&mut o);
            sink.finish_charged(&state, o, done).await;
        });
        charged
    }

    pub(super) async fn finish_cancelled(mut self) {
        mark_cancelled(self.outcome());
        self.finish(499).await;
    }

    pub(super) async fn finish_stalled(mut self) {
        let o = self.outcome();
        o.cancelled = true;
        o.error_kind = Some(CLIENT_STALLED.to_owned());
        o.error_message =
            Some("The app stopped reading the stream for too long; Kaeso ended it.".to_owned());
        self.finish(499).await;
    }

    pub(super) async fn fail(self, err: &KppError, status: u16) {
        self.fail_as(err.kind.as_str(), err, status).await;
    }

    pub(super) async fn fail_as(mut self, kind: &str, err: &KppError, status: u16) {
        let o = self.outcome();
        o.error_kind = Some(kind.to_owned());
        o.error_message = Some(err.message.clone());
        // Units are counted when the request is read; one refused before its model was known
        // made and asked nothing of any model.
        if o.model_id.is_none() {
            o.media = None;
        }
        self.finish(status).await;
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        if let Some(mut o) = self.outcome.take() {
            mark_cancelled(&mut o);
            o.status = 499;
            o.duration = self.started.elapsed();
            if let Some(input) = self.sent_input
                && o.usage.is_none()
            {
                o.usage = Some(kaeso_kpp::Usage::new(input, 0));
                o.usage_estimated = true;
            }
            drop(self.hand_over(o));
        }
    }
}

pub(crate) fn over_limit(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut cause = Some(error);
    while let Some(e) = cause {
        if e.is::<http_body_util::LengthLimitError>() {
            return true;
        }
        cause = e.source();
    }
    false
}

pub(super) fn shutting_down() -> KppError {
    error(
        ErrorKind::ProviderError,
        503,
        "Kaeso is restarting and ended this request. Send it again.",
    )
}

fn mark_cancelled(o: &mut Outcome) {
    o.cancelled = true;
    o.error_kind = Some(CLIENT_DISCONNECTED.to_owned());
    o.error_message =
        Some("The app closed the connection before the reply was complete.".to_owned());
}

pub(crate) fn blank_outcome(request_id: RequestId) -> Outcome {
    Outcome {
        request_id,
        caller: None,
        sees_cost: false,
        sees_limits: false,
        source: Source::Api,
        ingress: DialectId::OpenAiChat,
        kind: RequestKind::Generate,
        egress: None,
        provider: None,
        custom_provider: None,
        requested_model: None,
        catalogue_model_id: None,
        model_id: None,
        response_model: None,
        prices: None,
        stream: false,
        status: 0,
        error_kind: None,
        error_message: None,
        usage: None,
        started_at: time::OffsetDateTime::UNIX_EPOCH,
        duration: std::time::Duration::ZERO,
        ttfb: None,
        upload: None,
        overhead: None,
        stream_processing: None,
        user_agent: None,
        tags: Vec::new(),
        trace: None,
        client_ip: None,
        upstream_request_id: None,
        request_body: None,
        request_logged: None,
        reply: None,
        cancelled: false,
        stream_progress: None,
        usage_estimated: false,
        content_kept: false,
        media_kept: false,
        media_reply: None,
        media_reply_logged: None,
        cache_breakpoints: None,
        response_cache: None,
        alias: None,
        fallbacks: Vec::new(),
        gemini: None,
        image: None,
        transcription: None,
        elevenlabs: None,
        realtime: None,
        media: None,
        video: None,
        generation: None,
        content_type: None,
        guardrails: Vec::new(),
        log_masks: Vec::new(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stage {
    Caller,
    Request,
    Catalogue,
    UnknownModel(String),
    AmbiguousModel(String),
    ModelAccess(String),
    Setup,
    CustomProvidersOff,
    Capability,
    Limits,
    MissingCredential(String),
    SignedOut(&'static str),
    NoApiAccess(&'static str),
    NeedsApiKey {
        product: &'static str,
        provider: String,
        api: &'static str,
    },
    Credential,
    Upstream,
    Provider,
    KeyRefused,
    Unserved(String),
    Reply,
    Guardrail,
    Shutdown,
    /// An account's plan reached its usage limit: like a provider refusal, but it holds every
    /// model of the account.
    PlanLimit,
    /// `Cache-Control: only-if-cached` and nothing in the cache answers.
    NotCached,
}

#[derive(Clone, Debug)]
pub struct Failure {
    pub err: KppError,
    pub served: Served,
    pub stage: Stage,
    pub provider_said: Option<KppError>,
    /// The provider couldn't be reached or broke the connection: a second try may get
    /// through, unlike an address the install refuses or a reply too large to take.
    pub dropped: bool,
    // The provider's 404 is about something the request names, such as a file, while the
    // model is served: a refusal of this request, which holds no route step.
    pub not_found_in_request: bool,
}

impl Failure {
    fn new(err: KppError, stage: Stage) -> Failure {
        Failure {
            err,
            served: Served::default(),
            stage,
            provider_said: None,
            dropped: false,
            not_found_in_request: false,
        }
    }

    pub fn logged(&self) -> &KppError {
        self.provider_said.as_ref().unwrap_or(&self.err)
    }
}

pub async fn serve(state: AppState, inc: Incoming) -> Response {
    let ingress = inc.ingress;
    serve_with(state, inc, move |f: &Failure| {
        respond(ingress, &f.err, &f.served)
    })
    .await
}

pub async fn serve_with(
    state: AppState,
    inc: Incoming,
    answer: impl Fn(&Failure) -> Response,
) -> Response {
    let _in_flight = state.in_flight.token();
    let now = state.clock.now();
    let mut outcome = blank_outcome(inc.request_id);
    outcome.ingress = inc.ingress;
    outcome.kind = inc.kind;
    outcome.gemini = inc.gemini.as_ref().map(|c| c.action);
    outcome.image = inc.image;
    outcome.transcription = inc.transcription;
    outcome.elevenlabs = inc.elevenlabs.as_ref().map(|c| c.operation);
    outcome.video = inc.video.as_ref().map(|v| v.call);
    outcome.generation = inc.generation;
    outcome.source = inc.source;
    outcome.caller.clone_from(&inc.caller);
    outcome.started_at = now;
    outcome.client_ip = inc.client_ip;
    outcome.user_agent = inc
        .headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    outcome.trace = crate::services::otel::TraceParent::from_headers(&inc.headers);
    let mut rec = Recording {
        state: state.clone(),
        sink: state.sink.clone(),
        outcome: Some(outcome),
        started: Instant::now(),
        sent_at: None,
        waited: Duration::ZERO,
        reservation: Reservation::default(),
        sent_input: None,
        held: None,
        keep_in_cache: None,
        kept: None,
        logging: None,
        serving: Some(crate::metrics::Serving::start()),
        step_wait: None,
        limits: None,
    };
    let cut = state.in_flight.cut();
    let done = tokio::select! {
        done = run(&state, inc, &mut rec) => Some(done),
        () = cut.cancelled() => None,
    };
    let Some(done) = done else {
        let failure = Failure::new(shutting_down(), Stage::Shutdown);
        let mut resp = answer(&failure);
        rec.time(&mut resp);
        Box::pin(rec.fail_as(SHUT_DOWN, &failure.err, resp.status().as_u16())).await;
        return resp;
    };
    match done {
        Ok(Done::Complete(mut resp, status)) => {
            rec.time(&mut resp);
            stamp_guardrails(&mut resp, &rec.outcome().guardrails);
            rec.stamp_limits(&mut resp);
            let outcome = rec.outcome();
            // The start of a video job or an image generation is billed when the job ends.
            if (outcome.source == super::hooks::Source::Playground || outcome.sees_cost)
                && !crate::services::usage::billed_when_done(outcome)
            {
                outcome.status = status;
                cost_header(outcome, &mut resp);
            }
            rec.finish(status).await;
            resp
        }
        Ok(Done::Streaming(mut resp)) => {
            rec.outcome = None;
            rec.stamp_limits(&mut resp);
            resp
        }
        Ok(Done::Refused(failure)) => {
            rec.outcome = None;
            let mut resp = answer(&failure);
            rec.stamp_limits(&mut resp);
            resp
        }
        Err(failure) => {
            let mut resp = answer(&failure);
            rec.stamp_limits(&mut resp);
            if let Some(v) = rec.outcome().response_cache {
                resp.headers_mut()
                    .insert(response_cache::HEADER, v.header());
            }
            rec.time(&mut resp);
            stamp_guardrails(&mut resp, &rec.outcome().guardrails);
            Box::pin(rec.fail(failure.logged(), resp.status().as_u16())).await;
            resp
        }
    }
}

/// What a complete reply was charged, as its request log will say: the amount, `0` for a
/// response cache hit, or `unknown` where the model has no price for what it made. Playground
/// replies carry it, and model API replies whose key's owner may see the project's costs.
pub const COST_HEADER: &str = "kaeso-cost";

fn cost_header(outcome: &Outcome, resp: &mut Response) {
    let value = if crate::services::usage::cost_known(outcome) {
        crate::services::usage::costed(outcome)
            .1
            .normalize()
            .to_string()
    } else {
        "unknown".to_owned()
    };
    if let Ok(v) = HeaderValue::from_str(&value) {
        resp.headers_mut().insert(COST_HEADER, v);
    }
}

enum Done {
    Complete(Response, u16),
    Streaming(Response),
    // A stream that failed before its first byte: the stream's task logged it, and the app gets
    // the error in its own format.
    Refused(Failure),
}

pub(super) fn header_list(headers: &HeaderMap) -> HeaderList {
    headers
        .iter()
        .filter_map(|(n, v)| {
            v.to_str()
                .ok()
                .map(|v| (n.as_str().to_owned(), v.to_owned()))
        })
        .collect()
}

pub(crate) fn base_url<'a>(state: &'a AppState, provider: &'a ProviderRef) -> Option<&'a Url> {
    match provider.custom() {
        Some(c) => Some(c.base_url()),
        None => state
            .config
            .providers
            .get(provider.id())
            .map(|p| &p.base_url),
    }
}

pub(super) fn transport_kind(e: &UpstreamError) -> &'static str {
    match e {
        UpstreamError::Timeout(_) => "timeout",
        UpstreamError::Connect(_) => "connect",
        UpstreamError::Transfer(_) => "transfer",
        UpstreamError::TooLarge(_) => "too_large",
        UpstreamError::Refused(_) => "refused",
        UpstreamError::Path => "path",
        UpstreamError::Busy => "busy",
        UpstreamError::Client(_) | UpstreamError::Method(_) => "client",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EstimateReason {
    Unreported,
    Cancelled,
    Failed,
}

impl EstimateReason {
    fn as_str(self) -> &'static str {
        match self {
            EstimateReason::Unreported => "unreported",
            EstimateReason::Cancelled => "cancelled",
            EstimateReason::Failed => "failed",
        }
    }
}

pub(super) fn count_estimate(reason: EstimateReason, provider: &'static str) {
    metrics::counter!(crate::metrics::USAGE_ESTIMATES, "reason" => reason.as_str(), "provider" => provider)
        .increment(1);
}

pub(super) fn count_upstream_error(kind: &'static str, provider: &'static str) {
    metrics::counter!(crate::metrics::UPSTREAM_ERRORS, "kind" => kind, "provider" => provider)
        .increment(1);
}

pub(super) fn limited(refusal: super::hooks::Refusal) -> KppError {
    let kind = match refusal.status {
        503 => ErrorKind::ProviderError,
        403 => ErrorKind::PermissionDenied,
        _ => ErrorKind::RateLimited,
    };
    let mut e = error(kind, refusal.status, refusal.message);
    if let Some(r) = refusal.retry_after {
        e = e.with_retry_after(r);
    }
    if let Some(p) = refusal.param {
        e = e.with_param(p);
    }
    e
}

/// Counts a request that reaches the provider outside [`serve`] against the caller's request
/// rates.
pub(super) async fn admit_follow_up(
    state: &AppState,
    caller: &Caller,
    model: Option<uuid::Uuid>,
) -> Result<(), KppError> {
    crate::services::limits::admit_follow_up(state, caller, model, state.clock.now())
        .await
        .map_err(limited)
}

pub(super) fn key_caller(key: &kaeso_store::pg::keys::KeyAuth) -> Caller {
    Caller {
        org_id: key.org_id,
        project_id: key.project_id,
        key_id: Some(key.id),
        user_id: key.user_id,
        service_account_id: key.service_account_id,
    }
}

/// Custom providers reach only the networks the install opened for them; built-in providers'
/// base URLs are the operator's own settings.
pub(crate) fn guard_for(provider: &ProviderRef) -> kaeso_upstream::Guard {
    if provider.custom().is_some() {
        kaeso_upstream::Guard::Install
    } else {
        kaeso_upstream::Guard::Off
    }
}

pub(crate) fn upstream_error(e: &UpstreamError) -> KppError {
    match e {
        UpstreamError::Path => error(
            ErrorKind::InvalidRequest,
            400,
            "An id in this request can't be part of the provider's URL; use the real id.",
        ),
        UpstreamError::Timeout(what) => error(
            ErrorKind::Timeout,
            504,
            format!(
                "The provider did not answer in time ({what}). Kaeso does not retry; try again or raise the model's timeout."
            ),
        ),
        UpstreamError::Connect(_) | UpstreamError::Refused(_) => error(
            ErrorKind::ProviderError,
            502,
            "Kaeso cannot reach the provider. Try again shortly.",
        ),
        UpstreamError::Transfer(_) => error(
            ErrorKind::ProviderError,
            502,
            "The connection to the provider failed before the response was complete.",
        ),
        UpstreamError::Client(_) | UpstreamError::Method(_) => {
            tracing::error!(error = %e, cause = ?std::error::Error::source(e), "cannot prepare a provider call");
            error(
                ErrorKind::ProviderError,
                500,
                "Kaeso cannot prepare the call to the provider.",
            )
        }
        UpstreamError::Busy => answers_busy(),
        UpstreamError::TooLarge(limit) => {
            tracing::warn!(
                limit,
                "a provider response was larger than body_limits.provider_response"
            );
            error(
                ErrorKind::ProviderError,
                502,
                "The provider's response is larger than this install accepts.",
            )
        }
    }
}

// A connection that couldn't be made or broke: the only transport failures a second try may
// get past.
fn dropped(e: &UpstreamError) -> bool {
    matches!(e, UpstreamError::Connect(_) | UpstreamError::Transfer(_))
}

pub(crate) fn upstream_error_from(e: &UpstreamError, provider: &ProviderRef) -> KppError {
    let Some(custom) = provider.custom() else {
        return upstream_error(e);
    };
    let id = custom.id();
    match e {
        UpstreamError::Connect(_) => error(
            ErrorKind::ProviderError,
            502,
            format!("Kaeso cannot reach the custom provider `{id}`; check its base URL."),
        ),
        UpstreamError::Refused(_) => error(
            ErrorKind::ProviderError,
            502,
            format!(
                "The custom provider `{id}` is at an address this install does not allow; ask its operator."
            ),
        ),
        other => upstream_error(other),
    }
}

fn refused_custom_key(err: &KppError, provider: &ProviderRef, keyless: bool) -> Option<KppError> {
    let id = provider.custom()?.id();
    if !matches!(
        err.kind,
        ErrorKind::Authentication | ErrorKind::PermissionDenied | ErrorKind::Billing
    ) {
        return None;
    }
    let message = if keyless {
        format!(
            "The custom provider `{id}` asks for a key; an organization admin sets how it signs in under Providers."
        )
    } else {
        format!(
            "The custom provider `{id}` refused this project's key; ask an organization admin to check it under Providers."
        )
    };
    Some(error(ErrorKind::ProviderError, 502, message))
}

fn unreadable(err: KppError, provider: &ProviderRef, dialect: DialectId) -> KppError {
    match provider.custom() {
        Some(c) if err.kind == ErrorKind::ProviderError && err.raw.is_none() => {
            tracing::info!(provider = %c.id(), %dialect, error = %err.message, "a custom provider answered in a form its dialect cannot read");
            error(
                ErrorKind::ProviderError,
                502,
                format!(
                    "The custom provider `{}` answered in a form `{dialect}` cannot read; check its API format.",
                    c.id()
                ),
            )
        }
        _ => err,
    }
}

pub(super) const KEY_REFUSED: &str = "The provider refused this project's provider key. Ask an organization admin to check it under Providers.";

const PERMISSION_REFUSED: &str = "The provider doesn't allow this request with this project's provider key, for example for this model, plan or region. Ask an organization admin to check it under Providers.";

const AZURE_ROLE_REFUSED: &str = "Azure refused this project's Microsoft Entra ID service principal. It needs the Cognitive Services OpenAI User role on the resource; ask an organization admin to check it under Providers.";

const ENTRA_REFUSED: &str = "Microsoft Entra ID refused the service principal of this project's Azure key, so it is signed out. Ask an organization admin to replace it under Providers.";

const ENTRA_UNAVAILABLE: &str =
    "Kaeso can't get an access token from Microsoft Entra ID right now. Try again shortly.";

// The access token of an Azure key that is a service principal. A secret Entra ID refuses signs
// the key out, and the people who change provider keys are told.
async fn entra_token(
    state: &AppState,
    credential: &crate::services::credentials::Resolved,
    key: &crate::services::azure_auth::Principal,
) -> Result<secrecy::SecretString, (Stage, KppError)> {
    use crate::services::azure_auth::{self, TokenError};
    let org = key.org;
    match azure_auth::access_token(state, credential.id.uuid(), credential.revision, key).await {
        Ok(token) => Ok(token),
        Err(TokenError::Refused { .. }) => {
            let id = credential.id.uuid();
            if let Err(e) = kaeso_store::pg::credentials::sign_out(
                &state.pg,
                id,
                crate::services::credentials::SIGNED_OUT,
            )
            .await
            {
                tracing::error!(error = %e, "cannot mark a refused service principal signed out");
            }
            crate::services::credentials::forget_org(state, org).await;
            crate::services::notifications::system::account_signed_out(state, org, id).await;
            Err((
                Stage::SignedOut(crate::services::credentials::ENTRA_ID),
                error(ErrorKind::ProviderError, 502, ENTRA_REFUSED),
            ))
        }
        Err(e) => {
            tracing::warn!(error = %e, "cannot get an access token from Microsoft Entra ID");
            Err((
                Stage::Credential,
                error(ErrorKind::ProviderError, 503, ENTRA_UNAVAILABLE),
            ))
        }
    }
}

const ACCOUNT_REFUSED: &str = "The provider refused this project's provider key for billing or quota reasons. Ask an organization admin to check it under Providers.";

pub(crate) fn refused_key(err: &KppError) -> Option<KppError> {
    let message = match err.kind {
        ErrorKind::Authentication => KEY_REFUSED,
        ErrorKind::PermissionDenied => PERMISSION_REFUSED,
        ErrorKind::Billing => ACCOUNT_REFUSED,
        _ => return None,
    };
    Some(error(ErrorKind::ProviderError, 502, message))
}

fn redacted(mut err: KppError) -> KppError {
    err.message = kaeso_core::secret::redact_tokens(&err.message);
    err.raw = None;
    err
}

// For large work that borrows what it needs and so can't move to a blocking thread: the worker
// hands its other tasks to the rest of the runtime while it runs. A current-thread runtime, as
// in tests, has nowhere to hand them, so the work runs in place there.
pub(crate) fn in_place<T>(bytes: usize, work: impl FnOnce() -> T) -> T {
    let shared = tokio::runtime::Handle::try_current()
        .is_ok_and(|h| h.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread);
    if bytes >= OFF_THREAD_BYTES && shared {
        tokio::task::block_in_place(work)
    } else {
        work()
    }
}

// A large body is written as the log keeps it on a blocking thread while the request is
// decoded, so the raw body is let go once both are done instead of being held for the whole
// call. The allowance it leaves goes with it, so the record writes only the reply.
fn logged_early(body: Bytes, keep_media: bool) -> tokio::task::JoinHandle<(Bytes, Keep)> {
    tokio::task::spawn_blocking(move || {
        let mut keep = if keep_media {
            Keep::media()
        } else {
            Keep::nothing()
        };
        let logged = crate::services::log_media::shared_body(body, &mut keep);
        (logged, keep)
    })
}

// An upload as the log keeps it. Kept media is written as base64, which for a large upload is
// work for a blocking thread; the clone shares the upload's bytes.
async fn logged_upload<R: Clone + Send + 'static>(
    bytes: usize,
    request: &R,
    write: fn(&R, crate::services::log_media::Keep) -> Bytes,
    keep_media: bool,
) -> Option<Bytes> {
    let keep = if keep_media {
        crate::services::log_media::Keep::media()
    } else {
        crate::services::log_media::Keep::nothing()
    };
    let request = request.clone();
    off_thread(bytes, move || write(&request, keep)).await.ok()
}

fn unread_encoding(refused: Refused) -> KppError {
    let message = match refused {
        Refused::Unsupported(coding) => format!(
            "Kaeso reads request bodies sent as `gzip` or `zstd`, not `{}`.",
            coding.chars().take(40).collect::<String>()
        ),
        Refused::Stacked(codings) => format!(
            "Kaeso reads request bodies with one content encoding, not `{}`.",
            codings.chars().take(40).collect::<String>()
        ),
    };
    error(ErrorKind::InvalidRequest, 415, message)
}

async fn decoded_body(encoding: Encoding, raw: Bytes, limit: usize) -> Result<Bytes, KppError> {
    if encoding == Encoding::Identity {
        return Ok(raw);
    }
    // A small compressed body can decode to the whole limit, so it is never decoded inline.
    let bytes = raw.len().max(OFF_THREAD_BYTES);
    off_thread(bytes, move || decompress::decode(encoding, raw, limit))
        .await?
        .map_err(|failed| match failed {
            Failed::TooLarge => error(
                ErrorKind::RequestTooLarge,
                413,
                format!("The request body is larger than {limit} bytes once decoded."),
            ),
            Failed::Corrupt => error(
                ErrorKind::InvalidRequest,
                400,
                format!(
                    "The request body is not valid `{}`; check its `Content-Encoding`.",
                    encoding.name()
                ),
            ),
        })
}

pub(crate) async fn off_thread<T, F>(bytes: usize, work: F) -> Result<T, KppError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    if bytes < OFF_THREAD_BYTES {
        return Ok(work());
    }
    tokio::task::spawn_blocking(work).await.map_err(|e| {
        tracing::error!(error = %e, "a translation on a blocking thread did not finish");
        error(
            ErrorKind::ProviderError,
            500,
            "Kaeso cannot translate this request right now. Try again shortly.",
        )
    })
}

// A model the key may not use answers like one that doesn't exist, so a restricted key
// can't list the project's catalogue by trying names.
fn no_such_model(model: &str) -> KppError {
    error(
        ErrorKind::NotFound,
        404,
        format!("The model `{model}` does not exist or this key may not use it."),
    )
    .with_param("model")
}

fn answers_busy() -> KppError {
    error(
        ErrorKind::Overloaded,
        503,
        "Kaeso is busy reading other providers' answers; try again shortly.",
    )
    .with_retry_after(super::memory::WAIT)
}

fn translating_busy() -> KppError {
    error(
        ErrorKind::Overloaded,
        503,
        "Kaeso is busy translating other requests; try again shortly.",
    )
    .with_retry_after(super::memory::WAIT)
}

pub(crate) fn unserved(eff: &Effective) -> KppError {
    error(
        ErrorKind::NotFound,
        404,
        format!(
            "{} does not serve `{}` to this project's key; check the key's access.",
            eff.provider.name(),
            eff.model_id
        ),
    )
    .with_param("model")
}

async fn run(state: &AppState, inc: Incoming, rec: &mut Recording) -> Result<Done, Failure> {
    let Incoming {
        ingress,
        kind,
        headers,
        body,
        client_ip,
        caller,
        stream_only,
        max_body,
        alias,
        gemini,
        image,
        transcription,
        elevenlabs,
        video,
        generation,
        source,
        ..
    } = inc;
    let caller = match caller {
        Some(c) => c,
        None => {
            let key = auth::authenticate(state, &headers, client_ip)
                .await
                .map_err(|e| Failure::new(e, Stage::Caller))?;
            let rights = crate::auth::keys::project_rights(&key);
            rec.outcome().sees_cost = rights.contains(kaeso_core::rights::ProjectRight::CostsView);
            rec.outcome().sees_limits =
                rights.contains(kaeso_core::rights::ProjectRight::LimitsView);
            Caller {
                org_id: key.org_id,
                project_id: key.project_id,
                key_id: Some(key.id),
                user_id: key.user_id,
                service_account_id: key.service_account_id,
            }
        }
    };
    rec.outcome().caller = Some(caller.clone());
    rec.outcome().tags = request_tags(&headers).map_err(|e| Failure::new(e, Stage::Request))?;
    let at = |stage: Stage| move |e: KppError| Failure::new(e, stage.clone());

    let encoding = match source {
        Source::Api => decompress::encoding(&headers)
            .map_err(|r| Failure::new(unread_encoding(r), Stage::Request))?,
        Source::Playground => Encoding::Identity,
    };
    let decision_upload = kind == RequestKind::Decide
        && headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| kaeso_codecs::multipart::form_boundary(ct).is_ok());
    let limit = max_body;
    let reading = Instant::now();
    let ((read, upload), ()) = tokio::join!(
        async {
            let read = crate::http::body::read(body, limit).await;
            (read, reading.elapsed())
        },
        preload(state, caller.project_id),
    );
    rec.outcome().upload = Some(upload);
    let body = read.map_err(|e| {
        let refused = if crate::http::body_timeout::timed_out(&e) {
            error(
                ErrorKind::InvalidRequest,
                400,
                "The request body did not arrive in time; send it again.",
            )
        } else if over_limit(&e) {
            error(
                ErrorKind::RequestTooLarge,
                413,
                format!("The request body is larger than {limit} bytes."),
            )
        } else {
            error(
                ErrorKind::InvalidRequest,
                400,
                "The request body broke off before it was complete; send it again.",
            )
        };
        Failure::new(refused, Stage::Request)
    })?;
    let body = decoded_body(encoding, body, limit)
        .await
        .map_err(at(Stage::Request))?;
    let kept = crate::services::logs::keeps(state, caller.project_id).await;
    // `kaeso-log-content: off` keeps one request's content out of a project that logs it; it
    // never turns content on.
    let (keep_content, keep_media) = if content_off(&headers).map_err(at(Stage::Request))? {
        (false, false)
    } else {
        (kept.content, kept.media)
    };
    if keep_content {
        let o = rec.outcome();
        o.content_kept = true;
        o.media_kept = keep_media;
        let uploads = transcription.is_some()
            || elevenlabs
                .as_ref()
                .is_some_and(|c| c.kind() == RequestKind::Transcription)
            || decision_upload;
        if image.is_none() && video.is_none() && generation.is_none() && !uploads {
            if body.len() < OFF_THREAD_BYTES {
                o.request_body = Some(body.clone());
            } else {
                rec.logging = Some(logged_early(body.clone(), keep_media));
            }
        }
    }
    let request_bytes = body.len();
    let checked = off_thread(request_bytes, move || {
        kaeso_codecs::limits::Checked::new(ingress, body)
    })
    .await
    .and_then(|r| r)
    .map_err(at(Stage::Request))?;
    let shape = *checked.shape();
    let holder = caller
        .key_id
        .or(caller.user_id)
        .or(caller.service_account_id)
        .unwrap_or(caller.project_id);
    // Held for the whole call, after the translation's share is returned: the app's
    // decoded request, kept to encode the reply, the request sent while the input
    // counter may still need it, and the body's size again for its log entry, raw or written.
    let logged = u64::try_from(if keep_content { request_bytes } else { 0 }).unwrap_or(u64::MAX);
    let decoded = super::memory::decoded(&shape);
    let retained = |copies: u64| decoded.saturating_mul(copies).saturating_add(logged);
    rec.held = Some(
        state
            .translating
            .hold(
                holder,
                super::memory::estimate(kind, request_bytes, &shape).saturating_add(retained(2)),
            )
            .await
            .map_err(|_| Failure::new(translating_busy(), Stage::Limits))?,
    );
    let listed = header_list(&headers);
    let remix = video.as_ref().and_then(|v| v.remix.clone());
    let req = off_thread(request_bytes, move || {
        match (gemini, image, transcription) {
            _ if remix.is_some() => {
                kaeso_codecs::openai_videos::decode_remix(checked.body().as_ref(), &listed)
            }
            _ if elevenlabs.is_some() => match &elevenlabs {
                Some(call) => dispatch::decode_checked_elevenlabs_request(call, &checked, &listed),
                None => dispatch::decode_checked_request(kind, &checked, &listed),
            },
            (Some(call), _, _) => {
                dispatch::decode_checked_gemini_request(&call.model, call.action, &checked, &listed)
            }
            (None, Some(operation), _) => {
                dispatch::decode_checked_image_request(operation, &checked, &listed)
            }
            (None, None, Some(operation)) => {
                dispatch::decode_checked_transcription_request(operation, &checked, &listed)
            }
            (None, None, None) => dispatch::decode_checked_request(kind, &checked, &listed),
        }
    })
    .await
    .and_then(|r| r)
    .map_err(at(Stage::Request))?;
    let mut req = req;
    if let (Request::Video(r), Some(source)) =
        (&mut req, video.as_ref().and_then(|v| v.remix.as_ref()))
    {
        source.fill(r);
    }
    req.validate().map_err(at(Stage::Request))?;
    if let Request::Video(r) = &req {
        super::videos::refuse_sora(r).map_err(at(Stage::Request))?;
    }
    match &req {
        // An upload is logged as its fields; its media as type and size, unless media is kept.
        Request::Image(r) => {
            if keep_content {
                rec.outcome().request_body =
                    logged_upload(request_bytes, r, super::media::logged_request, keep_media).await;
            }
            rec.outcome().media = Some(super::media::units_of_request(r));
        }
        Request::Transcription(r) => {
            if keep_content {
                rec.outcome().request_body = logged_upload(
                    request_bytes,
                    r,
                    super::media::logged_transcription,
                    keep_media,
                )
                .await;
            }
            rec.outcome().media = Some(super::media::units_of_transcription(r));
        }
        Request::Speech(r) => {
            if keep_content && r.audio.is_some() {
                rec.outcome().request_body =
                    logged_upload(request_bytes, r, super::media::logged_speech, keep_media).await;
            }
            rec.outcome().media = Some(super::media::units_of_speech(r));
        }
        Request::Video(r) => {
            if keep_content {
                rec.outcome().request_body =
                    logged_upload(request_bytes, r, super::videos::logged_request, keep_media)
                        .await;
            }
            rec.outcome().media = Some(super::videos::units_of_request(r));
        }
        // A System One upload is logged as its fields; a JSON body as sent, its media taken out
        // when the log is written. Decisions are billed by token, so they have no media units.
        Request::Decide(r) if keep_content && decision_upload => {
            rec.outcome().request_body =
                logged_upload(request_bytes, r, super::media::logged_decision, keep_media).await;
        }
        _ => {}
    }
    let stream = !req.stream_mode().is_off();
    rec.outcome().stream = stream;
    rec.outcome().requested_model = Some(req.model().to_owned());
    if stream_only && !stream {
        return Err(Failure::new(
            error(
                ErrorKind::InvalidRequest,
                400,
                "Only streamed requests are served here; send `stream: true`.",
            )
            .with_param("stream"),
            Stage::Request,
        ));
    }

    let timeout = requested_timeout(&headers).map_err(|e| Failure::new(e, Stage::Request))?;

    let models = project_models(
        state,
        caller.project_id,
        caller.key_id,
        caller.service_account_id,
    )
    .await
    .map_err(|e| Failure::new(e, Stage::Catalogue))?;
    let target = match &alias {
        Some(a) => Resolved::Alias(a),
        None => models
            .catalogue
            .resolve(req.model())
            .map_err(|e| match &e {
                ResolveError::NotInCatalogue(name) => {
                    Failure::new(no_such_model(name), Stage::UnknownModel(name.clone()))
                }
                ResolveError::Ambiguous { name, .. } => Failure::new(
                    error(ErrorKind::InvalidRequest, 400, e.to_string()).with_param("model"),
                    Stage::AmbiguousModel(name.clone()),
                ),
            })?,
    };
    if let Resolved::Model(entry, _) = &target
        && !models
            .access(caller.key_id, caller.user_id, caller.service_account_id)
            .allows(entry.id)
    {
        return Err(Failure::new(
            no_such_model(req.model()),
            Stage::ModelAccess(req.model().to_owned()),
        ));
    }
    let guards = Box::pin(guard_request(
        state,
        rec,
        &caller,
        (source, ingress, request_bytes),
        &mut req,
    ))
    .await?;
    let sh = Shared {
        ingress,
        kind,
        video: video.as_ref(),
        generation: generation.is_some(),
        headers: &headers,
        caller: &caller,
        models: &models,
        stream,
        keep_content,
        request_bytes,
        decoded,
        logged,
        timeout,
        guards: guards.as_ref(),
    };
    match target {
        Resolved::Model(entry, _) => {
            let app_req = Arc::new(req.clone());
            // Boxed so every future around a request holds a pointer, not the attempt's own
            // state: the request's futures are moved whole each time one is wrapped.
            let mut fell = Vec::new();
            let model = format!("{}/{}", entry.provider, entry.model_id);
            let step = StepTry {
                path: "",
                model: &model,
                timeout: None,
                retries: models.retries,
            };
            Box::pin(tries(state, rec, &sh, entry, &app_req, step, &mut fell))
                .await
                .0
        }
        Resolved::Alias(alias) => {
            rec.outcome().alias = Some(alias.name.clone());
            route(state, rec, &sh, alias, req).await
        }
    }
}

// After a pause every cached entry of a project has run out at once, and the request would
// wait for each load in turn. They are loaded beside each other while the body arrives; the
// steps below then find them, and report a load that failed as they always did.
async fn preload(state: &AppState, project: Uuid) {
    if state
        .caches
        .project_models
        .cached(&project.to_string())
        .await
        .is_some()
    {
        return;
    }
    Box::pin(async {
        let _ = tokio::join!(
            crate::services::logs::keeps(state, project),
            crate::services::catalogue::project_models(state, project),
            crate::services::limits::project_limits(state, project),
            response_cache::active(state, project),
        );
    })
    .await;
}

// The request phase: the project's guardrails see the decoded request once, before the response
// cache and the first step, so a blocked prompt is never answered and every step of an alias
// gets the checked request. Only chat requests are checked.
async fn guard_request(
    state: &AppState,
    rec: &mut Recording,
    caller: &Caller,
    (source, ingress, request_bytes): (Source, DialectId, usize),
    req: &mut Request,
) -> Result<Option<Guards>, Failure> {
    let Request::Generate(g) = req else {
        return Ok(None);
    };
    let armed = crate::services::guardrails::of_project(state, caller.project_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "cannot load a project's guardrails");
            Failure::new(crate::services::guardrails::unloaded(), Stage::Guardrail)
        })?;
    if armed.is_empty() {
        return Ok(None);
    }
    let now = state.clock.now();
    crate::services::limits::within_request_rates(state, caller, now)
        .await
        .map_err(|r| Failure::new(limited(r), Stage::Limits))?;
    let ctx = crate::services::guardrails::Context {
        request_id: rec.outcome().request_id.to_string(),
        organization: OrgId::from_uuid(caller.org_id).to_string(),
        project: ProjectId::from_uuid(caller.project_id).to_string(),
        key: caller.key_id.map(|k| ApiKeyId::from_uuid(k).to_string()),
        member: caller.user_id.map(|u| UserId::from_uuid(u).to_string()),
        service_account: caller
            .service_account_id
            .map(|s| ServiceAccountId::from_uuid(s).to_string()),
        model: g.model.clone(),
        format: ingress.as_str(),
        source: source.as_str(),
        request_bytes,
    };
    let mut runs = Vec::new();
    let mut masks = crate::services::guardrails::Masks::default();
    let checked = Box::pin(crate::services::guardrails::check_request(
        state, &armed, &ctx, g, &mut runs, &mut masks,
    ))
    .await;
    rec.outcome().guardrails = runs;
    rec.outcome().log_masks = masks.logged;
    if let Err(e) = checked {
        // A blocked request reached no provider; it still counts against the request rates.
        let _ = crate::services::limits::admit_follow_up(state, caller, None, now).await;
        return Err(Failure::new(e, Stage::Guardrail));
    }
    Ok(Some(Guards {
        armed,
        ctx,
        restorer: masks.restorer,
    }))
}

// The reply phase, on a whole reply: the guardrails that check replies see it before it is
// encoded for the app.
async fn guard_reply(
    state: &AppState,
    rec: &mut Recording,
    sh: &Shared<'_>,
    asked: &Request,
    reply: &mut Reply,
    stream: bool,
) -> Result<(), KppError> {
    let (Some(guards), Request::Generate(g), Reply::Generate(r)) = (sh.guards, asked, reply) else {
        return Ok(());
    };
    if !guards.armed.checks_replies(stream) {
        return Ok(());
    }
    let mut runs = std::mem::take(&mut rec.outcome().guardrails);
    let checked = Box::pin(crate::services::guardrails::check_reply(
        state,
        &guards.armed,
        &guards.ctx,
        g,
        r,
        stream,
        &mut runs,
    ))
    .await;
    rec.outcome().guardrails = runs;
    checked
}

fn stamp_guardrails(resp: &mut Response, runs: &[crate::services::guardrails::Run]) {
    if let Some(v) = crate::services::guardrails::header_value(runs) {
        resp.headers_mut()
            .insert(crate::services::guardrails::HEADER, v);
    }
}

#[derive(Clone, Copy)]
struct Shared<'a> {
    ingress: DialectId,
    kind: RequestKind,
    video: Option<&'a super::videos::VideoIncoming>,
    generation: bool,
    headers: &'a HeaderMap,
    caller: &'a Caller,
    models: &'a ProjectModels,
    stream: bool,
    keep_content: bool,
    request_bytes: usize,
    decoded: u64,
    logged: u64,
    timeout: Option<u32>,
    guards: Option<&'a Guards>,
}

// A project's guardrails and what they are told about the request, for the reply phase.
struct Guards {
    armed: Arc<crate::services::guardrails::Armed>,
    ctx: crate::services::guardrails::Context,
    restorer: Option<kaeso_core::masking::Restorer>,
}

impl Guards {
    fn restore(&self, reply: &mut Reply) {
        if let (Some(r), Reply::Generate(g)) = (&self.restorer, reply) {
            crate::services::guardrails::rules::restore_reply(r, g);
        }
    }
}

impl Shared<'_> {
    fn retained(&self, copies: u64) -> u64 {
        self.decoded
            .saturating_mul(copies)
            .saturating_add(self.logged)
    }
}

/// `off` keeps a request's content and media out of the request log: `kaeso-log-content`.
pub const LOG_CONTENT_HEADER: &str = "kaeso-log-content";

fn content_off(headers: &HeaderMap) -> Result<bool, KppError> {
    match headers.get(LOG_CONTENT_HEADER).map(|v| v.to_str().map(str::trim)) {
        None => Ok(false),
        Some(Ok(v)) if v.eq_ignore_ascii_case("off") => Ok(true),
        Some(_) => Err(error(
            ErrorKind::InvalidRequest,
            400,
            "The header `kaeso-log-content` takes only `off`, which keeps this request's content out of the request log.",
        )
        .with_param(LOG_CONTENT_HEADER)),
    }
}

/// The app's own labels for a request, such as a feature or an evaluation run: `kaeso-tags`.
pub const TAGS_HEADER: &str = "kaeso-tags";

/// The most tags one request carries.
pub const MAX_TAGS: usize = 10;

/// A tag is 1 to 64 lowercase letters, digits, `.`, `_`, `:` and `-`.
pub fn valid_tag(tag: &str) -> bool {
    (1..=64).contains(&tag.len())
        && tag.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b':' | b'-')
        })
}

// A bad tag is refused, never dropped, so an app learns its tags aren't recorded.
fn request_tags(headers: &HeaderMap) -> Result<Vec<String>, KppError> {
    let mut tags: Vec<String> = Vec::new();
    for value in headers.get_all(TAGS_HEADER) {
        let text = value.to_str().map_err(|_| tags_refused())?;
        for tag in text.split(',').map(str::trim) {
            if !valid_tag(tag) {
                return Err(tags_refused());
            }
            if !tags.iter().any(|t| t == tag) {
                tags.push(tag.to_owned());
            }
        }
    }
    if tags.len() > MAX_TAGS {
        return Err(tags_refused());
    }
    Ok(tags)
}

fn tags_refused() -> KppError {
    error(
        ErrorKind::InvalidRequest,
        400,
        "The header `kaeso-tags` takes up to 10 comma-separated tags of 1 to 64 lowercase letters, digits, `.`, `_`, `:` or `-`.",
    )
    .with_param(TAGS_HEADER)
}

/// How long the app lets each provider take, in seconds, at most: `kaeso-timeout`.
pub const TIMEOUT_HEADER: &str = "kaeso-timeout";

fn requested_timeout(headers: &HeaderMap) -> Result<Option<u32>, KppError> {
    let Some(value) = headers.get(TIMEOUT_HEADER) else {
        return Ok(None);
    };
    value
        .to_str()
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .filter(|s| (1..=3600).contains(s))
        .map(Some)
        .ok_or_else(|| {
            error(
                ErrorKind::InvalidRequest,
                400,
                "The header `kaeso-timeout` must be a whole number of seconds from 1 to 3600.",
            )
            .with_param(TIMEOUT_HEADER)
        })
}

fn begin_attempt(rec: &mut Recording) {
    rec.sent_input = None;
    rec.sent_at = None;
    rec.keep_in_cache = None;
    let o = rec.outcome();
    o.egress = None;
    o.provider = None;
    o.custom_provider = None;
    o.catalogue_model_id = None;
    o.model_id = None;
    o.prices = None;
    o.ttfb = None;
    o.overhead = None;
    o.upstream_request_id = None;
    o.cache_breakpoints = None;
    o.response_cache = None;
}

/// A step as it is tried: where it sits in its route and what it calls, for its entries in the
/// request log, its own timeout and how many times it may try its model again.
#[derive(Clone, Copy)]
struct StepTry<'a> {
    path: &'a str,
    model: &'a str,
    timeout: Option<u32>,
    retries: u8,
}

/// One try of a step: the step's own timeout, and for a try after the first the time the step
/// has left. A later try keeps the first's reservation and skips the response cache.
#[derive(Clone, Copy)]
struct TryAt {
    step_timeout: Option<u32>,
    again: Option<Duration>,
}

const RETRY_WAIT_LEAST: Duration = Duration::from_millis(250);
const RETRY_WAIT_MOST: Duration = Duration::from_secs(2);
const LONGEST_RETRY_AFTER: Duration = Duration::from_secs(5);

// How long to wait before a step tries its model again, or none for a failure a second try
// won't change. Only a provider that couldn't be reached, answered 5xx or said it was
// overloaded; never a timeout, which the provider may still be working on and bill. A
// `Retry-After` of up to 5 seconds replaces the wait; a longer one ends the tries.
fn retry_wait(f: &Failure) -> Option<Duration> {
    let short_lived = match f.stage {
        Stage::Upstream => f.dropped,
        Stage::Provider => matches!(f.err.kind, ErrorKind::Overloaded | ErrorKind::ProviderError),
        _ => false,
    };
    if !short_lived {
        return None;
    }
    match f.err.retry_after {
        Some(after) => (after <= LONGEST_RETRY_AFTER).then_some(after),
        // Full jitter, so gateways that failed at once don't try again at once.
        None => {
            let [a, b] = kaeso_crypto::random_bytes::<2>();
            let share = f64::from(u16::from_be_bytes([a, b])) / f64::from(u16::MAX);
            Some(RETRY_WAIT_LEAST + (RETRY_WAIT_MOST - RETRY_WAIT_LEAST).mul_f64(share))
        }
    }
}

// One step: its model tried once, and again up to `retries` times after a short-lived failure.
// The tries share the step's deadline, its limit reservation and its one count against request
// rates; the response cache is looked up before the first only. Each try goes through the
// project's key pool for the provider. Each failed try that is tried again joins `fell`, so the
// request log shows it under the request. Gives the step's answer or its last failure, and how
// many times it sent the request, every key of the pool it tried included.
async fn tries(
    state: &AppState,
    rec: &mut Recording,
    sh: &Shared<'_>,
    entry: &Entry,
    app_req: &Arc<Request>,
    step: StepTry<'_>,
    fell: &mut Vec<FellBack>,
) -> (Result<Done, Failure>, u32) {
    let began = tokio::time::Instant::now();
    rec.step_wait = None;
    let mut again: Option<Duration> = None;
    // Tries of the step's model, and requests sent to the provider over every key of the pool.
    let mut round: u8 = 1;
    let mut sent: u32 = 0;
    loop {
        rec.outcome().fallbacks.clone_from(fell);
        let started = tokio::time::Instant::now();
        let (tried, keys) = Box::pin(attempt_pool(
            state,
            rec,
            sh,
            entry,
            app_req,
            TryAt {
                step_timeout: step.timeout,
                again,
            },
            began,
        ))
        .await;
        sent = sent.saturating_add(keys);
        let provider = entry.provider.to_string();
        let failure = match tried {
            Ok(done) => {
                if round > 1 {
                    metrics::counter!(crate::metrics::UPSTREAM_RETRIES, "provider" => provider, "outcome" => "answered").increment(1);
                }
                return (Ok(done), sent);
            }
            Err(f) => f,
        };
        let now = tokio::time::Instant::now();
        let left = rec.step_wait.map_or(Duration::MAX, |w| {
            (began + w).saturating_duration_since(now)
        });
        let wait = if round <= step.retries {
            retry_wait(&failure)
        } else {
            None
        };
        let Some(wait) = wait.filter(|w| *w < left) else {
            if round > 1 {
                metrics::counter!(crate::metrics::UPSTREAM_RETRIES, "provider" => provider, "outcome" => "gave_up").increment(1);
            }
            return (Err(failure), sent);
        };
        rec.attempt_failed(Instant::now());
        let said = failure.logged();
        let status = failure.err.status.unwrap_or(500);
        tracing::info!(model = step.model, status, error = %said.message, "a step's model failed for a moment; trying it again");
        fell.push(FellBack {
            catalogue_model_id: entry.id,
            path: step.path.to_owned(),
            model: step.model.to_owned(),
            status,
            error_kind: said.kind.as_str().to_owned(),
            error_message: said.message.clone(),
            duration_ms: u64::try_from(now.duration_since(started).as_millis()).unwrap_or(u64::MAX),
            skipped: false,
            retried: true,
        });
        tokio::time::sleep(wait).await;
        // The wait is the provider's doing, not Kaeso's work: its overhead leaves it out.
        rec.waited = rec.waited.saturating_add(wait);
        again = Some(left.saturating_sub(wait));
        round = round.saturating_add(1);
    }
}

// One try of a step through the keys of the project's pool for its provider, in the order this
// request tries them: a weighted draw that keeps each caller (its API key, service account or
// member) on one key, so the provider's prompt cache keeps working, with the keys held moments
// ago last. A key that is refused, out of credit, signed out or rate limited gives way to the
// next, which keeps the step's reservation and the time it has left since `began`, as a try made
// again does. Gives the answer or the last failure, and how many keys it sent the request to.
async fn attempt_pool(
    state: &AppState,
    rec: &mut Recording,
    sh: &Shared<'_>,
    entry: &Entry,
    app_req: &Arc<Request>,
    at: TryAt,
    began: tokio::time::Instant,
) -> (Result<Done, Failure>, u32) {
    let caller = sh.caller;
    let provider = entry.provider.to_string();
    let pool = match credentials::resolve(
        state,
        OrgId::from_uuid(caller.org_id),
        ProjectId::from_uuid(caller.project_id),
        &provider,
    )
    .await
    {
        // A video job is polled and fetched later with the project's first key, so it starts there.
        Ok(primary) if primary.pool.len() > 1 && sh.kind != RequestKind::Video => {
            primary.pool.clone()
        }
        _ => {
            return (
                attempt_with(state, rec, sh, entry, app_req, at, None).await,
                1,
            );
        }
    };
    let seed = caller
        .key_id
        .or(caller.service_account_id)
        .or(caller.user_id)
        .map_or_else(|| rec.outcome().request_id.to_string(), |id| id.to_string());
    let drawn = crate::services::key_pool::order(&pool, &format!("{seed}:{provider}"));
    let now = state.clock.now();
    let held = state
        .cooldowns
        .held_keys(&state.valkey, caller.project_id, &drawn, now)
        .await;
    let (ready, waiting): (Vec<_>, Vec<_>) = drawn.iter().zip(held).partition(|(_, h)| !h);
    let order: Vec<Uuid> = ready.into_iter().chain(waiting).map(|(k, _)| *k).collect();
    let Some((last, before)) = order.split_last() else {
        return (
            attempt_with(state, rec, sh, entry, app_req, at, None).await,
            1,
        );
    };
    let mut at = at;
    let mut keys = 0;
    for key in before {
        keys += 1;
        match attempt_with(state, rec, sh, entry, app_req, at, Some(*key)).await {
            Err(f) => {
                if !hold_pool_key(state, caller.project_id, *key, &f, now).await {
                    return (Err(f), keys);
                }
                rec.attempt_failed(Instant::now());
                tracing::info!(
                    provider,
                    kind = f.err.kind.as_str(),
                    "a key of the project's pool gave way to the next"
                );
                let left = rec.step_wait.map_or(Duration::MAX, |w| {
                    (began + w).saturating_duration_since(tokio::time::Instant::now())
                });
                at = TryAt {
                    step_timeout: at.step_timeout,
                    again: Some(left),
                };
            }
            done => return (done, keys),
        }
    }
    let tried = attempt_with(state, rec, sh, entry, app_req, at, Some(*last)).await;
    if let Err(f) = &tried {
        hold_pool_key(state, caller.project_id, *last, f, now).await;
    }
    (tried, keys + 1)
}

// One try with one key of the pool, or the project's own key for `None`.
async fn attempt_with(
    state: &AppState,
    rec: &mut Recording,
    sh: &Shared<'_>,
    entry: &Entry,
    app_req: &Arc<Request>,
    at: TryAt,
    key: Option<Uuid>,
) -> Result<Done, Failure> {
    Box::pin(attempt(
        state,
        rec,
        sh,
        entry,
        Arc::clone(app_req),
        (**app_req).clone(),
        at,
        key,
    ))
    .await
}

// Holds a key of a project's pool after a failure that is the key's own; returns whether it was.
async fn hold_pool_key(
    state: &AppState,
    project: Uuid,
    key: Uuid,
    f: &Failure,
    now: time::OffsetDateTime,
) -> bool {
    let Some(hold) = crate::services::key_pool::gives_way(&f.stage, &f.err) else {
        return false;
    };
    if let Some(length) = hold {
        let said = f.logged();
        let held = cooldowns::Held {
            until: now + length,
            reason: match classify(f) {
                Next::Fall(reason) => reason,
                Next::Stop => RouteFailure::Unavailable,
            },
            status: f.err.status.unwrap_or(500),
            error_kind: said.kind.as_str().to_owned(),
            error_message: said.message.clone(),
        };
        state
            .cooldowns
            .hold_key(&state.valkey, project, key, held, now)
            .await;
    }
    true
}

#[allow(clippy::too_many_arguments)]
async fn attempt(
    state: &AppState,
    rec: &mut Recording,
    sh: &Shared<'_>,
    entry: &Entry,
    app_req: Arc<Request>,
    mut req: Request,
    at: TryAt,
    member: Option<Uuid>,
) -> Result<Done, Failure> {
    let TryAt {
        step_timeout,
        again,
    } = at;
    let Shared {
        ingress,
        kind,
        headers,
        caller,
        models,
        stream,
        keep_content,
        request_bytes,
        timeout,
        ..
    } = *sh;
    let retained = |copies: u64| sh.retained(copies);
    begin_attempt(rec);
    let eff = crate::services::catalogue::effective(state, entry, &models.providers).ok_or_else(|| {
        let turned_off = models
            .providers
            .get(&entry.provider)
            .is_some_and(|p| p.custom().is_some());
        if turned_off {
            return Failure::new(
                error(
                    ErrorKind::ProviderError,
                    500,
                    format!(
                        "Custom providers are turned off on this install, so `{}` cannot be called.",
                        entry.provider
                    ),
                ),
                Stage::CustomProvidersOff,
            );
        }
        Failure::new(
            error(
                ErrorKind::ProviderError,
                500,
                format!("The provider `{}` is not available.", entry.provider),
            ),
            Stage::Setup,
        )
    })?;
    let served = Served {
        model: Some(eff.model_id.clone()),
        provider: Some(eff.provider.id().to_owned()),
        left_out: Vec::new(),
        adapted: Vec::new(),
        route: Vec::new(),
        upstream_request_id: None,
    };
    let fail = |stage: Stage| {
        let served = served.clone();
        move |err: KppError| Failure {
            err,
            served,
            stage,
            provider_said: None,
            dropped: false,
            not_found_in_request: false,
        }
    };
    {
        let o = rec.outcome();
        o.catalogue_model_id = Some(entry.id);
        o.model_id = Some(eff.model_id.clone());
        o.provider = Some(eff.provider.metric_label());
        o.custom_provider = eff.provider.custom().map(|c| c.id().to_string());
        o.prices = Some(if kind == RequestKind::Decide {
            kaeso_core::cost::decision_prices(&eff.fields)
        } else {
            eff.fields.clone()
        });
    }
    let mut plan = capability::prepare(&mut req, ingress, &eff).map_err(|u| {
        let mut e = KppError::unsupported(u.capability, u.message).with_status(400);
        if let Some(p) = u.param {
            e = e.with_param(p);
        }
        fail(Stage::Capability)(e)
    })?;
    rec.outcome().egress = Some(plan.dialect);
    if sh.generation && plan.dialect != DialectId::ElevenLabs {
        return Err(fail(Stage::Capability)(super::generations::not_elevenlabs(
            &eff,
        )));
    }
    // The provider gets the model's `upstream_model`, such as an Azure deployment's name, when
    // it has one.
    let sent_model = eff.sent_model().to_owned();
    match &mut req {
        Request::Generate(g) | Request::CountTokens(g) => g.model.clone_from(&sent_model),
        Request::Embed(e) => e.model.clone_from(&sent_model),
        Request::Image(i) => i.model.clone_from(&sent_model),
        Request::Speech(r) => r.model.clone_from(&sent_model),
        Request::Transcription(r) => r.model.clone_from(&sent_model),
        Request::Video(r) => r.model.clone_from(&sent_model),
        Request::Decide(d) => d.model.clone_from(&sent_model),
        Request::Rerank(r) => r.model.clone_from(&sent_model),
        _ => {}
    }
    let playground = rec.outcome().source == super::hooks::Source::Playground;
    let fetch = super::links::Fetch::of(&mut req, &eff.provider, &eff.model_id, playground);
    if let Request::Image(r) = &req
        && let Some(units) = rec.outcome().media.as_mut()
    {
        units.extended_prompt = eff
            .provider
            .images(&eff.model_id)
            .is_some_and(|e| e.extended_prompt_doubles)
            && kaeso_codecs::dashscope::images::extends_prompt(r);
    }
    if let Some(source) = sh.video.and_then(|v| v.remix.as_ref()) {
        source
            .check_provider(eff.provider.id())
            .map_err(fail(Stage::Capability))?;
    }
    if let Request::Generate(g) = &mut req
        && let StreamMode::On { include_usage, .. } = &mut g.stream
    {
        *include_usage = true;
    }

    let u = &state.config.upstream;
    let timeouts = Timeouts {
        request: u.request_timeout,
        first_byte: u.first_byte_timeout,
        idle: u.stream_idle_timeout,
    }
    .with_model_override(eff.fields.timeout_seconds)
    .with_step_override(step_timeout, stream)
    .shortened_to(timeout, stream);
    let timeouts = match again {
        Some(left) => Timeouts {
            first_byte: timeouts.first_byte.min(left),
            request: if stream {
                timeouts.request
            } else {
                timeouts.request.min(left)
            },
            ..timeouts
        },
        None => {
            rec.step_wait = Some(if stream {
                timeouts.first_byte
            } else {
                timeouts.request
            });
            timeouts
        }
    };
    let credential = credentials::for_provider_member(
        state,
        OrgId::from_uuid(caller.org_id),
        ProjectId::from_uuid(caller.project_id),
        &eff.provider,
        member,
    )
    .await
    .map_err(|e| match e {
        CredentialError::Missing { .. } => fail(Stage::MissingCredential(eff.provider.name().to_owned()))(error(ErrorKind::ProviderError, 502, e.to_string())),
        CredentialError::Crypto(_) => {
            tracing::error!(provider = eff.provider.id(), "a provider credential cannot be decrypted");
            fail(Stage::Credential)(error(ErrorKind::ProviderError, 500, "Kaeso cannot decrypt the provider credential. The operator must check the configured master keys."))
        }
        CredentialError::Store(err) => {
            tracing::error!(error = %err, "cannot load a provider credential");
            fail(Stage::Credential)(error(ErrorKind::ProviderError, 503, "Kaeso cannot load the provider credential right now. Try again shortly."))
        }
        CredentialError::SignedOut { product } if product == crate::services::credentials::ENTRA_ID => {
            fail(Stage::SignedOut(product))(error(ErrorKind::ProviderError, 502, ENTRA_REFUSED))
        }
        CredentialError::SignedOut { product } => fail(Stage::SignedOut(product))(error(
            ErrorKind::ProviderError,
            502,
            format!("The {product} account of this project's {provider} key is signed out. Ask an organization admin to sign in again under Providers.", provider = eff.provider.name()),
        )),
        CredentialError::NoApiAccess { product } => fail(Stage::NoApiAccess(product))(error(
            ErrorKind::ProviderError,
            502,
            format!("The {product} plan of this project's {provider} key has no API access. Ask an organization admin to add an API key under Providers.", provider = eff.provider.name()),
        )),
        CredentialError::Unrenewed { product } => fail(Stage::Credential)(error(
            ErrorKind::ProviderError,
            503,
            format!("Kaeso cannot renew the {product} account's access right now. Try again shortly."),
        )),
    })?;
    let account = eff
        .provider
        .account()
        .filter(|a| a.method == credential.method);
    let keeping = crate::services::stored_responses::admit(
        state,
        &crate::services::stored_responses::Asked {
            caller,
            ingress,
            egress: plan.dialect,
            provider: &eff.provider,
            model: &eff.model_id,
            credential: &credential,
        },
        &req,
    )
    .await
    .map_err(|r| match r {
        crate::services::stored_responses::Refusal::Unsupported(e) => fail(Stage::Capability)(e),
        crate::services::stored_responses::Refusal::Final(e) => fail(Stage::Request)(e),
    })?;
    rec.kept =
        keeping
            .as_ref()
            .filter(|k| k.records)
            .map(|_| crate::services::stored_responses::Kept {
                credential: credential.id.uuid(),
                provider: eff.provider.id().to_owned(),
                model: eff.model_id.clone(),
                catalogue_model_id: entry.id,
                response_id: None,
            });
    let chatgpt = account.filter(|a| a.method == AuthMethod::ChatGpt);
    if let Some(a) = chatgpt {
        let stage =
            super::accounts::media_api(kind).map_or(Stage::Capability, |api| Stage::NeedsApiKey {
                product: a.product,
                provider: eff.provider.name().to_owned(),
                api,
            });
        plan.dialect =
            super::accounts::prepare(&mut req, a, eff.provider.name()).map_err(fail(stage))?;
        rec.outcome().egress = Some(plan.dialect);
    }

    let admission = Admission {
        request_id: rec.outcome().request_id,
        caller: caller.clone(),
        catalogue_model_id: entry.id,
        kind,
        at: rec.outcome().started_at,
        timeout: timeouts.request,
        worst: WorstCase::continuing(
            &req,
            request_bytes,
            keeping.as_ref().map_or(
                0,
                crate::services::stored_responses::Keeping::earlier_tokens,
            ),
            &eff.fields,
            &eff.provider,
        ),
        unpriced: kaeso_core::cost::unpriced_model(
            &eff.fields,
            &entry.overrides,
            kind,
            &eff.model_id,
        )
        .or_else(|| kaeso_core::cost::unpriced(&req, plan.dialect, eff.provider.responses())),
    };
    if again.is_none() {
        rec.reservation = match state.limits.admit(state, &admission).await {
            Ok(held) => held,
            Err(refusal) => {
                rec.found_limits(refusal.left.as_slice());
                return Err(fail(Stage::Limits)(limited(refusal)));
            }
        };
        let left = std::mem::take(&mut rec.reservation.left);
        rec.found_limits(&left);
    }

    if entry.is_unavailable()
        && !state
            .availability
            .claim_recheck(credential.key(), entry, state.clock.now())
            .await
    {
        return Err(fail(Stage::Unserved(eff.model_id.clone()))(unserved(&eff)));
    }

    let carry = caller.key_id.filter(|_| {
        continuity::applies(state, ingress, plan.dialect) && kind == RequestKind::Generate
    });
    if let Some(key_id) = carry
        && let Request::Generate(g) = &mut req
    {
        continuity::restore(state, key_id, g).await;
    }

    let platform = match crate::services::agent_platform::egress(
        state,
        &eff.provider,
        &credential,
        plan.dialect,
        &eff.model_id,
    )
    .await
    {
        Ok(p) => p,
        Err(crate::services::agent_platform::EgressError::Refused(e)) => {
            return Err(fail(Stage::Capability)(e));
        }
        Err(e) => {
            let Some((status, reason)) = e.refusal() else {
                return Err(fail(Stage::Credential)(e.into_kpp()));
            };
            let message = format!("Google refused the service account key: {reason}");
            Box::pin(crate::services::notifications::system::key_refused(
                state,
                caller.org_id,
                credential.id.uuid(),
                crate::services::notifications::system::Refusal {
                    status,
                    message: &message,
                    billing: false,
                },
            ))
            .await;
            return Err(fail(Stage::KeyRefused)(e.into_kpp()));
        }
    };
    let mut ectx = egress_ctx(
        &app_req,
        &eff.provider,
        &eff.model_id,
        ModelFlags::of(&plan),
    );
    if let Some(videos) = eff.provider.videos(&eff.model_id) {
        ectx = ectx.with_videos_profile(videos.profile);
    }
    let media = super::media::egress_of(&eff.provider, kind, &eff.model_id);
    if let Some(p) = &platform {
        ectx = p.apply(ectx);
    }
    let scoped = crate::services::account_scope::of(&eff.provider, &credential)
        .map_err(fail(Stage::Credential))?;
    if let Some(s) = &scoped {
        ectx = s.apply(ectx);
    }
    let regional = crate::services::regional::egress(
        state,
        &eff.provider,
        &credential,
        plan.dialect,
        &eff.model_id,
        eff.fields.egress_dialect.is_some(),
    )
    .map_err(|e| match e {
        crate::services::regional::EgressError::Refused(e) => fail(Stage::Capability)(e),
        crate::services::regional::EgressError::Setup(e) => fail(Stage::Credential)(e),
    })?;
    if let Some(r) = &regional {
        ectx = r.apply(ectx);
    }
    let dialect = plan.dialect;
    let (req, mut ectx, encoded) = off_thread(request_bytes, move || {
        let encoded = dispatch::encode_request(dialect, &req, &mut ectx);
        (req, ectx, encoded)
    })
    .await
    .map_err(fail(Stage::Capability))?;
    let mut encoded = encoded.map_err(fail(Stage::Capability))?;
    if let Some(s) = &scoped {
        s.add_headers(&mut encoded.headers);
    }
    ectx.left_out.splice(0..0, plan.left_out.iter().cloned());
    ectx.adapted.splice(0..0, plan.adapted.iter().cloned());
    rec.outcome().cache_breakpoints = ectx.cache_breakpoints;
    if !ectx.left_out.is_empty() {
        tracing::info!(
            left_out = %ectx.left_out.join(", "),
            "left out of the request what the provider has no equivalent for"
        );
    }
    if !ectx.adapted.is_empty() {
        tracing::info!(
            adapted = %ectx.adapted.join(", "),
            "sent what the provider takes only in another form, such as a merged tool schema"
        );
    }
    let served = Served {
        left_out: ectx.left_out.clone(),
        adapted: ectx.adapted.clone(),
        ..served.clone()
    };
    let fail = |stage: Stage| {
        let served = served.clone();
        move |err: KppError| Failure {
            err,
            served,
            stage,
            provider_said: None,
            dropped: false,
            not_found_in_request: false,
        }
    };
    // A retry or a stored response goes to its provider, which an answer from the cache would
    // skip; a stored one is counted as passing the cache by.
    let cached = if again.is_some() || keeping.is_some() {
        if keeping.is_some()
            && response_cache::active(state, caller.project_id)
                .await
                .is_some()
        {
            rec.outcome().response_cache = Some(Verdict::Bypass);
            response_cache::count_lookup(Verdict::Bypass);
        }
        Consulted::Skip
    } else {
        consult_cache(
            state,
            rec,
            headers,
            response_cache::Request {
                org: caller.org_id,
                project: caller.project_id,
                kind,
                dialect: plan.dialect,
                provider: eff.provider.id(),
                entry: entry.id,
                model: &eff.model_id,
                encoded: &encoded,
            },
            stream,
        )
        .await
    };
    rec.keep_in_cache = match cached {
        Consulted::Hit(stored) => {
            let mut stored = *stored;
            Box::pin(guard_reply(
                state,
                rec,
                sh,
                &app_req,
                &mut stored.reply,
                false,
            ))
            .await
            .map_err(fail(Stage::Guardrail))?;
            if let Some(g) = sh.guards {
                g.restore(&mut stored.reply);
            }
            let encode_ctx = EncodeCtx::for_request(Arc::clone(&app_req)).with_created_at(
                u64::try_from(rec.outcome().started_at.unix_timestamp()).unwrap_or(0),
            );
            return replay(state, rec, stored, &encode_ctx, carry, &served)
                .await
                .map_err(fail(Stage::Reply));
        }
        Consulted::Store(slot) => Some(slot),
        Consulted::Skip => None,
        Consulted::NotCached => return Err(fail(Stage::NotCached)(not_cached())),
    };
    let sent_req = counted_at_provider(account, &eff.provider, plan.dialect).then_some(req);
    let kept = retained(1 + u64::from(sent_req.is_some()));
    let sent_bytes = encoded.body.len();
    let sent = u64::try_from(sent_bytes).unwrap_or(u64::MAX);
    // reqwest's redirect layer keeps a clone of the body until the provider answers, even
    // with redirects off, however slowly the provider reads the upload.
    if let Some(held) = &mut rec.held {
        held.keep(kept.saturating_add(sent));
    }
    let entra = match credential
        .azure
        .as_deref()
        .filter(|_| credential.method == AuthMethod::AzureServicePrincipal)
    {
        Some(principal) => Some(
            Box::pin(entra_token(state, &credential, principal))
                .await
                .map_err(|(stage, e)| fail(stage)(e))?,
        ),
        None => None,
    };
    let keyed = crate::services::key_endpoint::base(state, &eff.provider, &credential)
        .map_err(fail(Stage::Credential))?;
    let account_base = match account {
        Some(a) => Some(
            state
                .accounts
                .hosts(a.method)
                .map(crate::services::provider_accounts::Hosts::api_base)
                .and_then(Result::ok)
                .ok_or_else(|| {
                    fail(Stage::Setup)(error(
                        ErrorKind::ProviderError,
                        500,
                        "No base URL is configured for this provider.",
                    ))
                })?,
        ),
        None => None,
    };
    let base = match (&account_base, &platform, &regional) {
        (Some(base), _, _)
        | (None, Some(crate::services::agent_platform::Egress { base, .. }), _)
        | (None, None, Some(crate::services::regional::Egress { base, .. })) => base,
        (None, None, None) => match &keyed {
            Some(base) => base,
            None => base_url(state, &eff.provider).ok_or_else(|| {
                fail(Stage::Setup)(error(
                    ErrorKind::ProviderError,
                    500,
                    "No base URL is configured for this provider.",
                ))
            })?,
        },
    };
    let rooted;
    let base = match media {
        Some(i) if i.from_root => {
            rooted = super::media::host_root(base);
            &rooted
        }
        _ => base,
    };
    let provider = eff.provider.metric_label();
    rec.sent_input = Some(estimate::tokens_of_bytes(sent));
    let sent_at = Instant::now();
    rec.sent_at = Some(sent_at);
    rec.outcome().overhead = Some(rec.own_time(sent_at));
    let gathers = chatgpt.is_some() && !stream && kind == RequestKind::Generate;
    let deadline = tokio::time::Instant::now() + timeouts.request;
    let late = |_: tokio::time::error::Elapsed| {
        let e = UpstreamError::Timeout("request");
        count_upstream_error(transport_kind(&e), provider);
        fail(Stage::Upstream)(upstream_error(&e))
    };
    let auth = match (&platform, &regional, media.and_then(|i| i.auth)) {
        (Some(p), _, _) => Some(p.auth),
        (None, Some(r), _) => Some(r.auth),
        (None, None, Some(auth)) => Some(auth),
        (None, None, None) => eff.provider.auth(),
    };
    // An Entra ID token goes as a bearer in place of the resource key's `api-key`.
    let auth = if entra.is_some() {
        Some(kaeso_core::provider::AuthStyle::Bearer)
    } else {
        auth
    };
    let secret = entra
        .as_ref()
        .or(platform.as_ref().map(|p| &p.secret))
        .unwrap_or(&credential.secret);
    // A key that signs with AWS credentials signs the request in place of an auth header, in
    // its own region whatever host the base URL names.
    let sigv4 = match (credential.method.is_aws(), credential.region.as_deref()) {
        (true, Some(region)) => Some(
            crate::services::aws_auth::signer(state, &credential, region)
                .await
                .map_err(fail(Stage::Credential))?,
        ),
        _ => None,
    };
    let auth = if sigv4.is_some() { None } else { auth };
    let sending = state.upstream.send(Call {
        base_url: base,
        auth,
        guard: guard_for(&eff.provider),
        credential: secret,
        sigv4,
        request: Outgoing::from(encoded),
        stream: match kind {
            RequestKind::Generate => stream || chatgpt.is_some(),
            RequestKind::Image | RequestKind::Transcription => !ectx.stream.is_off(),
            RequestKind::Speech => dispatch::reads_reply_as_stream(plan.dialect, &ectx),
            RequestKind::Decide => false,
            _ => false,
        },
        timeouts,
        max_body: state.config.body_limits.provider_response.as_usize(),
    });
    let sent = if gathers {
        tokio::time::timeout_at(deadline, sending)
            .await
            .map_err(late)?
    } else {
        sending.await
    };
    let resp = sent.map_err(|e| {
        count_upstream_error(transport_kind(&e), provider);
        if let UpstreamError::Refused(r) = &e {
            tracing::warn!(provider = eff.provider.id(), host = %r.host, "a custom provider's address is not allowed on this install");
        }
        Failure {
            dropped: dropped(&e),
            not_found_in_request: false,
            ..fail(Stage::Upstream)(upstream_error_from(&e, &eff.provider))
        }
    })?;
    if let Some(held) = &mut rec.held {
        held.keep(kept);
    }
    rec.outcome().ttfb = Some(sent_at.elapsed());
    rec.outcome().upstream_request_id = resp
        .headers
        .get("request-id")
        .or_else(|| resp.headers.get("x-request-id"))
        // Cloudflare AI Gateway's log entry for the request.
        .or_else(|| resp.headers.get("cf-aig-log-id"))
        .map(str::to_owned);
    // Replies and errors from here on carry the provider's id for its support.
    let served = Served {
        upstream_request_id: rec.outcome().upstream_request_id.clone(),
        ..served.clone()
    };
    let fail = |stage: Stage| {
        let served = served.clone();
        move |err: KppError| Failure {
            err,
            served,
            stage,
            provider_said: None,
            dropped: false,
            not_found_in_request: false,
        }
    };
    if plan.dialect == DialectId::ElevenLabs
        && let Some(billed) = kaeso_codecs::elevenlabs::billed_characters(&resp.headers)
        && let Some(media) = rec.outcome().media.as_mut()
        && kind == RequestKind::Speech
    {
        media.characters = billed;
    }
    let encode_ctx = EncodeCtx::for_request(Arc::clone(&app_req))
        .with_created_at(u64::try_from(rec.outcome().started_at.unix_timestamp()).unwrap_or(0));
    let status = resp.status;
    let success = (200..300).contains(&status);
    let learnt = |availability: Availability| Answer {
        org_id: caller.org_id,
        project_id: caller.project_id,
        entry: entry.id,
        provider: eff.provider.id(),
        key: credential.key(),
        availability,
        at: state.clock.now(),
    };
    if success && entry.is_unavailable() {
        Box::pin(availability::record(
            state,
            &learnt(Availability::Available),
        ))
        .await;
    }
    if success {
        crate::services::notifications::system::key_answered(credential.id.uuid());
    }

    let started_req = Arc::clone(&app_req);
    let counter = InputCounter {
        state: state.clone(),
        credential: credential.clone(),
        provider: eff.provider.clone(),
        dialect: plan.dialect,
        asked: sent_req.map(|req| (app_req, req)),
        platform: platform.clone(),
        flags: ModelFlags::of(&plan),
        timeouts,
        sent_bytes,
    };

    match resp.body {
        ResponseBody::Stream(chunks) if success && chatgpt.is_some() && !stream => {
            let decoder =
                dispatch::stream_decoder(plan.dialect, &ectx).map_err(fail(Stage::Reply))?;
            let max = state.config.body_limits.provider_response.as_usize();
            let reply = tokio::time::timeout_at(
                deadline,
                stream::gather(chunks, decoder, plan.dialect, max),
            )
            .await
            .map_err(late)?
            .map_err(|g| match g {
                stream::Gathered::Transport(e) => {
                    count_upstream_error(transport_kind(&e), provider);
                    Failure {
                        dropped: dropped(&e),
                        not_found_in_request: false,
                        ..fail(Stage::Upstream)(upstream_error(&e))
                    }
                }
                stream::Gathered::Provider(err) => {
                    count_upstream_error(err.kind.as_str(), provider);
                    let (stage, err) = provider_failed(account, err);
                    fail(stage)(err)
                }
                stream::Gathered::Broken(err) => fail(Stage::Reply)(err),
            })?;
            let mut reply = reply;
            if let Err(e) =
                Box::pin(guard_reply(state, rec, sh, &started_req, &mut reply, false)).await
            {
                record_usage(rec, &reply, counter).await;
                return Err(fail(Stage::Guardrail)(e));
            }
            if let Some(g) = sh.guards {
                g.restore(&mut reply);
            }
            let out = dispatch::encode_response(ingress, &reply, &encode_ctx);
            reply_with(state, rec, reply, out, counter, carry, &served)
                .await
                .map_err(fail(Stage::Reply))
        }
        ResponseBody::Stream(mut chunks) if success => {
            let media_type = dispatch::stream_media_type(ingress, &encode_ctx);
            // An event stream answers once it carries output: one that fails before, such as an
            // account at its plan's limit, fails this attempt, and a route falls back.
            if media_type.starts_with("text/event-stream") {
                let peeker =
                    dispatch::stream_decoder(plan.dialect, &ectx).map_err(fail(Stage::Reply))?;
                if let Some(mut err) =
                    stream::peek(&mut chunks, peeker, state.config.streams.keepalive_interval).await
                {
                    redact_key_fragments(&mut err);
                    count_upstream_error(err.kind.as_str(), provider);
                    let (stage, mut err) = provider_failed(account, err);
                    // The status the app's format gives this kind, as the app will get it.
                    if err.status.is_none() {
                        err.status = Some(dispatch::encode_error(ingress, &err).status);
                    }
                    return Err(fail(stage)(err));
                }
            }
            let chunks = if kind == RequestKind::Generate
                && sh.guards.is_some_and(|g| g.armed.checks_replies(true))
            {
                let max = state.config.body_limits.provider_response.as_usize();
                let held = stream::hold(chunks, max).await.map_err(|e| {
                    count_upstream_error(transport_kind(&e), provider);
                    fail(Stage::Upstream)(upstream_error(&e))
                })?;
                let decoder =
                    dispatch::stream_decoder(plan.dialect, &ectx).map_err(fail(Stage::Reply))?;
                let gathered = stream::gather(
                    kaeso_upstream::ByteStream::replay(held.clone()),
                    decoder,
                    plan.dialect,
                    max,
                )
                .await;
                // A stream that ends in an error or breaks off goes to the app as it came, and
                // the app sees its end as without guardrails.
                if let Ok(mut reply) = gathered
                    && let Err(e) =
                        Box::pin(guard_reply(state, rec, sh, &started_req, &mut reply, true)).await
                {
                    if let Reply::Generate(r) = &reply {
                        rec.outcome().usage = Some(r.usage.clone());
                    }
                    return Err(fail(Stage::Guardrail)(e));
                }
                kaeso_upstream::ByteStream::replay(held)
            } else {
                chunks
            };
            let decoder =
                dispatch::stream_decoder(plan.dialect, &ectx).map_err(fail(Stage::Reply))?;
            let encoder =
                dispatch::stream_encoder(ingress, &encode_ctx).map_err(fail(Stage::Reply))?;
            let (tx, rx) = tokio::sync::mpsc::channel::<Bytes>(32);
            let taken = stream::Taken::default();
            let timing = rec.timing();
            let verdict = rec.outcome().response_cache;
            let checked = crate::services::guardrails::header_value(&rec.outcome().guardrails);
            let task_rec = Recording {
                state: rec.state.clone(),
                sink: rec.sink.clone(),
                outcome: rec.outcome.take(),
                started: rec.started,
                sent_at: rec.sent_at,
                waited: rec.waited,
                reservation: std::mem::take(&mut rec.reservation),
                sent_input: None,
                held: rec.held.take(),
                keep_in_cache: None,
                kept: rec.kept.take(),
                logging: rec.logging.take(),
                serving: rec.serving.take(),
                step_wait: None,
                limits: None,
            };
            let keepalive = state.config.streams.keepalive_interval;
            let carry = carry.map(|key_id| continuity::StreamCarry::new(state.clone(), key_id));
            let parts = stream::Parts {
                chunks,
                decoder,
                encoder,
                app: ingress,
                keepalive: stream::Keepalive::new(keepalive),
                counter,
                carry,
                keep_reply: keep_content,
                account,
                taken: taken.clone(),
                binary: None,
                restoring: sh
                    .guards
                    .and_then(|g| g.restorer.clone())
                    .filter(|_| kind == RequestKind::Generate)
                    .map(crate::services::guardrails::rules::Restoring::new),
            };
            let aborted = stream::Aborted::default();
            // Audio has no room for an error once it has begun, so its status waits for the
            // provider's first bytes; event streams answer at once and carry errors as events.
            let (parts, first) = if media_type.starts_with("text/event-stream") {
                (parts, None)
            } else {
                let (first, wait) = tokio::sync::oneshot::channel();
                let binary = stream::Binary {
                    first,
                    aborted: aborted.clone(),
                };
                (
                    stream::Parts {
                        binary: Some(binary),
                        ..parts
                    },
                    Some(wait),
                )
            };
            state.in_flight.spawn(stream::pump(parts, tx, task_rec));
            if let Some(wait) = first
                && let Ok(Some(err)) = wait.await
            {
                let mut failure = Failure::new(err, Stage::Provider);
                failure.served = served.clone();
                return Ok(Done::Refused(failure));
            }
            let mut resp = Response::new(stream::body(rx, taken, aborted));
            let h = resp.headers_mut();
            h.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_str(&media_type).unwrap_or_else(|_| {
                    HeaderValue::from_static("text/event-stream; charset=utf-8")
                }),
            );
            h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
            h.insert("x-accel-buffering", HeaderValue::from_static("no"));
            h.insert(TIMING_HEADER, timing);
            if let Some(v) = verdict {
                h.insert(response_cache::HEADER, v.header());
            }
            if let Some(v) = checked {
                h.insert(crate::services::guardrails::HEADER, v);
            }
            stamp(&mut resp, &served);
            Ok(Done::Streaming(resp))
        }
        body => {
            let bytes = match body {
                ResponseBody::Full(b) => b,
                ResponseBody::Stream(mut s) => {
                    let mut buf = Vec::new();
                    while let Some(chunk) = s.next().await {
                        let chunk = chunk.map_err(|e| {
                            count_upstream_error(transport_kind(&e), provider);
                            Failure {
                                dropped: dropped(&e),
                                not_found_in_request: false,
                                ..fail(Stage::Upstream)(upstream_error(&e))
                            }
                        })?;
                        if buf.len() + chunk.len() > MAX_ERROR_BODY {
                            break;
                        }
                        buf.extend_from_slice(&chunk);
                    }
                    Bytes::from(buf)
                }
            };
            if !success {
                let status = if (400..600).contains(&status) {
                    status
                } else {
                    502
                };
                let err = provider_error(
                    &eff.provider,
                    plan.dialect,
                    status,
                    &resp.headers,
                    &bytes,
                    &ectx,
                );
                count_upstream_error(err.kind.as_str(), provider);
                let (stage, err) = provider_failed(account, err);
                if err.kind == ErrorKind::NotFound && !credential.is_keyless() {
                    match Box::pin(state.availability.probe(state, &credential, &eff)).await {
                        Probe::Unserved => {
                            tracing::info!(provider, model = %eff.model_id, upstream = %err.message, "the provider does not serve a catalogue model to the project's key");
                            Box::pin(availability::record(
                                state,
                                &learnt(Availability::Unavailable),
                            ))
                            .await;
                            return Err(fail(Stage::Unserved(eff.model_id.clone()))(unserved(
                                &eff,
                            )));
                        }
                        Probe::Served => {
                            let mut failure = fail(stage)(err);
                            failure.not_found_in_request = true;
                            return Err(failure);
                        }
                        Probe::Inconclusive(_) | Probe::Failed(_) => {}
                    }
                }
                if let Some(refused) =
                    refused_custom_key(&err, &eff.provider, credential.is_keyless())
                        .or_else(|| refused_key(&err))
                        .map(|r| {
                            if credential.method == AuthMethod::AzureServicePrincipal {
                                error(ErrorKind::ProviderError, 502, AZURE_ROLE_REFUSED)
                            } else {
                                r
                            }
                        })
                {
                    // Temporary AWS credentials AWS refused, expired ones included, are fetched
                    // again for the next request.
                    if credential.method == kaeso_core::provider::AuthMethod::AwsRole {
                        crate::services::aws_auth::forget(&credential);
                    }
                    let said = redacted(err);
                    tracing::warn!(provider, status, kind = said.kind.as_str(), upstream = %said.message, "the provider refused the project's provider key");
                    if !credential.is_keyless() {
                        Box::pin(crate::services::notifications::system::key_refused(
                            state,
                            caller.org_id,
                            credential.id.uuid(),
                            crate::services::notifications::system::Refusal {
                                status,
                                message: &said.message,
                                billing: said.kind == ErrorKind::Billing,
                            },
                        ))
                        .await;
                    }
                    let mut failure = fail(Stage::KeyRefused)(refused);
                    failure.provider_said = Some(said);
                    return Err(failure);
                }
                return Err(fail(stage)(err));
            }
            if sh.generation {
                let generation =
                    kaeso_codecs::elevenlabs::decode_generation(status, &resp.headers, &bytes)
                        .map_err(|e| unreadable(e, &eff.provider, dialect))
                        .map_err(fail(Stage::Reply))?;
                let started = super::generations::Started {
                    caller,
                    entry: entry.id,
                    eff: &eff,
                    credential: credential.id,
                    dialect,
                    app: &started_req,
                };
                let out = super::generations::register(state, rec, &started, generation)
                    .await
                    .map_err(fail(Stage::Reply))?;
                let mut resp = Response::new(Body::from(out));
                resp.headers_mut().insert(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                );
                stamp(&mut resp, &served);
                return Ok(Done::Complete(resp, StatusCode::OK.as_u16()));
            }
            let (status, headers, bytes) = match dispatch::pending_task(dialect, kind, &bytes) {
                Some(task) => {
                    let asked = super::media::Asked {
                        base,
                        auth,
                        guard: guard_for(&eff.provider),
                        credential: secret,
                        timeouts,
                        max_body: state.config.body_limits.provider_response.as_usize(),
                    };
                    super::media::wait_for_task(state, &asked, dialect, &ectx, &task, deadline)
                        .await
                        .map_err(|e| without_key_fragments(redacted(e)))
                        .map_err(fail(Stage::Provider))?
                }
                None => (status, resp.headers, bytes),
            };
            if kind == RequestKind::Video {
                let reply = decode_reply(dialect, status, &headers, &bytes, &ectx)
                    .map_err(|e| unreadable(e, &eff.provider, dialect))
                    .map_err(fail(Stage::Reply))?;
                let Reply::Video(job) = reply else {
                    return Err(fail(Stage::Reply)(KppError::provider(
                        "The provider answered a video job with another kind of reply.",
                    )));
                };
                let started = super::videos::Started {
                    caller,
                    entry: entry.id,
                    eff: &eff,
                    credential: credential.id,
                    dialect,
                    app: &started_req,
                    remix: sh.video.and_then(|v| v.remix.as_ref()),
                };
                let job = super::videos::register(state, rec, &started, job)
                    .await
                    .map_err(fail(Stage::Reply))?;
                let reply = Reply::Video(job);
                let out = dispatch::encode_response(ingress, &reply, &encode_ctx);
                return reply_with(state, rec, reply, out, counter, carry, &served)
                    .await
                    .map_err(fail(Stage::Reply));
            }
            let decoded_by = eff.provider.clone();
            let model_id = eff.model_id.clone();
            let audio_format = ectx.audio_format.clone();
            let size = bytes.len();
            let mut reply = off_thread(size, move || {
                let mut reply = decode_reply(dialect, status, &headers, &bytes, &ectx)
                    .map_err(|e| unreadable(e, &decoded_by, dialect))?;
                // Gemini's embeddings replies and most image replies name no model.
                match &mut reply {
                    Reply::Embed(e) if e.model.is_empty() => e.model = model_id,
                    Reply::Image(i) if i.model.is_empty() => i.model = model_id,
                    Reply::Speech(r) if r.model.is_empty() => r.model = model_id,
                    Reply::Transcription(r) if r.model.is_empty() => r.model = model_id,
                    Reply::Decide(r) if r.model.is_empty() => r.model = model_id,
                    Reply::Rerank(r) if r.model.is_empty() => r.model = model_id,
                    _ => {}
                }
                Ok::<_, KppError>(reply)
            })
            .await
            .and_then(|r| r)
            .map_err(fail(Stage::Reply))?;
            if let Some(f) = &fetch {
                f.resolve(state, audio_format.as_ref(), &mut reply)
                    .await
                    .map_err(fail(Stage::Upstream))?;
            }
            if let Err(e) =
                Box::pin(guard_reply(state, rec, sh, &started_req, &mut reply, false)).await
            {
                record_usage(rec, &reply, counter).await;
                return Err(fail(Stage::Guardrail)(e));
            }
            if let Some(g) = sh.guards {
                g.restore(&mut reply);
            }
            let (reply, out, media_type) = off_thread(size, move || {
                let out = dispatch::encode_response(ingress, &reply, &encode_ctx);
                let media_type = dispatch::response_media_type(ingress, &reply, &encode_ctx);
                (reply, out, media_type)
            })
            .await
            .map_err(fail(Stage::Reply))?;
            if matches!(kind, RequestKind::Speech | RequestKind::Transcription) {
                rec.outcome().content_type = Some(media_type);
            }
            reply_with(state, rec, reply, out, counter, carry, &served)
                .await
                .map_err(fail(Stage::Reply))
        }
    }
}

fn ready(
    sh: &Shared<'_>,
    access: &Access<'_>,
    held: &cooldowns::Lookup,
    project: Uuid,
    model: CatalogueModelId,
    now: OffsetDateTime,
) -> bool {
    sh.models.catalogue.model(model).is_some_and(|entry| {
        let provider = entry.provider.to_string();
        let at = cooldowns::Step {
            project,
            provider: &provider,
            model: entry.id,
        };
        access.allows(entry.id) && held.held(&at, now).is_none()
    })
}

async fn hold(
    state: &AppState,
    at: &cooldowns::Step<'_>,
    failure: &Failure,
    status: u16,
) -> Option<(cooldowns::Reach, cooldowns::Held)> {
    let cause = match failure.stage {
        Stage::Upstream => cooldowns::Cause::Unreachable,
        Stage::KeyRefused => cooldowns::Cause::KeyRefused,
        Stage::Provider if failure.not_found_in_request => return None,
        Stage::Provider => cooldowns::Cause::Provider(&failure.err),
        Stage::PlanLimit => cooldowns::Cause::PlanLimit(&failure.err),
        _ => return None,
    };
    let (Some((reach, length)), Next::Fall(reason)) = (cooldowns::length(cause), classify(failure))
    else {
        return None;
    };
    let said = failure.logged();
    let now = state.clock.now();
    let held = cooldowns::Held {
        until: now + length,
        reason,
        status,
        error_kind: said.kind.as_str().to_owned(),
        error_message: said.message.clone(),
    };
    state
        .cooldowns
        .hold(&state.valkey, at, reach, held.clone(), now)
        .await;
    Some((reach, held))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Next {
    Fall(RouteFailure),
    Stop,
}

// A provider's error as the attempt fails with it: an account's reached plan limit holds the
// whole account, any other error what its kind says.
fn provider_failed(
    account: Option<kaeso_core::provider::AccountSignIn>,
    err: KppError,
) -> (Stage, KppError) {
    match super::accounts::plan_limit(account, &err) {
        Some(limited) => (Stage::PlanLimit, limited),
        None => (
            Stage::Provider,
            super::accounts::account_error(account, err),
        ),
    }
}

fn classify(f: &Failure) -> Next {
    match &f.stage {
        Stage::Setup
        | Stage::CustomProvidersOff
        | Stage::MissingCredential(_)
        | Stage::SignedOut(_)
        | Stage::NoApiAccess(_)
        | Stage::Credential
        | Stage::Upstream
        | Stage::KeyRefused
        | Stage::Unserved(_) => Next::Fall(RouteFailure::Unavailable),
        Stage::Capability | Stage::NeedsApiKey { .. } => Next::Fall(RouteFailure::Unsupported),
        // Kaeso can't read the counters: every other step would be refused the same way.
        Stage::Limits if f.err.status == Some(503) => Next::Stop,
        Stage::Limits => Next::Fall(RouteFailure::Limited),
        Stage::Provider if f.not_found_in_request => Next::Fall(RouteFailure::Rejected),
        Stage::Provider => Next::Fall(provider_failure(&f.err)),
        Stage::PlanLimit => Next::Fall(RouteFailure::Limited),
        Stage::Caller
        | Stage::Request
        | Stage::Catalogue
        | Stage::UnknownModel(_)
        | Stage::AmbiguousModel(_)
        | Stage::ModelAccess(_)
        | Stage::Reply
        | Stage::Guardrail
        | Stage::Shutdown
        // The app asked for no provider; another step would be refused the same way.
        | Stage::NotCached => Next::Stop,
    }
}

fn provider_failure(err: &KppError) -> RouteFailure {
    match err.kind {
        ErrorKind::RateLimited | ErrorKind::Billing => RouteFailure::Limited,
        ErrorKind::InvalidRequest
        | ErrorKind::ContextTooLong
        | ErrorKind::ContentFiltered
        | ErrorKind::RequestTooLarge
        | ErrorKind::Unsupported { .. } => RouteFailure::Rejected,
        _ => RouteFailure::Unavailable,
    }
}

fn removed_models(alias: &str) -> KppError {
    error(
        ErrorKind::NotFound,
        404,
        format!("The alias `{alias}` has no model left for this request; ask a project admin."),
    )
    .with_param("model")
}

async fn route(
    state: &AppState,
    rec: &mut Recording,
    sh: &Shared<'_>,
    alias: &Alias,
    req: Request,
) -> Result<Done, Failure> {
    let caller = sh.caller;
    let listed = header_list(sh.headers);
    let uses = route::features_of(&req);
    let facts = route::Facts {
        alias: alias.id,
        request: rec.outcome().request_id.uuid(),
        caller: caller
            .key_id
            .or(caller.user_id)
            .or(caller.service_account_id),
        format: route::AppFormat::of(sh.ingress),
        headers: &listed,
        uses: &uses,
        input_tokens: estimate::tokens_of_bytes(
            u64::try_from(sh.request_bytes).unwrap_or(u64::MAX),
        ),
        key: caller.key_id.map(ApiKeyId::from_uuid),
        member: caller.user_id.map(UserId::from_uuid),
        service_account: caller.service_account_id.map(ServiceAccountId::from_uuid),
    };
    let plan = alias.route.plan(&facts);
    let access = sh
        .models
        .access(caller.key_id, caller.user_id, caller.service_account_id);
    let app_req = Arc::new(req);
    let request_id = rec.outcome().request_id.to_string();
    let providers: Vec<(String, Uuid)> = plan
        .iter()
        .filter_map(|p| sh.models.catalogue.model(p.step.model))
        .map(|e| (e.provider.to_string(), e.id))
        .collect();
    let steps: Vec<cooldowns::Step<'_>> = providers
        .iter()
        .map(|(provider, model)| cooldowns::Step {
            project: caller.project_id,
            provider,
            model: *model,
        })
        .collect();
    let mut held_steps = state
        .cooldowns
        .lookup(&state.valkey, &steps, state.clock.now())
        .await;
    let mut failed: Vec<FellBack> = Vec::new();
    let mut walked: Vec<Walked> = Vec::new();
    let mut last: Option<Failure> = None;
    let mut refused = false;
    for (i, planned) in plan.iter().enumerate() {
        let step = planned.step;
        let Some(entry) = sh.models.catalogue.model(step.model) else {
            walked.push(Walked {
                path: planned.path.clone(),
                model: None,
                tried: None,
            });
            continue;
        };
        if !access.allows(entry.id) {
            refused = true;
            walked.push(Walked {
                path: planned.path.clone(),
                model: None,
                tried: None,
            });
            continue;
        }
        let model = format!("{}/{}", entry.provider, entry.model_id);
        let provider_id = entry.provider.to_string();
        let at = cooldowns::Step {
            project: caller.project_id,
            provider: &provider_id,
            model: entry.id,
        };
        let now = state.clock.now();
        if let Some(held) = held_steps.held(&at, now)
            && step.fall_back_on.contains(&held.reason)
            && plan.get(i + 1..).unwrap_or_default().iter().any(|p| {
                ready(
                    sh,
                    &access,
                    &held_steps,
                    caller.project_id,
                    p.step.model,
                    now,
                )
            })
        {
            tracing::debug!(alias = %alias.name, %model, reason = held.reason.as_str(), "an alias step failed moments ago; skipping it");
            metrics::counter!(crate::metrics::ALIAS_FALLBACKS, "provider" => provider_id.clone(), "reason" => held.reason.as_str())
                .increment(1);
            walked.push(Walked {
                path: planned.path.clone(),
                model: Some(model.clone()),
                tried: None,
            });
            failed.push(FellBack {
                catalogue_model_id: entry.id,
                path: planned.path.clone(),
                model,
                status: held.status,
                error_kind: held.error_kind,
                error_message: held.error_message,
                duration_ms: 0,
                skipped: true,
                retried: false,
            });
            continue;
        }
        let began = Instant::now();
        let named = StepTry {
            path: &planned.path,
            model: &model,
            timeout: step.timeout_seconds,
            retries: step.retries.unwrap_or(sh.models.retries),
        };
        let (tried, sent) =
            Box::pin(tries(state, rec, sh, entry, &app_req, named, &mut failed)).await;
        let ended = Instant::now();
        let took = u64::try_from(ended.duration_since(began).as_millis()).unwrap_or(u64::MAX);
        let mut failure = match tried {
            Ok(done) => {
                state.cooldowns.release(&state.valkey, &at).await;
                walked.push(Walked {
                    path: planned.path.clone(),
                    model: Some(model),
                    tried: Some(Tried {
                        status: done.status(),
                        duration_ms: None,
                        tries: sent,
                    }),
                });
                return Ok(done.with_route(&walked));
            }
            Err(f) => {
                rec.attempt_failed(ended);
                f
            }
        };
        let status = failure.err.status.unwrap_or(500);
        walked.push(Walked {
            path: planned.path.clone(),
            model: Some(model.clone()),
            tried: Some(Tried {
                status,
                duration_ms: Some(took),
                tries: sent,
            }),
        });
        if let Some((reach, held)) = hold(state, &at, &failure, status).await {
            held_steps.add(&at, reach, held);
        }
        let falls = i + 1 < plan.len()
            && matches!(classify(&failure), Next::Fall(why) if step.fall_back_on.contains(&why));
        if !falls {
            failure.served.route = walked;
            return Err(failure);
        }
        let why = match classify(&failure) {
            Next::Fall(why) => why.as_str(),
            Next::Stop => "stop",
        };
        let said = failure.logged();
        tracing::info!(alias = %alias.name, %model, status, reason = why, error = %said.message, "an alias step failed; falling back");
        metrics::counter!(crate::metrics::ALIAS_FALLBACKS, "provider" => rec.outcome().provider.unwrap_or("unknown"), "reason" => why)
            .increment(1);
        failed.push(FellBack {
            catalogue_model_id: entry.id,
            path: planned.path.clone(),
            model,
            status,
            error_kind: said.kind.as_str().to_owned(),
            error_message: said.message.clone(),
            duration_ms: took,
            skipped: false,
            retried: false,
        });
        let held = std::mem::take(&mut rec.reservation);
        state.limits.release(state, &held, &request_id).await;
        last = Some(failure);
    }
    match last {
        Some(mut failure) => {
            failed.pop();
            rec.outcome().fallbacks.clone_from(&failed);
            failure.served.route = walked;
            Err(failure)
        }
        None if refused => {
            let mut f = Failure::new(
                no_such_model(&alias.name),
                Stage::ModelAccess(alias.name.clone()),
            );
            f.served.route = walked;
            Err(f)
        }
        None => {
            let mut f = Failure::new(
                removed_models(&alias.name),
                Stage::UnknownModel(alias.name.clone()),
            );
            f.served.route = walked;
            Err(f)
        }
    }
}

impl Done {
    fn status(&self) -> u16 {
        match self {
            Done::Complete(_, status) => *status,
            Done::Streaming(resp) => resp.status().as_u16(),
            Done::Refused(f) => f.err.status.unwrap_or(502),
        }
    }

    fn with_route(self, walked: &[Walked]) -> Done {
        match self {
            Done::Complete(mut resp, status) => {
                stamp_route(&mut resp, walked);
                Done::Complete(resp, status)
            }
            Done::Streaming(mut resp) => {
                stamp_route(&mut resp, walked);
                Done::Streaming(resp)
            }
            Done::Refused(mut f) => {
                f.served.route = walked.to_vec();
                Done::Refused(f)
            }
        }
    }
}

async fn reply_with(
    state: &AppState,
    rec: &mut Recording,
    reply: Reply,
    out: Result<Vec<u8>, KppError>,
    counter: InputCounter,
    carry: Option<uuid::Uuid>,
    served: &Served,
) -> Result<Done, KppError> {
    record_usage(rec, &reply, counter).await;
    let out = out?;
    if let Some(slot) = rec.keep_in_cache.take() {
        let o = rec.outcome();
        let stored = response_cache::Stored {
            reply: reply.clone(),
            usage: o.usage.clone().unwrap_or_default(),
            usage_estimated: o.usage_estimated,
            stored_at_ms: response_cache::unix_ms(state.clock.now()),
        };
        response_cache::put(state, slot, stored);
    }
    answer_with(state, rec, reply, out, carry, served).await
}

async fn answer_with(
    state: &AppState,
    rec: &mut Recording,
    reply: Reply,
    out: Vec<u8>,
    carry: Option<uuid::Uuid>,
    served: &Served,
) -> Result<Done, KppError> {
    if let Some(key_id) = carry
        && let Reply::Generate(r) = &reply
    {
        continuity::keep(state, key_id, &reasoning_for_tool_calls(&r.blocks)).await;
    }
    let reply_kind = reply.kind();
    // Speech is audio and a transcript may be plain text or events: the app's codec names the
    // media type of what it wrote.
    let media_type = rec.outcome().content_type.take();
    match reply {
        Reply::Generate(r) => rec.outcome().reply = Some(r),
        Reply::Image(_)
        | Reply::Speech(_)
        | Reply::Transcription(_)
        | Reply::Decide(_)
        | Reply::Rerank(_)
            if rec.outcome().content_kept =>
        {
            rec.outcome().media_reply = Some(reply);
        }
        _ => {}
    }
    // An image stream the provider answered whole is written as its finished events.
    let events = matches!(reply_kind, RequestKind::Image) && rec.outcome().stream;
    let mut resp = Response::new(Body::from(out));
    let h = resp.headers_mut();
    if let Some(media_type) = media_type.and_then(|m| HeaderValue::from_str(&m).ok()) {
        h.insert(header::CONTENT_TYPE, media_type);
        if rec.outcome().stream {
            h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        }
    } else if events {
        h.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/event-stream; charset=utf-8"),
        );
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    } else {
        h.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
    }
    if let Some(v) = rec.outcome().response_cache {
        h.insert(response_cache::HEADER, v.header());
    }
    stamp(&mut resp, served);
    Ok(Done::Complete(resp, StatusCode::OK.as_u16()))
}

enum Consulted {
    Skip,
    Store(response_cache::Slot),
    Hit(Box<response_cache::Stored>),
    /// `only-if-cached` and no stored reply answers: the provider isn't called.
    NotCached,
}

async fn consult_cache(
    state: &AppState,
    rec: &mut Recording,
    headers: &HeaderMap,
    request: response_cache::Request<'_>,
    stream: bool,
) -> Consulted {
    let directive = response_cache::directive(headers);
    let skip = if directive.only_if_cached {
        Consulted::NotCached
    } else {
        Consulted::Skip
    };
    let Some(active) = response_cache::active(state, request.project).await else {
        return skip;
    };
    let decided = |rec: &mut Recording, v: Verdict| {
        rec.outcome().response_cache = Some(v);
        response_cache::count_lookup(v);
    };
    if !response_cache::cacheable(request.kind, stream) || !directive.write && !directive.read {
        decided(rec, Verdict::Bypass);
        return skip;
    }
    let slot = in_place(request.encoded.body.len(), || {
        response_cache::slot(state, active, &request)
    });
    if directive.read
        && let Some(stored) = response_cache::get(state, &slot).await
        && directive.takes(stored.age(state.clock.now()))
    {
        decided(rec, Verdict::Hit);
        return Consulted::Hit(Box::new(stored));
    }
    // A reply older than `max-age` is a miss too, and the new one replaces it.
    decided(rec, Verdict::Miss);
    if directive.only_if_cached {
        Consulted::NotCached
    } else if directive.write {
        Consulted::Store(slot)
    } else {
        Consulted::Skip
    }
}

fn not_cached() -> KppError {
    error(
        ErrorKind::Timeout,
        504,
        "No cached reply answers this request, and `only-if-cached` keeps it from the provider.",
    )
}

async fn replay(
    state: &AppState,
    rec: &mut Recording,
    stored: response_cache::Stored,
    encode_ctx: &EncodeCtx,
    carry: Option<uuid::Uuid>,
    served: &Served,
) -> Result<Done, KppError> {
    let age = stored.age(state.clock.now());
    let out = dispatch::encode_response(rec.outcome().ingress, &stored.reply, encode_ctx)?;
    let overhead = rec.own_time(Instant::now());
    let o = rec.outcome();
    o.usage = Some(stored.usage);
    o.usage_estimated = stored.usage_estimated;
    o.overhead = Some(overhead);
    o.response_model = match &stored.reply {
        Reply::Generate(r) => Some(r.model.clone()),
        Reply::Embed(e) => Some(e.model.clone()),
        Reply::Decide(d) => Some(d.model.clone()),
        Reply::Rerank(r) => Some(r.model.clone()),
        _ => None,
    };
    let mut done = answer_with(state, rec, stored.reply, out, carry, served).await?;
    if let Done::Complete(resp, _) = &mut done {
        resp.headers_mut()
            .insert(header::AGE, HeaderValue::from(age.as_secs()));
    }
    Ok(done)
}

fn provider_error(
    provider: &ProviderRef,
    dialect: DialectId,
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> KppError {
    without_key_fragments(declared_key_refusal(
        provider,
        dispatch::decode_provider_error(dialect, status, headers, body, ctx),
    ))
}

fn decode_reply(
    dialect: DialectId,
    status: u16,
    headers: &HeaderList,
    body: &[u8],
    ctx: &EgressCtx,
) -> Result<Reply, KppError> {
    dispatch::decode_response(dialect, status, headers, body, ctx).map_err(without_key_fragments)
}

pub(crate) fn without_key_fragments(mut err: KppError) -> KppError {
    redact_key_fragments(&mut err);
    err
}

pub(crate) fn redact_key_fragments(err: &mut KppError) {
    err.message = kaeso_core::secret::redact_tokens(&err.message);
    if let Some(raw) = err.raw.as_mut() {
        kaeso_core::secret::redact_json(raw);
    }
}

pub(crate) fn declared_key_refusal(provider: &ProviderRef, mut err: KppError) -> KppError {
    if err.kind != ErrorKind::Authentication && provider.refuses_key(&err.message) {
        err.kind = ErrorKind::Authentication;
    } else if err.kind == ErrorKind::InvalidRequest && provider.exceeds_context(&err.message) {
        err.kind = ErrorKind::ContextTooLong;
    }
    err
}

async fn record_usage(rec: &mut Recording, reply: &Reply, counter: InputCounter) {
    if let (Reply::Generate(r), Some(kept)) = (reply, rec.kept.as_mut()) {
        kept.response_id = Some(r.id.clone());
    }
    let (usage, model) = match reply {
        Reply::Generate(r) => (&r.usage, &r.model),
        Reply::Embed(e) => (&e.usage, &e.model),
        Reply::Decide(d) => (&d.usage, &d.model),
        Reply::Rerank(r) => (&r.usage, &r.model),
        // Speech and transcripts are billed by characters, duration or the tokens reported.
        Reply::Speech(r) => {
            let o = rec.outcome();
            o.usage = Some(r.usage.clone());
            o.response_model = Some(r.model.clone());
            if let Some(media) = o.media.as_mut() {
                super::media::add_speech(media, r);
            }
            return;
        }
        Reply::Transcription(r) => {
            let o = rec.outcome();
            o.usage = Some(r.usage.clone());
            o.response_model = Some(r.model.clone());
            if let Some(media) = o.media.as_mut() {
                super::media::add_transcript(media, r);
            }
            return;
        }
        // Providers that bill per image report no tokens, so there is nothing to estimate.
        Reply::Image(i) => {
            let o = rec.outcome();
            o.usage = Some(i.usage.clone());
            o.response_model = Some(i.model.clone());
            if let Some(media) = o.media.as_mut() {
                super::media::add_reply(media, i);
            }
            return;
        }
        _ => return,
    };
    let o = rec.outcome();
    o.usage = Some(usage.clone());
    o.response_model = Some(model.clone());
    if usage.raw.is_some() {
        return;
    }
    let mut tally = Tally::new();
    if let Reply::Generate(r) = reply {
        tally.feed_blocks(&r.blocks);
    }
    let input = if usage.total_input_tokens() == 0 {
        Some(Box::pin(counter.count()).await.tokens())
    } else {
        None
    };
    let o = rec.outcome();
    o.usage = Some(estimate::complete(usage, &tally, input));
    o.usage_estimated = true;
    count_estimate(EstimateReason::Unreported, o.provider.unwrap_or_default());
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ModelFlags {
    pub(super) adaptive_thinking: Option<bool>,
    pub(super) thinking_binding: Option<bool>,
    pub(super) reasoning: Option<bool>,
}

impl ModelFlags {
    fn of(plan: &kaeso_core::catalogue::capability::Plan) -> Self {
        ModelFlags {
            adaptive_thinking: plan.adaptive_thinking,
            thinking_binding: plan.thinking_binding,
            reasoning: plan.reasoning,
        }
    }
}

pub(super) fn egress_ctx(
    app_req: &Request,
    provider: &ProviderRef,
    model: &str,
    flags: ModelFlags,
) -> EgressCtx {
    let mut ctx = EgressCtx::new(app_req)
        .with_provider(provider.id())
        .with_chat_profile(provider.chat())
        .with_responses_profile(provider.responses())
        .with_messages_profile(provider.messages())
        .with_gemini_profile(provider.gemini())
        .with_embeddings_profile(provider.embeddings_profile());
    // A key's account scope, applied later, takes the place of this root.
    if let Some(root) = provider.api_root() {
        ctx = ctx.with_api_root(root);
    }
    if let Some(images) = provider.images(model) {
        ctx = ctx.with_images_profile(images.profile);
    }
    if let Some(speech) = provider.speech(model) {
        ctx = ctx.with_speech_profile(speech.profile);
    }
    if let Some(transcriptions) = provider.transcriptions(model) {
        ctx = ctx.with_transcriptions_profile(transcriptions.profile);
    }
    if let Some(decisions) = provider.decisions(model) {
        ctx = ctx
            .with_system_one_profile(decisions.system_one)
            .with_decisions_profile(decisions.openai);
        if let Some(path) = provider
            .decisions_path()
            .filter(|_| app_req.kind() == RequestKind::Decide)
        {
            ctx = ctx.with_base_path(path);
        }
    }
    if let Some(a) = flags.adaptive_thinking {
        ctx = ctx.with_adaptive_thinking(a);
    }
    if let Some(b) = flags.thinking_binding {
        ctx = ctx.with_thinking_binding(b);
    }
    if let Some(r) = flags.reasoning {
        ctx = ctx.with_reasoning(r);
    }
    ctx
}

// An account's token is valid only at its own API host; the input count goes to the
// provider's configured base URL, so no account request may reach it.
fn counted_at_provider(
    account: Option<kaeso_core::provider::AccountSignIn>,
    provider: &ProviderRef,
    dialect: DialectId,
) -> bool {
    account.is_none() && provider.count_tokens_for(dialect) == Some(dialect)
}

#[cfg(test)]
mod tests {
    #[test]
    fn no_account_request_is_counted_at_the_providers_base_url() {
        let openai = ProviderRef::from(kaeso_core::provider::provider("openai").unwrap());
        let grok = kaeso_core::provider::provider("xai")
            .and_then(|p| p.account)
            .unwrap();
        let counted =
            |account| super::counted_at_provider(account, &openai, DialectId::OpenAiResponses);
        assert!(counted(None));
        assert!(!counted(Some(grok)));
        assert!(!counted(openai.account()));
    }

    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn large_work_in_place_runs_beside_the_other_tasks_of_its_worker() {
        let other = tokio::spawn(async { 7 });
        let done = in_place(OFF_THREAD_BYTES, || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            1
        });
        assert_eq!(done + other.await.unwrap(), 8);
        assert_eq!(in_place(OFF_THREAD_BYTES - 1, || 2), 2);
    }

    #[tokio::test]
    async fn work_in_place_runs_inline_without_other_workers() {
        assert_eq!(in_place(usize::MAX, || 3), 3);
    }

    #[test]
    fn the_providers_profiles_and_plan_reach_the_egress_context() {
        let req = Request::Generate(kaeso_kpp::GenerateRequest::new("m"));
        for id in ["openai", "xai", "mistral", "minimax", "google"] {
            let p = ProviderRef::from(kaeso_core::provider::provider(id).unwrap());
            let flags = ModelFlags {
                adaptive_thinking: Some(true),
                thinking_binding: Some(false),
                reasoning: Some(false),
            };
            let ctx = egress_ctx(&req, &p, "m", flags);
            assert_eq!(ctx.provider.as_deref(), Some(id));
            assert_eq!(ctx.chat_profile, p.chat(), "{id}");
            assert_eq!(ctx.responses_profile, p.responses(), "{id}");
            assert_eq!(ctx.messages_profile, p.messages(), "{id}");
            assert_eq!(ctx.gemini_profile, p.gemini(), "{id}");
            assert_eq!(
                (ctx.adaptive_thinking, ctx.thinking_binding, ctx.reasoning),
                (Some(true), Some(false), Some(false))
            );
        }
        let xai = ProviderRef::from(kaeso_core::provider::provider("xai").unwrap());
        assert_eq!(
            egress_ctx(&req, &xai, "m", ModelFlags::default())
                .responses_profile
                .web_search,
            kaeso_codecs::openai_responses::WebSearchTool::Xai
        );
    }

    #[test]
    fn a_custom_providers_decisions_path_reaches_only_its_decision_requests() {
        use kaeso_core::providers::{
            CustomAuth, CustomProvider, DecisionsApi, DecisionsFormat, Draft, ImagePlacement,
            MediaApi,
        };
        let custom = ProviderRef::Custom(Arc::new(
            CustomProvider::new(Draft {
                uuid: uuid::Uuid::now_v7(),
                id: kaeso_core::provider::ProviderId::parse("liquid").unwrap(),
                name: "Liquid".into(),
                base_url: "https://api.liquid.example".into(),
                auth: CustomAuth::Bearer,
                generation: DialectId::OpenAiChat,
                embeddings: None,
                count_tokens: None,
                server: None,
                media: vec![MediaApi::Decisions],
                decisions: Some(DecisionsApi {
                    format: DecisionsFormat::SystemOne,
                    path: "/decisions/v1/systemone".into(),
                    media: Some(ImagePlacement::Root),
                }),
            })
            .unwrap(),
        ));
        let decide = Request::Decide(kaeso_kpp::DecideRequest::new(
            "d1",
            kaeso_kpp::DecisionInput::Text("a cat".into()),
        ));
        let ctx = egress_ctx(&decide, &custom, "d1", ModelFlags::default());
        assert_eq!(ctx.base_path.as_deref(), Some("/decisions/v1/systemone"));
        assert_eq!(
            ctx.system_one_profile.media,
            kaeso_codecs::system_one::MediaPlacement::Root
        );
        let chat = Request::Generate(kaeso_kpp::GenerateRequest::new("d1"));
        assert_eq!(
            egress_ctx(&chat, &custom, "d1", ModelFlags::default()).base_path,
            None
        );
        let openai = ProviderRef::from(kaeso_core::provider::provider("openai").unwrap());
        assert_eq!(
            egress_ctx(&decide, &openai, "gpt-6-luna", ModelFlags::default()).base_path,
            None
        );
    }

    #[test]
    fn a_provider_errors_upstream_reason_reaches_its_message_without_key_fragments() {
        let openrouter = ProviderRef::from(kaeso_core::provider::provider("openrouter").unwrap());
        let body = br#"{"error": {"message": "Provider returned error", "code": 429, "metadata": {"raw": "Rate limit reached for key sk-proj-Ab1xYz789 in organization org-AbCdEf123456."}}}"#;
        let req = Request::Generate(kaeso_kpp::GenerateRequest::new("m"));
        let ctx = EgressCtx::new(&req).with_chat_profile(openrouter.chat());
        let err = provider_error(
            &openrouter,
            DialectId::OpenAiChat,
            429,
            &HeaderList::new(),
            body,
            &ctx,
        );
        assert_eq!(err.kind, ErrorKind::RateLimited);
        assert_eq!(
            err.message,
            "Provider returned error: Rate limit reached for key sk-[redacted] in organization org-[redacted]."
        );

        let in_a_reply =
            decode_reply(DialectId::OpenAiChat, 200, &HeaderList::new(), body, &ctx).unwrap_err();
        assert_eq!(in_a_reply.message, err.message);
    }

    #[test]
    fn a_same_format_error_body_reaches_every_app_without_key_fragments_or_account_ids() {
        let cases: [(&str, DialectId, &[u8]); 3] = [
            (
                "openrouter",
                DialectId::OpenAiChat,
                br#"{"error": {"message": "Provider returned error", "code": 429, "metadata": {"raw": "Rate limit reached for key sk-proj-Ab1xYz789 in organization org-AbCdEf123456."}}, "user_id": "user_2xQ9mLp7Vt4Kd8Rf"}"#,
            ),
            (
                "openai",
                DialectId::OpenAiResponses,
                br#"{"error": {"message": "Rate limit reached in organization org-AbCdEf123456 for key sk-proj-Ab1xYz789.", "type": "requests", "param": null, "code": "rate_limit_exceeded"}}"#,
            ),
            (
                "anthropic",
                DialectId::AnthropicMessages,
                br#"{"type": "error", "error": {"type": "rate_limit_error", "message": "This request would exceed your organization's rate limit (org: 0d5b7a5e-1c2f-4a59-9a8e-3f1d2c4b5a6e, model: claude-opus-5-5)."}, "request_id": "req_011CXyZ"}"#,
            ),
        ];
        for (provider, dialect, body) in cases {
            let p = ProviderRef::from(kaeso_core::provider::provider(provider).unwrap());
            let req = Request::Generate(kaeso_kpp::GenerateRequest::new("m"));
            let ctx = EgressCtx::new(&req).with_chat_profile(p.chat());
            let err = provider_error(&p, dialect, 429, &HeaderList::new(), body, &ctx);
            for app in [
                DialectId::OpenAiChat,
                DialectId::OpenAiResponses,
                DialectId::AnthropicMessages,
            ] {
                let sent = dispatch::encode_error(app, &err).body;
                let text = String::from_utf8_lossy(&sent);
                for secret in ["Ab1xYz789", "AbCdEf123456", "2xQ9mLp7", "0d5b7a5e"] {
                    assert!(!text.contains(secret), "{provider} to {app}: {text}");
                }
                assert!(text.contains("[redacted]"), "{provider} to {app}: {text}");
            }
        }
    }

    #[test]
    fn only_a_dropped_connection_or_a_provider_that_failed_for_a_moment_is_tried_again() {
        let failure = |stage: Stage, kind: ErrorKind, status: u16, dropped: bool| Failure {
            dropped,
            ..Failure::new(error(kind, status, "failed"), stage)
        };
        let jittered = |f: &Failure| {
            let wait = retry_wait(f).expect("tried again");
            assert!(
                (RETRY_WAIT_LEAST..=RETRY_WAIT_MOST).contains(&wait),
                "{wait:?}"
            );
        };
        jittered(&failure(
            Stage::Upstream,
            ErrorKind::ProviderError,
            502,
            true,
        ));
        jittered(&failure(Stage::Provider, ErrorKind::Overloaded, 529, false));
        jittered(&failure(
            Stage::Provider,
            ErrorKind::ProviderError,
            500,
            false,
        ));
        for f in [
            // An address the install refuses, or a reply too large to take.
            failure(Stage::Upstream, ErrorKind::ProviderError, 502, false),
            // The provider may still be working on it, and bill it.
            failure(Stage::Upstream, ErrorKind::Timeout, 504, false),
            failure(Stage::Provider, ErrorKind::Timeout, 504, false),
            failure(Stage::Provider, ErrorKind::RateLimited, 429, false),
            failure(Stage::Provider, ErrorKind::InvalidRequest, 400, false),
            failure(Stage::KeyRefused, ErrorKind::Authentication, 401, false),
            failure(Stage::Limits, ErrorKind::RateLimited, 429, false),
            failure(Stage::Reply, ErrorKind::ProviderError, 502, true),
        ] {
            assert_eq!(retry_wait(&f), None, "{:?} {:?}", f.stage, f.err.kind);
        }
        let after = |secs: u64| {
            let mut f = failure(Stage::Provider, ErrorKind::Overloaded, 503, false);
            f.err.retry_after = Some(Duration::from_secs(secs));
            retry_wait(&f)
        };
        assert_eq!(after(3), Some(Duration::from_secs(3)));
        assert_eq!(after(5), Some(Duration::from_secs(5)));
        assert_eq!(after(6), None, "a longer wait ends the tries");
    }
}
