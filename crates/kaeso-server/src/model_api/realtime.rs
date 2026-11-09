//! ElevenLabs' realtime WebSocket APIs for ElevenLabs-format apps: text to speech input
//! streaming and its multi-context form, the text to dialogue sockets, and realtime speech to
//! text. A session is one request: authenticated and admitted when it opens, with the project's
//! ElevenLabs key on the provider's socket, held to the project's spend budgets while it runs,
//! and logged with its characters or audio length when it ends. Messages pass between the app
//! and ElevenLabs as they are, except the credentials an app's first message names.

use std::time::Duration;

use axum::extract::ws::{self, WebSocket, WebSocketUpgrade, rejection::WebSocketUpgradeRejection};
use axum::extract::{Extension, OriginalUri, State};
use axum::http::HeaderMap;
use axum::response::Response;
use futures::{SinkExt, StreamExt};
use kaeso_codecs::elevenlabs::realtime::{self as rt, RealtimeApi};
use kaeso_core::catalogue::Effective;
use kaeso_core::catalogue::resolve::Resolved;
use kaeso_core::cost::MediaUnits;
use kaeso_core::id::{OrgId, ProjectId, RequestId};
use kaeso_core::limits::WorstCase;
use kaeso_kpp::{DialectId, ErrorKind, KppError, RequestKind};
use kaeso_store::pg::keys::KeyAuth;
use kaeso_upstream::ws::{CloseCode, CloseFrame, Dial, Message, Socket};
use time::OffsetDateTime;
use url::Url;

use super::errors::{Served, error, respond};
use super::hooks::{Admission, Caller, Reservation};
use crate::http::client_ip::ClientIp;
use crate::state::AppState;

const D: DialectId = DialectId::ElevenLabs;

// ElevenLabs suggests realtime audio chunks of a fraction of a second; a mebibyte of base64 holds
// over 15 seconds of 16 kHz PCM, and a text message is far smaller.
const MAX_MESSAGE: usize = 1 << 20;
// The reservation a session holds lives this long, and so may the session.
const MAX_SESSION: Duration = Duration::from_secs(60 * 60);
const BUDGET_CHECKS: Duration = Duration::from_secs(30);

fn refuse(err: &KppError) -> Response {
    respond(D, err, &Served::default())
}

/// `GET` on an ElevenLabs realtime path, upgraded to a WebSocket.
pub(super) async fn socket(
    State(state): State<AppState>,
    Extension(request_id): Extension<RequestId>,
    ip: ClientIp,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    let Some((api, voice)) = RealtimeApi::from_path(uri.path()) else {
        return refuse(&error(
            ErrorKind::NotFound,
            404,
            format!("Kaeso does not serve `{}`.", uri.path()),
        ));
    };
    let Ok(upgrade) = upgrade else {
        return refuse(&error(
            ErrorKind::InvalidRequest,
            400,
            format!("Open `{}` as a WebSocket.", uri.path()),
        ));
    };
    let opening = match Opening::new(
        state,
        request_id,
        ip,
        api,
        voice,
        uri.query().unwrap_or(""),
        &headers,
    ) {
        Ok(o) => o,
        Err(e) => return refuse(&e),
    };
    let upgrade = upgrade
        .max_message_size(MAX_MESSAGE)
        .max_frame_size(MAX_MESSAGE);
    match super::auth::presented(&headers).filter(|s| !s.is_empty()) {
        Some(secret) => match opening.open(secret).await {
            Ok(session) => upgrade.on_upgrade(move |client| session.run(client, None)),
            Err(e) => refuse(&e),
        },
        None if api.takes_key_in_first_message() => {
            upgrade.on_upgrade(move |client| opening.open_from_first_message(client))
        }
        None => refuse(&super::auth::missing_key()),
    }
}

/// A session's request before the app's key is checked.
struct Opening {
    state: AppState,
    request_id: RequestId,
    ip: ClientIp,
    api: RealtimeApi,
    voice: Option<String>,
    model: String,
    query: Vec<(String, String)>,
    user_agent: Option<String>,
    started_at: OffsetDateTime,
}

fn param_error(message: impl Into<String>, param: &'static str) -> KppError {
    error(ErrorKind::InvalidRequest, 400, message).with_param(param)
}

