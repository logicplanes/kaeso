use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_core::id::{OrgId, RoleId};
use kaeso_core::rights::{
    Deviation, OrgHolding, OrgPreset, OrgRights, ProjectPreset, ProjectRights,
    check_org_role_holders, check_project_change, check_project_role_holders,
};
use kaeso_store::pg::roles::{self, HolderDeviation, Role, RoleWrite};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};

use crate::platform::access::{Ctx, OrgScope};
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::grants::{parse_org, parse_project};
use crate::platform::json::Body;
use crate::platform::pagination::List;
use crate::platform::params::{Path, Query};
use crate::state::AppState;

pub const MAX_CUSTOM_ROLES: i64 = 50;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A role: built-in or one of the org's custom roles.")]
pub struct RoleView {
    #[doc = "The built-in role's name (`owner`, …) or the custom role's id (`role_…`)."]
    #[schema(example = "role_01k7a3f8m2xq4v9c5r6t8w0yz1")]
    pub id: String,
    #[doc = "`org` or `project`."]
    #[schema(example = "org")]
    pub scope: &'static str,
    #[doc = "`builtin` or `custom`."]
    #[schema(example = "custom")]
    pub kind: &'static str,
    #[doc = "The name shown in the console."]
    #[schema(example = "Support lead")]
    pub name: String,
    #[doc = "What the role is for."]
    #[schema(example = "Reads logs with prompts and manages member limits.")]
    pub description: String,
    #[doc = "Its rights of its scope, in catalogue order."]
    pub rights: Vec<&'static str>,
    #[doc = "Org roles: the project rights it holds in every project; empty for\n\
        project roles."]
    pub project_rights: Vec<&'static str>,
    #[doc = "Members, project members and active service accounts with it, and\n\
        pending invitations naming it."]
    pub holders: i64,
    #[doc = "When a custom role was created; `null` for built-in roles."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
    #[doc = "When a custom role last changed; `null` for built-in roles."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub updated_at: Option<OffsetDateTime>,
}

fn org_builtin(p: OrgPreset, holders: i64) -> RoleView {
    RoleView {
        id: p.as_str().into(),
        scope: "org",
        kind: "builtin",
        name: p.name().into(),
        description: p.description().into(),
        rights: p.rights().names(),
        project_rights: p.project_rights().names(),
        holders,
        created_at: None,
        updated_at: None,
    }
}

fn project_builtin(p: ProjectPreset, holders: i64) -> RoleView {
    RoleView {
        id: p.as_str().into(),
        scope: "project",
        kind: "builtin",
        name: p.name().into(),
        description: p.description().into(),
        rights: p.rights().names(),
        project_rights: Vec::new(),
        holders,
        created_at: None,
        updated_at: None,
    }
}

fn custom_view(r: &Role, extra_holders: i64) -> ApiResult<RoleView> {
    let (scope, rights) = if r.scope == "org" {
        ("org", parse_org(&r.rights)?.names())
    } else {
        ("project", parse_project(&r.rights)?.names())
    };
    Ok(RoleView {
        id: RoleId::from_uuid(r.id).to_string(),
        scope,
        kind: "custom",
        name: r.name.clone(),
        description: r.description.clone(),
        rights,
        project_rights: parse_project(&r.project_rights)?.names(),
        holders: r.holders + extra_holders,
        created_at: Some(r.created_at),
        updated_at: Some(r.updated_at),
    })
}

