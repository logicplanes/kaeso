//! Webhooks that receive an organization's operational notices: the events, their outbox, and
//! deliveries signed as Standard Webhooks specifies (https://www.standardwebhooks.com/).
//!
//! An event is queued for every webhook that takes it, in PostgreSQL, before anyone's email
//! choices apply; the `webhooks` job sends what is due, at least once, and tries a failed
//! delivery again with growing waits. A receiver that fails 20 times in a row is turned off.

use std::sync::OnceLock;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use futures::StreamExt;
use kaeso_core::id::{OrgId, ProjectId, WebhookId, WebhookMessageId};
use kaeso_core::notifications::Kind;
use kaeso_core::rights::OrgRight;
use kaeso_mail::Locale;
use kaeso_mail::notices::{self, Fact, Membership, ModelChange, Price, ProviderKey, Security};
use kaeso_mail::{Actor, Budget, BudgetScope, LimitKind, LimitPeriod};
use kaeso_store::pg::webhooks as store;
use kaeso_upstream::guard::{self, GuardedResolver, Lookup, Reach};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use url::Url;
use uuid::Uuid;

use crate::jobs::JobError;
use crate::platform::error::{ApiError, ApiResult};
use crate::services::notifications::dispatch::{Send, deliver, org_holders, settings_path};
use crate::state::AppState;

/// The notices a webhook can take: those about an organization, never one person's.
pub const KINDS: [Kind; 12] = [
    Kind::BudgetAlerts,
    Kind::LimitReached,
    Kind::KeyExpiry,
    Kind::ModelDeprecated,
    Kind::ModelShutdownSoon,
    Kind::ModelLines,
    Kind::ContentLogging,
    Kind::McpServers,
    Kind::OrgSecurity,
    Kind::OrgMembers,
    Kind::ProviderKeys,
    Kind::ModelChanges,
];

/// A webhook's first try is at once; each later one waits longer, five tries in all.
const WAITS: [Duration; 4] = [
    Duration::from_secs(60),
    Duration::from_secs(5 * 60),
    Duration::from_secs(15 * 60),
    Duration::from_secs(60 * 60),
];
const TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// The answer's body is read this far, so the connection can be reused, and ignored.
const ANSWER_LIMIT: usize = 64 * 1024;
/// Failed tries in a row after which a webhook is turned off.
pub const DISABLE_AFTER: i32 = 20;
/// Deliveries are kept this long, for the list of each webhook's last ones.
pub const KEEP: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const BATCH: i64 = 100;
const AT_ONCE: usize = 8;
/// A signing secret: `whsec_` and 32 random bytes in base64, as Standard Webhooks writes them.
const SECRET_PREFIX: &str = "whsec_";
const ERROR_CHARS: usize = 300;

pub fn takes(kind: Kind) -> bool {
    KINDS.contains(&kind)
}

/// The HTTP clients deliveries go through: `https` to public addresses and the allowed
/// networks, `http` only to the allowed networks, never through a proxy, never redirected,
/// every resolved address checked.
#[derive(Debug)]
pub struct Http {
    reach: Reach,
    lookup: Lookup,
    clients: OnceLock<Option<(reqwest::Client, reqwest::Client)>>,
}

impl Http {
    pub fn new(allowed: Vec<ipnet::IpNet>) -> Http {
        Http {
            reach: Reach::networks(allowed),
            lookup: Lookup::default(),
            clients: OnceLock::new(),
        }
    }

    fn reach_for(&self, url: &Url) -> Reach {
        if url.scheme() == "https" {
            self.reach.clone()
        } else {
            self.reach.without_public()
        }
    }

    fn build(&self, reach: Reach) -> Option<reqwest::Client> {
        reqwest::Client::builder()
            .no_proxy()
            .dns_resolver(std::sync::Arc::new(GuardedResolver::new(
                reach,
                self.lookup.clone(),
            )))
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("kaeso-gateway/", env!("CARGO_PKG_VERSION")))
            .build()
            .inspect_err(|e| tracing::error!(error = %e, "cannot build the webhook client"))
            .ok()
    }

    fn client(&self, url: &Url) -> Result<&reqwest::Client, String> {
        guard::check_literal(url, &self.reach_for(url)).map_err(|_| REFUSED.to_owned())?;
        let clients = self
            .clients
            .get_or_init(|| {
                Some((
                    self.build(self.reach.clone())?,
                    self.build(self.reach.without_public())?,
                ))
            })
            .as_ref()
            .ok_or_else(|| "Kaeso can't make HTTP requests right now.".to_owned())?;
        Ok(if url.scheme() == "https" {
            &clients.0
        } else {
            &clients.1
        })
    }
}