impl Opening {
    fn new(
        state: AppState,
        request_id: RequestId,
        ip: ClientIp,
        api: RealtimeApi,
        voice: Option<String>,
        query: &str,
        headers: &HeaderMap,
    ) -> Result<Opening, KppError> {
        let query: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes())
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        if let Some((k, _)) = query
            .iter()
            .find(|(k, _)| rt::CREDENTIAL_PARAMS.contains(&k.as_str()))
        {
            return Err(error(
                ErrorKind::Authentication,
                401,
                format!(
                    "Kaeso takes no `{k}` in the address; send a Kaeso API key as `xi-api-key`."
                ),
            ));
        }
        let model = query
            .iter()
            .find(|(k, _)| k == "model_id")
            .map(|(_, v)| v.clone())
            .filter(|m| !m.is_empty())
            .ok_or_else(|| {
                param_error(
                    "Name the model in `model_id`, so Kaeso knows its price and limits.",
                    "model_id",
                )
            })?;
        api.path(voice.as_deref())?;
        let now = state.clock.now();
        Ok(Opening {
            state,
            request_id,
            ip,
            api,
            voice,
            model,
            query,
            user_agent: headers
                .get(axum::http::header::USER_AGENT)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned),
            started_at: now,
        })
    }

    async fn open_from_first_message(self, mut client: WebSocket) {
        let first = match tokio::time::timeout(
            self.state.config.upstream.first_byte_timeout,
            client.recv(),
        )
        .await
        {
            Ok(Some(Ok(ws::Message::Text(text)))) => text,
            _ => {
                close_with(&mut client, self.api, &super::auth::missing_key()).await;
                return;
            }
        };
        let read = match rt::read_client_message(self.api, first.as_str()) {
            Ok(r) => r,
            Err(e) => return close_with(&mut client, self.api, &e).await,
        };
        let Some(secret) = read.credential.clone() else {
            return close_with(&mut client, self.api, &super::auth::missing_key()).await;
        };
        let api = self.api;
        match self.open(&secret).await {
            Ok(session) => session.run(client, Some(read)).await,
            Err(e) => close_with(&mut client, api, &e).await,
        }
    }

    async fn open(self, secret: &str) -> Result<Session, KppError> {
        let state = &self.state;
        let key = super::auth::authenticate_secret(state, secret, self.ip.0).await?;
        let models = super::listing::project_models(
            state,
            key.project_id,
            Some(key.id),
            key.service_account_id,
        )
        .await?;
        let not_found = || {
            error(
                ErrorKind::NotFound,
                404,
                format!(
                    "The model `{}` does not exist or this key may not use it.",
                    self.model
                ),
            )
            .with_param("model_id")
        };
        let entry = match models.catalogue.resolve(&self.model) {
            Ok(Resolved::Model(entry, _)) => entry,
            Ok(Resolved::Alias(_)) => {
                return Err(param_error(
                    "Aliases don't route realtime sessions; name an ElevenLabs model in `model_id`.",
                    "model_id",
                ));
            }
            Err(_) => return Err(not_found()),
        };
        if !models
            .access(Some(key.id), key.user_id, key.service_account_id)
            .allows(entry.id)
        {
            return Err(not_found());
        }
        let eff = crate::services::catalogue::effective(state, entry, &models.providers)
            .filter(|e| e.provider.custom().is_none() && e.provider.id() == "elevenlabs")
            .ok_or_else(|| {
                param_error(
                    format!(
                        "`{}` is not an ElevenLabs model; realtime sessions reach ElevenLabs only.",
                        self.model
                    ),
                    "model_id",
                )
            })?;
        let fits = if self.api.speaks() {
            eff.fields
                .output_modalities
                .as_ref()
                .is_none_or(|m| m.iter().any(|x| x == "audio"))
        } else {
            eff.fields
                .input_modalities
                .as_ref()
                .is_none_or(|m| m.iter().any(|x| x == "audio"))
        };
        if !fits {
            return Err(param_error(
                format!(
                    "`{}` does not serve this socket; pick a model that does.",
                    self.model
                ),
                "model_id",
            ));
        }
        let caller = Caller {
            org_id: key.org_id,
            project_id: key.project_id,
            key_id: Some(key.id),
            user_id: key.user_id,
            service_account_id: key.service_account_id,
        };
        let kind = if self.api.speaks() {
            RequestKind::Speech
        } else {
            RequestKind::Transcription
        };
        let admission = Admission {
            request_id: self.request_id,
            caller: caller.clone(),
            catalogue_model_id: entry.id,
            kind,
            at: self.started_at,
            timeout: MAX_SESSION,
            worst: WorstCase::default(),
            unpriced: kaeso_core::cost::unpriced_model(
                &eff.fields,
                &entry.overrides,
                kind,
                &eff.model_id,
            ),
        };
        let reservation = state
            .limits
            .admit(state, &admission)
            .await
            .map_err(super::pipeline::limited)?;
        let opened = self.dial(&key, &eff).await;
        let upstream = match opened {
            Ok(u) => u,
            Err(e) => {
                state
                    .limits
                    .release(state, &reservation, &self.request_id.to_string())
                    .await;
                return Err(e);
            }
        };
        Ok(Session {
            opening: self,
            caller,
            catalogue_model_id: entry.id,
            eff,
            reservation,
            upstream,
        })
    }

    async fn dial(&self, key: &KeyAuth, eff: &Effective) -> Result<Socket, KppError> {
        let state = &self.state;
        let credential = crate::services::credentials::for_provider(
            state,
            OrgId::from_uuid(key.org_id),
            ProjectId::from_uuid(key.project_id),
            &eff.provider,
        )
        .await
        .map_err(|e| match e {
            crate::services::credentials::ResolveError::Missing { .. } => {
                error(ErrorKind::InvalidRequest, 400, e.to_string())
            }
            other => {
                tracing::error!(error = %other, "cannot load the ElevenLabs credential");
                error(
                    ErrorKind::ProviderError,
                    503,
                    "Kaeso cannot load the provider credential right now. Try again shortly.",
                )
            }
        })?;
        let url = self.upstream_url(eff)?;
        kaeso_upstream::ws::dial(Dial {
            url: &url,
            auth: Some(("xi-api-key", &credential.secret)),
            user_agent: concat!("kaeso-gateway/", env!("CARGO_PKG_VERSION")),
            connect_timeout: state.config.upstream.connect_timeout,
            max_message: MAX_MESSAGE,
        })
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "ElevenLabs did not open a realtime socket");
            match e {
                kaeso_upstream::ws::WsError::Refused {
                    status: 401 | 403, ..
                } => error(ErrorKind::ProviderError, 502, super::pipeline::KEY_REFUSED),
                kaeso_upstream::ws::WsError::Refused { status, .. }
                    if (400..500).contains(&status) =>
                {
                    error(
                        ErrorKind::InvalidRequest,
                        400,
                        "ElevenLabs refused to open the socket; check its query parameters.",
                    )
                }
                _ => error(
                    ErrorKind::ProviderError,
                    502,
                    "Kaeso cannot reach ElevenLabs right now; try again shortly.",
                ),
            }
        })
    }

    // ElevenLabs' sockets answer under the same host as its HTTP API, `wss` for `https`.
    fn upstream_url(&self, eff: &Effective) -> Result<Url, KppError> {
        let setup = || {
            error(
                ErrorKind::ProviderError,
                500,
                "No base URL is configured for this provider.",
            )
        };
        let mut url = super::pipeline::base_url(&self.state, &eff.provider)
            .cloned()
            .ok_or_else(setup)?;
        let scheme = match url.scheme() {
            "https" => "wss",
            "http" => "ws",
            _ => return Err(setup()),
        };
        url.set_scheme(scheme).map_err(|()| setup())?;
        url.set_path(&self.api.path(self.voice.as_deref())?);
        let mut pairs = url::form_urlencoded::Serializer::new(String::new());
        for (k, v) in &self.query {
            let v = if k == "model_id" { &eff.model_id } else { v };
            pairs.append_pair(k, v);
        }
        url.set_query(Some(&pairs.finish()));
        Ok(url)
    }
}

