use kaeso_core::id::{ApiKeyId, CatalogueModelId, InviteId, OrgId, ProjectId, UserId};
use kaeso_core::notifications::Kind;
use kaeso_core::rights::{InstanceRight, OrgRight, ProjectRight};
use kaeso_mail::Actor;
use kaeso_mail::notices::{
    Access, Fact, Instance, KeyChange, Membership, ModelChange, Security, Update,
};
use kaeso_store::pg::{catalogue, invites, keys, orgs, projects, users};
use serde_json::Value;
use uuid::Uuid;

use super::dispatch::{
    Event, Recipients, Send, about, deliver, instance_admins, org_holders, org_path,
    project_holders, project_path, settings_path,
};
use crate::platform::audit::Action as A;
use crate::platform::error::ApiResult;
use crate::state::AppState;

fn text<'v>(v: Option<&'v Value>, key: &str) -> Option<&'v str> {
    v.and_then(|v| v.get(key)).and_then(Value::as_str)
}

fn uuid_of(raw: Option<&str>) -> Option<Uuid> {
    let raw = raw?;
    raw.parse::<Uuid>().ok().or_else(|| {
        raw.split_once('_')
            .and_then(|(_, b32)| kaeso_core::id::decode_base32(b32).ok().map(Uuid::from_u128))
    })
}

pub(crate) async fn person(state: &AppState, user: Uuid) -> ApiResult<String> {
    Ok(users::get(&state.pg, user)
        .await?
        .and_then(|u| u.name.filter(|n| !n.trim().is_empty()).or(u.email))
        .unwrap_or_else(|| UserId::from_uuid(user).to_string()))
}

pub(crate) async fn org_name(state: &AppState, org: Uuid) -> ApiResult<String> {
    Ok(orgs::get(&state.pg, org)
        .await?
        .map_or_else(|| OrgId::from_uuid(org).to_string(), |o| o.name))
}

pub(crate) async fn project_name(state: &AppState, org: Uuid, project: Uuid) -> ApiResult<String> {
    Ok(projects::get(&state.pg, org, project)
        .await?
        .map_or_else(|| ProjectId::from_uuid(project).to_string(), |p| p.name))
}

async fn actor_name(state: &AppState, e: &Event) -> ApiResult<Option<String>> {
    match e.actor {
        Some(user) => Ok(Some(person(state, user).await?)),
        None => Ok(None),
    }
}

fn by(name: &Option<String>) -> Actor<'_> {
    name.as_deref().map_or(Actor::Administrator, Actor::Person)
}

fn subject(e: &Event, extra: &str) -> String {
    format!(
        "{}:{}:{extra}",
        e.action.as_str(),
        e.target.as_deref().unwrap_or_default()
    )
}

pub async fn on_event(state: &AppState, kind: Kind, e: &Event) -> ApiResult<usize> {
    match kind {
        Kind::NewApp => new_app(state, e).await,
        Kind::PortalClients => portal_client(state, e).await,
        Kind::InstanceUpdates => update_finished(state, e).await,
        Kind::InstancePeople => people(state, e).await,
        Kind::InstanceSettings => instance_settings(state, e).await,
        Kind::OrgSecurity => org_security(state, e).await,
        Kind::OrgMembers => org_members(state, e).await,
        Kind::InviteAccepted => invite_accepted(state, e).await,
        Kind::AccessChanged => access_changed(state, e).await,
        Kind::ModelChanges => model_changes(state, e).await,
        Kind::KeyChanged => key_changed(state, e).await,
        Kind::ContentLogging => content_logging(state, e).await,
        // No audit action maps to these: the account's security emails, ownership and deletion,
        // budget alerts and expiring keys are sent where they happen, and the rest come from
        // system triggers and jobs.
        Kind::AccountSecurity
        | Kind::OrgOwnership
        | Kind::BudgetAlerts
        | Kind::KeyExpiry
        | Kind::ExportReady
        | Kind::WeeklySummary
        | Kind::LimitReached
        | Kind::McpServers
        | Kind::ConnectionExpired
        | Kind::ProviderKeys
        | Kind::CostReport => Ok(0),
    }
}

