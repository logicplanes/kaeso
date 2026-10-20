use kaeso_core::id::RoleId;
use kaeso_core::rights::{
    OrgGrant, OrgHolding, OrgPreset, OrgRights, ProjectGrant, ProjectPreset, ProjectRights,
    check_project_change,
};
use kaeso_store::pg::members::Grant;
use kaeso_store::pg::roles::{self, Role};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::access::OrgScope;
use super::error::{ApiError, ApiResult};

const DELETED_ROLE: &str = "Deleted role";

pub fn requested_role(req: &GrantRequest) -> Option<Uuid> {
    req.role_id()
        .ok()
        .and_then(|r| r.parse::<RoleId>().ok())
        .map(|r| r.uuid())
}

pub async fn lock_roles(
    tx: &mut sqlx::PgConnection,
    org: Uuid,
    ids: &[Option<Uuid>],
) -> ApiResult<()> {
    let ids: Vec<Uuid> = ids.iter().flatten().copied().collect();
    if !ids.is_empty() {
        roles::lock_shared(&mut *tx, org, &ids).await?;
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[schema(description = "A role as other resources name it.")]
pub struct RoleRef {
    #[doc = "The built-in role's name (`owner`, `admin`, `billing`, `developer`,\n\
        `auditor`, `member`; `admin`, `developer`, `viewer` for projects) or\n\
        the custom role's id (`role_…`)."]
    #[schema(example = "developer")]
    pub id: String,
    #[doc = "`builtin` or `custom`."]
    #[schema(example = "builtin")]
    pub kind: &'static str,
    #[doc = "The name shown in the console."]
    #[schema(example = "Developer")]
    pub name: String,
}

impl RoleRef {
    pub fn org_builtin(p: OrgPreset) -> RoleRef {
        RoleRef {
            id: p.as_str().to_owned(),
            kind: "builtin",
            name: p.name().to_owned(),
        }
    }

    pub fn project_builtin(p: ProjectPreset) -> RoleRef {
        RoleRef {
            id: p.as_str().to_owned(),
            kind: "builtin",
            name: p.name().to_owned(),
        }
    }

    pub fn custom(id: Option<Uuid>, name: Option<&str>) -> RoleRef {
        RoleRef {
            id: id.map_or_else(
                || "deleted".to_owned(),
                |u| RoleId::from_uuid(u).to_string(),
            ),
            kind: "custom",
            name: name.unwrap_or(DELETED_ROLE).to_owned(),
        }
    }

    pub fn of_org(g: &Grant) -> RoleRef {
        match g.preset.parse::<OrgPreset>() {
            Ok(p) => RoleRef::org_builtin(p),
            Err(_) => RoleRef::custom(g.role_id, g.role_name.as_deref()),
        }
    }

    pub fn of_project(g: &Grant) -> RoleRef {
        match g.preset.parse::<ProjectPreset>() {
            Ok(p) => RoleRef::project_builtin(p),
            Err(_) => RoleRef::custom(g.role_id, g.role_name.as_deref()),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, ToSchema)]
#[schema(description = "Rights as a role plus individual changes.")]
pub struct GrantRequest {
    #[doc = "The role: a built-in role's name or a custom role's id (`role_…`) of\n\
        the right scope. Org roles: `owner`, `admin`, `billing`, `developer`,\n\
        `auditor`, `member`; project roles: `admin`, `developer`, `viewer`."]
    #[schema(example = "developer")]
    pub role: Option<String>,
    #[doc = "Deprecated: the built-in role's name; send `role` instead."]
    #[schema(deprecated)]
    pub preset: Option<String>,
    #[doc = "Rights added on top of the role."]
    #[serde(default)]
    #[schema(example = json!(["logs.export"]))]
    pub rights_added: Vec<String>,
    #[doc = "Rights removed from the role."]
    #[serde(default)]
    pub rights_removed: Vec<String>,
}

impl GrantRequest {
    pub fn role_id(&self) -> ApiResult<&str> {
        match (&self.role, &self.preset) {
            (Some(r), None) | (None, Some(r)) => Ok(r),
            (Some(_), Some(_)) => Err(ApiError::invalid(
                "role",
                "Send `role` only; `preset` is its old name.",
            )),
            (None, None) => Err(ApiError::invalid("role", "`role` is required.")),
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "Stored rights and what they add up to.")]
pub struct GrantView {
    #[doc = "The role."]
    pub role: RoleRef,
    #[doc = "Deprecated: the built-in role's name; `null` for a custom role."]
    #[schema(example = "developer", deprecated)]
    pub preset: Option<String>,
    #[doc = "Rights added on top of the role."]
    pub rights_added: Vec<&'static str>,
    #[doc = "Rights removed from the role."]
    pub rights_removed: Vec<&'static str>,
    #[doc = "The effective rights: the role's, plus added, minus removed."]
    pub rights: Vec<&'static str>,
    #[doc = "Org grants only: the project rights the org role holds in every\n\
        project of the org."]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_rights: Option<Vec<&'static str>>,
}

#[derive(Clone, Debug, Default)]
pub struct RoleBook {
    roles: Vec<Role>,
}

fn unknown_role(param: &str) -> ApiError {
    ApiError::invalid(param, "No such role in this organization; see its roles.")
}

impl RoleBook {
    pub async fn load(db: impl sqlx::PgExecutor<'_>, org: Uuid) -> ApiResult<RoleBook> {
        Ok(RoleBook {
            roles: roles::list(db, org).await?,
        })
    }

    pub fn get(&self, id: Uuid) -> Option<&Role> {
        self.roles.iter().find(|r| r.id == id)
    }

    fn custom(&self, s: &str, scope: &str, param: &str) -> ApiResult<&Role> {
        let id: RoleId = s.parse().map_err(|_| unknown_role(param))?;
        let role = self.get(id.uuid()).ok_or_else(|| unknown_role(param))?;
        if role.scope != scope {
            return Err(ApiError::invalid(
                param,
                if scope == "org" {
                    "This is a project role; choose an org role."
                } else {
                    "This is an org role; choose a project role."
                },
            ));
        }
        Ok(role)
    }

    pub fn org_role(&self, s: &str, param: &str) -> ApiResult<(OrgGrant, RoleRef)> {
        if let Ok(p) = s.parse::<OrgPreset>() {
            return Ok((OrgGrant::preset(p), RoleRef::org_builtin(p)));
        }
        let role = self.custom(s, "org", param)?;
        Ok((
            OrgGrant::custom(
                RoleId::from_uuid(role.id),
                parse_org(&role.rights)?,
                parse_project(&role.project_rights)?,
            ),
            RoleRef::custom(Some(role.id), Some(&role.name)),
        ))
    }

    pub fn project_role(&self, s: &str, param: &str) -> ApiResult<(ProjectGrant, RoleRef)> {
        if let Ok(p) = s.parse::<ProjectPreset>() {
            return Ok((ProjectGrant::preset(p), RoleRef::project_builtin(p)));
        }
        let role = self.custom(s, "project", param)?;
        Ok((
            ProjectGrant::custom(RoleId::from_uuid(role.id), parse_project(&role.rights)?),
            RoleRef::custom(Some(role.id), Some(&role.name)),
        ))
    }

    pub fn org_grant(&self, req: &GrantRequest) -> ApiResult<(OrgGrant, RoleRef)> {
        let (base, role) = self.org_role(req.role_id()?, "role")?;
        let added = OrgRights::parse(req.rights_added.iter().map(String::as_str))
            .map_err(|e| ApiError::invalid("rights_added", format!("{e}.")))?;
        let removed = OrgRights::parse(req.rights_removed.iter().map(String::as_str))
            .map_err(|e| ApiError::invalid("rights_removed", format!("{e}.")))?;
        if let Some(right) = added.intersect(OrgRights::owner_only()).iter().next() {
            return Err(ApiError::invalid(
                "rights_added",
                format!("`{right}` is held only by owners; make the member an owner instead."),
            ));
        }
        Ok((
            OrgGrant {
                added,
                removed,
                ..base
            }
            .normalised(),
            role,
        ))
    }

    pub fn project_grant(&self, req: &GrantRequest) -> ApiResult<(ProjectGrant, RoleRef)> {
        let (base, role) = self.project_role(req.role_id()?, "role")?;
        let added = ProjectRights::parse(req.rights_added.iter().map(String::as_str))
            .map_err(|e| ApiError::invalid("rights_added", format!("{e}.")))?;
        let removed = ProjectRights::parse(req.rights_removed.iter().map(String::as_str))
            .map_err(|e| ApiError::invalid("rights_removed", format!("{e}.")))?;
        Ok((
            ProjectGrant {
                added,
                removed,
                ..base
            }
            .normalised(),
            role,
        ))
    }
}

pub fn parse_org(names: &[String]) -> ApiResult<OrgRights> {
    OrgRights::parse(names.iter().map(String::as_str)).map_err(ApiError::internal)
}

pub fn parse_project(names: &[String]) -> ApiResult<ProjectRights> {
    ProjectRights::parse(names.iter().map(String::as_str)).map_err(ApiError::internal)
}

fn stored_role_id(g: &Grant) -> RoleId {
    RoleId::from_uuid(g.role_id.unwrap_or_default())
}

pub fn org_from_store(g: &Grant) -> ApiResult<OrgGrant> {
    let base = match g.preset.parse::<OrgPreset>() {
        Ok(p) => OrgGrant::preset(p),
        Err(_) if g.preset == "custom" => OrgGrant::custom(
            stored_role_id(g),
            parse_org(g.role_rights.as_deref().unwrap_or_default())?,
            parse_project(g.role_project_rights.as_deref().unwrap_or_default())?,
        ),
        Err(e) => return Err(ApiError::internal(e)),
    };
    Ok(OrgGrant {
        added: parse_org(&g.rights_added)?,
        removed: parse_org(&g.rights_removed)?,
        ..base
    })
}

pub fn project_from_store(g: &Grant) -> ApiResult<ProjectGrant> {
    let base = match g.preset.parse::<ProjectPreset>() {
        Ok(p) => ProjectGrant::preset(p),
        Err(_) if g.preset == "custom" => ProjectGrant::custom(
            stored_role_id(g),
            parse_project(g.role_rights.as_deref().unwrap_or_default())?,
        ),
        Err(e) => return Err(ApiError::internal(e)),
    };
    Ok(ProjectGrant {
        added: parse_project(&g.rights_added)?,
        removed: parse_project(&g.rights_removed)?,
        ..base
    })
}

fn names(r: impl IntoIterator<Item = &'static str>) -> Vec<String> {
    r.into_iter().map(str::to_owned).collect()
}

pub fn org_to_store(g: &OrgGrant) -> Grant {
    let (preset, role_id) = match g.role {
        kaeso_core::rights::OrgRole::Builtin(p) => (p.as_str().to_owned(), None),
        kaeso_core::rights::OrgRole::Custom(id) => ("custom".to_owned(), Some(id.uuid())),
    };
    Grant {
        preset,
        role_id,
        rights_added: names(g.added.names()),
        rights_removed: names(g.removed.names()),
        ..Grant::default()
    }
}

pub fn project_to_store(g: &ProjectGrant) -> Grant {
    let (preset, role_id) = match g.role {
        kaeso_core::rights::ProjectRole::Builtin(p) => (p.as_str().to_owned(), None),
        kaeso_core::rights::ProjectRole::Custom(id) => ("custom".to_owned(), Some(id.uuid())),
    };
    Grant {
        preset,
        role_id,
        rights_added: names(g.added.names()),
        rights_removed: names(g.removed.names()),
        ..Grant::default()
    }
}

pub fn org_view(g: &OrgGrant, role: RoleRef) -> GrantView {
    GrantView {
        preset: g.preset_role().map(|p| p.as_str().to_owned()),
        role,
        rights_added: g.added.names(),
        rights_removed: g.removed.names(),
        rights: g.effective().names(),
        project_rights: Some(g.every_project().names()),
    }
}

pub fn project_view(g: &ProjectGrant, role: RoleRef) -> GrantView {
    GrantView {
        preset: g.preset_role().map(|p| p.as_str().to_owned()),
        role,
        rights_added: g.added.names(),
        rights_removed: g.removed.names(),
        rights: g.effective().names(),
        project_rights: None,
    }
}

pub fn check_org(caller: &OrgScope, before: Option<&OrgGrant>, after: &OrgGrant) -> ApiResult<()> {
    let before = before.map(OrgHolding::of).unwrap_or_default();
    Ok(caller
        .holding()
        .check_change(&before, &OrgHolding::of(after))?)
}

pub fn check_org_peer(caller: &OrgScope, target: &OrgGrant) -> ApiResult<()> {
    if caller.holding().covers(&OrgHolding::of(target)) {
        Ok(())
    } else {
        Err(ApiError::PermissionDenied(
            "You can't change a member who holds rights you don't hold.".into(),
        ))
    }
}

pub fn check_project(
    caller: ProjectRights,
    before: Option<&ProjectGrant>,
    after: &ProjectGrant,
) -> ApiResult<()> {
    let before = before.map_or(ProjectRights::EMPTY, ProjectGrant::effective);
    Ok(check_project_change(caller, before, after.effective())?)
}

pub fn check_project_peer(caller: ProjectRights, target: &ProjectGrant) -> ApiResult<()> {
    if target.effective().is_subset(caller) {
        Ok(())
    } else {
        Err(ApiError::PermissionDenied(
            "You can't change a member who holds rights you don't hold.".into(),
        ))
    }
}

pub fn check_service_account_peer(caller: ProjectRights, target: &ProjectGrant) -> ApiResult<()> {
    if target.effective().is_subset(caller) {
        Ok(())
    } else {
        Err(ApiError::PermissionDenied(
            "You can't change a service account that holds rights you don't hold.".into(),
        ))
    }
}
