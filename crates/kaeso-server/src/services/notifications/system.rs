use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};

use kaeso_core::id::{ApiKeyId, CredentialId, LimitId, OrgId};
use kaeso_core::limits::{Limit, Subject};
use kaeso_core::notifications::Kind;
use kaeso_core::rights::{OrgRight, ProjectRight};
use kaeso_mail::BudgetScope;
use kaeso_mail::notices::{Because, Export, Fact, ProviderKey, Update};
use kaeso_store::pg::credentials;
use kaeso_store::pg::mcp_servers::ServerRow;
use uuid::Uuid;

use super::dispatch::{
    Recipients, Send, about, deliver, org_holders, project_holders, project_path, settings_path,
};
use super::triggers::{org_name, project_name};
use crate::platform::error::ApiResult;
use crate::state::AppState;

// Spec: a provider key needs attention after three refusals in a row.
const REFUSALS: u8 = 3;

// Every successful model request clears its key's count, so it checks a flag before the lock.
static COUNTING: AtomicBool = AtomicBool::new(false);

fn refusals() -> &'static Mutex<HashMap<Uuid, u8>> {
    static COUNTS: OnceLock<Mutex<HashMap<Uuid, u8>>> = OnceLock::new();
    COUNTS.get_or_init(Mutex::default)
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
        provider_key(state, org, credential, ProviderKey::SignedOut).await,
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

pub async fn key_refused(state: &AppState, org: Uuid, credential: Uuid) {
    let count = {
        let mut counts = refusals().lock().unwrap_or_else(PoisonError::into_inner);
        let n = counts.entry(credential).or_default();
        *n = n.saturating_add(1);
        COUNTING.store(true, Ordering::Relaxed);
        *n
    };
    if count == REFUSALS {
        warn(
            provider_key(state, org, credential, ProviderKey::Refused).await,
            "key_refused",
        );
    }
}

async fn provider_key(
    state: &AppState,
    org: Uuid,
    credential: Uuid,
    change: ProviderKey,
) -> ApiResult<usize> {
    let Some(row) = credentials::get(&state.pg, org, credential).await? else {
        return Ok(0);
    };
    let place = org_name(state, org).await?;
    let send = Send {
        kind: Kind::ProviderKeys,
        org: Some(org),
        project: None,
        place: Some(&place),
        subject: format!("{change:?}:{}", CredentialId::from_uuid(credential)),
        open: format!("{}/provider-keys", settings_path(org)),
        actor: None,
    };
    let to = org_holders(state, org, &[OrgRight::ProviderCredentialsReplace]).await?;
    deliver(
        state,
        &send,
        &Fact::ProviderKey {
            key: &row.name,
            change,
        },
        to,
    )
    .await
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

pub async fn limit_reached(
    state: &AppState,
    org: Uuid,
    project: Uuid,
    limit: &Limit,
    budget: &kaeso_mail::Budget,
    period: time::Date,
) {
    let result = async {
        let place = project_name(state, org, project).await?;
        let mut to = project_holders(state, org, project, &[ProjectRight::LimitsManage]).await?;
        let key_id;
        let scope = match limit.subject {
            Subject::Key(key) => {
                if let Some(owner) = kaeso_store::pg::keys::get(&state.pg, project, key)
                    .await?
                    .and_then(|k| k.user_id)
                {
                    to.insert(owner, Because::Right(ProjectRight::KeysOwnCreate.as_str()));
                }
                key_id = ApiKeyId::from_uuid(key).to_string();
                BudgetScope::Key(&key_id)
            }
            Subject::Project(_) => BudgetScope::Project,
            Subject::User(_) | Subject::ServiceAccount(_) => BudgetScope::Member,
        };
        let send = Send {
            kind: Kind::LimitReached,
            org: Some(org),
            project: Some(project),
            place: Some(&place),
            subject: format!("limit:{}:{period}", LimitId::from_uuid(limit.id)),
            open: format!("{}/limits", project_path(org, project)),
            actor: None,
        };
        deliver(state, &send, &Fact::LimitReached { budget, scope }, to).await
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
