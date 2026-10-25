use kaeso_core::id::{AuditEventId, OrgId, ProjectId, UserId};
use kaeso_store::pg::audit::{self, NewEvent};
use serde::Serialize;
use serde_json::Value;

use super::access::{Caller, Ctx};
use super::error::ApiResult;

macro_rules! actions {
    (pub const $list:ident = [$($variant:ident = $name:literal,)*];) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Action {
            $($variant,)*
        }

        impl Action {
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)*
                }
            }
        }

        pub const $list: &[&str] = &[$($name,)*];
    };
}

// Spelled as a constant because the console's test reads the action names from this list.
actions! {
    pub const ACTIONS = [
        AliasCreate = "alias.create",
        AliasDelete = "alias.delete",
        AliasUpdate = "alias.update",
        AuditLogExport = "audit_log.export",
        CatalogueModelCheck = "catalogue_model.check",
        CatalogueModelCreate = "catalogue_model.create",
        CatalogueModelDelete = "catalogue_model.delete",
        CatalogueModelPriceChange = "catalogue_model.price_change",
        CatalogueModelUpdate = "catalogue_model.update",
        CredentialAssign = "credential.assign",
        CredentialCreate = "credential.create",
        CredentialDefault = "credential.default",
        CredentialDelete = "credential.delete",
        CredentialRename = "credential.rename",
        CredentialReplace = "credential.replace",
        CustomProviderCreate = "custom_provider.create",
        CustomProviderDelete = "custom_provider.delete",
        CustomProviderUpdate = "custom_provider.update",
        InstanceAdminSsoUpdate = "instance.admin_sso_update",
        InstanceInvite = "instance.invite",
        InstanceSettingsUpdate = "instance.settings_update",
        InstanceSetup = "instance.setup",
        InstanceUpdateCheck = "instance.update_check",
        InstanceUpdateFinish = "instance.update_finish",
        InstanceUpdateSettings = "instance.update_settings",
        InstanceUpdateStart = "instance.update_start",
        InviteAccept = "invite.accept",
        InviteCreate = "invite.create",
        InviteResend = "invite.resend",
        InviteRevoke = "invite.revoke",
        KeyArchive = "key.archive",
        KeyCreate = "key.create",
        KeyDisable = "key.disable",
        KeyEnable = "key.enable",
        KeyRotate = "key.rotate",
        LimitCreate = "limit.create",
        LimitDelete = "limit.delete",
        LimitThresholdReached = "limit.threshold_reached",
        LimitUpdate = "limit.update",
        LogsDelete = "logs.delete",
        LogsExport = "logs.export",
        LogsProjectContentLogging = "logs.project_content_logging",
        LogsSettingsUpdate = "logs.settings_update",
        MemberJoinBySso = "member.join_by_sso",
        MemberRemove = "member.remove",
        MemberRightsUpdate = "member.rights_update",
        ModelAccessUpdate = "model_access.update",
        ModelsDevModelDelisted = "models_dev.model_delisted",
        ModelsDevPriceChange = "models_dev.price_change",
        ModelsDevSync = "models_dev.sync",
        OrgCreate = "org.create",
        OrgDeleteFinal = "org.delete_final",
        OrgDeleteNow = "org.delete_now",
        OrgOwnerAppointed = "org.owner_appointed",
        OrgOwnerAppointedByInstanceAdmin = "org.owner_appointed_by_instance_admin",
        OrgOwnershipTransfer = "org.ownership_transfer",
        OrgRestore = "org.restore",
        OrgScheduleDeletion = "org.schedule_deletion",
        OrgUpdate = "org.update",
        PlatformKeyArchive = "platform_key.archive",
        PlatformKeyCreate = "platform_key.create",
        PlatformKeyDisable = "platform_key.disable",
        PlatformKeyEnable = "platform_key.enable",
        PlatformKeyRotate = "platform_key.rotate",
        ProjectArchive = "project.archive",
        ProjectCreate = "project.create",
        ProjectDefault = "project.default",
        ProjectKeyPolicy = "project.key_policy",
        ProjectMemberAdd = "project.member_add",
        ProjectMemberRemove = "project.member_remove",
        ProjectMemberRightsUpdate = "project.member_rights_update",
        ProjectUpdate = "project.update",
        RoleCreate = "role.create",
        RoleDelete = "role.delete",
        RoleUpdate = "role.update",
        SecurityKeyPolicy = "security.key_policy",
        SecurityRequire2fa = "security.require_2fa",
        SecurityRequireSso = "security.require_sso",
        SecuritySsoDelete = "security.sso_delete",
        SecuritySsoDomainVerify = "security.sso_domain_verify",
        SecuritySsoUpdate = "security.sso_update",
        ServiceAccountArchive = "service_account.archive",
        ServiceAccountCreate = "service_account.create",
        ServiceAccountUpdate = "service_account.update",
        User2faReset = "user.2fa_reset",
        UserCreateByInvite = "user.create_by_invite",
        UserCreateBySso = "user.create_by_sso",
        UserDeactivate = "user.deactivate",
        UserDelete = "user.delete",
        UserEmailChange = "user.email_change",
        UserEmailChangeCancel = "user.email_change_cancel",
        UserEmailChangeRequest = "user.email_change_request",
        UserInstanceAdmin = "user.instance_admin",
        UserNotifications = "user.notifications",
        UserPasswordChange = "user.password_change",
        UserPasswordReset = "user.password_reset",
        UserReactivate = "user.reactivate",
        UserRecoveryCodesRegenerate = "user.recovery_codes_regenerate",
        UserSessionsRevoke = "user.sessions_revoke",
        UserSignInConfirm = "user.sign_in_confirm",
        UserSignInLink = "user.sign_in_link",
        UserSignup = "user.signup",
        UserSsoLink = "user.sso_link",
        UserSsoUnlink = "user.sso_unlink",
        UserTotpDisable = "user.totp_disable",
        UserTotpEnable = "user.totp_enable",
        UserUpdate = "user.update",
    ];
}

