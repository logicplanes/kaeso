use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};

use kaeso_core::id::{ApiKeyId, CredentialId, LimitId, OrgId, ServiceAccountId};
use kaeso_core::limits::{Limit, Subject};
use kaeso_core::notifications::Kind;
use kaeso_core::rights::{OrgRight, ProjectRight};
use kaeso_mail::BudgetScope;
use kaeso_mail::notices::{Answer, Because, Credential, Export, Fact, ProviderKey, Update};
use kaeso_store::pg::mcp_servers::ServerRow;
use kaeso_store::pg::{credentials, keys, service_accounts};
use time::{OffsetDateTime, PrimitiveDateTime};
use uuid::Uuid;

use super::dispatch::{
    Recipients, Send, about, deliver, org_holders, project_holders, project_path, settings_path,
};
use super::triggers::{org_name, person, project_name, provider_name};
use crate::platform::error::ApiResult;
use crate::state::AppState;
use crate::time_zone::local;

// Spec: a provider key needs attention after three refusals in a row.
const REFUSALS: u8 = 3;

// A provider's answer is quoted in the email; past this many characters it is cut.
const ANSWER_CHARS: usize = 200;

// Every successful model request clears its key's count, so it checks a flag before the lock.
static COUNTING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug)]
struct Streak {
    count: u8,
    since: OffsetDateTime,
}

fn refusals() -> &'static Mutex<HashMap<Uuid, Streak>> {
    static COUNTS: OnceLock<Mutex<HashMap<Uuid, Streak>>> = OnceLock::new();
    COUNTS.get_or_init(Mutex::default)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal<'a> {
    pub status: u16,
    pub message: &'a str,
    pub billing: bool,
}

enum Change<'a> {
    Refused {
        refusal: Refusal<'a>,
        since: OffsetDateTime,
    },
    SignedOut,
}

fn clip(s: &str, max: usize) -> String {
    let mut out: String = s.chars().take(max).collect();
    if out.len() < s.len() {
        out.push('…');
    }
    out
}

fn warn(result: ApiResult<usize>, what: &str) {
    if let Err(e) = result {
        tracing::warn!(error = %e, what, "a notification couldn't be sent");
    }
}

pub async fn server_signed_out(state: &AppState, server: &ServerRow, person: Option<Uuid>) {
    warn(
        signed_out(state, server, person).await,
        "mcp_server_signed_out",
    );
}

async fn signed_out(
    state: &AppState,
    server: &ServerRow,
    person: Option<Uuid>,
) -> ApiResult<usize> {
    let place = org_name(state, server.org_id).await?;
    match person {
        Some(user) => {
            let send = Send {
                kind: Kind::ConnectionExpired,
                org: Some(server.org_id),
                project: None,
                place: Some(&place),
                subject: format!("signed_out:{}", server.id),
                open: "settings/account/connections".to_owned(),
                actor: None,
            };
            deliver(
                state,
                &send,
                &Fact::ConnectionExpired {
                    server: &server.name,
                },
                about(user),
            )
            .await
        }
        None => {
            let send = Send {
                kind: Kind::McpServers,
                org: Some(server.org_id),
                project: None,
                place: Some(&place),
                subject: format!("signed_out:{}", server.id),
                open: format!("orgs/{}/mcp-servers", OrgId::from_uuid(server.org_id)),
                actor: None,
            };
            let to = org_holders(state, server.org_id, &[OrgRight::McpServersManage]).await?;
            deliver(
                state,
                &send,
                &Fact::ServerSignedOut {
                    server: &server.name,
                },
                to,
            )
            .await
        }
    }
}

