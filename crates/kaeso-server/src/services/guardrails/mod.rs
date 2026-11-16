pub mod contract;
pub mod rules;

use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use bytes::Bytes;
use kaeso_crypto::{CryptoError, ExposeSecret, SecretBox, SecretString};
use kaeso_kpp::{ErrorKind, GenerateRequest, KppError, Response};
use kaeso_store::StoreError;
use kaeso_store::pg::guardrails::{self as store, GuardrailRow};
use kaeso_upstream::UpstreamError;
use kaeso_upstream::mcp::Limits;
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use self::contract::{Asked, Said, Verdict};
use crate::model_api::errors::error;
use crate::state::AppState;

/// The guardrails a request passed, in the order they ran: `kaeso-guardrails`.
pub const HEADER: &str = "kaeso-guardrails";

/// The largest answer Kaeso reads from a guardrail.
pub const MAX_ANSWER: usize = 16 << 20;

pub const SECRET_PREFIX: &str = "whsec_";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = GuardrailPhase, description = "When a guardrail checks: before a request goes to a model, or before a\n\
    reply goes back.")]
pub enum Phase {
    #[doc = "The request, before it goes to a model."]
    Request,
    #[doc = "The model's reply, before it goes back to the app."]
    Reply,
}

impl Phase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Phase::Request => "request",
            Phase::Reply => "reply",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = GuardrailAction, description = "What a guardrail did with a request or its reply.")]
pub enum Action {
    #[doc = "Let it through unchanged."]
    Allow,
    #[doc = "Refused it; the app got an error."]
    Block,
    #[doc = "Let it through with the text the guardrail sent back."]
    Replace,
}

impl Action {
    pub const ALL: [Action; 3] = [Action::Allow, Action::Block, Action::Replace];

    pub const fn as_str(self) -> &'static str {
        match self {
            Action::Allow => "allow",
            Action::Block => "block",
            Action::Replace => "replace",
        }
    }
}

/// Why a guardrail gave no usable answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = GuardrailFailure, description = "Why a guardrail gave no usable answer; its On error setting then\n\
    decided.")]
pub enum Failure {
    #[doc = "It didn't answer within its timeout."]
    Timeout,
    #[doc = "Kaeso couldn't connect to it."]
    Unreachable,
    #[doc = "Its address is on a network this install doesn't allow."]
    Refused,
    #[doc = "It answered with an HTTP status other than 2xx."]
    Status,
    #[doc = "Its answer wasn't a verdict, or a replacement didn't have the shape of what was\n\
        sent."]
    Invalid,
}

impl Failure {
    pub const fn as_str(self) -> &'static str {
        match self {
            Failure::Timeout => "timeout",
            Failure::Unreachable => "unreachable",
            Failure::Refused => "refused",
            Failure::Status => "status",
            Failure::Invalid => "invalid",
        }
    }
}

/// One guardrail's check of a request or reply, as the request log keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub guardrail: Uuid,
    pub name: String,
    pub phase: Phase,
    pub action: Action,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<Failure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    pub duration_ms: u64,
    /// For built-in rules: how many values each kind found, such as `api_key`.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub found: std::collections::BTreeMap<String, u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnError {
    Block,
    Allow,
}

impl OnError {
    pub fn of(s: &str) -> OnError {
        if s == "allow" {
            OnError::Allow
        } else {
            OnError::Block
        }
    }

    const fn action(self) -> Action {
        match self {
            OnError::Block => Action::Block,
            OnError::Allow => Action::Allow,
        }
    }
}

/// How a guardrail checks: an HTTP service of the organization, or built-in rules.
#[derive(Debug)]
pub enum Checker {
    Http { url: Url, key: SecretBox<Vec<u8>> },
    Rules(Arc<kaeso_core::masking::Scanner>),
}

#[derive(Debug)]
pub struct Guardrail {
    pub id: Uuid,
    pub name: String,
    pub checker: Checker,
    pub timeout: Duration,
    pub on_error: OnError,
    pub request: bool,
    pub reply: bool,
    pub hold: bool,
}