async fn close_with(client: &mut WebSocket, api: RealtimeApi, err: &KppError) {
    let (body, code) = rt::error_message(api, err);
    let _ = client.send(ws::Message::Text(body.into())).await;
    let _ = client
        .send(ws::Message::Close(Some(ws::CloseFrame {
            code,
            reason: short_reason(&err.message).into(),
        })))
        .await;
}

// A close frame's reason is at most 123 bytes (RFC 6455, section 5.5).
fn short_reason(message: &str) -> String {
    let mut out = String::new();
    for c in message.chars() {
        if out.len() + c.len_utf8() > 123 {
            break;
        }
        out.push(c);
    }
    out
}

/// An open session: the app's socket is about to be served by ElevenLabs'.
struct Session {
    opening: Opening,
    caller: Caller,
    catalogue_model_id: uuid::Uuid,
    eff: Effective,
    reservation: Reservation,
    upstream: Socket,
}

#[derive(Default)]
struct Measured {
    characters: u64,
    audio_ms: u64,
}

impl Measured {
    fn add(&mut self, read: &rt::ClientMessage, audio_format: Option<&str>) {
        self.characters = self.characters.saturating_add(read.characters);
        let per_second = match (read.sample_rate, audio_format) {
            (Some(rate), Some(f)) if f.starts_with("pcm") => Some(u64::from(rate) * 2),
            (_, Some(f)) => rt::bytes_per_second(f),
            _ => None,
        };
        if let Some(per_second) = per_second.filter(|p| *p > 0) {
            self.audio_ms = self
                .audio_ms
                .saturating_add(read.audio_bytes.saturating_mul(1000) / per_second);
        }
    }