async fn invite_holders(state: &AppState, org: uuid::Uuid, role: uuid::Uuid) -> ApiResult<i64> {
    Ok(
        roles::pending_project_invites(&state.pg, org, &RoleId::from_uuid(role).to_string())
            .await?,
    )
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct RoleFilter {
    #[doc = "Only roles of this scope: `org` or `project`."]
    pub scope: Option<String>,
}

fn scope_param(scope: Option<&str>) -> ApiResult<Option<&'static str>> {
    match scope {
        None => Ok(None),
        Some("org") => Ok(Some("org")),
        Some("project") => Ok(Some("project")),
        Some(_) => Err(ApiError::invalid(
            "scope",
            "`scope` must be org or project.",
        )),
    }
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/roles",
    summary = "The org's roles: built-in org and project roles first, then custom roles by scope and name.",
    tag = "orgs",
    params(("org" = OrgId, Path), RoleFilter),
    responses((status = 200, body = List<RoleView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Query(filter): Query<RoleFilter>,
) -> ApiResult<Json<List<RoleView>>> {
    let scope = scope_param(filter.scope.as_deref())?;
    let org = ctx.org()?.org.id;
    let counts = roles::builtin_holders(&state.pg, org).await?;
    let count = |scope: &str, preset: &str| {
        counts
            .iter()
            .find(|(s, p, _)| s == scope && p == preset)
            .map_or(0, |(_, _, n)| *n)
    };
    let mut data = Vec::new();
    if scope != Some("project") {
        data.extend(
            OrgPreset::ALL
                .into_iter()
                .map(|p| org_builtin(p, count("org", p.as_str()))),
        );
    }
    if scope != Some("org") {
        data.extend(
            ProjectPreset::ALL
                .into_iter()
                .map(|p| project_builtin(p, count("project", p.as_str()))),
        );
    }
    let invited: std::collections::HashMap<String, i64> =
        roles::pending_project_invites_by_role(&state.pg, org)
            .await?
            .into_iter()
            .collect();
    for r in roles::list(&state.pg, org).await? {
        if scope.is_none_or(|s| s == r.scope) {
            let extra = invited
                .get(&RoleId::from_uuid(r.id).to_string())
                .copied()
                .unwrap_or(0);
            data.push(custom_view(&r, extra)?);
        }
    }
    Ok(Json(List::complete(data)))
}

async fn custom(state: &AppState, scope: &OrgScope, id: &str) -> ApiResult<Role> {
    let id: RoleId = id.parse().map_err(|_| ApiError::not_found("role"))?;
    roles::get(&state.pg, scope.org.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("role"))
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/roles/{role}",
    summary = "One role. Built-in roles are named by their name; `admin` and\n\
    `developer` exist in both scopes, so `scope` chooses (default `org`).",
    tag = "orgs",
    params(("org" = OrgId, Path), ("role" = String, Path, description = "A built-in role's name or a custom role's id (`role_…`)."), RoleFilter),
    responses((status = 200, body = RoleView))
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, role)): Path<(String, String)>,
    Query(filter): Query<RoleFilter>,
) -> ApiResult<Json<RoleView>> {
    let scope = ctx.org()?;
    let wanted = scope_param(filter.scope.as_deref())?.unwrap_or("org");
    let counts = roles::builtin_holders(&state.pg, scope.org.id).await?;
    let count = |s: &str| {
        counts
            .iter()
            .find(|(sc, p, _)| sc == wanted && p == s)
            .map_or(0, |(_, _, n)| *n)
    };
    if wanted == "org"
        && let Ok(p) = role.parse::<OrgPreset>()
    {
        return Ok(Json(org_builtin(p, count(p.as_str()))));
    }
    if wanted == "project"
        && let Ok(p) = role.parse::<ProjectPreset>()
    {
        return Ok(Json(project_builtin(p, count(p.as_str()))));
    }
    let r = custom(&state, scope, &role).await?;
    let extra = invite_holders(&state, scope.org.id, r.id).await?;
    Ok(Json(custom_view(&r, extra)?))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "A new custom role.")]
pub struct RoleCreate {
    #[doc = "`org` or `project`; fixed once created."]
    #[schema(example = "project")]
    pub scope: String,
    #[doc = "1 to 50 characters, unique in the org and scope, not a built-in\n\
        role's name."]
    #[schema(example = "Support lead")]
    pub name: String,
    #[doc = "What the role is for, up to 200 characters."]
    #[serde(default)]
    pub description: String,
    #[doc = "Rights of the scope; each with the rights it needs."]
    #[schema(example = json!(["project.view", "logs.metadata.view", "logs.content.view"]))]
    pub rights: Vec<String>,
    #[doc = "Org roles only: project rights held in every project."]
    #[serde(default)]
    pub project_rights: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "Changes to a custom role.")]
