use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use kaeso_core::id::{OrgId, ProjectId, UserId};
use kaeso_core::notifications::{Delivery, Kind};
use kaeso_core::rights::{OrgRight, ProjectRight, Right, effective_project_rights};
use kaeso_mail::Locale;
use kaeso_mail::notices::{self, Because, Fact, Links, Unsubscribe};
use kaeso_store::pg::{members, notifications, users};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::auth::throttle::{self, Outage, Throttle};
use crate::platform::error::ApiResult;
use crate::platform::grants::{org_from_store, project_from_store};
use crate::state::AppState;

// Spec: at most 20 immediate emails per person and hour; the rest wait for the digest.
const NOW_PER_HOUR: Throttle = Throttle {
    name: "notices_now",
    limit: 20,
    window: std::time::Duration::from_secs(60 * 60),
    on_outage: Outage::Allow,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub action: crate::platform::audit::Action,
    pub org: Option<Uuid>,
    pub project: Option<Uuid>,
    pub actor: Option<Uuid>,
    pub target: Option<String>,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

tokio::task_local! {
    static PENDING: RefCell<Vec<Event>>;
}

pub fn record(event: Event) {
    let _ = PENDING.try_with(|p| p.borrow_mut().push(event));
}

pub async fn collecting<T>(work: impl Future<Output = T>) -> (T, Vec<Event>) {
    PENDING
        .scope(RefCell::new(Vec::new()), async move {
            let out = work.await;
            let events = PENDING.with(|p| std::mem::take(&mut *p.borrow_mut()));
            (out, events)
        })
        .await
}

pub fn spawn(state: &AppState, events: Vec<Event>) {
    if events.is_empty() {
        return;
    }
    let task = state.clone();
    state.in_flight.finish(async move {
        dispatch(&task, &events).await;
    });
}

pub async fn dispatch(state: &AppState, events: &[Event]) {
    for event in events {
        for kind in kinds(event.action) {
            if let Err(e) = super::triggers::on_event(state, *kind, event).await {
                tracing::warn!(error = %e, action = event.action.as_str(), kind = kind.as_str(), "a notification couldn't be sent");
            }
        }
    }
}

pub fn kinds(action: crate::platform::audit::Action) -> &'static [Kind] {
    use crate::platform::audit::Action as A;
    match action {
        A::AgentAuthorize => &[Kind::NewApp],
        A::PortalGrantCreate => &[Kind::NewApp, Kind::PortalClients],
        A::InstanceUpdateFinish => &[Kind::InstanceUpdates],
        A::OrgCreate | A::UserCreateBySso | A::UserSignup => &[Kind::InstancePeople],
        A::InstanceSettingsUpdate | A::InstanceAdminSsoUpdate | A::InstanceAgentsPolicy => {
            &[Kind::InstanceSettings]
        }
        A::SecurityAgentsPolicy
        | A::SecurityKeyPolicy
        | A::SecurityRequireSso
        | A::SecurityRequire2fa
        | A::SecuritySsoDelete
        | A::SecuritySsoDomainVerify
        | A::SecuritySsoUpdate
        | A::PlatformKeyCreate
        | A::PlatformKeyRotate => &[Kind::OrgSecurity],
        A::InviteAccept => &[Kind::OrgMembers, Kind::InviteAccepted],
        A::MemberJoinBySso => &[Kind::OrgMembers],
        A::MemberRemove => &[Kind::OrgMembers, Kind::AccessChanged],
        A::KpmrModelDelisted | A::CatalogueModelPriceChange => &[Kind::ModelChanges],
        A::ProjectMemberAdd
        | A::ProjectMemberRemove
        | A::ProjectMemberRightsUpdate
        | A::MemberRightsUpdate
        | A::UserInstanceAdmin => &[Kind::AccessChanged],
        A::KeyDisable | A::KeyRotate | A::KeyArchive => &[Kind::KeyChanged],
        A::LogsProjectContentLogging => &[Kind::ContentLogging],
        // Sent where they happen, before this dispatcher existed: the account's security
        // emails, ownership and deletion of an organization, and budget alerts.
        A::UserEmailChange
        | A::UserEmailChangeRequest
        | A::UserPasswordChange
        | A::UserPasswordReset
        | A::UserSessionsRevoke
        | A::UserTotpDisable
        | A::UserTotpEnable
        | A::UserDeactivate
        | A::User2faReset
        | A::OrgOwnershipTransfer
        | A::OrgScheduleDeletion
        | A::LimitThresholdReached => &[],
        A::AgentOrgAccessRevoke
        | A::AgentRevoke
        | A::AgentRightsUpdate
        | A::AgentUpdate
        | A::AliasCreate
        | A::AliasDelete
        | A::AliasUpdate
        | A::AuditLogExport
        | A::CatalogueModelCheck
        | A::CatalogueModelCreate
        | A::CatalogueModelDelete
        | A::CatalogueModelUpdate
        | A::CredentialAssign
        | A::CredentialCreate
        | A::CredentialDefault
        | A::CredentialDelete
        | A::CredentialRename
        | A::CredentialReplace
        | A::CustomProviderCreate
        | A::CustomProviderDelete
        | A::CustomProviderUpdate
        | A::InstanceInvite
        | A::InstanceSetup
        | A::InstanceUpdateCheck
        | A::InstanceUpdateSettings
        | A::InstanceUpdateStart
        | A::InviteCreate
        | A::InviteResend
        | A::InviteRevoke
        | A::KeyCreate
        | A::KeyEnable
        | A::KpmrPriceChange
        | A::LimitCreate
        | A::LimitDelete
        | A::LimitUpdate
        | A::LogsDelete
        | A::LogsExport
        | A::LogsSettingsUpdate
        | A::McpServerConnect
        | A::McpServerCreate
        | A::McpServerDelete
        | A::McpServerDisconnect
        | A::McpServerUpdate
        | A::ModelAccessUpdate
        | A::OrgDeleteFinal
        | A::OrgDeleteNow
        | A::OrgOwnerAppointed
        | A::OrgOwnerAppointedByInstanceAdmin
        | A::OrgRestore
        | A::OrgUpdate
        | A::PlatformKeyArchive
        | A::PlatformKeyDisable
        | A::PlatformKeyEnable
        | A::PortalGrantRevoke
        | A::ProjectArchive
        | A::ProjectCreate
        | A::ProjectKeyPolicy
        | A::ProjectMcpServersUpdate
        | A::ProjectToolsUpdate
        | A::ProjectUpdate
        | A::ResponseCacheClear
        | A::ResponseCacheOrgUpdate
        | A::ResponseCacheProjectUpdate
        | A::RoleCreate
        | A::RoleDelete
        | A::RoleUpdate
        | A::ServiceAccountArchive
        | A::ServiceAccountCreate
        | A::ServiceAccountUpdate
        | A::ToolApprove
        | A::ToolReject
        | A::ToolAccessUpdate
        | A::UserCreateByInvite
        | A::UserDelete
        | A::UserEmailChangeCancel
        | A::UserNotifications
        | A::UserReactivate
        | A::UserRecoveryCodesRegenerate
        | A::UserSignInConfirm
        | A::UserSignInLink
        | A::UserSsoLink
        | A::UserSsoUnlink
        | A::UserUpdate => &[],
    }
}