/// The guardrails a project uses, in order, ready to call.
#[derive(Debug, Default)]
pub struct Armed(pub Vec<Guardrail>);

impl Armed {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn checks_replies(&self, stream: bool) -> bool {
        self.0.iter().any(|g| g.reply && (!stream || g.hold))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    #[error("a guardrail's rules can't be read")]
    Rules,
}

fn aad(id: Uuid) -> Vec<u8> {
    format!("guardrail:{id}").into_bytes()
}

/// A new signing secret in Standard Webhooks' form: `whsec_` and 32 random bytes in base64.
pub fn new_secret() -> SecretString {
    let bytes = kaeso_crypto::random_bytes::<32>();
    SecretString::from(format!("{SECRET_PREFIX}{}", STANDARD.encode(bytes)))
}

pub fn seal(
    state: &AppState,
    id: Uuid,
    secret: &SecretString,
) -> Result<kaeso_crypto::Sealed, CryptoError> {
    kaeso_crypto::seal(&state.keyring, secret.expose_secret().as_bytes(), &aad(id))
}

fn key_of(secret: &[u8]) -> Result<SecretBox<Vec<u8>>, CryptoError> {
    let text = std::str::from_utf8(secret).map_err(|_| CryptoError::Malformed)?;
    let encoded = text
        .strip_prefix(SECRET_PREFIX)
        .ok_or(CryptoError::Malformed)?;
    let key = STANDARD
        .decode(encoded)
        .map_err(|_| CryptoError::Malformed)?;
    Ok(SecretBox::new(Box::new(key)))
}

pub fn opened(
    state: &AppState,
    row: &GuardrailRow,
    sealed: Option<store::SealedSecret>,
) -> Result<Guardrail, LoadError> {
    let checker = match (&row.rules, &row.url, sealed) {
        (Some(rules), _, _) => {
            let rules: kaeso_core::masking::Rules =
                serde_json::from_value(rules.clone()).map_err(|_| LoadError::Rules)?;
            Checker::Rules(Arc::new(rules.compile().map_err(|_| LoadError::Rules)?))
        }
        (None, Some(url), Some(sealed)) => {
            let secret = kaeso_crypto::open(
                &state.keyring,
                &kaeso_crypto::Sealed {
                    ciphertext: sealed.ciphertext,
                    nonce: sealed.nonce,
                    wrapped_key: sealed.wrapped_key,
                    key_id: sealed.key_id,
                },
                &aad(row.id),
            )?;
            Checker::Http {
                url: Url::parse(url).map_err(|_| CryptoError::Malformed)?,
                key: key_of(secret.expose_secret())?,
            }
        }
        _ => return Err(LoadError::Rules),
    };
    Ok(Guardrail {
        id: row.id,
        name: row.name.clone(),
        checker,
        timeout: Duration::from_millis(u64::try_from(row.timeout_ms).unwrap_or(2000)),
        on_error: OnError::of(&row.on_error),
        request: row.request_phase,
        reply: row.reply_phase,
        hold: row.streams == "hold",
    })
}

/// Whether a guardrail's address is one this install lets Kaeso reach.
pub async fn reachability(
    state: &AppState,
    url: &Url,
) -> crate::services::custom_providers::Reachability {
    use crate::services::custom_providers::Reachability;
    use kaeso_upstream::guard;
    let (Some(host), Some(port)) = (url.host_str(), url.port_or_known_default()) else {
        return Reachability::Refused;
    };
    let reach = state.guardrails.reach_for(url);
    match guard::resolve(host, port, &reach, state.guardrails.lookup()).await {
        Ok(_) => Reachability::Allowed,
        Err(guard::ResolveError::Refused(_)) => Reachability::Refused,
        Err(guard::ResolveError::Lookup { .. }) => Reachability::Unknown,
    }
}

/// The guardrails of a project, from the cache.
pub async fn of_project(state: &AppState, project: Uuid) -> Result<Arc<Armed>, LoadError> {
    let found = state
        .caches
        .guardrails
        .get_or_load(&project.to_string(), "all", || async {
            let rows = store::armed_for_project(&state.pg, project).await?;
            let armed = rows
                .into_iter()
                .map(|a| opened(state, &a.row, a.secret))
                .collect::<Result<Vec<_>, LoadError>>()?;
            Ok::<_, LoadError>(Some(Armed(armed)))
        })
        .await?;
    Ok(found.unwrap_or_default())
}

pub async fn forget(state: &AppState, projects: impl IntoIterator<Item = Uuid>) {
    for p in projects {
        state
            .caches
            .guardrails
            .invalidate_group(&p.to_string())
            .await;
    }
}

/// Standard Webhooks' signature of a delivery: `v1,` and the base64 HMAC-SHA256 of
/// `id.timestamp.body` under the secret's key.
pub fn signature(key: &[u8], id: &str, timestamp: i64, body: &[u8]) -> String {
    let ts = timestamp.to_string();
    let signed = [id.as_bytes(), b".", ts.as_bytes(), b".", body].concat();
    let tag = kaeso_crypto::hmac_sha256(key, &signed);
    format!("v1,{}", STANDARD.encode(tag))
}

/// Who sent a request and how, as a guardrail is told.
#[derive(Clone, Debug)]
pub struct Context {
    pub request_id: String,
    pub organization: String,
    pub project: String,
    pub key: Option<String>,
    pub member: Option<String>,
    pub service_account: Option<String>,
    pub model: String,
    pub format: &'static str,
    pub source: &'static str,
    /// The request's size, so built-in rules scan a large one off the async threads.
    pub request_bytes: usize,
}

/// What built-in rules leave for later steps of a request: the values its log masks and the
/// placeholders its reply gets back.
#[derive(Clone, Debug, Default)]
pub struct Masks {
    pub logged: Vec<(String, String)>,
    pub restorer: Option<kaeso_core::masking::Restorer>,
}

/// A guardrail's answer, or why there is none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    Verdict(Verdict),
    Failed {
        failure: Failure,
        status: Option<u16>,
    },
}