pub struct RoleUpdate {
    #[doc = "A new name."]
    pub name: Option<String>,
    #[doc = "A new description."]
    pub description: Option<String>,
    #[doc = "The new rights of its scope."]
    pub rights: Option<Vec<String>>,
    #[doc = "Org roles only: the new project rights held in every project."]
    pub project_rights: Option<Vec<String>>,
}

fn holding_of(scope: &str, rights: &[String], project_rights: &[String]) -> ApiResult<OrgHolding> {
    let parse_org_param = |names: &[String]| {
        OrgRights::parse(names.iter().map(String::as_str))
            .map_err(|e| ApiError::invalid("rights", format!("{e}.")))
    };
    let parse_project_param = |names: &[String], param: &str| {
        ProjectRights::parse(names.iter().map(String::as_str))
            .map_err(|e| ApiError::invalid(param, format!("{e}.")))
    };
    if scope == "org" {
        let org = parse_org_param(rights)?;
        if let Some(right) = org.intersect(OrgRights::owner_only()).iter().next() {
            return Err(ApiError::invalid(
                "rights",
                format!("`{right}` is held only by owners, so a role can't include it."),
            ));
        }
        Ok(OrgHolding {
            org,
            every_project: parse_project_param(project_rights, "project_rights")?,
        })
    } else {
        if !project_rights.is_empty() {
            return Err(ApiError::invalid(
                "project_rights",
                "Only org roles have `project_rights`.",
            ));
        }
        Ok(OrgHolding {
            org: OrgRights::EMPTY,
            every_project: parse_project_param(rights, "rights")?,
        })
    }
}

fn check_change(
    caller: &OrgScope,
    scope: &str,
    before: &OrgHolding,
    after: &OrgHolding,
) -> ApiResult<()> {
    if scope == "org" {
        caller.holding().check_change(before, after)?;
    } else {
        check_project_change(
            caller.every_project,
            before.every_project,
            after.every_project,
        )?;
    }
    Ok(())
}

fn check_holders(
    scope: &str,
    before: &OrgHolding,
    after: &OrgHolding,
    holders: &[HolderDeviation],
) -> ApiResult<()> {
    if scope == "org" {
        let holders = holders
            .iter()
            .map(|h| {
                Ok(Deviation {
                    added: parse_org(&h.rights_added)?,
                    removed: parse_org(&h.rights_removed)?,
                })
            })
            .collect::<ApiResult<Vec<_>>>()?;
        check_org_role_holders(before.org, after.org, &holders)?;
    } else {
        let holders = holders
            .iter()
            .map(|h| {
                Ok(Deviation {
                    added: parse_project(&h.rights_added)?,
                    removed: parse_project(&h.rights_removed)?,
                })
            })
            .collect::<ApiResult<Vec<_>>>()?;
        check_project_role_holders(before.every_project, after.every_project, &holders)?;
    }
    Ok(())
}

fn check_holds(caller: &OrgScope, role: &OrgHolding) -> ApiResult<()> {
    if caller.holding().covers(role) {
        Ok(())
    } else {
        Err(ApiError::PermissionDenied(
            "You can't change a role with rights you don't hold.".into(),
        ))
    }
}

fn validate_name(scope: &str, name: &str) -> ApiResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 50 {
        return Err(ApiError::invalid(
            "name",
            "`name` must be 1 to 50 characters.",
        ));
    }
    let lower = name.to_lowercase();
    let builtin = if scope == "org" {
        OrgPreset::ALL.iter().any(|p| p.as_str() == lower)
    } else {
        ProjectPreset::ALL.iter().any(|p| p.as_str() == lower)
    };
    if builtin || lower == "custom" {
        return Err(ApiError::invalid(
            "name",
            "That's the name of a built-in role; choose another.",
        ));
    }
    Ok(name.to_owned())
}