pub async fn tools_held(state: &AppState, server: &ServerRow, tools: &[String]) {
    if tools.is_empty() {
        return;
    }
    let result = async {
        let place = org_name(state, server.org_id).await?;
        let to = org_holders(state, server.org_id, &[OrgRight::McpServersManage]).await?;
        let mut sent = 0;
        for tool in tools {
            let send = Send {
                kind: Kind::McpServers,
                org: Some(server.org_id),
                project: None,
                place: Some(&place),
                subject: format!("held:{}:{tool}", server.id),
                open: format!("orgs/{}/mcp-servers", OrgId::from_uuid(server.org_id)),
                actor: None,
            };
            sent += deliver(
                state,
                &send,
                &Fact::ToolHeld {
                    server: &server.name,
                    tool,
                },
                to.clone(),
            )
            .await?;
        }
        Ok(sent)
    }
    .await;
    warn(result, "tool_held");
}

pub async fn account_signed_out(state: &AppState, org: Uuid, credential: Uuid) {
    warn(
        provider_key(state, org, credential, Change::SignedOut).await,
        "account_signed_out",
    );
}

pub fn key_answered(credential: Uuid) {
    if !COUNTING.load(Ordering::Relaxed) {
        return;
    }
    let mut counts = refusals().lock().unwrap_or_else(PoisonError::into_inner);
    counts.remove(&credential);
    COUNTING.store(!counts.is_empty(), Ordering::Relaxed);
}

pub async fn key_refused(state: &AppState, org: Uuid, credential: Uuid, refusal: Refusal<'_>) {
    let now = state.clock.now();
    let streak = {
        let mut counts = refusals().lock().unwrap_or_else(PoisonError::into_inner);
        let s = counts.entry(credential).or_insert(Streak {
            count: 0,
            since: now,
        });
        s.count = s.count.saturating_add(1);
        COUNTING.store(true, Ordering::Relaxed);
        *s
    };
    if streak.count == REFUSALS {
        let change = Change::Refused {
            refusal,
            since: streak.since,
        };
        warn(
            provider_key(state, org, credential, change).await,
            "key_refused",
        );
    }
}

async fn provider_key(
    state: &AppState,
    org: Uuid,
    credential: Uuid,
    change: Change<'_>,
) -> ApiResult<usize> {
    let Some(row) = credentials::get(&state.pg, org, credential).await? else {
        return Ok(0);
    };
    let place = org_name(state, org).await?;
    let provider = provider_name(state, org, &row.provider).await?;
    let projects: Vec<String> = credentials::assignments(&state.pg, org, Some(credential))
        .await?
        .into_iter()
        .map(|a| a.project_name)
        .collect();
    let zone_name = kaeso_store::pg::instance::time_zone(&state.pg).await?;
    let zone = crate::time_zone::or_utc(&zone_name);
    let now = state.clock.now();
    let key = Credential {
        organization: &place,
        provider: &provider,
        name: &row.name,
        last4: row.account_label.is_none().then_some(row.last4.as_str()),
        account: row.account_label.as_deref(),
        default: row.is_default,
        projects: &projects,
        last_used: row.last_used_at.map(|at| local(at, &zone)),
        zone: &zone_name,
    };
    let message;
    let (subject, change) = match change {
        Change::Refused { refusal, since } => {
            message = clip(refusal.message, ANSWER_CHARS);
            (
                "refused",
                ProviderKey::Refused {
                    answer: Answer {
                        status: refusal.status,
                        message: &message,
                        billing: refusal.billing,
                    },
                    first: local(since, &zone),
                    last: local(now, &zone),
                },
            )
        }
        Change::SignedOut => (
            "signed_out",
            ProviderKey::SignedOut {
                at: local(now, &zone),
            },
        ),
    };
    let send = Send {
        kind: Kind::ProviderKeys,
        org: Some(org),
        project: None,
        place: Some(&place),
        subject: format!("{subject}:{}", CredentialId::from_uuid(credential)),
        open: format!("{}/provider-keys", settings_path(org)),
        actor: None,
    };
    let to = org_holders(state, org, &[OrgRight::ProviderCredentialsReplace]).await?;
    deliver(state, &send, &Fact::ProviderKey { key: &key, change }, to).await
}

pub fn when_done<S: futures::Stream>(
    stream: S,
    after: impl Future<Output = ()>,
) -> impl futures::Stream<Item = S::Item> {
    use futures::StreamExt;
    stream.chain(futures::stream::once(after).filter_map(|()| std::future::ready(None)))
}

