use kaeso_core::id::{AuditEventId, OrgId, ProjectId, UserId};
use kaeso_store::pg::audit::{self, NewEvent};
use serde::Serialize;
use serde_json::Value;

use super::access::{Caller, Ctx};
use super::error::ApiResult;

pub const ACTIONS: &[&str] = &[
    "alias.create",
    "alias.delete",
    "alias.update",
    "audit_log.export",
    "catalogue_model.check",
    "catalogue_model.create",
    "catalogue_model.delete",
    "catalogue_model.price_change",
    "catalogue_model.update",
    "credential.assign",
    "credential.create",
    "credential.default",
    "credential.delete",
    "credential.rename",
    "credential.replace",
    "instance.invite",
    "instance.settings_update",
    "instance.setup",
    "instance.update_check",
    "instance.update_finish",
    "instance.update_settings",
    "instance.update_start",
    "invite.accept",
    "invite.create",
    "invite.resend",
    "invite.revoke",
    "key.archive",
    "key.create",
    "key.disable",
    "key.enable",
    "limit.create",
    "limit.delete",
    "limit.threshold_reached",
    "limit.update",
    "logs.delete",
    "logs.export",
    "logs.project_content_logging",
    "logs.settings_update",
    "member.join_by_sso",
    "member.remove",
    "member.rights_update",
    "model_access.update",
    "models_dev.model_delisted",
    "models_dev.price_change",
    "models_dev.sync",
    "org.create",
    "org.delete_final",
    "org.delete_now",
    "org.owner_appointed",
    "org.owner_appointed_by_instance_admin",
    "org.ownership_transfer",
    "org.restore",
    "org.schedule_deletion",
    "org.update",
    "platform_key.archive",
    "platform_key.create",
    "platform_key.disable",
    "platform_key.enable",
    "project.archive",
    "project.create",
    "project.default",
    "project.key_policy",
    "project.member_add",
    "project.member_remove",
    "project.member_rights_update",
    "project.update",
    "role.create",
    "role.delete",
    "role.update",
    "security.key_policy",
    "security.require_2fa",
    "security.require_sso",
    "security.sso_delete",
    "security.sso_domain_verify",
    "security.sso_update",
    "service_account.archive",
    "service_account.create",
    "service_account.update",
    "user.2fa_reset",
    "user.create_by_invite",
    "user.create_by_sso",
    "user.deactivate",
    "user.delete",
    "user.email_change",
    "user.email_change_cancel",
    "user.email_change_request",
    "user.instance_admin",
    "user.notifications",
    "user.password_change",
    "user.password_reset",
    "user.reactivate",
    "user.recovery_codes_regenerate",
    "user.sessions_revoke",
    "user.sign_in_confirm",
    "user.sign_in_link",
    "user.signup",
    "user.sso_link",
    "user.sso_unlink",
    "user.totp_disable",
    "user.totp_enable",
    "user.update",
];

#[derive(Debug)]
#[must_use = "an audit event must be written with `write`"]
pub struct Event<'c> {
    ctx: &'c Ctx,
    org: Option<OrgId>,
    project: Option<ProjectId>,
    action: &'static str,
    target: Option<(&'static str, String)>,
    before: Option<Value>,
    after: Option<Value>,
    actor: Option<UserId>,
}

impl Ctx {
    pub fn audit(&self, org: Option<OrgId>, action: &'static str) -> Event<'_> {
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
        self.before = snapshot(self.action, "before", v);
        self
    }

    pub fn after(mut self, v: impl Serialize) -> Self {
        self.after = snapshot(self.action, "after", v);
        self
    }

    pub async fn write(self, db: &mut sqlx::PgConnection) -> ApiResult<()> {
        debug_assert_listed(self.action);
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
                action: self.action,
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

fn debug_assert_listed(action: &str) {
    debug_assert!(
        ACTIONS.binary_search(&action).is_ok(),
        "audit action `{action}` isn't in ACTIONS, so the console has no label for it"
    );
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
    action: &'static str,
    target: Option<(&'static str, String)>,
    after: Option<Value>,
    at: time::OffsetDateTime,
) -> ApiResult<()> {
    system_change(db, org, action, target, None, after, at).await
}

pub async fn system_change(
    db: &mut sqlx::PgConnection,
    org: Option<OrgId>,
    action: &'static str,
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
    action: &'static str,
    target: Option<(&'static str, String)>,
    before: Option<Value>,
    after: Option<Value>,
    at: time::OffsetDateTime,
) -> Result<(), kaeso_store::StoreError> {
    debug_assert_listed(action);
    audit::insert(
        db,
        &NewEvent {
            id: AuditEventId::new().uuid(),
            org_id: org.map(|o| o.uuid()),
            project_id: None,
            actor_kind: "system",
            actor_id: None,
            action,
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