#[derive(Clone, Debug)]
pub struct Send<'a> {
    pub kind: Kind,
    pub org: Option<Uuid>,
    pub project: Option<Uuid>,
    pub place: Option<&'a str>,
    pub subject: String,
    pub open: String,
    pub actor: Option<Uuid>,
}

pub type Recipients = BTreeMap<Uuid, Because<'static>>;

pub fn about(user: Uuid) -> Recipients {
    BTreeMap::from([(user, Because::AboutYou)])
}

pub async fn org_holders(
    state: &AppState,
    org: Uuid,
    rights: &[OrgRight],
) -> ApiResult<Recipients> {
    let mut out = Recipients::new();
    for m in members::all_org(&state.pg, org).await? {
        let held = org_from_store(&m.grant())?.effective();
        if let Some(r) = rights.iter().find(|r| held.contains(**r)) {
            out.insert(m.user_id, Because::Right(Right::Org(*r).as_str()));
        }
    }
    Ok(out)
}

pub async fn project_holders(
    state: &AppState,
    org: Uuid,
    project: Uuid,
    rights: &[ProjectRight],
) -> ApiResult<Recipients> {
    let mut out = Recipients::new();
    for g in members::project_grants(&state.pg, org, project).await? {
        let own = g
            .project_grant
            .as_ref()
            .map(project_from_store)
            .transpose()?;
        let held = effective_project_rights(Some(&org_from_store(&g.org_grant)?), own.as_ref());
        if let Some(r) = rights.iter().find(|r| held.contains(**r)) {
            out.insert(g.user_id, Because::Right(Right::Project(*r).as_str()));
        }
    }
    Ok(out)
}