#[derive(Debug)]
#[must_use = "an audit event must be written with `write`"]
pub struct Event<'c> {
    ctx: &'c Ctx,
    org: Option<OrgId>,
    project: Option<ProjectId>,
    action: Action,
    target: Option<(&'static str, String)>,
    before: Option<Value>,
    after: Option<Value>,
    actor: Option<UserId>,
}

impl Ctx {
    pub fn audit(&self, org: Option<OrgId>, action: Action) -> Event<'_> {
        let project = match (&self.project, org) {
            (Some(p), Some(o)) if p.project.org_id == o.uuid() => Some(p.id()),
            _ => None,
        };
        Event {
            ctx: self,
            org,
            project,
            action,
            target: None,
            before: None,
            after: None,
            actor: None,
        }
    }
}

impl Event<'_> {
    pub fn target(mut self, kind: &'static str, id: impl ToString) -> Self {
        self.target = Some((kind, id.to_string()));
        self
    }

    pub fn project(mut self, project: ProjectId) -> Self {
        self.project = Some(project);
        self
    }

    pub fn by(mut self, user: UserId) -> Self {
        self.actor = Some(user);
        self
    }

    pub fn before(mut self, v: impl Serialize) -> Self {
        self.before = snapshot(self.action.as_str(), "before", v);
        self
    }

    pub fn after(mut self, v: impl Serialize) -> Self {
        self.after = snapshot(self.action.as_str(), "after", v);
        self
    }

    pub async fn write(self, db: &mut sqlx::PgConnection) -> ApiResult<()> {
        let (actor_kind, actor_id) = match (&self.actor, &self.ctx.caller) {
            (Some(user), _) => ("user", Some(user.uuid())),
            (None, Caller::Session(s)) => ("user", Some(s.user_id)),
            (None, Caller::ProjectKey(k)) => ("api_key", Some(k.id)),
            (None, Caller::OrgKey(k)) => ("org_platform_key", Some(k.id)),
            (None, Caller::Anonymous) => ("system", None),
        };
        audit::insert(
            db,
            &NewEvent {
                id: AuditEventId::new().uuid(),
                org_id: self.org.map(|o| o.uuid()),
                project_id: self.project.map(|p| p.uuid()),
                actor_kind,
                actor_id,
                action: self.action.as_str(),
                target_kind: self.target.as_ref().map(|(k, _)| *k),
                target_id: self.target.as_ref().map(|(_, id)| id.as_str()),
                before: self.before,
                after: self.after,
                ip: self.ctx.ip,
                user_agent: self.ctx.user_agent.as_deref(),
                request_id: Some(self.ctx.request_id.uuid()),
                at: self.ctx.now,
            },
        )
        .await?;
        Ok(())
    }
}

fn snapshot(action: &'static str, side: &'static str, v: impl Serialize) -> Option<Value> {
    serde_json::to_value(v)
        .inspect_err(|e| {
            tracing::error!(error = %e, action, side, "audit snapshot could not be serialized");
        })
        .ok()
}

pub async fn system(
    db: &mut sqlx::PgConnection,
    org: Option<OrgId>,
    action: Action,
    target: Option<(&'static str, String)>,
    after: Option<Value>,
    at: time::OffsetDateTime,
) -> ApiResult<()> {
    system_change(db, org, action, target, None, after, at).await
}

pub async fn system_change(
    db: &mut sqlx::PgConnection,
    org: Option<OrgId>,
    action: Action,
    target: Option<(&'static str, String)>,
    before: Option<Value>,
    after: Option<Value>,
    at: time::OffsetDateTime,
) -> ApiResult<()> {
    system_event(db, org, action, target, before, after, at).await?;
    Ok(())
}

pub async fn system_event(
    db: &mut sqlx::PgConnection,
    org: Option<OrgId>,
    action: Action,
    target: Option<(&'static str, String)>,
    before: Option<Value>,
    after: Option<Value>,
    at: time::OffsetDateTime,
) -> Result<(), kaeso_store::StoreError> {
    audit::insert(
        db,
        &NewEvent {
            id: AuditEventId::new().uuid(),
            org_id: org.map(|o| o.uuid()),
            project_id: None,
            actor_kind: "system",
            actor_id: None,
            action: action.as_str(),
            target_kind: target.as_ref().map(|(k, _)| *k),
            target_id: target.as_ref().map(|(_, id)| id.as_str()),
            before,
            after,
            ip: None,
            user_agent: None,
            request_id: None,
            at,
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::{ACTIONS, snapshot};
    use serde::{Serialize, Serializer};

    struct Unserializable;

    impl Serialize for Unserializable {
        fn serialize<S: Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("no snapshot"))
        }
    }

    #[test]
    fn actions_are_sorted_and_named_kind_dot_verb() {
        assert!(
            ACTIONS.windows(2).all(|w| w[0] < w[1]),
            "ACTIONS must be sorted and unique"
        );
        for action in ACTIONS {
            let (kind, verb) = action.split_once('.').unwrap_or_default();
            assert!(
                !kind.is_empty() && !verb.is_empty() && !verb.contains('.'),
                "{action}"
            );
        }
    }

    #[test]
    fn snapshots_that_cannot_be_serialized_are_left_out_not_written_as_null() {
        assert_eq!(snapshot("org.update", "before", Unserializable), None);
        assert_eq!(
            snapshot("org.update", "after", serde_json::json!({ "name": "Acme" })),
            Some(serde_json::json!({ "name": "Acme" }))
        );
    }
}