fn validate_description(d: &str) -> ApiResult<String> {
    let d = d.trim();
    if d.chars().count() > 200 {
        return Err(ApiError::invalid(
            "description",
            "`description` must be at most 200 characters.",
        ));
    }
    Ok(d.to_owned())
}

fn name_taken(e: kaeso_store::StoreError) -> ApiError {
    if roles::is_name_taken(&e) {
        ApiError::Conflict("A custom role with this name already exists.".into())
    } else {
        e.into()
    }
}

fn names_of(h: &OrgHolding, scope: &str) -> (Vec<String>, Vec<String>) {
    let own = |v: Vec<&'static str>| v.into_iter().map(str::to_owned).collect::<Vec<_>>();
    if scope == "org" {
        (own(h.org.names()), own(h.every_project.names()))
    } else {
        (own(h.every_project.names()), Vec::new())
    }
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/roles",
    summary = "Creates a custom role with rights the caller holds.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = RoleCreate,
    responses((status = 201, body = RoleView))
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<RoleCreate>,
) -> ApiResult<(StatusCode, Json<RoleView>)> {
    let scope = ctx.org()?;
    let role_scope = match req.scope.as_str() {
        "org" => "org",
        "project" => "project",
        _ => {
            return Err(ApiError::invalid(
                "scope",
                "`scope` must be org or project.",
            ));
        }
    };
    let name = validate_name(role_scope, &req.name)?;
    let description = validate_description(&req.description)?;
    let holding = holding_of(role_scope, &req.rights, &req.project_rights)?;
    check_change(scope, role_scope, &OrgHolding::default(), &holding)?;
    let (rights, project_rights) = names_of(&holding, role_scope);
    let id = RoleId::new();
    let mut tx = state.pg.begin().await?;
    if roles::count(&mut *tx, scope.org.id).await? >= MAX_CUSTOM_ROLES {
        return Err(ApiError::Conflict(format!(
            "An organization can have at most {MAX_CUSTOM_ROLES} custom roles."
        )));
    }
    roles::insert(
        &mut *tx,
        &RoleWrite {
            id: id.uuid(),
            org_id: scope.org.id,
            scope: role_scope,
            name: &name,
            description: &description,
            rights: &rights,
            project_rights: &project_rights,
            created_by: ctx.acting_user().map(|u| u.uuid()),
        },
    )
    .await
    .map_err(name_taken)?;
    ctx.audit(Some(scope.id()), Action::RoleCreate)
        .target("role", id)
        .after(serde_json::json!({
            "scope": role_scope, "name": name, "description": description,
            "rights": rights, "project_rights": project_rights,
        }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let row = roles::get(&state.pg, scope.org.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("role"))?;
    Ok((StatusCode::CREATED, Json(custom_view(&row, 0)?)))
}

fn builtin_refused(role: &str) -> ApiResult<()> {
    if role.parse::<OrgPreset>().is_ok() || role.parse::<ProjectPreset>().is_ok() {
        return Err(ApiError::Conflict(
            "Built-in roles can't be changed; create a custom role instead.".into(),
        ));
    }
    Ok(())
}

#[utoipa::path(
    patch,
    path = "/orgs/{org}/roles/{role}",
    summary = "Changes a custom role. It applies at once to everyone who holds it; the\n\
    caller must hold every right the role has before and after. A change that\n\
    would leave a holder with a right without a right it needs is refused.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("role" = String, Path, description = "A custom role's id (`role_…`).")),
    request_body = RoleUpdate,
    responses((status = 200, body = RoleView))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, role)): Path<(String, String)>,
    Body(req): Body<RoleUpdate>,
) -> ApiResult<Json<RoleView>> {
    builtin_refused(&role)?;
    let scope = ctx.org()?;
    let id: RoleId = role.parse().map_err(|_| ApiError::not_found("role"))?;
    let mut tx = state.pg.begin().await?;
    if !roles::lock(&mut *tx, scope.org.id, id.uuid()).await? {
        return Err(ApiError::not_found("role"));
    }
    let before = roles::get(&mut *tx, scope.org.id, id.uuid())
        .await?
        .ok_or_else(|| ApiError::not_found("role"))?;
    let role_scope = if before.scope == "org" {
        "org"
    } else {
        "project"
    };
    let before_holding = holding_of(role_scope, &before.rights, &before.project_rights)?;
    check_holds(scope, &before_holding)?;
    let name = match &req.name {
        Some(n) => validate_name(role_scope, n)?,
        None => before.name.clone(),
    };
    let description = match &req.description {
        Some(d) => validate_description(d)?,
        None => before.description.clone(),
    };
    let after_holding = holding_of(
        role_scope,
        req.rights.as_ref().unwrap_or(&before.rights),
        req.project_rights
            .as_ref()
            .unwrap_or(&before.project_rights),
    )?;
    check_change(scope, role_scope, &before_holding, &after_holding)?;
    let (rights, project_rights) = names_of(&after_holding, role_scope);
    let holders = roles::holder_deviations(&mut *tx, scope.org.id, before.id).await?;
    check_holders(role_scope, &before_holding, &after_holding, &holders)?;
    roles::update(
        &mut *tx,
        &RoleWrite {
            id: before.id,
            org_id: scope.org.id,
            scope: role_scope,
            name: &name,
            description: &description,
            rights: &rights,
            project_rights: &project_rights,
            created_by: before.created_by,
        },
    )
    .await
    .map_err(name_taken)?;
    let extra = invite_holders(&state, scope.org.id, before.id).await?;
    ctx.audit(Some(scope.id()), Action::RoleUpdate)
        .target("role", RoleId::from_uuid(before.id))
        .before(serde_json::json!({
            "name": before.name, "description": before.description,
            "rights": before.rights, "project_rights": before.project_rights,
        }))
        .after(serde_json::json!({
            "name": name, "description": description,
            "rights": rights, "project_rights": project_rights,
            "holders": before.holders + extra,
        }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    crate::services::keys::forget_org(&state, scope.org.id).await;
    let row = roles::get(&state.pg, scope.org.id, before.id)
        .await?
        .ok_or_else(|| ApiError::not_found("role"))?;
    Ok(Json(custom_view(&row, extra)?))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/roles/{role}",
    summary = "Deletes a custom role nobody holds. The caller must hold all its rights.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("role" = String, Path, description = "A custom role's id (`role_…`).")),
    responses((status = 204, description = "Deleted."))
)]
pub async fn delete(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, role)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    builtin_refused(&role)?;
    let scope = ctx.org()?;
    let before = custom(&state, scope, &role).await?;
    let role_scope = if before.scope == "org" {
        "org"
    } else {
        "project"
    };
    check_holds(
        scope,
        &holding_of(role_scope, &before.rights, &before.project_rights)?,
    )?;
    let mut tx = state.pg.begin().await?;
    if !roles::lock(&mut *tx, scope.org.id, before.id).await? {
        return Err(ApiError::not_found("role"));
    }
    let current = roles::get(&mut *tx, scope.org.id, before.id)
        .await?
        .ok_or_else(|| ApiError::not_found("role"))?;
    let holders = current.holders
        + roles::pending_project_invites(
            &mut *tx,
            scope.org.id,
            &RoleId::from_uuid(before.id).to_string(),
        )
        .await?;
    if holders > 0 {
        return Err(ApiError::Conflict(format!(
            "{holders} members, service accounts or invites hold this role; reassign them first."
        )));
    }
    if scope.org.sso_default_role_id == Some(before.id) {
        return Err(ApiError::Conflict(
            "This role is the default role of SSO sign-ins; choose another one first.".into(),
        ));
    }
    roles::delete(&mut *tx, scope.org.id, before.id).await?;
    ctx.audit(Some(scope.id()), Action::RoleDelete)
        .target("role", RoleId::from_uuid(before.id))
        .before(serde_json::json!({
            "scope": before.scope, "name": before.name, "description": before.description,
            "rights": before.rights, "project_rights": before.project_rights,
        }))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