pub async fn instance_admins(
    state: &AppState,
    right: kaeso_core::rights::InstanceRight,
) -> ApiResult<Recipients> {
    Ok(notifications::instance_admins(&state.pg)
        .await?
        .into_iter()
        .map(|u| (u, Because::Right(Right::Instance(right).as_str())))
        .collect())
}

pub async fn deliver(
    state: &AppState,
    send: &Send<'_>,
    fact: &Fact<'_>,
    to: Recipients,
) -> ApiResult<usize> {
    let kind = send.kind;
    let to: Recipients = to
        .into_iter()
        .filter(|(user, _)| kind.always() || send.actor != Some(*user))
        .collect();
    if to.is_empty() {
        return Ok(0);
    }
    let ids: Vec<Uuid> = to.keys().copied().collect();
    let people = users::recipients(&state.pg, &ids).await?;
    let chosen: BTreeMap<Uuid, Delivery> =
        notifications::deliveries_of(&state.pg, &ids, kind.as_str())
            .await?
            .into_iter()
            .filter_map(|(u, d)| d.parse().ok().map(|d| (u, d)))
            .collect();
    let muted: BTreeSet<Uuid> = match send.org {
        Some(org) if !kind.always() => notifications::mutes_of(&state.pg, &ids)
            .await?
            .into_iter()
            .filter(|(_, o)| *o == org)
            .map(|(u, _)| u)
            .collect(),
        _ => BTreeSet::new(),
    };
    let now = state.clock.now();
    let zone = crate::time_zone::or_utc(&kaeso_store::pg::instance::time_zone(&state.pg).await?);
    let today = crate::time_zone::local(now, &zone).date();
    let base = &state.config.public_url;
    let mut sent = 0;
    for person in people {
        let (Some(email), Some(why)) = (&person.email, to.get(&person.id)) else {
            continue;
        };
        if person.state != "active" || muted.contains(&person.id) {
            continue;
        }
        let delivery = kind.chosen(chosen.get(&person.id).copied());
        if delivery == Delivery::Off {
            continue;
        }
        if !notifications::claim_subject(&state.pg, person.id, kind.as_str(), &send.subject, today)
            .await?
        {
            continue;
        }
        let lang = Locale::choose(person.locale.as_deref(), None);
        let now_allowed = delivery == Delivery::Now
            && (kind.always()
                || throttle::hit(state, NOW_PER_HOUR, &person.id.to_string())
                    .await
                    .is_ok());
        if !now_allowed {
            notifications::queue(
                &state.pg,
                &notifications::DigestEntry {
                    id: Uuid::now_v7(),
                    user_id: person.id,
                    kind: kind.as_str(),
                    org_id: send.org,
                    project_id: send.project,
                    payload: json!({ "line": notices::line(lang, send.place, fact) }),
                    at: now,
                },
            )
            .await?;
            sent += 1;
            continue;
        }
        let user = UserId::from_uuid(person.id);
        let links = Links {
            settings: format!("{base}settings/account/notifications"),
            unsubscribe: (!kind.always()).then(|| {
                let token = super::unsubscribe_token(state, user, kind);
                Unsubscribe {
                    page: format!("{base}unsubscribe#token={token}"),
                    one_click: format!("{base}platform/v1/notifications/unsubscribe?token={token}"),
                }
            }),
        };
        state.mailer.send(notices::notice(
            lang,
            email,
            send.place,
            fact,
            *why,
            &format!("{base}{}", send.open),
            &links,
        ));
        sent += 1;
    }
    Ok(sent)
}

pub fn org_path(org: Uuid) -> String {
    format!("orgs/{}", OrgId::from_uuid(org))
}

pub fn settings_path(org: Uuid) -> String {
    format!("settings/orgs/{}", OrgId::from_uuid(org))
}

pub fn project_path(org: Uuid, project: Uuid) -> String {
    format!(
        "orgs/{}/projects/{}",
        OrgId::from_uuid(org),
        ProjectId::from_uuid(project)
    )
}