const REFUSED: &str = "The address isn't allowed: it resolves to a private, loopback, link-local or metadata address.";

/// Whether deliveries may reach `url`: `None` when they may, otherwise why not.
pub async fn refusal(state: &AppState, url: &Url) -> Option<&'static str> {
    let http = &state.webhooks;
    let (Some(host), Some(port)) = (url.host_str(), url.port_or_known_default()) else {
        return Some(
            "`url` must be a full URL with a host, such as `https://hooks.example.com/kaeso`.",
        );
    };
    match guard::resolve(host, port, &http.reach_for(url), &http.lookup).await {
        Ok(_) | Err(guard::ResolveError::Lookup { .. }) => None,
        Err(guard::ResolveError::Refused(_)) => {
            let mut https = url.clone();
            let public = url.scheme() == "http"
                && https.set_scheme("https").is_ok()
                && guard::resolve(host, port, &http.reach_for(&https), &http.lookup)
                    .await
                    .is_ok();
            Some(if public {
                "`url` uses `http` for a public address; use `https`."
            } else {
                "`url` points to an address this install does not allow."
            })
        }
    }
}

/// A new signing secret, shown once.
pub fn new_secret() -> SecretString {
    SecretString::from(format!(
        "{SECRET_PREFIX}{}",
        STANDARD.encode(kaeso_crypto::random_bytes::<32>())
    ))
}

fn aad(webhook: Uuid) -> Vec<u8> {
    format!("kaeso:webhook:{webhook}").into_bytes()
}

pub fn seal(
    state: &AppState,
    webhook: Uuid,
    secret: &SecretString,
) -> Result<kaeso_crypto::Sealed, ApiError> {
    kaeso_crypto::seal(
        &state.keyring,
        secret.expose_secret().as_bytes(),
        &aad(webhook),
    )
    .map_err(|e| ApiError::internal(format!("cannot seal a webhook secret: {e}")))
}

fn open_secret(state: &AppState, webhook: Uuid, s: &store::Due) -> Option<Vec<u8>> {
    let sealed = kaeso_crypto::Sealed {
        ciphertext: s.ciphertext.clone(),
        nonce: s.nonce.clone(),
        wrapped_key: s.wrapped_key.clone(),
        key_id: s.key_id.clone(),
    };
    let plain = kaeso_crypto::open(&state.keyring, &sealed, &aad(webhook))
        .inspect_err(|e| tracing::error!(error = %e, webhook = %WebhookId::from_uuid(webhook), "cannot open a webhook's secret"))
        .ok()?;
    let text = String::from_utf8(plain.expose_secret().to_vec()).ok()?;
    STANDARD
        .decode(text.strip_prefix(SECRET_PREFIX).unwrap_or(&text))
        .ok()
}

/// Standard Webhooks' signature: `v1,` and the base64 HMAC-SHA256 of `id.timestamp.body`.
pub fn signature(secret: &[u8], id: &str, timestamp: i64, body: &str) -> String {
    let signed = format!("{id}.{timestamp}.{body}");
    format!(
        "v1,{}",
        STANDARD.encode(kaeso_crypto::hmac_sha256(secret, signed.as_bytes()))
    )
}

/// An event for the webhooks of an organization.
#[derive(Clone, Debug)]
pub struct Event {
    pub kind: Kind,
    pub org: Uuid,
    pub project: Option<Uuid>,
    pub event_type: &'static str,
    pub data: Value,
    /// The notice's one line, in English.
    pub summary: String,
    /// The console page it is about, relative to the install's address.
    pub open: String,
}