async fn new_app(state: &AppState, e: &Event) -> ApiResult<usize> {
    let Some(user) = e.actor else { return Ok(0) };
    let app = text(e.after.as_ref(), "client")
        .or(text(e.after.as_ref(), "name"))
        .unwrap_or("An app");
    let open = match (e.action, e.org, e.project) {
        (A::PortalGrantCreate, Some(org), Some(project)) => {
            format!("{}/tools?tab=connect", project_path(org, project))
        }
        _ => "settings/account/agents".to_owned(),
    };
    let send = Send {
        kind: Kind::NewApp,
        org: e.org,
        project: e.project,
        place: None,
        subject: subject(e, ""),
        open,
        actor: e.actor,
    };
    deliver(state, &send, &Fact::NewApp { app }, about(user)).await
}

async fn portal_client(state: &AppState, e: &Event) -> ApiResult<usize> {
    let (Some(org), Some(project)) = (e.org, e.project) else {
        return Ok(0);
    };
    let client = text(e.after.as_ref(), "client").unwrap_or("An MCP client");
    let place = project_name(state, org, project).await?;
    let who = actor_name(state, e).await?;
    let send = Send {
        kind: Kind::PortalClients,
        org: Some(org),
        project: Some(project),
        place: Some(&place),
        subject: subject(e, ""),
        open: format!("{}/tools?tab=connect", project_path(org, project)),
        actor: e.actor,
    };
    let to = project_holders(state, org, project, &[ProjectRight::ToolsManage]).await?;
    deliver(
        state,
        &send,
        &Fact::PortalClient {
            client,
            by: by(&who),
        },
        to,
    )
    .await
}

async fn update_finished(state: &AppState, e: &Event) -> ApiResult<usize> {
    let Some(version) = text(e.after.as_ref(), "version") else {
        return Ok(0);
    };
    let change = match text(e.after.as_ref(), "result") {
        Some("succeeded") => Update::Finished,
        Some("rolled_back") => Update::RolledBack,
        _ => Update::Failed,
    };
    update(state, version, change, subject(e, "")).await
}

pub(crate) async fn update(
    state: &AppState,
    version: &str,
    change: Update,
    subject: String,
) -> ApiResult<usize> {
    let send = Send {
        kind: Kind::InstanceUpdates,
        org: None,
        project: None,
        place: None,
        subject,
        open: "settings/instance/updates".to_owned(),
        actor: None,
    };
    let to = instance_admins(state, InstanceRight::InstanceUpdatesManage).await?;
    deliver(state, &send, &Fact::InstanceUpdate { version, change }, to).await
}

async fn people(state: &AppState, e: &Event) -> ApiResult<usize> {
    let to = instance_admins(state, InstanceRight::InstanceUsersView).await?;
    if e.action == A::OrgCreate {
        let Some(org) = uuid_of(e.target.as_deref()) else {
            return Ok(0);
        };
        let name = org_name(state, org).await?;
        let who = actor_name(state, e).await?;
        let send = Send {
            kind: Kind::InstancePeople,
            org: None,
            project: None,
            place: None,
            subject: format!("org:{org}"),
            open: format!("settings/instance/orgs/{}", OrgId::from_uuid(org)),
            actor: e.actor,
        };
        let fact = Fact::InstanceChanged {
            change: Instance::OrgCreated(&name),
            by: by(&who),
        };
        return deliver(state, &send, &fact, to).await;
    }
    let Some(user) = uuid_of(e.target.as_deref()) else {
        return Ok(0);
    };
    let name = person(state, user).await?;
    let send = Send {
        kind: Kind::InstancePeople,
        org: None,
        project: None,
        place: None,
        subject: format!("user:{user}"),
        open: "settings/instance/users".to_owned(),
        actor: e.actor,
    };
    deliver(state, &send, &Fact::SignedUp { person: &name }, to).await
}

async fn instance_settings(state: &AppState, e: &Event) -> ApiResult<usize> {
    let change = match e.action {
        A::InstanceAdminSsoUpdate => Instance::AdminSso,
        A::InstanceAgentsPolicy => Instance::AgentPolicy,
        _ => Instance::Settings,
    };
    let who = actor_name(state, e).await?;
    let send = Send {
        kind: Kind::InstanceSettings,
        org: None,
        project: None,
        place: None,
        subject: e.action.as_str().to_owned(),
        open: "settings/instance/general".to_owned(),
        actor: e.actor,
    };
    let to = instance_admins(state, InstanceRight::InstanceSettingsManage).await?;
    deliver(
        state,
        &send,
        &Fact::InstanceChanged {
            change,
            by: by(&who),
        },
        to,
    )
    .await
}

