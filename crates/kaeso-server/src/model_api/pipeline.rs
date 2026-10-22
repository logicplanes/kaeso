use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use bytes::Bytes;
use futures::StreamExt;
use kaeso_codecs::continuity::reasoning_for_tool_calls;
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_core::catalogue::resolve::{ResolveError, access_message};
use kaeso_core::catalogue::{Availability, Effective, capability};
use kaeso_core::estimate::{self, Tally};
use kaeso_core::id::{OrgId, ProjectId, RequestId};
use kaeso_core::limits::WorstCase;
use kaeso_kpp::{DialectId, ErrorKind, KppError, Reply, Request, RequestKind, StreamMode};
use kaeso_store::pg::catalogue::Answer;
use kaeso_upstream::{Call, Outgoing, ResponseBody, Timeouts, UpstreamError};
use url::Url;

use super::auth;
use super::errors::{Served, error, respond, stamp};
use super::hooks::{
    Admission, CLIENT_DISCONNECTED, Caller, Outcome, Reservation, SHUT_DOWN, Sink, Source,
};
use super::input_count::InputCounter;
use super::listing::project_models;
use super::stream;
use crate::services::availability::{self, Probe};
use crate::services::continuity;
use crate::services::credentials::{self, ResolveError as CredentialError};
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
}

pub(super) struct Recording {
    state: AppState,
    sink: Arc<dyn Sink>,
    outcome: Option<Outcome>,
    started: Instant,
    reservation: Reservation,
    sent_input: Option<u64>,
    held: Option<super::memory::Held>,
}