/// The event of a notice, with `ids` added to its data; `None` for a kind no webhook takes.
pub fn of_notice(send: &Send<'_>, fact: &Fact<'_>, ids: Option<Value>) -> Option<Event> {
    let org = send.org?;
    if !takes(send.kind) {
        return None;
    }
    let (event_type, mut data) = typed(fact)?;
    if let (Some(Value::Object(ids)), Value::Object(data)) = (ids, &mut data) {
        for (k, v) in ids {
            data.insert(k, v);
        }
    }
    Some(Event {
        kind: send.kind,
        org,
        project: send.project,
        event_type,
        data,
        summary: notices::line(Locale::En, fact),
        open: send.open.clone(),
    })
}

fn actor(a: &Actor<'_>) -> Value {
    match a {
        Actor::Person(name) | Actor::OrgAdmin(name) => json!(name),
        _ => Value::Null,
    }
}

fn at(t: time::PrimitiveDateTime) -> Value {
    t.assume_utc()
        .format(&Rfc3339)
        .map_or(Value::Null, Value::from)
}

/// `data` with the fields of `more` added.
pub fn with(mut data: Value, more: Value) -> Value {
    if let (Value::Object(fields), Value::Object(more)) = (&mut data, more) {
        fields.extend(more);
    }
    data
}

// As the Platform API names limits.
pub fn budget_data(budget: &Budget) -> Value {
    let kind = match budget.kind {
        LimitKind::RequestRate => "request_rate",
        LimitKind::TokenRate => "token_rate",
        LimitKind::TokenBudget => "token_budget",
        LimitKind::SpendBudget => "spend_budget",
        LimitKind::ToolCallRate => "tool_call_rate",
    };
    let period = match budget.period {
        LimitPeriod::Minute => "minute",
        LimitPeriod::Day => "day",
        LimitPeriod::Month => "month",
        LimitPeriod::Total => "total",
    };
    json!({ "kind": kind, "period": period, "amount": budget.amount })
}

fn price(p: Price) -> &'static str {
    match p {
        Price::Input => "input",
        Price::Output => "output",
        Price::CacheRead => "cache_read",
        Price::CacheWrite => "cache_write",
        Price::CacheWriteHour => "cache_write_1h",
    }
}

pub fn applies_to(scope: BudgetScope<'_>) -> Value {
    match scope {
        BudgetScope::Project => json!({ "type": "project" }),
        BudgetScope::Member(name) => json!({ "type": "member", "name": name }),
        BudgetScope::Key(name) => json!({ "type": "key", "name": name }),
    }
}