async fn org_security(state: &AppState, e: &Event) -> ApiResult<usize> {
    let Some(org) = e.org else { return Ok(0) };
    let key = text(e.after.as_ref(), "name")
        .unwrap_or_default()
        .to_owned();
    let (change, page) = match e.action {
        A::PlatformKeyCreate => (Security::PlatformKeyCreated(&key), "platform-keys"),
        A::PlatformKeyRotate => (Security::PlatformKeyRotated(&key), "platform-keys"),
        A::SecurityKeyPolicy => (Security::KeyPolicy, "security"),
        A::SecurityAgentsPolicy => (Security::AgentPolicy, "security"),
        A::SecurityRequireSso | A::SecurityRequire2fa => (Security::SignInRules, "security"),
        _ => (Security::SingleSignOn, "security"),
    };
    let place = org_name(state, org).await?;
    let who = actor_name(state, e).await?;
    let send = Send {
        kind: Kind::OrgSecurity,
        org: Some(org),
        project: None,
        place: Some(&place),
        subject: subject(e, ""),
        open: format!("{}/{page}", settings_path(org)),
        actor: e.actor,
    };
    let to = org_holders(
        state,
        org,
        &[OrgRight::SecuritySsoManage, OrgRight::PlatformKeysView],
    )
    .await?;
    deliver(
        state,
        &send,
        &Fact::OrgSecurity {
            change,
            by: by(&who),
        },
        to,
    )
    .await
}

async fn org_members(state: &AppState, e: &Event) -> ApiResult<usize> {
    let Some(org) = e.org else { return Ok(0) };
    let member = match e.action {
        A::InviteAccept => e.actor,
        _ => uuid_of(e.target.as_deref()),
    };
    let Some(member) = member else { return Ok(0) };
    let name = person(state, member).await?;
    let who = actor_name(state, e).await?;
    let change = match e.action {
        A::InviteAccept => Membership::Joined,
        A::MemberJoinBySso => Membership::JoinedBySso,
        _ if e.actor == Some(member) => Membership::Left,
        _ => Membership::Removed(by(&who)),
    };
    let place = org_name(state, org).await?;
    let send = Send {
        kind: Kind::OrgMembers,
        org: Some(org),
        project: None,
        place: Some(&place),
        subject: format!("{}:{member}", e.action.as_str()),
        open: format!("{}/members", settings_path(org)),
        actor: e.actor,
    };
    let to: Recipients = org_holders(state, org, &[OrgRight::MembersInvite])
        .await?
        .into_iter()
        .filter(|(u, _)| *u != member)
        .collect();
    deliver(
        state,
        &send,
        &Fact::Member {
            person: &name,
            change,
        },
        to,
    )
    .await
}

async fn invite_accepted(state: &AppState, e: &Event) -> ApiResult<usize> {
    let (Some(org), Some(joiner), Some(invite)) = (e.org, e.actor, uuid_of(e.target.as_deref()))
    else {
        return Ok(0);
    };
    let Some(inviter) = invites::get(&state.pg, Some(org), invite)
        .await?
        .and_then(|i| i.invited_by)
    else {
        return Ok(0);
    };
    let name = person(state, joiner).await?;
    let place = org_name(state, org).await?;
    let send = Send {
        kind: Kind::InviteAccepted,
        org: Some(org),
        project: None,
        place: Some(&place),
        subject: format!("invite:{}", InviteId::from_uuid(invite)),
        open: format!("{}/members", settings_path(org)),
        actor: e.actor,
    };
    let to = Recipients::from([(
        inviter,
        kaeso_mail::notices::Because::Right(OrgRight::MembersInvite.as_str()),
    )]);
    deliver(state, &send, &Fact::InviteAccepted { person: &name }, to).await
}

