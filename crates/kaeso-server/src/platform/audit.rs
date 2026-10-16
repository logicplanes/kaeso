use kaeso_core::id::{AuditEventId, OrgId, ProjectId, UserId};
use kaeso_store::pg::audit::{self, NewEvent};
use serde_json::Value;

use super::access::{Caller, Ctx};
use super::error::ApiResult;

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

    pub fn before(mut self, v: Value) -> Self {
        self.before = Some(v);
        self
    }

    pub fn after(mut self, v: Value) -> Self {
        self.after = Some(v);
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
    .await?;
    Ok(())
}
