use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{InviteId, OrgId, ProjectId, RoleId, UserId};
use kaeso_core::rights::{
    OrgGrant, OrgPreset, ProjectGrant, ProjectPreset, ProjectRights, effective_project_rights,
};
use kaeso_core::secret;
use kaeso_crypto::random_token;
use kaeso_mail::Actor;
use kaeso_store::pg::invites::{self, Invite, NewInvite};
use kaeso_store::pg::{members, projects, users};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use utoipa::{IntoParams, ToSchema};

use crate::platform::access::{Ctx, OrgScope};
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::grants::{
    RoleBook, RoleRef, check_org, check_project, org_to_store, project_from_store, project_to_store,
};
use crate::platform::json::Body;
use crate::platform::pagination::{List, PageParams};
use crate::platform::params::{Path, Query};
use crate::platform::validate::validate_email;
use crate::services::invites::ProjectRole;
use crate::state::AppState;

pub const INVITE_LIFETIME: Duration = Duration::days(21);

const MAX_PROJECTS: usize = 100;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A project an invitee joins, with its role.")]
pub struct InviteProjectRole {
    #[doc = "The project."]
    pub project_id: ProjectId,
    #[doc = "The project role."]
    pub role: RoleRef,
    #[doc = "Deprecated: the built-in role's name, `custom` for a custom role."]
    #[schema(example = "developer", deprecated)]
    pub preset: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An invitation.")]
pub struct InviteView {
    #[doc = "Id."]
    pub id: InviteId,
    #[doc = "The invited address."]
    #[schema(example = "ada@example.com")]
    pub email: String,
    #[doc = "The org role they get."]
    pub role: Option<RoleRef>,
    #[doc = "Deprecated: the org role's built-in name, `custom` for a custom role."]
    #[schema(example = "developer", deprecated)]
    pub preset: Option<String>,
    #[doc = "The projects they join besides the default project, with roles."]
    pub project_roles: Vec<InviteProjectRole>,
    #[doc = "Who invited."]
    pub invited_by: Option<UserId>,
    #[doc = "`pending`, `accepted`, `revoked` or `expired`."]
    #[schema(example = "pending")]
    pub status: &'static str,
    #[doc = "When it expires."]
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    #[doc = "Created."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl InviteView {
    pub fn from_row(i: &Invite, now: OffsetDateTime, book: &RoleBook) -> InviteView {
        let status = if i.accepted_at.is_some() {
            "accepted"
        } else if i.revoked_at.is_some() {
            "revoked"
        } else if i.expires_at <= now {
            "expired"
        } else {
            "pending"
        };
        let named = |id: Option<uuid::Uuid>| {
            RoleRef::custom(id, id.and_then(|id| book.get(id)).map(|r| r.name.as_str()))
        };
        let role = i
            .org_preset
            .as_deref()
            .map(|p| match p.parse::<OrgPreset>() {
                Ok(p) => RoleRef::org_builtin(p),
                Err(_) => named(i.org_role_id),
            });
        let stored: Vec<ProjectRole> =
            serde_json::from_value(i.project_roles.clone()).unwrap_or_default();
        InviteView {
            id: InviteId::from_uuid(i.id),
            email: i.email.clone(),
            role,
            preset: i.org_preset.clone(),
            project_roles: stored
                .into_iter()
                .map(|r| InviteProjectRole {
                    project_id: r.project_id,
                    role: match r.preset.parse::<ProjectPreset>() {
                        Ok(p) => RoleRef::project_builtin(p),
                        Err(_) => named(r.role_id.map(|id| id.uuid())),
                    },
                    preset: r.preset,
                })
                .collect(),
            invited_by: i.invited_by.map(UserId::from_uuid),
            status,
            expires_at: i.expires_at,
            created_at: i.created_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "A project to join, with its role.")]
pub struct InviteProjectRequest {
    #[doc = "An active project of the org."]
    pub project_id: ProjectId,
    #[doc = "The project role: `admin`, `developer`, `viewer` or a custom project\n\
        role's id."]
    #[schema(example = "developer")]
    pub role: Option<String>,
    #[doc = "Deprecated: the built-in project role; send `role` instead."]
    #[schema(deprecated)]
    pub preset: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "A new invitation.")]
pub struct InviteRequest {
    #[doc = "The address to invite."]
    #[schema(example = "ada@example.com")]
    pub email: String,
    #[doc = "The org role: a built-in role's name (`owner`, `admin`, `billing`,\n\
        `developer`, `auditor`, `member`) or a custom org role's id. Only a\n\
        role whose rights the inviter holds, and whose project role in the\n\
        default project the inviter could grant there unless `project_roles`\n\
        names it; only owners invite owners."]
    #[schema(example = "developer")]
    pub role: Option<String>,
    #[doc = "Deprecated: the built-in org role; send `role` instead."]
    #[schema(deprecated)]
    pub preset: Option<String>,
    #[doc = "Projects to join besides the default project, with project roles."]
    #[serde(default)]
    pub project_roles: Vec<InviteProjectRequest>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct InviteFilter {
    #[doc = "Only pending invites (default true)."]
    pub pending: Option<bool>,
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/invites",
    summary = "Invites someone by email. The invitation expires after 21 days.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = InviteRequest,
    responses((status = 201, body = InviteView))
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<InviteRequest>,
) -> ApiResult<(StatusCode, Json<InviteView>)> {
    let scope = ctx.org()?;
    let email = validate_email("email", &req.email)?;
    let book = RoleBook::load(&state.pg, scope.org.id).await?;
    let role_name = req
        .role
        .as_deref()
        .or(req.preset.as_deref())
        .ok_or_else(|| ApiError::invalid("role", "`role` is required."))?;
    let (grant, role) = book.org_role(role_name, "role")?;
    check_org_role(scope, &grant)?;
    let stored = org_to_store(&grant);
    if req.project_roles.len() > MAX_PROJECTS {
        return Err(ApiError::invalid(
            "project_roles",
            format!("An invitation can name at most {MAX_PROJECTS} projects."),
        ));
    }
    let mut roles = Vec::new();
    for (i, wanted) in req.project_roles.iter().enumerate() {
        if req
            .project_roles
            .iter()
            .take(i)
            .any(|p| p.project_id == wanted.project_id)
        {
            return Err(ApiError::invalid(
                &format!("project_roles[{i}]"),
                "Each project can be named only once.",
            ));
        }
        let param = format!("project_roles[{i}]");
        let name = wanted
            .role
            .as_deref()
            .or(wanted.preset.as_deref())
            .ok_or_else(|| ApiError::invalid(&param, "Each project needs a `role`."))?;
        let (project_grant, _) = book.project_role(name, &param)?;
        let project = projects::get(&state.pg, scope.org.id, wanted.project_id.uuid())
            .await?
            .filter(|p| p.archived_at.is_none())
            .ok_or_else(|| {
                ApiError::invalid(&param, "No such active project in this organization.")
            })?;
        let caller_rights = caller_project_rights(&state, &ctx, project.id).await?;
        check_project(caller_rights, None, &project_grant)?;
        let p = project_to_store(&project_grant);
        roles.push(ProjectRole {
            project_id: ProjectId::from_uuid(project.id),
            preset: p.preset,
            role_id: p.role_id.map(RoleId::from_uuid),
        });
    }
    let named: Vec<uuid::Uuid> = roles.iter().map(|r| r.project_id.uuid()).collect();
    check_default_join(&state, &ctx, &grant, &named).await?;
    if let Some(user) = users::get_by_email(&state.pg, &email).await?
        && members::get_org(&state.pg, scope.org.id, user.id)
            .await?
            .is_some()
    {
        return Err(ApiError::Conflict(format!(
            "{email} is already a member of this organization."
        )));
    }
    let token = random_token();
    let id = InviteId::new();
    let roles_json = serde_json::to_value(&roles).map_err(ApiError::internal)?;
    let mut tx = state.pg.begin().await?;
    invites::insert(
        &mut *tx,
        &NewInvite {
            id: id.uuid(),
            org_id: Some(scope.org.id),
            email: &email,
            org_preset: Some(stored.preset.as_str()),
            org_role_id: stored.role_id,
            project_roles: roles_json.clone(),
            token_hash: &secret::hash(&token),
            invited_by: ctx.acting_user().map(|u| u.uuid()),
            expires_at: ctx.now + INVITE_LIFETIME,
        },
    )
    .await?;
    ctx.audit(Some(scope.id()), "invite.create")
        .target("invite", id)
        .after(serde_json::json!({ "email": email, "role": role, "project_roles": roles_json }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    send(
        &state,
        &ctx,
        &scope.org.name,
        &email,
        email_role(&role),
        &token,
    )
    .await?;
    let row = invites::get(&state.pg, Some(scope.org.id), id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("invite"))?;
    Ok((
        StatusCode::CREATED,
        Json(InviteView::from_row(&row, ctx.now, &book)),
    ))
}

fn check_org_role(scope: &OrgScope, grant: &OrgGrant) -> ApiResult<()> {
    if grant.is_owner() && !scope.is_owner() {
        return Err(ApiError::PermissionDenied(
            "Only owners can invite owners.".into(),
        ));
    }
    check_org(scope, None, grant)
}

async fn check_resend(
    state: &AppState,
    ctx: &Ctx,
    invite: &Invite,
    book: &RoleBook,
) -> ApiResult<()> {
    let scope = ctx.org()?;
    let org_role = match (invite.org_preset.as_deref(), invite.org_role_id) {
        (Some("custom"), Some(id)) if book.get(id).is_some() => RoleId::from_uuid(id).to_string(),
        (Some(p), _) if p.parse::<OrgPreset>().is_ok() => p.to_owned(),
        _ => OrgPreset::Member.as_str().to_owned(),
    };
    let (grant, _) = book.org_role(&org_role, "role")?;
    check_org_role(scope, &grant)?;
    let stored: Vec<ProjectRole> =
        serde_json::from_value(invite.project_roles.clone()).unwrap_or_default();
    let named: Vec<uuid::Uuid> = stored.iter().map(|r| r.project_id.uuid()).collect();
    check_default_join(state, ctx, &grant, &named).await?;
    for wanted in stored {
        let name = match wanted.role_id {
            Some(id) if book.get(id.uuid()).is_none() => continue,
            Some(id) => id.to_string(),
            None => wanted.preset.clone(),
        };
        let Ok((project_grant, _)) = book.project_role(&name, "project_roles") else {
            continue;
        };
        let Some(project) = projects::get(&state.pg, scope.org.id, wanted.project_id.uuid())
            .await?
            .filter(|p| p.archived_at.is_none())
        else {
            continue;
        };
        let caller_rights = caller_project_rights(state, ctx, project.id).await?;
        check_project(caller_rights, None, &project_grant)?;
    }
    Ok(())
}

fn email_role(role: &RoleRef) -> &str {
    if role.kind == "builtin" {
        &role.id
    } else {
        &role.name
    }
}

pub(crate) async fn check_default_join(
    state: &AppState,
    ctx: &Ctx,
    grant: &OrgGrant,
    named: &[uuid::Uuid],
) -> ApiResult<()> {
    let Some(default) = projects::default_of(&state.pg, ctx.org()?.org.id).await? else {
        return Ok(());
    };
    if named.contains(&default.id) {
        return Ok(());
    }
    let caller_rights = caller_project_rights(state, ctx, default.id).await?;
    let joins = ProjectGrant::preset(grant.default_project_preset());
    check_project(caller_rights, None, &joins)
}

pub(crate) async fn caller_project_rights(
    state: &AppState,
    ctx: &Ctx,
    project: uuid::Uuid,
) -> ApiResult<ProjectRights> {
    let scope = ctx.org()?;
    let own = match ctx.acting_user() {
        Some(u) => members::get_project(&state.pg, project, u.uuid())
            .await?
            .map(|m| project_from_store(&m.grant()))
            .transpose()?,
        None => None,
    };
    Ok(match &scope.grant {
        Some(g) => effective_project_rights(Some(g), own.as_ref()),
        None => scope.every_project,
    })
}

async fn send(
    state: &AppState,
    ctx: &Ctx,
    org_name: &str,
    email: &str,
    preset: &str,
    token: &str,
) -> ApiResult<()> {
    let (name, inviter_lang) = match ctx.acting_user() {
        Some(u) => (
            users::get(&state.pg, u.uuid()).await?.and_then(|u| u.name),
            crate::services::mail_lang::chosen(&state.pg, u.uuid()).await,
        ),
        None => (None, None),
    };
    let inviter = name.as_ref().map_or(Actor::OrgAdmin(org_name), Actor::from);
    let lang = crate::services::mail_lang::of_address(
        &state.pg,
        email,
        inviter_lang.as_deref(),
        ctx.accept_language.as_deref(),
    )
    .await;
    let url = format!("{}invite#token={token}", state.config.public_url);
    state.mailer.send(kaeso_mail::templates::invitation(
        lang, email, inviter, org_name, preset, &url,
    ));
    Ok(())
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/invites",
    summary = "The org's invitations.",
    tag = "orgs",
    params(("org" = OrgId, Path), PageParams, InviteFilter),
    responses((status = 200, body = List<InviteView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Query(page): Query<PageParams>,
    Query(filter): Query<InviteFilter>,
) -> ApiResult<Json<List<InviteView>>> {
    let page = page.page()?;
    let rows = invites::list(
        &state.pg,
        Some(ctx.org()?.org.id),
        filter.pending.unwrap_or(true),
        ctx.now,
        page.after,
        page.fetch(),
    )
    .await?;
    let now = ctx.now;
    let book = RoleBook::load(&state.pg, ctx.org()?.org.id).await?;
    Ok(Json(List::from_rows(
        &page,
        rows,
        |i| i.id,
        |i| InviteView::from_row(&i, now, &book),
    )))
}

fn invite_param(invite: &str) -> ApiResult<InviteId> {
    invite.parse().map_err(|_| ApiError::not_found("invite"))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/invites/{invite}/resend",
    summary = "Sends a pending invitation again, with a new link valid 21 days. Only a\n\
    caller who could send the invitation now can resend it.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("invite" = InviteId, Path)),
    responses((status = 200, body = InviteView))
)]
pub async fn resend(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, invite)): Path<(String, String)>,
) -> ApiResult<Json<InviteView>> {
    let scope = ctx.org()?;
    let id = invite_param(&invite)?;
    let book = RoleBook::load(&state.pg, scope.org.id).await?;
    let pending = invites::get(&state.pg, Some(scope.org.id), id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("pending invite"))?;
    check_resend(&state, &ctx, &pending, &book).await?;
    let token = random_token();
    let mut tx = state.pg.begin().await?;
    if !invites::renew(
        &mut *tx,
        Some(scope.org.id),
        id.uuid(),
        &secret::hash(&token),
        ctx.now + INVITE_LIFETIME,
    )
    .await?
    {
        return Err(ApiError::not_found("pending invite"));
    }
    ctx.audit(Some(scope.id()), "invite.resend")
        .target("invite", id)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let row = invites::get(&state.pg, Some(scope.org.id), id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("invite"))?;
    let view = InviteView::from_row(&row, ctx.now, &book);
    let role = view
        .role
        .as_ref()
        .map_or(OrgPreset::Member.as_str(), email_role);
    send(&state, &ctx, &scope.org.name, &row.email, role, &token).await?;
    Ok(Json(view))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/invites/{invite}",
    summary = "Revokes a pending invitation; its link stops working.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("invite" = InviteId, Path)),
    responses((status = 204, description = "Revoked."))
)]
pub async fn revoke(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, invite)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    let scope = ctx.org()?;
    let id = invite_param(&invite)?;
    let mut tx = state.pg.begin().await?;
    if !invites::revoke(&mut *tx, Some(scope.org.id), id.uuid(), ctx.now).await? {
        return Err(ApiError::not_found("pending invite"));
    }
    ctx.audit(Some(scope.id()), "invite.revoke")
        .target("invite", id)
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/invites/{invite}",
    summary = "One invitation.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("invite" = InviteId, Path)),
    responses((status = 200, body = InviteView))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, invite)): Path<(String, String)>,
) -> ApiResult<Json<InviteView>> {
    let id = invite_param(&invite)?;
    let row = invites::get(&state.pg, Some(ctx.org()?.org.id), id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("invite"))?;
    let book = RoleBook::load(&state.pg, ctx.org()?.org.id).await?;
    Ok(Json(InviteView::from_row(&row, ctx.now, &book)))
}