impl Recording {
    pub(super) fn outcome(&mut self) -> &mut Outcome {
        self.outcome.get_or_insert_with(blank_outcome)
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

    pub(super) async fn finish(mut self, status: u16) {
        let elapsed = self.started.elapsed();
        if let Some(mut o) = self.outcome.take() {
            o.status = status;
            if o.duration.is_zero() {
                o.duration = elapsed;
            }
            let charged = self.hand_over(o);
            if self.reservation.is_held() {
                let _ = charged.await;
            }
        }
    }

    fn hand_over(&self, o: Outcome) -> tokio::sync::oneshot::Receiver<()> {
        let (done, charged) = tokio::sync::oneshot::channel();
        let (sink, state) = (self.sink.clone(), self.state.clone());
        self.state.in_flight.finish(async move {
            sink.finish(&state, o).await;
            let _ = done.send(());
        });
        charged
    }

    pub(super) async fn finish_cancelled(mut self) {
        mark_cancelled(self.outcome());
        self.finish(499).await;
    }

    pub(super) async fn fail(self, err: &KppError, status: u16) {
        self.fail_as(err.kind.as_str(), err, status).await;
    }

    pub(super) async fn fail_as(mut self, kind: &str, err: &KppError, status: u16) {
        let o = self.outcome();
        o.error_kind = Some(kind.to_owned());
        o.error_message = Some(err.message.clone());
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

fn blank_outcome() -> Outcome {
    Outcome {
        request_id: RequestId::new(),
        caller: None,
        source: Source::Api,
        ingress: DialectId::OpenAiChat,
        kind: RequestKind::Generate,
        egress: None,
        provider: None,
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
        user_agent: None,
        client_ip: None,
        upstream_request_id: None,
        request_body: None,
        reply: None,
        cancelled: false,
        stream_progress: None,
        usage_estimated: false,
        content_kept: false,
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
    Capability,
    Limits,
    MissingCredential(&'static str),
    Credential,
    Upstream,
    Provider,
    KeyRefused,
    Unserved(String),
    Reply,
    Shutdown,
}

#[derive(Clone, Debug)]
pub struct Failure {
    pub err: KppError,
    pub served: Served,
    pub stage: Stage,
    pub provider_said: Option<KppError>,
}

impl Failure {
    fn new(err: KppError, stage: Stage) -> Failure {
        Failure {
            err,
            served: Served::default(),
            stage,
            provider_said: None,
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
    let mut outcome = blank_outcome();
    outcome.request_id = inc.request_id;
    outcome.ingress = inc.ingress;
    outcome.kind = inc.kind;
    outcome.source = inc.source;
    outcome.caller.clone_from(&inc.caller);
    outcome.started_at = now;
    outcome.client_ip = inc.client_ip;
    outcome.user_agent = inc
        .headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let mut rec = Recording {
        state: state.clone(),
        sink: state.sink.clone(),
        outcome: Some(outcome),
        started: Instant::now(),
        reservation: Reservation::default(),
        sent_input: None,
        held: None,
    };
    let done = tokio::select! {
        done = run(&state, inc, &mut rec) => Some(done),
        () = state.in_flight.cut() => None,
    };
    let Some(done) = done else {
        let failure = Failure::new(shutting_down(), Stage::Shutdown);
        let resp = answer(&failure);
        rec.fail_as(SHUT_DOWN, &failure.err, resp.status().as_u16())
            .await;
        return resp;
    };
    match done {
        Ok(Done::Complete(resp, status)) => {
            rec.finish(status).await;
            resp
        }
        Ok(Done::Streaming(resp)) => {
            rec.outcome = None;
            resp
        }
        Err(failure) => {
            let resp = answer(&failure);
            rec.fail(failure.logged(), resp.status().as_u16()).await;
            resp
        }
    }
}

enum Done {
    Complete(Response, u16),
    Streaming(Response),
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

pub(crate) fn base_url<'a>(state: &'a AppState, provider: &str) -> Option<&'a Url> {
    state.config.providers.get(provider).map(|p| &p.base_url)
}

pub(super) fn transport_kind(e: &UpstreamError) -> &'static str {
    match e {
        UpstreamError::Timeout(_) => "timeout",
        UpstreamError::Connect(_) => "connect",
        UpstreamError::Transfer(_) => "transfer",
        UpstreamError::TooLarge(_) => "too_large",
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

pub(crate) fn upstream_error(e: &UpstreamError) -> KppError {
    match e {
        UpstreamError::Timeout(what) => error(
            ErrorKind::Timeout,
            504,
            format!(
                "The provider did not answer in time ({what}). Kaeso does not retry; try again or raise the model's timeout."
            ),
        ),
        UpstreamError::Connect(_) => error(
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

const KEY_REFUSED: &str = "The provider refused this project's provider key. Ask an organization admin to check it under Providers.";

const ACCOUNT_REFUSED: &str = "The provider refused this project's provider key for billing or quota reasons. Ask an organization admin to check it under Providers.";

pub(crate) fn refused_key(err: &KppError) -> Option<KppError> {
    let message = match err.kind {
        ErrorKind::Authentication | ErrorKind::PermissionDenied => KEY_REFUSED,
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
            eff.provider.name, eff.model_id
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
        ..
    } = inc;
    let caller = match caller {
        Some(c) => c,
        None => {
            let key = auth::authenticate(state, &headers, client_ip)
                .await
                .map_err(|e| Failure::new(e, Stage::Caller))?;
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
    let at = |stage: Stage| move |e: KppError| Failure::new(e, stage.clone());

    let limit = max_body;
    let body = axum::body::to_bytes(body, limit).await.map_err(|e| {
        let refused = if crate::http::body_timeout::timed_out(&e) {
            error(
                ErrorKind::InvalidRequest,
                400,
                "The request body did not arrive in time; send it again.",
            )
        } else {
            error(
                ErrorKind::RequestTooLarge,
                413,
                format!("The request body is larger than {limit} bytes."),
            )
        };
        Failure::new(refused, Stage::Request)
    })?;
    let keep_content = crate::services::logs::keeps_content(state, caller.project_id).await;
    if keep_content {
        let o = rec.outcome();
        o.content_kept = true;
        o.request_body = Some(body.clone());
    }
    let request_bytes = body.len();
    let scanned = body.clone();
    let shape = off_thread(request_bytes, move || {
        kaeso_codecs::limits::request_shape(ingress, &scanned)
    })
    .await
    .and_then(|r| r)
    .map_err(at(Stage::Request))?;
    let holder = caller
        .key_id
        .or(caller.user_id)
        .or(caller.service_account_id)
        .unwrap_or(caller.project_id);
    // Held for the whole call, after the translation's share is returned: the app's
    // decoded request, kept to encode the reply, and the raw body until it is logged.
    let logged = if keep_content { request_bytes } else { 0 };
    let retained =
        super::memory::decoded(&shape).saturating_add(u64::try_from(logged).unwrap_or(u64::MAX));
    rec.held = Some(
        state
            .translating
            .hold(
                holder,
                super::memory::estimate(kind, &shape).saturating_add(retained),
            )
            .await
            .map_err(|_| Failure::new(translating_busy(), Stage::Limits))?,
    );
    let listed = header_list(&headers);
    let mut req = off_thread(request_bytes, move || {
        dispatch::decode_request(ingress, kind, &body, &listed)
    })
    .await
    .and_then(|r| r)
    .map_err(at(Stage::Request))?;
    req.validate().map_err(at(Stage::Request))?;
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

    let models = project_models(state, caller.project_id)
        .await
        .map_err(|e| Failure::new(e, Stage::Catalogue))?;
    let (entry, _) = models
        .catalogue
        .resolve(req.model())
        .map_err(|e| match &e {
            ResolveError::NotInCatalogue(name) => Failure::new(
                error(ErrorKind::NotFound, 404, e.to_string()).with_param("model"),
                Stage::UnknownModel(name.clone()),
            ),
            ResolveError::Ambiguous { name, .. } => Failure::new(
                error(ErrorKind::InvalidRequest, 400, e.to_string()).with_param("model"),
                Stage::AmbiguousModel(name.clone()),
            ),
        })?;
    if let Err(level) = models
        .access(caller.key_id, caller.user_id, caller.service_account_id)
        .check(entry.id)
    {
        return Err(Failure::new(
            error(
                ErrorKind::PermissionDenied,
                403,
                access_message(req.model(), level),
            )
            .with_param("model"),
            Stage::ModelAccess(req.model().to_owned()),
        ));
    }
    let eff = crate::services::catalogue::effective(state, entry).ok_or_else(|| {
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
        provider: Some(eff.provider.id),
    };
    let fail = |stage: Stage| {
        let served = served.clone();
        move |err: KppError| Failure {
            err,
            served,
            stage,
            provider_said: None,
        }
    };
    {
        let o = rec.outcome();
        o.catalogue_model_id = Some(entry.id);
        o.model_id = Some(eff.model_id.clone());
        o.provider = Some(eff.provider.id);
        o.prices = Some(eff.fields.clone());
    }
    let app_req = Arc::new(req.clone());
    let plan = capability::prepare(&mut req, ingress, &eff).map_err(|u| {
        let mut e = KppError::unsupported(u.capability, u.message).with_status(400);
        if let Some(p) = u.param {
            e = e.with_param(p);
        }
        fail(Stage::Capability)(e)
    })?;
    rec.outcome().egress = Some(plan.dialect);
    match &mut req {
        Request::Generate(g) | Request::CountTokens(g) => g.model.clone_from(&eff.model_id),
        Request::Embed(e) => e.model.clone_from(&eff.model_id),
        _ => {}
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
    .with_model_override(eff.fields.timeout_seconds);
    let admission = Admission {
        request_id: rec.outcome().request_id,
        caller: caller.clone(),
        catalogue_model_id: entry.id,
        kind,
        at: rec.outcome().started_at,
        timeout: timeouts.request,
        worst: WorstCase::of(&req, request_bytes, &eff.fields, eff.provider),
        unpriced: kaeso_core::cost::unpriced(&req, plan.dialect),
    };
    rec.reservation = state
        .limits
        .admit(state, &admission)
        .await
        .map_err(|refusal| {
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
            fail(Stage::Limits)(e)
        })?;

    let credential = credentials::resolve(
        state,
        OrgId::from_uuid(caller.org_id),
        ProjectId::from_uuid(caller.project_id),
        eff.provider.id,
    )
    .await
    .map_err(|e| match e {
        CredentialError::Missing { .. } => fail(Stage::MissingCredential(eff.provider.id))(error(ErrorKind::InvalidRequest, 400, e.to_string()).with_param("model")),
        CredentialError::Crypto(_) => {
            tracing::error!(provider = eff.provider.id, "a provider credential cannot be decrypted");
            fail(Stage::Credential)(error(ErrorKind::ProviderError, 500, "Kaeso cannot decrypt the provider credential. The operator must check the configured master keys."))
        }
        CredentialError::Store(err) => {
            tracing::error!(error = %err, "cannot load a provider credential");
            fail(Stage::Credential)(error(ErrorKind::ProviderError, 503, "Kaeso cannot load the provider credential right now. Try again shortly."))
        }
    })?;

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

    let mut ectx = EgressCtx::new(&app_req)
        .with_provider(eff.provider.id)
        .with_chat_profile(eff.provider.chat);
    if let Some(a) = plan.adaptive_thinking {
        ectx = ectx.with_adaptive_thinking(a);
    }
    if let Some(r) = plan.reasoning {
        ectx = ectx.with_reasoning(r);
    }
    let dialect = plan.dialect;
    let (req, ectx, encoded) = off_thread(request_bytes, move || {
        let encoded = dispatch::encode_request(dialect, &req, &mut ectx);
        (req, ectx, encoded)
    })
    .await
    .map_err(fail(Stage::Capability))?;
    let encoded = encoded.map_err(fail(Stage::Capability))?;
    if let Some(held) = &mut rec.held {
        held.keep(retained);
    }
    let base = base_url(state, eff.provider.id).ok_or_else(|| {
        fail(Stage::Setup)(error(
            ErrorKind::ProviderError,
            500,
            "No base URL is configured for this provider.",
        ))
    })?;
    let sent_bytes = encoded.body.len();
    let provider = eff.provider.id;
    rec.sent_input = Some(estimate::tokens_of_bytes(
        u64::try_from(sent_bytes).unwrap_or(u64::MAX),
    ));
    let sent_at = Instant::now();
    let resp = state
        .upstream
        .send(Call {
            base_url: base,
            auth: eff.provider.auth,
            credential: &credential.secret,
            request: Outgoing::from(encoded),
            stream: stream && kind == RequestKind::Generate,
            timeouts,
            max_body: state.config.body_limits.provider_response.as_usize(),
        })
        .await
        .map_err(|e| {
            count_upstream_error(transport_kind(&e), provider);
            fail(Stage::Upstream)(upstream_error(&e))
        })?;
    rec.outcome().ttfb = Some(sent_at.elapsed());
    rec.outcome().upstream_request_id = resp
        .headers
        .get("request-id")
        .or_else(|| resp.headers.get("x-request-id"))
        .map(str::to_owned);
    let encode_ctx = EncodeCtx::for_request(Arc::clone(&app_req))
        .with_created_at(u64::try_from(rec.outcome().started_at.unix_timestamp()).unwrap_or(0));
    let status = resp.status;
    let success = (200..300).contains(&status);
    let learnt = |availability: Availability| Answer {
        org_id: caller.org_id,
        project_id: caller.project_id,
        entry: entry.id,
        provider: eff.provider.id,
        key: credential.key(),
        availability,
        at: state.clock.now(),
    };
    if success && entry.is_unavailable() {
        availability::record(state, &learnt(Availability::Available)).await;
    }

    let counter = InputCounter {
        state: state.clone(),
        credential: credential.clone(),
        provider,
        auth: eff.provider.auth,
        dialect: plan.dialect,
        asked: (eff.provider.count_tokens == Some(plan.dialect)).then_some((app_req, req)),
        adaptive_thinking: plan.adaptive_thinking,
        reasoning: plan.reasoning,
        timeouts,
        sent_bytes,
    };

    match resp.body {
        ResponseBody::Stream(chunks) if success => {
            let decoder =
                dispatch::stream_decoder(plan.dialect, &ectx).map_err(fail(Stage::Reply))?;
            let encoder =
                dispatch::stream_encoder(ingress, &encode_ctx).map_err(fail(Stage::Reply))?;
            let (tx, rx) = tokio::sync::mpsc::channel::<Bytes>(32);
            let task_rec = Recording {
                state: rec.state.clone(),
                sink: rec.sink.clone(),
                outcome: rec.outcome.take(),
                started: rec.started,
                reservation: std::mem::take(&mut rec.reservation),
                sent_input: None,
                held: rec.held.take(),
            };
            let keepalive = state.config.streams.keepalive_interval;
            let carry = carry.map(|key_id| continuity::StreamCarry::new(state.clone(), key_id));
            let parts = stream::Parts {
                chunks,
                decoder,
                encoder,
                keepalive: stream::Keepalive::new(keepalive),
                counter,
                carry,
                keep_reply: keep_content,
            };
            state.in_flight.spawn(stream::pump(parts, tx, task_rec));
            let body = Body::from_stream(futures::stream::unfold(rx, |mut rx| async move {
                rx.recv()
                    .await
                    .map(|b| (Ok::<_, std::convert::Infallible>(b), rx))
            }));
            let mut resp = Response::new(body);
            let h = resp.headers_mut();
            h.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/event-stream; charset=utf-8"),
            );
            h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
            h.insert("x-accel-buffering", HeaderValue::from_static("no"));
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
                            fail(Stage::Upstream)(upstream_error(&e))
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
                let err = provider_error(
                    eff.provider,
                    plan.dialect,
                    status,
                    &resp.headers,
                    &bytes,
                    &ectx,
                );
                count_upstream_error(err.kind.as_str(), provider);
                if err.kind == ErrorKind::NotFound
                    && let Probe::Unserved =
                        state.availability.probe(state, &credential, &eff).await
                {
                    tracing::info!(provider, model = %eff.model_id, upstream = %err.message, "the provider does not serve a catalogue model to the project's key");
                    availability::record(state, &learnt(Availability::Unavailable)).await;
                    return Err(fail(Stage::Unserved(eff.model_id.clone()))(unserved(&eff)));
                }
                if let Some(refused) = refused_key(&err) {
                    let said = redacted(err);
                    tracing::warn!(provider, status, kind = said.kind.as_str(), upstream = %said.message, "the provider refused the project's provider key");
                    let mut failure = fail(Stage::KeyRefused)(refused);
                    failure.provider_said = Some(said);
                    return Err(failure);
                }
                return Err(fail(Stage::Provider)(err));
            }
            let headers = resp.headers;
            let (reply, out) = off_thread(bytes.len(), move || {
                let reply = decode_reply(dialect, status, &headers, &bytes, &ectx)?;
                let out = dispatch::encode_response(ingress, &reply, &encode_ctx);
                Ok::<_, KppError>((reply, out))
            })
            .await
            .and_then(|r| r)
            .map_err(fail(Stage::Reply))?;
            record_usage(rec, &reply, counter).await;
            let out = out.map_err(fail(Stage::Reply))?;
            if let Some(key_id) = carry
                && let Reply::Generate(r) = &reply
            {
                continuity::keep(state, key_id, &reasoning_for_tool_calls(&r.blocks)).await;
            }
            if let Reply::Generate(r) = reply {
                rec.outcome().reply = Some(r);
            }
            let mut resp = Response::new(Body::from(out));
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            stamp(&mut resp, &served);
            Ok(Done::Complete(resp, StatusCode::OK.as_u16()))
        }
    }
}

fn provider_error(
    provider: &kaeso_core::provider::Provider,
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

pub(crate) fn declared_key_refusal(
    provider: &kaeso_core::provider::Provider,
    mut err: KppError,
) -> KppError {
    if err.kind != ErrorKind::Authentication && provider.refuses_key(&err.message) {
        err.kind = ErrorKind::Authentication;
    }
    err
}

async fn record_usage(rec: &mut Recording, reply: &Reply, counter: InputCounter) {
    let (usage, model) = match reply {
        Reply::Generate(r) => (&r.usage, &r.model),
        Reply::Embed(e) => (&e.usage, &e.model),
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
        Some(counter.count().await.tokens())
    } else {
        None
    };
    let o = rec.outcome();
    o.usage = Some(estimate::complete(usage, &tally, input));
    o.usage_estimated = true;
    count_estimate(EstimateReason::Unreported, o.provider.unwrap_or_default());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_provider_errors_upstream_reason_reaches_its_message_without_key_fragments() {
        let openrouter = kaeso_core::provider::provider("openrouter").unwrap();
        let body = br#"{"error": {"message": "Provider returned error", "code": 429, "metadata": {"raw": "Rate limit reached for key sk-proj-Ab1xYz789 in organization org-AbCdEf123456."}}}"#;
        let req = Request::Generate(kaeso_kpp::GenerateRequest::new("m"));
        let ctx = EgressCtx::new(&req).with_chat_profile(openrouter.chat);
        let err = provider_error(
            openrouter,
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
            let p = kaeso_core::provider::provider(provider).unwrap();
            let req = Request::Generate(kaeso_kpp::GenerateRequest::new("m"));
            let ctx = EgressCtx::new(&req).with_chat_profile(p.chat);
            let err = provider_error(p, dialect, 429, &HeaderList::new(), body, &ctx);
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
}