pub async fn ask(state: &AppState, g: &Guardrail, event_id: &str, body: Bytes) -> Answer {
    let Checker::Http { url, key } = &g.checker else {
        return Answer::Failed {
            failure: Failure::Invalid,
            status: None,
        };
    };
    let timestamp = state.clock.now().unix_timestamp();
    let headers = [
        ("webhook-id", event_id.to_owned()),
        ("webhook-timestamp", timestamp.to_string()),
        (
            "webhook-signature",
            signature(key.expose_secret(), event_id, timestamp, &body),
        ),
    ];
    let limits = Limits {
        first_byte: g.timeout,
        idle: g.timeout,
        max_body: MAX_ANSWER,
    };
    match state
        .guardrails
        .post_json_with(url, body, &headers, limits)
        .await
    {
        Ok((status, body)) if (200..300).contains(&status) => {
            match serde_json::from_slice::<Verdict>(&body) {
                Ok(v) => Answer::Verdict(v),
                Err(_) => Answer::Failed {
                    failure: Failure::Invalid,
                    status: Some(status),
                },
            }
        }
        Ok((status, _)) => Answer::Failed {
            failure: Failure::Status,
            status: Some(status),
        },
        Err(UpstreamError::Timeout(_)) => Answer::Failed {
            failure: Failure::Timeout,
            status: None,
        },
        Err(UpstreamError::Refused(r)) => {
            tracing::warn!(guardrail = %g.id, host = %r.host, "a guardrail's address is not allowed on this install");
            Answer::Failed {
                failure: Failure::Refused,
                status: None,
            }
        }
        Err(e) => {
            tracing::warn!(guardrail = %g.id, error = %e, "a guardrail cannot be reached");
            Answer::Failed {
                failure: Failure::Unreachable,
                status: None,
            }
        }
    }
}

fn blocked(name: &str, phase: Phase, message: Option<String>) -> KppError {
    let text = match (phase, message) {
        (Phase::Request, Some(m)) => format!("The guardrail `{name}` blocked this request: {m}"),
        (Phase::Request, None) => format!("The guardrail `{name}` blocked this request."),
        (Phase::Reply, Some(m)) => format!("The guardrail `{name}` blocked the model's reply: {m}"),
        (Phase::Reply, None) => format!("The guardrail `{name}` blocked the model's reply."),
    };
    error(ErrorKind::ContentFiltered, 400, text)
}