// The type and data of each notice a webhook can take; documented in console/webhooks.md.
fn typed(fact: &Fact<'_>) -> Option<(&'static str, Value)> {
    Some(match fact {
        Fact::LimitReached {
            budget,
            scope,
            resets,
            ..
        } => {
            let data = with(
                budget_data(budget),
                json!({ "applies_to": applies_to(*scope), "resets_at": resets.map_or(Value::Null, at) }),
            );
            ("limit.reached", data)
        }
        Fact::ProviderKey { key, change } => {
            let base = json!({
                "provider": key.provider,
                "name": key.name,
                "default": key.default,
                "projects": key.projects,
            });
            match change {
                ProviderKey::Refused {
                    answer,
                    first,
                    last,
                } => {
                    let refusal = json!({
                        "status": answer.status,
                        "message": answer.message,
                        "billing": answer.billing,
                        "first_at": at(*first),
                        "last_at": at(*last),
                    });
                    (
                        "provider_key.refused",
                        with(base, json!({ "refusal": refusal })),
                    )
                }
                ProviderKey::SignedOut { at: when } => (
                    "provider_key.signed_out",
                    with(base, json!({ "signed_out_at": at(*when) })),
                ),
                ProviderKey::SpendLimitReached { at: when, resets } => (
                    "provider_key.spend_limit_reached",
                    with(
                        base,
                        json!({ "refused_at": at(*when), "resets_at": at(*resets) }),
                    ),
                ),
                ProviderKey::Unlinked { at: when } => (
                    "provider_key.unlinked",
                    with(base, json!({ "unlinked_at": at(*when) })),
                ),
            }
        }
        Fact::ServerSignedOut { server } => ("mcp_server.signed_out", json!({ "server": server })),
        Fact::ToolHeld { server, tool } => (
            "mcp_server.tool_held",
            json!({ "server": server, "tool": tool }),
        ),
        Fact::Model {
            provider,
            model,
            change,
            ..
        } => match change {
            ModelChange::Delisted => (
                "model.delisted",
                json!({ "provider": provider, "model": model }),
            ),
            ModelChange::Deprecated {
                shutdown_on,
                replacement,
            } => (
                "model.deprecated",
                json!({
                    "provider": provider,
                    "model": model,
                    "shutdown_on": shutdown_on.map(kaeso_core::kpmr::day::format),
                    "replacement": replacement,
                }),
            ),
            ModelChange::ShutdownSoon {
                shutdown_on,
                days,
                replacement,
                requests,
            } => (
                "model.shutdown_soon",
                json!({
                    "provider": provider,
                    "model": model,
                    "shutdown_on": kaeso_core::kpmr::day::format(*shutdown_on),
                    "days_left": days,
                    "replacement": replacement,
                    "requests_7d": requests,
                }),
            ),
            ModelChange::Moved {
                replacement,
                aliases,
            } => (
                "model.moved",
                json!({
                    "provider": provider,
                    "model": model,
                    "replacement": replacement,
                    "aliases": aliases,
                }),
            ),
            ModelChange::Price(prices) => (
                "model.price_changed",
                json!({
                    "provider": provider,
                    "model": model,
                    "prices": prices.iter().map(|p| json!({
                        "price": price(p.price),
                        "from": p.from,
                        "to": p.to,
                    })).collect::<Vec<_>>(),
                }),
            ),
        },
        Fact::OrgSecurity {
            change: Security::SharedLinkCreated { name, public },
            by,
        } => (
            "share.created",
            json!({
                "title": name,
                "audience": if *public { "anyone" } else { "members" },
                "by": actor(by),
            }),
        ),
        Fact::Line {
            provider,
            line,
            change,
            ..
        } => {
            let prices = |prices: &[kaeso_mail::notices::LinePrice]| {
                prices
                    .iter()
                    .map(|p| json!({ "price": p.price, "from": p.from, "to": p.to }))
                    .collect::<Vec<_>>()
            };
            match change {
                kaeso_mail::notices::LineChange::Moved {
                    from,
                    to,
                    prices: changed,
                    how,
                } => (
                    "line.moved",
                    json!({ "provider": provider, "line": line, "from": from, "to": to, "prices": prices(changed), "policy": how }),
                ),
                kaeso_mail::notices::LineChange::Available {
                    from,
                    to,
                    prices: changed,
                } => (
                    "line.move_available",
                    json!({ "provider": provider, "line": line, "from": from, "to": to, "prices": prices(changed) }),
                ),
            }
        }
        Fact::OrgSecurity { change, by } => {
            let (what, key) = match change {
                Security::SignInRules => ("sign_in_rules", None),
                Security::SingleSignOn => ("single_sign_on", None),
                Security::AgentPolicy => ("agent_policy", None),
                Security::KeyPolicy => ("key_policy", None),
                Security::PlatformKeyCreated(name) => ("platform_key_created", Some(*name)),
                Security::PlatformKeyRotated(name) => ("platform_key_rotated", Some(*name)),
                Security::SharedLinkCreated { .. } => ("shared_link_created", None),
            };
            (
                "organization.security_changed",
                json!({ "change": what, "platform_key": key, "by": actor(by) }),
            )
        }
        Fact::Member { person, change } => match change {
            Membership::Joined => (
                "organization.member_joined",
                json!({ "person": person, "sso": false }),
            ),
            Membership::JoinedBySso => (
                "organization.member_joined",
                json!({ "person": person, "sso": true }),
            ),
            Membership::Left => (
                "organization.member_left",
                json!({ "person": person, "removed_by": null }),
            ),
            Membership::Removed(by) => (
                "organization.member_left",
                json!({ "person": person, "removed_by": actor(by) }),
            ),
        },
        Fact::ContentLogging { on, by } => (
            "project.content_logging_changed",
            json!({ "on": on, "by": actor(by) }),
        ),
        _ => return None,
    })
}