pub async fn export_finished(
    state: &AppState,
    org: Uuid,
    project: Option<Uuid>,
    user: Uuid,
    what: Export,
) {
    let result = async {
        let (place, open) = match project {
            Some(p) => (
                project_name(state, org, p).await?,
                format!("{}/logs", project_path(org, p)),
            ),
            None => (
                org_name(state, org).await?,
                format!("{}/audit-log", settings_path(org)),
            ),
        };
        let send = Send {
            kind: Kind::ExportReady,
            org: Some(org),
            project,
            place: Some(&place),
            subject: format!("export:{}", Uuid::now_v7()),
            open,
            actor: None,
        };
        let right = match what {
            Export::Logs => ProjectRight::LogsExport.as_str(),
            Export::AuditLog => OrgRight::AuditLogExport.as_str(),
        };
        let to = Recipients::from([(user, Because::Right(right))]);
        deliver(state, &send, &Fact::ExportReady(what), to).await
    }
    .await;
    warn(result, "export_finished");
}

pub async fn scope_of(
    state: &AppState,
    project: Uuid,
    subject: Subject,
) -> ApiResult<(Option<String>, Option<Uuid>)> {
    Ok(match subject {
        Subject::Project(_) => (None, None),
        Subject::Key(key) => match keys::get(&state.pg, project, key).await? {
            Some(k) => (Some(k.name), k.user_id),
            None => (Some(ApiKeyId::from_uuid(key).to_string()), None),
        },
        Subject::User(user) => (Some(person(state, user).await?), None),
        Subject::ServiceAccount(id) => (
            Some(
                service_accounts::get(&state.pg, project, id)
                    .await?
                    .map_or_else(|| ServiceAccountId::from_uuid(id).to_string(), |s| s.name),
            ),
            None,
        ),
    })
}

pub fn scope<'a>(subject: Subject, name: Option<&'a str>) -> BudgetScope<'a> {
    match (subject, name) {
        (Subject::Key(_), Some(name)) => BudgetScope::Key(name),
        (Subject::User(_) | Subject::ServiceAccount(_), Some(name)) => BudgetScope::Member(name),
        _ => BudgetScope::Project,
    }
}

pub fn resets(limit: &Limit, at: OffsetDateTime) -> Option<PrimitiveDateTime> {
    limit.period.resets_at(at).map(|r| {
        let r = r.to_offset(time::UtcOffset::UTC);
        PrimitiveDateTime::new(r.date(), r.time())
    })
}

pub async fn limit_reached(
    state: &AppState,
    org: Uuid,
    project: Uuid,
    limit: &Limit,
    budget: &kaeso_mail::Budget,
    at: OffsetDateTime,
) {
    let result = async {
        let organization = org_name(state, org).await?;
        let place = project_name(state, org, project).await?;
        let mut to = project_holders(state, org, project, &[ProjectRight::LimitsManage]).await?;
        let (name, owner) = scope_of(state, project, limit.subject).await?;
        if let Some(owner) = owner {
            to.insert(owner, Because::Right(ProjectRight::KeysOwnCreate.as_str()));
        }
        let period = limit.period.start(at).unwrap_or(time::Date::MIN);
        let send = Send {
            kind: Kind::LimitReached,
            org: Some(org),
            project: Some(project),
            place: Some(&place),
            subject: format!("limit:{}:{period}", LimitId::from_uuid(limit.id)),
            open: format!("{}/limits", project_path(org, project)),
            actor: None,
        };
        let fact = Fact::LimitReached {
            organization: &organization,
            project: &place,
            budget,
            scope: scope(limit.subject, name.as_deref()),
            resets: resets(limit, at),
        };
        deliver(state, &send, &fact, to).await
    }
    .await;
    warn(result, "limit_reached");
}

pub async fn update_available(state: &AppState, version: &str) {
    warn(
        super::triggers::update(
            state,
            version,
            Update::Available,
            format!("available:{version}"),
        )
        .await,
        "update_available",
    );
}