    fn units(&self, format: Option<String>) -> MediaUnits {
        MediaUnits {
            characters: self.characters,
            input_ms: self.audio_ms,
            format,
            measured: self.audio_ms > 0,
            ..MediaUnits::default()
        }
    }
}

enum End {
    Done,
    Failed(KppError),
}

impl Session {
    fn audio_format(&self) -> Option<String> {
        let key = if self.opening.api.speaks() {
            "output_format"
        } else {
            "audio_format"
        };
        self.opening
            .query
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    }

    async fn run(mut self, mut client: WebSocket, first: Option<rt::ClientMessage>) {
        let state = self.opening.state.clone();
        let api = self.opening.api;
        let format = self.audio_format();
        let mut measured = Measured::default();
        let started = tokio::time::Instant::now();
        let mut end = End::Done;
        if let Some(first) = first {
            measured.add(&first, format.as_deref());
            if self
                .upstream
                .send(Message::text(first.forwarded))
                .await
                .is_err()
            {
                end = End::Failed(KppError::provider("ElevenLabs closed the session."));
            }
        }
        let idle = state.config.upstream.stream_idle_timeout;
        let deadline = started + MAX_SESSION;
        let mut last = started;
        let mut checks = tokio::time::interval_at(started + BUDGET_CHECKS, BUDGET_CHECKS);
        let _held = state.in_flight.token();
        let cut = state.in_flight.cut();
        let mut client_closed = false;
        let mut provider_closed = None;
        while matches!(end, End::Done) {
            tokio::select! {
                from_app = client.recv(), if !client_closed => {
                    last = tokio::time::Instant::now();
                    match from_app {
                        Some(Ok(ws::Message::Text(text))) => {
                            match rt::read_client_message(api, text.as_str()) {
                                Ok(read) => {
                                    measured.add(&read, format.as_deref());
                                    if self.upstream.send(Message::text(read.forwarded)).await.is_err() {
                                        break;
                                    }
                                }
                                Err(e) => end = End::Failed(e),
                            }
                        }
                        Some(Ok(ws::Message::Binary(bytes))) => {
                            if self.upstream.send(Message::binary(bytes)).await.is_err() {
                                break;
                            }
                        }
                        Some(Ok(ws::Message::Close(frame))) => {
                            client_closed = true;
                            let frame = frame.map(|f| CloseFrame {
                                code: CloseCode::from(f.code),
                                reason: f.reason.as_str().to_owned().into(),
                            });
                            let _ = self.upstream.send(Message::Close(frame)).await;
                        }
                        Some(Ok(_)) => {}
                        Some(Err(_)) | None => {
                            let _ = self.upstream.close(None).await;
                            break;
                        }
                    }
                }
                from_provider = self.upstream.next() => {
                    last = tokio::time::Instant::now();
                    match from_provider {
                        Some(Ok(Message::Text(text))) => {
                            if client.send(ws::Message::Text(text.as_str().into())).await.is_err() {
                                let _ = self.upstream.close(None).await;
                                break;
                            }
                        }
                        Some(Ok(Message::Binary(bytes))) => {
                            if client.send(ws::Message::Binary(bytes)).await.is_err() {
                                let _ = self.upstream.close(None).await;
                                break;
                            }
                        }
                        Some(Ok(Message::Close(frame))) => {
                            if let Some(f) = frame.as_ref().filter(|f| {
                                !matches!(f.code, CloseCode::Normal | CloseCode::Away)
                            }) {
                                provider_closed = Some(format!(
                                    "ElevenLabs closed the session ({}): {}",
                                    u16::from(f.code),
                                    f.reason.as_str()
                                ));
                            }
                            let frame = frame.map(|f| ws::CloseFrame {
                                code: u16::from(f.code),
                                reason: f.reason.as_str().into(),
                            });
                            let _ = client.send(ws::Message::Close(frame)).await;
                            break;
                        }
                        Some(Ok(_)) => {}
                        Some(Err(e)) => {
                            tracing::warn!(error = %e, "an ElevenLabs realtime session broke");
                            end = End::Failed(KppError::provider(
                                "The connection to ElevenLabs broke; open a new session.",
                            ));
                        }
                        None => {
                            end = End::Failed(KppError::provider(
                                "ElevenLabs ended the session without closing it.",
                            ));
                        }
                    }
                }
                () = tokio::time::sleep_until(last + idle) => {
                    end = End::Failed(error(
                        ErrorKind::Timeout,
                        408,
                        "The session was idle too long; open a new one when you need it.",
                    ));
                }
                () = cut.cancelled() => {
                    end = End::Failed(super::pipeline::shutting_down());
                }
                () = tokio::time::sleep_until(deadline) => {
                    end = End::Failed(error(
                        ErrorKind::InvalidRequest,
                        400,
                        "The session reached Kaeso's limit of one hour; open a new one.",
                    ));
                }
                _ = checks.tick() => {
                    let spent = kaeso_core::cost::nano_usd(kaeso_core::cost::media_cost(
                        &measured.units(format.clone()),
                        &self.eff.fields,
                    ));
                    if let Err(refusal) = crate::services::limits::session_within_budgets(
                        &state,
                        &self.caller,
                        self.catalogue_model_id,
                        state.clock.now(),
                        spent,
                    )
                    .await
                    {
                        end = End::Failed(super::pipeline::limited(refusal));
                    }
                }
            }
        }
        if let End::Failed(e) = &end {
            close_with(&mut client, api, e).await;
            let _ = self.upstream.close(None).await;
        }
        state
            .limits
            .release(
                &state,
                &self.reservation,
                &self.opening.request_id.to_string(),
            )
            .await;
        if let (End::Done, Some(said)) = (&end, provider_closed) {
            end = End::Failed(KppError::provider(said));
        }
        self.record(&state, &measured, format, started.elapsed(), &end)
            .await;
    }