/// An email's subject without its `[place]` prefix, as a summary.
pub fn without_place(subject: &str) -> String {
    match subject.strip_prefix('[').and_then(|s| s.split_once("] ")) {
        Some((_, rest)) => rest.to_owned(),
        None => subject.to_owned(),
    }
}

/// Queues `event` for every webhook of its organization that takes it; how many.
pub async fn queue(state: &AppState, event: &Event) -> ApiResult<usize> {
    let hooks = store::subscribed(&state.pg, event.org, event.project, event.kind.as_str()).await?;
    if hooks.is_empty() {
        return Ok(0);
    }
    let now = state.clock.now();
    let organization = crate::services::notifications::triggers::org_name(state, event.org).await?;
    let project = match event.project {
        Some(p) => Some((
            p,
            crate::services::notifications::triggers::project_name(state, event.org, p).await?,
        )),
        None => None,
    };
    let message = Uuid::now_v7();
    let place = match &project {
        Some((_, name)) => format!("{organization} › {name}"),
        None => organization.clone(),
    };
    let url = format!("{}{}", state.config.public_url, event.open);
    let json_body = json!({
        "id": WebhookMessageId::from_uuid(message).to_string(),
        "type": event.event_type,
        "created_at": now.format(&Rfc3339).unwrap_or_default(),
        "organization": { "id": OrgId::from_uuid(event.org).to_string(), "name": organization },
        "project": project.as_ref().map(|(id, name)| json!({ "id": ProjectId::from_uuid(*id).to_string(), "name": name })),
        "summary": event.summary,
        "data": event.data,
        "url": url,
    })
    .to_string();
    let slack_body = json!({ "text": format!("[{place}] {}\n{url}", event.summary) }).to_string();
    let mut tx = state.pg.begin().await?;
    for (webhook, format) in &hooks {
        store::enqueue(
            &mut *tx,
            &store::NewDelivery {
                id: Uuid::now_v7(),
                webhook_id: *webhook,
                message_id: message,
                event_type: event.event_type,
                body: if format == "slack" {
                    &slack_body
                } else {
                    &json_body
                },
                at: now,
            },
        )
        .await?;
    }
    tx.commit().await?;
    Ok(hooks.len())
}

/// Queues `event`, logging a failure: a notice never fails because of a webhook.
pub async fn queue_or_warn(state: &AppState, event: &Event) {
    if let Err(e) = queue(state, event).await {
        tracing::warn!(error = %e, event = event.event_type, "a webhook event couldn't be queued");
    }
}

/// The test event of one webhook, queued for it alone.
pub async fn queue_test(state: &AppState, row: &store::WebhookRow) -> ApiResult<Uuid> {
    let now = state.clock.now();
    let organization =
        crate::services::notifications::triggers::org_name(state, row.org_id).await?;
    let message = Uuid::now_v7();
    let url = format!(
        "{}{}/webhooks",
        state.config.public_url,
        settings_path(row.org_id)
    );
    let summary = format!("A test event for the webhook {}", row.name);
    let body = if row.format == "slack" {
        json!({ "text": format!("[{organization}] {summary}\n{url}") }).to_string()
    } else {
        json!({
            "id": WebhookMessageId::from_uuid(message).to_string(),
            "type": "webhook.test",
            "created_at": now.format(&Rfc3339).unwrap_or_default(),
            "organization": { "id": OrgId::from_uuid(row.org_id).to_string(), "name": organization },
            "project": null,
            "summary": summary,
            "data": { "webhook_id": WebhookId::from_uuid(row.id).to_string(), "name": row.name },
            "url": url,
        })
        .to_string()
    };
    let id = Uuid::now_v7();
    store::enqueue(
        &state.pg,
        &store::NewDelivery {
            id,
            webhook_id: row.id,
            message_id: message,
            event_type: "webhook.test",
            body: &body,
            at: now,
        },
    )
    .await?;
    Ok(id)
}

fn count(outcome: &'static str) {
    metrics::counter!(crate::metrics::WEBHOOK_DELIVERIES, "outcome" => outcome).increment(1);
}

fn clip(text: &str) -> String {
    let mut out: String = text.chars().take(ERROR_CHARS).collect();
    if text.chars().count() > ERROR_CHARS {
        out.push('…');
    }
    out
}

