use axum::body::Body as HttpBody;
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use futures::stream::{self, StreamExt};
use kaeso_core::id::{AgentId, AuditEventId, OrgId, ProjectId, RequestId, UserId};
use kaeso_store::pg::audit::{self, Event, Filter};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::error::{ApiError, ApiResult};
use super::pagination::{self, List};

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Who acted.")]
pub struct ActorView {
    #[doc = "`user`, `agent` (an agent acting for the user in `id`),\n\
        `org_platform_key`, `api_key` or `system`."]
    #[schema(example = "user")]
    pub kind: String,
    #[doc = "The actor's id (`usr_`, `opk_` or `key_`); for `agent`, the user the\n\
        agent acts for; `null` for the system."]
    #[schema(example = "usr_01k7a3f8m2xq4v9c5r6t8w0yz1")]
    pub id: Option<String>,
    #[doc = "The agent, for `agent`."]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentRef>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An agent as an audit event names it.")]
pub struct AgentRef {
    #[doc = "The agent."]
    pub id: AgentId,
    #[doc = "Its name; `null` once its user is deleted."]
    #[schema(example = "Release helper")]
    pub name: Option<String>,
    #[doc = "The model family it reported, such as `claude`, `gpt` or `unknown`;\n\
        `null` once its user is deleted."]
    #[schema(example = "claude")]
    pub model: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An audit event.")]
pub struct AuditEventView {
    #[doc = "Id."]
    pub id: AuditEventId,
    #[doc = "When."]
    #[serde(with = "time::serde::rfc3339")]
    pub at: OffsetDateTime,
    #[doc = "The org, for org events."]
    pub org_id: Option<OrgId>,
    #[doc = "The project the event is about, if any."]
    pub project_id: Option<ProjectId>,
    #[doc = "Who acted."]
    pub actor: ActorView,
    #[doc = "What happened, e.g. `project.create`."]
    #[schema(example = "project.create")]
    pub action: String,
    #[doc = "The kind of the target, e.g. `project`."]
    pub target_kind: Option<String>,
    #[doc = "The target's id."]
    pub target_id: Option<String>,
    #[doc = "State before, where it applies. Never contains secrets."]
    #[schema(value_type = Option<Object>)]
    pub before: Option<serde_json::Value>,
    #[doc = "State after, where it applies. Never contains secrets."]
    #[schema(value_type = Option<Object>)]
    pub after: Option<serde_json::Value>,
    #[doc = "Client address."]
    pub ip: Option<String>,
    #[doc = "Client user agent."]
    pub user_agent: Option<String>,
    #[doc = "The request that made the change."]
    pub request_id: Option<RequestId>,
}

impl AuditEventView {
    pub fn from_row(e: Event) -> AuditEventView {
        let actor_id = e.actor_id.map(|id| match e.actor_kind.as_str() {
            "user" | "agent" => UserId::from_uuid(id).to_string(),
            "api_key" => kaeso_core::id::ApiKeyId::from_uuid(id).to_string(),
            "org_platform_key" => kaeso_core::id::OrgPlatformKeyId::from_uuid(id).to_string(),
            _ => id.to_string(),
        });
        AuditEventView {
            id: AuditEventId::from_uuid(e.id),
            at: e.at,
            org_id: e.org_id.map(OrgId::from_uuid),
            project_id: e.project_id.map(ProjectId::from_uuid),
            actor: ActorView {
                kind: e.actor_kind,
                id: actor_id,
                agent: e.agent_id.map(|id| AgentRef {
                    id: AgentId::from_uuid(id),
                    name: e.agent_name,
                    model: e.agent_model,
                }),
            },
            action: e.action,
            target_kind: e.target_kind,
            target_id: e.target_id,
            before: e.before,
            after: e.after,
            ip: e.ip.map(|n| n.addr().to_string()),
            user_agent: e.user_agent,
            request_id: e.request_id.map(RequestId::from_uuid),
        }
    }
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AuditFilter {
    #[doc = "Only events by this user (`usr_…`)."]
    pub actor: Option<UserId>,
    #[doc = "Only actions starting with this, e.g. `project.` or `member.remove`."]
    pub action: Option<String>,
    #[doc = "Only events about this target id."]
    pub target: Option<String>,
    #[doc = "Only events about this project (`proj_…`); always the project of the\n\
        path in a project's audit log."]
    pub project: Option<ProjectId>,
    #[doc = "Only events done by agents (`true`)."]
    pub by_agents: Option<bool>,
    #[doc = "Not before this time (RFC 3339)."]
    #[serde(default, deserialize_with = "super::params::rfc3339")]
    #[param(value_type = Option<String>)]
    pub from: Option<OffsetDateTime>,
    #[doc = "Before this time (RFC 3339)."]
    #[serde(default, deserialize_with = "super::params::rfc3339")]
    #[param(value_type = Option<String>)]
    pub to: Option<OffsetDateTime>,
}

impl AuditFilter {
    fn store(&self) -> Filter<'_> {
        Filter {
            actor_id: self.actor.map(|a| a.uuid()),
            action_prefix: self.action.as_deref(),
            target_id: self.target.as_deref(),
            project_id: self.project.map(|p| p.uuid()),
            agents_only: self.by_agents.unwrap_or(false),
            from: self.from,
            to: self.to,
        }
    }
}

pub async fn page(
    pg: &PgPool,
    org: Option<OrgId>,
    page: pagination::Page,
    filter: &AuditFilter,
) -> ApiResult<List<AuditEventView>> {
    let rows = audit::list(
        pg,
        org.map(|o| o.uuid()),
        page.after,
        page.fetch(),
        &filter.store(),
    )
    .await?;
    Ok(List::from_rows(
        &page,
        rows,
        |e| e.id,
        AuditEventView::from_row,
    ))
}

pub fn export(pg: PgPool, org: Option<OrgId>, filter: AuditFilter) -> ApiResult<Response> {
    let state = (pg, org, filter, None::<Uuid>, false);
    let body = stream::unfold(state, |(pg, org, filter, before, done)| async move {
        if done {
            return None;
        }
        let rows = audit::list(&pg, org.map(|o| o.uuid()), before, 1000, &filter.store()).await;
        match rows {
            Ok(rows) => {
                let last = rows.last().map(|e| e.id);
                let finished = rows.len() < 1000;
                let mut chunk = Vec::new();
                for e in rows {
                    if serde_json::to_writer(&mut chunk, &AuditEventView::from_row(e)).is_ok() {
                        chunk.push(b'\n');
                    }
                }
                Some((
                    Ok::<_, std::io::Error>(chunk),
                    (pg, org, filter, last, finished || last.is_none()),
                ))
            }
            Err(e) => Some((
                Err(std::io::Error::other(e.to_string())),
                (pg, org, filter, before, true),
            )),
        }
    })
    .filter(|chunk| std::future::ready(!matches!(chunk, Ok(c) if c.is_empty())));
    let mut response = HttpBody::from_stream(body).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/x-ndjson"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"audit-log.jsonl\""),
    );
    Ok(response)
}

pub fn check_range(filter: &AuditFilter) -> ApiResult<()> {
    if let (Some(from), Some(to)) = (filter.from, filter.to)
        && from >= to
    {
        return Err(ApiError::invalid("from", "`from` must be before `to`."));
    }
    Ok(())
}

pub async fn one(pg: &PgPool, org: Option<OrgId>, id: &str) -> ApiResult<AuditEventView> {
    let id: kaeso_core::id::AuditEventId = id
        .parse()
        .map_err(|_| super::error::ApiError::not_found("audit event"))?;
    audit::get(pg, org.map(|o| o.uuid()), id.uuid())
        .await?
        .map(AuditEventView::from_row)
        .ok_or_else(|| super::error::ApiError::not_found("audit event"))
}