    async fn record(
        &self,
        state: &AppState,
        measured: &Measured,
        format: Option<String>,
        duration: Duration,
        end: &End,
    ) {
        let o = &self.opening;
        let mut out = super::pipeline::blank_outcome(o.request_id);
        out.caller = Some(self.caller.clone());
        out.ingress = D;
        out.kind = if o.api.speaks() {
            RequestKind::Speech
        } else {
            RequestKind::Transcription
        };
        out.egress = Some(D);
        out.provider = Some(self.eff.provider.metric_label());
        out.requested_model = Some(o.model.clone());
        out.catalogue_model_id = Some(self.catalogue_model_id);
        out.model_id = Some(self.eff.model_id.clone());
        out.response_model = Some(self.eff.model_id.clone());
        out.prices = Some(self.eff.fields.clone());
        out.stream = true;
        out.status = 101;
        if let End::Failed(e) = end {
            out.error_kind = Some(e.kind.as_str().to_owned());
            out.error_message = Some(e.message.clone());
        }
        out.started_at = o.started_at;
        out.duration = duration;
        out.user_agent.clone_from(&o.user_agent);
        out.client_ip = o.ip.0;
        out.realtime = Some(o.api);
        out.media = Some(measured.units(format));
        state.sink.finish(state, out).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_reasons_fit_a_frame() {
        let long = "é".repeat(100);
        assert!(short_reason(&long).len() <= 123);
        assert_eq!(short_reason("Idle."), "Idle.");
    }

    #[test]
    fn audio_is_measured_by_its_format_and_rate() {
        let mut m = Measured::default();
        let chunk = rt::read_client_message(
            RealtimeApi::SpeechToText,
            r#"{"message_type":"input_audio_chunk","audio_base_64":"AAAAAAAAAAAA","sample_rate":4}"#,
        )
        .unwrap();
        m.add(&chunk, Some("pcm_16000"));
        assert_eq!(m.audio_ms, 1125, "9 bytes at 4 Hz, 2 bytes a sample");
        let mut u = Measured::default();
        u.add(&chunk, Some("ulaw_8000"));
        assert_eq!(u.audio_ms, 1);
        let mut none = Measured::default();
        none.add(&chunk, None);
        assert_eq!(none.audio_ms, 0);
    }
}