async fn access_changed(state: &AppState, e: &Event) -> ApiResult<usize> {
    let Some(user) = uuid_of(e.target.as_deref()) else {
        return Ok(0);
    };
    let project = match e.action {
        A::ProjectMemberAdd | A::ProjectMemberRemove => uuid_of(text(e.after.as_ref(), "project")),
        _ => e.project,
    };
    let role = e
        .after
        .as_ref()
        .and_then(|a| a.pointer("/grant/role/name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let change = match e.action {
        A::ProjectMemberAdd => Access::Added(&role),
        A::ProjectMemberRemove | A::MemberRemove => Access::Removed,
        A::UserInstanceAdmin => Access::InstanceAdmin(
            e.after
                .as_ref()
                .and_then(|a| a.get("is_instance_admin"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
        _ => Access::Changed,
    };
    let (place, open) = match (e.org, project) {
        (Some(org), Some(project)) => (
            Some(project_name(state, org, project).await?),
            project_path(org, project),
        ),
        (Some(org), None) => (Some(org_name(state, org).await?), org_path(org)),
        _ => (None, "settings/instance".to_owned()),
    };
    let who = actor_name(state, e).await?;
    let send = Send {
        kind: Kind::AccessChanged,
        org: e.org,
        project,
        place: place.as_deref(),
        subject: subject(e, &project.map(|p| p.to_string()).unwrap_or_default()),
        open,
        actor: e.actor,
    };
    deliver(
        state,
        &send,
        &Fact::AccessChanged {
            change,
            by: by(&who),
        },
        about(user),
    )
    .await
}

async fn model_changes(state: &AppState, e: &Event) -> ApiResult<usize> {
    let entries = match e.action {
        A::CatalogueModelPriceChange => {
            let (Some(org), Some(id)) = (e.org, uuid_of(e.target.as_deref())) else {
                return Ok(0);
            };
            catalogue::in_org(&state.pg, org, id)
                .await?
                .into_iter()
                .collect()
        }
        _ => {
            let Some((provider, model)) = e.target.as_deref().and_then(|t| t.split_once('/'))
            else {
                return Ok(0);
            };
            catalogue::of_model(&state.pg, provider, model).await?
        }
    };
    let change = match e.action {
        A::CatalogueModelPriceChange => ModelChange::Price,
        _ => ModelChange::Delisted,
    };
    let mut sent = 0;
    for entry in entries {
        let place = project_name(state, entry.org_id, entry.project_id).await?;
        let send = Send {
            kind: Kind::ModelChanges,
            org: Some(entry.org_id),
            project: Some(entry.project_id),
            place: Some(&place),
            subject: format!(
                "{}:{}:{}",
                e.action.as_str(),
                CatalogueModelId::from_uuid(entry.id),
                entry.model_id
            ),
            open: format!("{}/models", project_path(entry.org_id, entry.project_id)),
            actor: None,
        };
        let to = project_holders(
            state,
            entry.org_id,
            entry.project_id,
            &[ProjectRight::ModelsManage],
        )
        .await?;
        sent += deliver(
            state,
            &send,
            &Fact::Model {
                model: &entry.model_id,
                change,
            },
            to,
        )
        .await?;
    }
    Ok(sent)
}

async fn key_changed(state: &AppState, e: &Event) -> ApiResult<usize> {
    let (Some(org), Some(project), Some(key)) = (e.org, e.project, uuid_of(e.target.as_deref()))
    else {
        return Ok(0);
    };
    let Some(row) = keys::get(&state.pg, project, key).await? else {
        return Ok(0);
    };
    let Some(owner) = row.user_id else {
        return Ok(0);
    };
    let change = match e.action {
        A::KeyDisable => KeyChange::Disabled,
        A::KeyRotate => KeyChange::Rotated,
        _ => KeyChange::Archived,
    };
    let place = project_name(state, org, project).await?;
    let who = actor_name(state, e).await?;
    let send = Send {
        kind: Kind::KeyChanged,
        org: Some(org),
        project: Some(project),
        place: Some(&place),
        subject: format!("{}:{}", e.action.as_str(), ApiKeyId::from_uuid(key)),
        open: format!("{}/keys", project_path(org, project)),
        actor: e.actor,
    };
    let fact = Fact::KeyChanged {
        key: &row.name,
        change,
        by: by(&who),
    };
    deliver(state, &send, &fact, about(owner)).await
}

async fn content_logging(state: &AppState, e: &Event) -> ApiResult<usize> {
    let (Some(org), Some(project)) = (e.org, e.project.or(uuid_of(e.target.as_deref()))) else {
        return Ok(0);
    };
    let on = e
        .after
        .as_ref()
        .and_then(|a| a.get("content_logging"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let place = project_name(state, org, project).await?;
    let who = actor_name(state, e).await?;
    let send = Send {
        kind: Kind::ContentLogging,
        org: Some(org),
        project: Some(project),
        place: Some(&place),
        subject: format!("{}:{on}", subject(e, "")),
        open: format!("{}/logs", project_path(org, project)),
        actor: e.actor,
    };
    let to = project_holders(state, org, project, &[ProjectRight::ProjectView]).await?;
    deliver(state, &send, &Fact::ContentLogging { on, by: by(&who) }, to).await
}