fn unchecked(name: &str) -> KppError {
    error(
        ErrorKind::ProviderError,
        503,
        format!(
            "The guardrail `{name}` could not check this request, so Kaeso refused it; try again shortly."
        ),
    )
}

/// Kaeso can't load a project's guardrails, so it sends nothing.
pub fn unloaded() -> KppError {
    error(
        ErrorKind::ProviderError,
        503,
        "Kaeso cannot load this project's guardrails right now. Try again shortly.",
    )
}

fn record(run: &Run) {
    metrics::counter!(crate::metrics::GUARDRAIL_RUNS, "phase" => run.phase.as_str(), "action" => run.action.as_str())
        .increment(1);
}

fn took(started: Instant) -> (Duration, u64) {
    let d = started.elapsed();
    (d, u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

enum Step {
    Pass,
    Stop(KppError),
}

// Asks one guardrail and settles its answer: a replacement is applied by `replace`, which says
// whether it fitted.
async fn check_one(
    state: &AppState,
    g: &Guardrail,
    phase: Phase,
    asked: &Asked<'_>,
    runs: &mut Vec<Run>,
    replace: impl FnOnce(&[Said]) -> bool,
) -> Step {
    let started = Instant::now();
    let body = match serde_json::to_vec(asked) {
        Ok(b) => Bytes::from(b),
        Err(e) => {
            tracing::error!(error = %e, "cannot write a guardrail's question");
            return Step::Stop(unchecked(&g.name));
        }
    };
    let answer = ask(state, g, asked.id, body).await;
    let (elapsed, duration_ms) = took(started);
    metrics::histogram!(crate::metrics::GUARDRAIL_DURATION, "phase" => phase.as_str())
        .record(elapsed.as_secs_f64());
    let mut run = Run {
        guardrail: g.id,
        name: g.name.clone(),
        phase,
        action: Action::Allow,
        category: None,
        failure: None,
        status: None,
        duration_ms,
        found: std::collections::BTreeMap::new(),
    };
    let failed = |run: &mut Run, failure: Failure, status: Option<u16>| {
        run.failure = Some(failure);
        run.status = status;
        run.action = g.on_error.action();
        match g.on_error {
            OnError::Allow => Step::Pass,
            OnError::Block => Step::Stop(unchecked(&g.name)),
        }
    };
    let step = match answer {
        Answer::Verdict(Verdict::Allow { category }) => {
            run.category = contract::category(category.as_deref());
            Step::Pass
        }
        Answer::Verdict(Verdict::Block { message, category }) => {
            run.action = Action::Block;
            run.category = contract::category(category.as_deref());
            Step::Stop(blocked(
                &g.name,
                phase,
                contract::message(message.as_deref()),
            ))
        }
        Answer::Verdict(Verdict::Replace { messages, category }) => {
            run.category = contract::category(category.as_deref());
            if replace(&messages) {
                run.action = Action::Replace;
                Step::Pass
            } else {
                failed(&mut run, Failure::Invalid, None)
            }
        }
        Answer::Failed { failure, status } => failed(&mut run, failure, status),
    };
    record(&run);
    runs.push(run);
    step
}

// Built-in rules run in Kaeso: a large request is scanned on a blocking thread.
async fn apply_rules(
    g: &Guardrail,
    scanner: &Arc<kaeso_core::masking::Scanner>,
    req: &mut GenerateRequest,
    runs: &mut Vec<Run>,
    masks: &mut Masks,
    bytes: usize,
) -> Step {
    let started = Instant::now();
    let scanner = Arc::clone(scanner);
    let taken = std::mem::take(req);
    let scanned = crate::model_api::pipeline::off_thread(bytes, move || {
        let mut taken = taken;
        let masked = rules::mask_request(&scanner, &mut taken);
        (taken, masked)
    })
    .await;
    let (back, masked) = match scanned {
        Ok(done) => done,
        Err(e) => return Step::Stop(e),
    };
    *req = back;
    let (elapsed, duration_ms) = took(started);
    metrics::histogram!(crate::metrics::GUARDRAIL_DURATION, "phase" => Phase::Request.as_str())
        .record(elapsed.as_secs_f64());
    for (kind, (detector, action)) in &masked.actions {
        let n = masked.found.get(kind).copied().unwrap_or(0);
        metrics::counter!(crate::metrics::MASKED_VALUES, "detector" => detector.clone(), "action" => action.as_str())
            .increment(u64::from(n));
    }
    masks.logged.extend(masked.logged.iter().cloned());
    if let Some(r) = &masked.restorer {
        masks
            .restorer
            .get_or_insert_with(kaeso_core::masking::Restorer::default)
            .extend(r);
    }
    let run = Run {
        guardrail: g.id,
        name: g.name.clone(),
        phase: Phase::Request,
        action: if masked.blocked.is_some() {
            Action::Block
        } else if masked.changed {
            Action::Replace
        } else {
            Action::Allow
        },
        category: masked.blocked.clone(),
        failure: None,
        status: None,
        duration_ms,
        found: masked.found.clone(),
    };
    record(&run);
    runs.push(run);
    match masked.blocked {
        Some(kind) => Step::Stop(blocked(
            &g.name,
            Phase::Request,
            contract::message(Some(&rules::what(&kind))),
        )),
        None => Step::Pass,
    }
}

fn asked<'a>(ctx: &'a Context, id: &'a str, phase: Phase, messages: Vec<Said>) -> Asked<'a> {
    Asked {
        id,
        phase,
        request_id: &ctx.request_id,
        organization: &ctx.organization,
        project: Some(&ctx.project),
        key: ctx.key.as_deref(),
        member: ctx.member.as_deref(),
        service_account: ctx.service_account.as_deref(),
        model: &ctx.model,
        format: ctx.format,
        source: ctx.source,
        messages,
        reply: None,
    }
}

/// Runs a project's request-phase guardrails in order on a request, applying replacements.
/// The first block, or a failure of a guardrail that blocks on errors, ends the check.
pub async fn check_request(
    state: &AppState,
    armed: &Armed,
    ctx: &Context,
    g: &mut GenerateRequest,
    runs: &mut Vec<Run>,
    masks: &mut Masks,
) -> Result<(), KppError> {
    let id = format!("{}.request", ctx.request_id);
    for guardrail in armed.0.iter().filter(|x| x.request) {
        let step = match &guardrail.checker {
            Checker::Rules(scanner) => {
                Box::pin(apply_rules(
                    guardrail,
                    scanner,
                    g,
                    runs,
                    masks,
                    ctx.request_bytes,
                ))
                .await
            }
            Checker::Http { .. } => {
                let question = asked(ctx, &id, Phase::Request, contract::conversation(g));
                check_one(state, guardrail, Phase::Request, &question, runs, |with| {
                    contract::replace_request(g, with).is_ok()
                })
                .await
            }
        };
        if let Step::Stop(e) = step {
            return Err(e);
        }
    }
    Ok(())
}

/// Runs a project's reply-phase guardrails in order on a reply. In a held stream a reply can't
/// be rewritten, so a replacement there counts as a guardrail without a usable answer.
pub async fn check_reply(
    state: &AppState,
    armed: &Armed,
    ctx: &Context,
    g: &GenerateRequest,
    r: &mut Response,
    stream: bool,
    runs: &mut Vec<Run>,
) -> Result<(), KppError> {
    let id = format!("{}.reply", ctx.request_id);
    for guardrail in armed.0.iter().filter(|x| x.reply && (!stream || x.hold)) {
        let mut question = asked(ctx, &id, Phase::Reply, contract::conversation(g));
        question.reply = Some(contract::reply(r));
        let step = check_one(state, guardrail, Phase::Reply, &question, runs, |with| {
            !stream && contract::replace_reply(r, with).is_ok()
        })
        .await;
        if let Step::Stop(e) = step {
            return Err(e);
        }
    }
    Ok(())
}

/// The `kaeso-guardrails` header: an RFC 9651 structured-field list of the runs, each the
/// guardrail's name with its phase, action, category, failure and time.
pub fn header_value(runs: &[Run]) -> Option<axum::http::HeaderValue> {
    if runs.is_empty() {
        return None;
    }
    let sf = |text: &str| -> String {
        let printable: String = text
            .chars()
            .map(|c| if matches!(c, ' '..='~') { c } else { '?' })
            .collect();
        format!(
            "\"{}\"",
            printable.replace('\\', "\\\\").replace('"', "\\\"")
        )
    };
    let items: Vec<String> = runs
        .iter()
        .map(|r| {
            let mut item = sf(&r.name);
            item.push_str(";phase=");
            item.push_str(r.phase.as_str());
            item.push_str(";action=");
            item.push_str(r.action.as_str());
            if let Some(c) = &r.category {
                item.push_str(";category=");
                item.push_str(&sf(c));
            }
            if let Some(f) = r.failure {
                item.push_str(";failure=");
                item.push_str(f.as_str());
            }
            item.push_str(&format!(";dur={}", r.duration_ms));
            item
        })
        .collect();
    axum::http::HeaderValue::from_str(&items.join(", ")).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deliveries_are_signed_as_standard_webhooks_signs_them() {
        // The example of the Standard Webhooks specification's reference implementations.
        let key = STANDARD.decode("MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw").unwrap();
        let sig = signature(
            &key,
            "msg_p5jXN8AQM9LWM0D4loKWxJek",
            1_614_265_330,
            br#"{"test": 2432232314}"#,
        );
        assert_eq!(sig, "v1,g0hM9SsE+OTPJTGt/tmIKtSyZlE3uFJELVlNIOLJ1OE=");
    }

    #[test]
    fn secrets_have_their_prefix_and_a_32_byte_key() {
        let s = new_secret();
        let key = key_of(s.expose_secret().as_bytes()).unwrap();
        assert_eq!(key.expose_secret().len(), 32);
        assert_ne!(s.expose_secret(), new_secret().expose_secret());
        assert!(key_of(b"plain").is_err());
        assert!(key_of(b"whsec_!!").is_err());
        assert!(key_of(&[0xff, 0xfe]).is_err());
    }

    #[test]
    fn the_header_lists_each_run_as_a_structured_field_item() {
        assert!(header_value(&[]).is_none());
        let run = |name: &str, phase, action, category: Option<&str>, failure| Run {
            guardrail: Uuid::nil(),
            name: name.to_owned(),
            phase,
            action,
            category: category.map(str::to_owned),
            failure,
            status: None,
            duration_ms: 12,
            found: std::collections::BTreeMap::new(),
        };
        let v = header_value(&[
            run(
                "PII \"check\"",
                Phase::Request,
                Action::Replace,
                Some("pii"),
                None,
            ),
            run(
                "Jailbreak",
                Phase::Request,
                Action::Allow,
                None,
                Some(Failure::Timeout),
            ),
            run("Résumé", Phase::Reply, Action::Block, None, None),
        ])
        .unwrap();
        assert_eq!(
            v.to_str().unwrap(),
            r#""PII \"check\"";phase=request;action=replace;category="pii";dur=12, "Jailbreak";phase=request;action=allow;failure=timeout;dur=12, "R?sum?";phase=reply;action=block;dur=12"#
        );
    }

    #[test]
    fn errors_name_the_guardrail() {
        let e = blocked("PII", Phase::Request, Some("No emails.".to_owned()));
        assert_eq!(e.status, Some(400));
        assert_eq!(e.kind, ErrorKind::ContentFiltered);
        assert_eq!(
            e.message,
            "The guardrail `PII` blocked this request: No emails."
        );
        assert_eq!(
            blocked("PII", Phase::Reply, None).message,
            "The guardrail `PII` blocked the model's reply."
        );
        assert_eq!(unchecked("PII").status, Some(503));
        assert_eq!(OnError::of("allow"), OnError::Allow);
        assert_eq!(OnError::of("block"), OnError::Block);
    }
}