/// One try: the receiver's status when it answered 2xx, otherwise its status and what failed.
async fn send(
    state: &AppState,
    d: &store::Due,
    secret: &[u8],
) -> Result<u16, (Option<u16>, String)> {
    let url = Url::parse(&d.url).map_err(|_| (None, "The URL can't be read.".to_owned()))?;
    let client = state.webhooks.client(&url).map_err(|e| (None, e))?;
    let id = WebhookMessageId::from_uuid(d.message_id).to_string();
    let timestamp = state.clock.now().unix_timestamp();
    let answer = client
        .post(url)
        .header("content-type", "application/json")
        .header("webhook-id", &id)
        .header("webhook-timestamp", timestamp.to_string())
        .header(
            "webhook-signature",
            signature(secret, &id, timestamp, &d.body),
        )
        .body(d.body.clone())
        .send()
        .await
        .map_err(|e| {
            let what = if guard::refused_in(&e) {
                REFUSED.to_owned()
            } else if e.is_timeout() {
                "The receiver didn't answer within 10 seconds.".to_owned()
            } else if e.is_connect() {
                "Kaeso couldn't connect to the receiver.".to_owned()
            } else {
                "The connection to the receiver failed.".to_owned()
            };
            (None, what)
        })?;
    let status = answer.status();
    let mut stream = answer.bytes_stream();
    let mut read = 0;
    while read < ANSWER_LIMIT {
        match stream.next().await {
            Some(Ok(chunk)) => read += chunk.len(),
            _ => break,
        }
    }
    if status.is_success() {
        Ok(status.as_u16())
    } else {
        Err((
            Some(status.as_u16()),
            format!("The receiver answered {status}."),
        ))
    }
}

/// Sends one due delivery and records how it went.
async fn deliver_one(state: &AppState, d: store::Due) -> Result<&'static str, JobError> {
    let now = state.clock.now();
    if !d.enabled {
        store::drop_pending(&state.pg, d.id, "The webhook is off.", now).await?;
        count("dropped");
        return Ok("dropped");
    }
    let tried = match open_secret(state, d.webhook_id, &d) {
        Some(secret) => send(state, &d, &secret).await,
        None => Err((
            None,
            "Kaeso can't read the webhook's secret; rotate it.".to_owned(),
        )),
    };
    let after = d.attempts + 1;
    let test = d.event_type == "webhook.test";
    let (status, error, next) = match &tried {
        Ok(status) => (Some(i32::from(*status)), None, None),
        Err((status, error)) => (
            status.map(i32::from),
            Some(clip(error)),
            usize::try_from(after - 1)
                .ok()
                .and_then(|i| WAITS.get(i))
                .filter(|_| !test)
                .map(|wait| now + *wait),
        ),
    };
    let mut conn = state
        .pg
        .acquire()
        .await
        .map_err(kaeso_store::StoreError::Postgres)?;
    let failures = store::record_attempt(
        &mut conn,
        d.id,
        d.webhook_id,
        &store::Attempt {
            status,
            error: error.as_deref(),
            delivered: tried.is_ok(),
            next,
            at: now,
        },
    )
    .await?;
    drop(conn);
    let outcome = match (&tried, next) {
        (Ok(_), _) => "delivered",
        (Err(_), Some(_)) => "retried",
        (Err(_), None) => "failed",
    };
    count(outcome);
    if tried.is_err() && failures >= DISABLE_AFTER {
        turn_off(state, &d, now).await?;
    }
    Ok(outcome)
}

async fn turn_off(state: &AppState, d: &store::Due, now: OffsetDateTime) -> Result<(), JobError> {
    if !store::disable_for_failures(&state.pg, d.webhook_id, now).await? {
        return Ok(());
    }
    tracing::warn!(webhook = %WebhookId::from_uuid(d.webhook_id), "a webhook failed 20 times in a row and was turned off");
    let mut conn = state
        .pg
        .acquire()
        .await
        .map_err(kaeso_store::StoreError::Postgres)?;
    crate::platform::audit::system_event(
        &mut conn,
        Some(OrgId::from_uuid(d.org_id)),
        crate::platform::audit::Action::WebhookUpdate,
        Some(("webhook", WebhookId::from_uuid(d.webhook_id).to_string())),
        Some(json!({ "enabled": true })),
        Some(json!({ "enabled": false, "disabled_reason": "failures" })),
        now,
    )
    .await?;
    drop(conn);
    let notified = async {
        let place = crate::services::notifications::triggers::org_name(state, d.org_id).await?;
        let send = Send {
            kind: Kind::Webhooks,
            org: Some(d.org_id),
            project: None,
            place: Some(&place),
            subject: format!(
                "disabled:{}:{}",
                WebhookId::from_uuid(d.webhook_id),
                now.unix_timestamp()
            ),
            open: format!("{}/webhooks", settings_path(d.org_id)),
            actor: None,
        };
        let to = org_holders(state, d.org_id, &[OrgRight::WebhooksManage]).await?;
        deliver(
            state,
            &send,
            &Fact::WebhookDisabled { webhook: &d.name },
            to,
        )
        .await
    }
    .await;
    if let Err(e) = notified {
        tracing::warn!(error = %e, "the people who manage a webhook couldn't be told it was turned off");
    }
    Ok(())
}

/// The `webhooks` job: sends what is due, a few at a time.
pub async fn run(state: AppState) -> Result<String, JobError> {
    let due = store::due(&state.pg, state.clock.now(), BATCH).await?;
    if due.is_empty() {
        return Ok("no webhook deliveries due".to_owned());
    }
    let outcomes: Vec<Result<&'static str, JobError>> = futures::stream::iter(due)
        .map(|d| deliver_one(&state, d))
        .buffer_unordered(AT_ONCE)
        .collect()
        .await;
    let mut delivered = 0;
    let mut failed = 0;
    for o in outcomes {
        match o? {
            "delivered" => delivered += 1,
            _ => failed += 1,
        }
    }
    Ok(format!("{delivered} webhook deliveries sent, {failed} not"))
}

/// Sends one webhook's due test delivery at once, so the console can say how it went.
pub async fn send_now(state: &AppState, delivery: Uuid) -> Result<(), JobError> {
    let due = store::due(&state.pg, state.clock.now(), BATCH).await?;
    if let Some(d) = due.into_iter().find(|d| d.id == delivery) {
        deliver_one(state, d).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deliveries_are_signed_as_standard_webhooks_specifies() {
        // The example of the Standard Webhooks specification.
        let secret = STANDARD.decode("MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw").unwrap();
        let sig = signature(
            &secret,
            "msg_p5jXN8AQM9LWM0D4loKWxJek",
            1_614_265_330,
            r#"{"test": 2432232314}"#,
        );
        assert_eq!(sig, "v1,g0hM9SsE+OTPJTGt/tmIKtSyZlE3uFJELVlNIOLJ1OE=");
    }

    #[test]
    fn secrets_are_32_random_bytes_after_whsec() {
        let a = new_secret();
        let b = new_secret();
        assert_ne!(a.expose_secret(), b.expose_secret());
        let raw = a.expose_secret().strip_prefix(SECRET_PREFIX).unwrap();
        assert_eq!(STANDARD.decode(raw).unwrap().len(), 32);
    }

    #[test]
    fn five_tries_wait_longer_each_time_and_span_over_an_hour() {
        assert!(WAITS.windows(2).all(|w| w[0] < w[1]));
        assert!(WAITS.iter().sum::<Duration>() > Duration::from_secs(60 * 60));
        assert_eq!(WAITS.len() + 1, 5);
    }

    #[test]
    fn only_notices_about_an_organization_go_to_webhooks() {
        for kind in Kind::ALL {
            let personal = matches!(
                kind,
                Kind::AccountSecurity
                    | Kind::NewApp
                    | Kind::ExportReady
                    | Kind::AccessChanged
                    | Kind::ConnectionExpired
            );
            if personal {
                assert!(!takes(*kind), "{kind}");
            }
        }
        assert!(takes(Kind::BudgetAlerts) && takes(Kind::ProviderKeys));
        assert!(
            !takes(Kind::Webhooks),
            "a webhook isn't told about webhooks"
        );
    }
}
