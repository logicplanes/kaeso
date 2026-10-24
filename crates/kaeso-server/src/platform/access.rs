use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::extract::rejection::RawPathParamsRejection;
use axum::extract::{ConnectInfo, FromRequestParts, MatchedPath, RawPathParams, Request, State};
use axum::http::request::Parts;
use axum::http::{Method, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use kaeso_core::id::{OrgId, ProjectId, RequestId, UserId};
use kaeso_core::rights::{
    OrgGrant, OrgHolding, OrgPreset, OrgRight, OrgRights, ProjectGrant, ProjectRight,
    ProjectRights, Right, effective_project_rights,
};
use kaeso_store::pg::keys::{KeyAuth, OrgKeyAuth};
use kaeso_store::pg::members;
use kaeso_store::pg::orgs::Org;
use kaeso_store::pg::projects::Project;
use kaeso_store::pg::sessions::SessionRow;
use time::OffsetDateTime;
use utoipa::openapi::RefOr;
use utoipa::openapi::path::{Operation, ParameterIn, PathItem};
use utoipa::openapi::security::SecurityRequirement;
use utoipa_axum::router::UtoipaMethodRouter;

use super::error::{ApiError, ApiResult};
use super::{cookies, params};
use crate::auth::keys::{self, KeyCaller, LookupError};
use crate::auth::sessions::{self, RECENT_SIGN_IN};
use crate::http::{client_ip, request_id};
use crate::state::AppState;

const WHILE_SCHEDULED: &[&str] = &["/orgs/{org}/restore"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Public,
    Session,
    Account,
    InstanceAdmin,
    Org(OrgRight),
    Project(ProjectRight),
}

impl Access {
    pub fn name(self) -> String {
        match self {
            Access::Public => "public".into(),
            Access::Session => "session".into(),
            Access::Account => "account".into(),
            Access::InstanceAdmin => "instance_admin".into(),
            Access::Org(r) => format!("right:{}", r.as_str()),
            Access::Project(r) => format!("right:{}", r.as_str()),
        }
    }

    fn describe(self) -> String {
        match self {
            Access::Public => "**Access:** public; no authentication.".into(),
            Access::Session => {
                "**Access:** a signed-in console session; no right is needed.".into()
            }
            Access::Account => "**Access:** a signed-in console session; one from the SSO of an \
                org that doesn't manage the account gets `other_sign_in_required`."
                .into(),
            Access::InstanceAdmin => "**Access:** a console session of an instance admin that \
                didn't come from an organization's SSO, or came from the SSO of the org chosen \
                for instance admins (`instance/settings/admin-sso`) while it manages the account."
                .into(),
            Access::Org(r) => format!(
                "**Right:** `{}` on the organization in the path{}.",
                r.as_str(),
                if r.needs_session() {
                    "; a console session only, signed in within the last 15 minutes"
                } else if r.needs_recent_sign_in() {
                    "; with a session, a sign-in within the last 15 minutes"
                } else {
                    ""
                }
            ),
            Access::Project(r) => format!(
                "**Right:** `{}` on the project in the path{}.",
                r.as_str(),
                if r.needs_recent_sign_in() {
                    "; with a session, a sign-in within the last 15 minutes"
                } else {
                    ""
                }
            ),
        }
    }

    fn right(self) -> Option<Right> {
        match self {
            Access::Org(r) => Some(Right::Org(r)),
            Access::Project(r) => Some(Right::Project(r)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Secure {
    state: Option<AppState>,
}

impl Secure {
    pub fn new(state: &AppState) -> Secure {
        Secure {
            state: Some(state.clone()),
        }
    }

    pub fn documentation_only() -> Secure {
        Secure { state: None }
    }

    pub fn route(
        &self,
        access: Access,
        routes: UtoipaMethodRouter<AppState>,
    ) -> UtoipaMethodRouter<AppState> {
        let (schemas, mut paths, router) = routes;
        for item in paths.paths.values_mut() {
            for op in operations(item) {
                document(op, access);
            }
        }
        match &self.state {
            Some(state) => {
                let layered = router.layer(middleware::from_fn_with_state(
                    (state.clone(), access),
                    authorize,
                ));
                (schemas, paths, layered)
            }
            None => (schemas, paths, router),
        }
    }
}

pub fn operations(item: &mut PathItem) -> impl Iterator<Item = &mut Operation> {
    [
        &mut item.get,
        &mut item.put,
        &mut item.post,
        &mut item.delete,
        &mut item.patch,
        &mut item.head,
        &mut item.options,
        &mut item.trace,
    ]
    .into_iter()
    .filter_map(Option::as_mut)
}

fn document(op: &mut Operation, access: Access) {
    let note = access.describe();
    op.description = Some(match op.description.take() {
        Some(d) if !d.is_empty() => format!("{d}\n\n{note}"),
        _ => note,
    });
    let mut ext = op.extensions.take().unwrap_or_default();
    ext.insert(
        "x-kaeso-access".to_owned(),
        serde_json::Value::String(access.name()),
    );
    op.extensions = Some(ext);
    op.security = Some(match access {
        Access::Public => vec![SecurityRequirement::default()],
        Access::Session | Access::Account | Access::InstanceAdmin => {
            vec![SecurityRequirement::new("session", Vec::<String>::new())]
        }
        Access::Org(r) if r.needs_session() => {
            vec![SecurityRequirement::new("session", Vec::<String>::new())]
        }
        Access::Org(_) | Access::Project(_) => vec![
            SecurityRequirement::new("session", Vec::<String>::new()),
            SecurityRequirement::new("api_key", Vec::<String>::new()),
        ],
    });
    for (status, name) in [
        ("400", "InvalidRequest"),
        ("401", "Unauthenticated"),
        ("403", "Forbidden"),
        ("404", "NotFound"),
        ("409", "Conflict"),
        ("412", "Stale"),
        ("413", "TooLarge"),
        ("429", "RateLimited"),
        ("500", "Internal"),
        ("503", "Unavailable"),
    ] {
        if access == Access::Public && status == "403" {
            continue;
        }
        if status == "413" && op.request_body.is_none() {
            continue;
        }
        if status == "412" && !accepts_if_match(op) {
            continue;
        }
        op.responses
            .responses
            .entry(status.to_owned())
            .or_insert_with(|| utoipa::openapi::Ref::from_response_name(name).into());
    }
}

fn accepts_if_match(op: &Operation) -> bool {
    op.parameters.iter().flatten().any(|p| {
        matches!(p, RefOr::T(p) if p.parameter_in == ParameterIn::Header
            && p.name.eq_ignore_ascii_case("If-Match"))
    })
}

#[derive(Clone, Debug)]
pub enum Caller {
    Anonymous,
    Session(Arc<SessionRow>),
    ProjectKey(Arc<KeyAuth>),
    OrgKey(Arc<OrgKeyAuth>),
}

#[derive(Clone, Debug)]
pub struct OrgScope {
    pub org: Org,
    pub rights: OrgRights,
    pub every_project: ProjectRights,
    pub grant: Option<OrgGrant>,
}

impl OrgScope {
    pub fn id(&self) -> OrgId {
        OrgId::from_uuid(self.org.id)
    }

    pub fn holding(&self) -> OrgHolding {
        OrgHolding {
            org: self.rights,
            every_project: self.every_project,
        }
    }

    pub fn is_owner(&self) -> bool {
        self.grant.as_ref().is_some_and(OrgGrant::is_owner)
    }
}

#[derive(Clone, Debug)]
pub struct ProjectScope {
    pub project: Project,
    pub rights: ProjectRights,
    pub grant: Option<ProjectGrant>,
}

impl ProjectScope {
    pub fn id(&self) -> ProjectId {
        ProjectId::from_uuid(self.project.id)
    }
}

#[derive(Clone, Debug)]
pub struct Ctx(Arc<CtxInner>);

#[derive(Debug)]
pub struct CtxInner {
    pub request_id: RequestId,
    pub now: OffsetDateTime,
    pub ip: Option<IpAddr>,
    pub user_agent: Option<String>,
    pub accept_language: Option<String>,
    pub caller: Caller,
    pub org: Option<OrgScope>,
    pub project: Option<ProjectScope>,
}

impl std::ops::Deref for Ctx {
    type Target = CtxInner;
    fn deref(&self) -> &CtxInner {
        &self.0
    }
}

impl CtxInner {
    pub fn session(&self) -> ApiResult<&Arc<SessionRow>> {
        match &self.caller {
            Caller::Session(s) => Ok(s),
            Caller::Anonymous => Err(ApiError::Authentication("Sign in first.".into())),
            Caller::ProjectKey(_) | Caller::OrgKey(_) => Err(ApiError::PermissionDenied(
                "This needs a console session; API keys cannot manage accounts or the instance."
                    .into(),
            )),
        }
    }

    pub fn user_id(&self) -> ApiResult<UserId> {
        self.session().map(|s| UserId::from_uuid(s.user_id))
    }

    pub fn acting_user(&self) -> Option<UserId> {
        match &self.caller {
            Caller::Session(s) => Some(UserId::from_uuid(s.user_id)),
            Caller::ProjectKey(k) => k.user_id.map(UserId::from_uuid),
            Caller::Anonymous | Caller::OrgKey(_) => None,
        }
    }

    pub fn acting_service_account(&self) -> Option<uuid::Uuid> {
        match &self.caller {
            Caller::ProjectKey(k) => k.service_account_id,
            _ => None,
        }
    }

    pub fn org(&self) -> ApiResult<&OrgScope> {
        self.org
            .as_ref()
            .ok_or_else(|| ApiError::internal("route without an org scope"))
    }

    pub fn project(&self) -> ApiResult<&ProjectScope> {
        self.project
            .as_ref()
            .ok_or_else(|| ApiError::internal("route without a project scope"))
    }

    pub fn require_recent_sign_in(&self) -> ApiResult<()> {
        match &self.caller {
            Caller::Session(s) if self.now - s.signed_in_at > RECENT_SIGN_IN => {
                Err(ApiError::RecentSignInRequired)
            }
            _ => Ok(()),
        }
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Ctx {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<Ctx>().cloned().ok_or_else(|| {
            ApiError::internal("Platform API route registered without access control")
        })
    }
}

async fn authorize(
    State((state, access)): State<(AppState, Access)>,
    params: Result<RawPathParams, RawPathParamsRejection>,
    req: Request,
    next: Next,
) -> Response {
    let params = match params {
        Ok(p) => p,
        Err(e) => return params::raw_path_error(&e).into_response(),
    };
    let (mut parts, body) = req.into_parts();
    match check(&state, access, &params, &parts).await {
        Ok(ctx) => {
            parts.extensions.insert(ctx);
            next.run(Request::from_parts(parts, body)).await
        }
        Err(e) => {
            let stale_cookie = matches!(e, ApiError::Authentication(_))
                && !parts.headers.contains_key(header::AUTHORIZATION);
            let clear = match cookies::session_token(&state, &parts.headers) {
                Some(token) if stale_cookie => !replaced(&state, &token).await,
                _ => false,
            };
            let mut response = e.into_response();
            if clear {
                response
                    .headers_mut()
                    .append(header::SET_COOKIE, cookies::clear(&state));
            }
            response
        }
    }
}

async fn replaced(state: &AppState, token: &str) -> bool {
    sessions::was_replaced(state, token)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "could not tell whether a session was replaced");
            false
        })
}

async fn check(
    state: &AppState,
    access: Access,
    params: &RawPathParams,
    req: &Parts,
) -> ApiResult<Ctx> {
    let now = state.clock.now();
    let peer = req
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip());
    let forwarded = req
        .headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect::<Vec<_>>()
        .join(",");
    let ip = client_ip::resolve(peer, &forwarded, &state.config.trusted_proxies);
    let user_agent = req
        .headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(512).collect());
    let accept_language = req
        .headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(256).collect());
    let request_id = request_id::current().unwrap_or_default();

    let caller = match authenticate(state, req, ip).await {
        Ok(caller) => caller,
        Err(ApiError::Authentication(_))
            if access == Access::Public && !req.headers.contains_key(header::AUTHORIZATION) =>
        {
            Caller::Anonymous
        }
        Err(e) => return Err(e),
    };
    let state_changing = !matches!(req.method, Method::GET | Method::HEAD | Method::OPTIONS);
    let browser = matches!(caller, Caller::Session(_)) || access == Access::Public;
    if state_changing && browser {
        check_origin(state, req)?;
    }
    let changes_org = state_changing
        && !req
            .extensions
            .get::<MatchedPath>()
            .is_some_and(|p| WHILE_SCHEDULED.iter().any(|w| p.as_str().ends_with(w)));

    let mut ctx = CtxInner {
        request_id,
        now,
        ip,
        user_agent,
        accept_language,
        caller,
        org: None,
        project: None,
    };
    match access {
        Access::Public => {}
        Access::Session => {
            ctx.session()?;
        }
        Access::Account => {
            require_policy(state, ctx.session()?, Area::Account).await?;
        }
        Access::InstanceAdmin => {
            let session = ctx.session()?;
            if !session.is_instance_admin {
                return Err(ApiError::PermissionDenied(
                    "Only instance admins can do this.".into(),
                ));
            }
            let admin_sso = if from_sso(session) {
                kaeso_store::pg::instance::admin_sso_org(&state.pg).await?
            } else {
                None
            };
            require_policy(state, session, Area::Instance { admin_sso }).await?;
        }
        Access::Org(right) => {
            let org = path_id::<OrgId>(params, "org", "organization")?;
            let scope = match &ctx.caller {
                Caller::Session(_) => org_scope(state, &ctx, org, None, changes_org).await?,
                Caller::OrgKey(_) | Caller::ProjectKey(_) => {
                    let scope = key_scope(state, &ctx, org, None).await?;
                    if right.needs_session() {
                        return Err(ApiError::PermissionDenied(
                            "Only a console session can change SSO; API keys can't.".into(),
                        ));
                    }
                    scope
                }
                Caller::Anonymous => return Err(ApiError::Authentication("Sign in first.".into())),
            };
            if !scope.0.rights.contains(right) {
                return Err(denied(Right::Org(right), "organization"));
            }
            ctx.org = Some(scope.0);
        }
        Access::Project(right) => {
            let org = path_id::<OrgId>(params, "org", "organization")?;
            let project = path_id::<ProjectId>(params, "project", "project")?;
            let (org_scope, project_scope) = match &ctx.caller {
                Caller::Session(_) => {
                    org_scope(state, &ctx, org, Some(project), changes_org).await?
                }
                Caller::OrgKey(_) | Caller::ProjectKey(_) => {
                    key_scope(state, &ctx, org, Some(project)).await?
                }
                Caller::Anonymous => return Err(ApiError::Authentication("Sign in first.".into())),
            };
            let project_scope = project_scope.ok_or_else(|| ApiError::not_found("project"))?;
            if !project_scope.rights.contains(right) {
                return Err(denied(Right::Project(right), "project"));
            }
            ctx.org = Some(org_scope);
            ctx.project = Some(project_scope);
        }
    }
    let ctx = Ctx(Arc::new(ctx));
    if access.right().is_some_and(Right::needs_recent_sign_in) {
        ctx.require_recent_sign_in()?;
    }
    Ok(ctx)
}

pub(crate) fn denied(right: Right, on: &str) -> ApiError {
    ApiError::MissingRight {
        right: right.as_str(),
        message: format!(
            "You need the right `{right}` in this {on}; ask one of its admins for it."
        ),
    }
}

fn path_id<T: std::str::FromStr>(params: &RawPathParams, name: &str, what: &str) -> ApiResult<T> {
    params
        .iter()
        .find(|(k, _)| *k == name)
        .and_then(|(_, v)| v.parse::<T>().ok())
        .ok_or_else(|| ApiError::not_found(what))
}

async fn authenticate(state: &AppState, req: &Parts, ip: Option<IpAddr>) -> ApiResult<Caller> {
    if let Some(value) = req.headers.get(header::AUTHORIZATION) {
        let presented = value.to_str().ok().and_then(keys::bearer).ok_or_else(|| {
            ApiError::Authentication("Send the key as `Authorization: Bearer <key>`.".into())
        })?;
        let found = keys::authenticate(state, presented, ip)
            .await
            .map_err(|e| match e {
                LookupError::Store(e) => ApiError::from(e),
                LookupError::Throttled(t) => {
                    metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "api_key", "reason" => "throttled").increment(1);
                    ApiError::RateLimited {
                        message: t.message(),
                        retry_after: Some(t.retry_after),
                    }
                }
            })?;
        return match found {
            Ok(KeyCaller::Project(k)) if !k.platform => {
                metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "api_key", "reason" => "no_platform_access").increment(1);
                Err(ApiError::PermissionDenied(
                    "This key has no Platform API access; use a key with platform access.".into(),
                ))
            }
            Ok(KeyCaller::Project(k)) => Ok(Caller::ProjectKey(k)),
            Ok(KeyCaller::Org(k)) => Ok(Caller::OrgKey(k)),
            Err(refusal) => {
                metrics::counter!(crate::metrics::AUTH_FAILURES, "method" => "api_key", "reason" => "refused").increment(1);
                Err(ApiError::Authentication(refusal.message().into()))
            }
        };
    }
    let Some(token) = cookies::session_token(state, &req.headers) else {
        return Ok(Caller::Anonymous);
    };
    match sessions::authenticate(state, &token).await? {
        Some(session) => Ok(Caller::Session(session)),
        None => Err(ApiError::Authentication(
            "Your session has ended; sign in again.".into(),
        )),
    }
}

fn check_origin(state: &AppState, req: &Parts) -> ApiResult<()> {
    let expected = state.config.public_origin();
    match req
        .headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
    {
        Some(origin) if origin == expected => Ok(()),
        _ => Err(ApiError::PermissionDenied(format!(
            "Open Kaeso at {expected}; it refuses requests from pages at other addresses."
        ))),
    }
}

async fn key_scope(
    state: &AppState,
    ctx: &CtxInner,
    org: OrgId,
    project: Option<ProjectId>,
) -> ApiResult<(OrgScope, Option<ProjectScope>)> {
    let (key_org, org_rights, project_rights, own_project) = match &ctx.caller {
        Caller::OrgKey(k) => (
            k.org_id,
            OrgPreset::Admin.rights(),
            OrgPreset::Admin.project_rights(),
            None,
        ),
        Caller::ProjectKey(k) => {
            if project.is_none() {
                return Err(ApiError::PermissionDenied(
                    "Project API keys work only in their own project; use an org platform key."
                        .into(),
                ));
            }
            (
                k.org_id,
                OrgRights::EMPTY,
                keys::project_rights(k),
                Some(k.project_id),
            )
        }
        _ => return Err(ApiError::Authentication("Sign in first.".into())),
    };
    if key_org != org.uuid() || (own_project.is_some() && own_project != project.map(|p| p.uuid()))
    {
        return Err(ApiError::not_found(if project.is_some() {
            "project"
        } else {
            "organization"
        }));
    }
    let mut conn = state.pg.acquire().await?;
    let found = members::access_context(&mut conn, org.uuid(), None, project.map(|p| p.uuid()))
        .await?
        .ok_or_else(|| ApiError::not_found("organization"))?;
    let project_scope = match (found.project, project) {
        (Some(p), Some(_)) => Some(ProjectScope {
            project: p,
            rights: project_rights,
            grant: None,
        }),
        (None, Some(_)) => return Err(ApiError::not_found("project")),
        _ => None,
    };
    Ok((
        OrgScope {
            org: found.org,
            rights: org_rights,
            every_project: if own_project.is_some() {
                ProjectRights::EMPTY
            } else {
                project_rights
            },
            grant: None,
        },
        project_scope,
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyGap {
    NeedsSso,
    NeedsOtherSignIn,
    Needs2fa,
}

pub fn from_sso(session: &SessionRow) -> bool {
    session.method == "oidc"
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    Org {
        id: uuid::Uuid,
        require_sso: bool,
        require_2fa: bool,
    },
    Account,
    Instance {
        admin_sso: Option<uuid::Uuid>,
    },
}

impl Area {
    pub fn of_org(org: &Org) -> Area {
        Area::Org {
            id: org.id,
            require_sso: org.require_sso,
            require_2fa: org.require_2fa,
        }
    }

    fn needs_manager(self, session: &SessionRow) -> bool {
        from_sso(session)
            || matches!(
                self,
                Area::Org {
                    require_sso: true,
                    ..
                }
            )
    }
}

// A session from SSO counts as a full sign-in only while the org whose provider signed it in
// manages the account; otherwise that provider vouches for nothing beyond its own org. The
// instance trusts no org's provider unless an instance admin, signed in another way, chose it.
pub fn policy_gap(
    session: &SessionRow,
    managed_by: Option<uuid::Uuid>,
    area: Area,
) -> Option<PolicyGap> {
    let sso = from_sso(session);
    let full = !sso || (session.oidc_org_id.is_some() && session.oidc_org_id == managed_by);
    match area {
        Area::Account if full => None,
        Area::Instance { .. } if !sso => None,
        Area::Instance { admin_sso } if full && admin_sso.is_some() => {
            (session.oidc_org_id != admin_sso).then_some(PolicyGap::NeedsOtherSignIn)
        }
        Area::Account | Area::Instance { .. } => Some(PolicyGap::NeedsOtherSignIn),
        Area::Org {
            id,
            require_sso,
            require_2fa,
        } => {
            if sso && session.oidc_org_id == Some(id) {
                None
            } else if require_sso && managed_by == Some(id) {
                Some(PolicyGap::NeedsSso)
            } else if !full {
                Some(PolicyGap::NeedsOtherSignIn)
            } else if require_2fa && !sso && !session.second_factor {
                Some(PolicyGap::Needs2fa)
            } else {
                None
            }
        }
    }
}

pub async fn managed_by(
    state: &AppState,
    session: &SessionRow,
) -> ApiResult<Option<kaeso_store::pg::oidc::Manager>> {
    Ok(crate::services::managed::of_user(&state.pg, session.user_id).await?)
}

pub async fn require_policy(state: &AppState, session: &SessionRow, area: Area) -> ApiResult<()> {
    let manager = if area.needs_manager(session) {
        managed_by(state, session).await?.map(|m| m.org_id)
    } else {
        None
    };
    refuse(policy_gap(session, manager, area))
}

pub fn refuse(gap: Option<PolicyGap>) -> ApiResult<()> {
    match gap {
        None => Ok(()),
        Some(PolicyGap::NeedsSso) => Err(ApiError::PermissionDenied(
            "This organization requires signing in with SSO.".into(),
        )),
        Some(PolicyGap::NeedsOtherSignIn) => Err(ApiError::OtherSignInRequired),
        Some(PolicyGap::Needs2fa) => Err(ApiError::PermissionDenied(
            "This organization requires two-factor authentication; set it up, then sign in again."
                .into(),
        )),
    }
}

async fn org_scope(
    state: &AppState,
    ctx: &CtxInner,
    org: OrgId,
    project: Option<ProjectId>,
    changes: bool,
) -> ApiResult<(OrgScope, Option<ProjectScope>)> {
    let Caller::Session(session) = &ctx.caller else {
        return Err(ApiError::Authentication("Sign in first.".into()));
    };
    let mut conn = state.pg.acquire().await?;
    let found = members::access_context(
        &mut conn,
        org.uuid(),
        Some(session.user_id),
        project.map(|p| p.uuid()),
    )
    .await?;
    let Some(found) = found else {
        return Err(ApiError::not_found("organization"));
    };
    let Some(grant) = found
        .org_grant
        .as_ref()
        .map(super::grants::org_from_store)
        .transpose()?
    else {
        return Err(ApiError::not_found("organization"));
    };
    if changes && found.org.state == "scheduled_for_deletion" {
        return Err(ApiError::OrgScheduledForDeletion);
    }
    require_policy(state, session, Area::of_org(&found.org)).await?;
    let project_scope = match (found.project, project) {
        (Some(p), Some(_)) => {
            let project_grant = found
                .project_grant
                .as_ref()
                .map(super::grants::project_from_store)
                .transpose()?;
            let rights = effective_project_rights(Some(&grant), project_grant.as_ref());
            if rights.is_empty() {
                return Err(ApiError::not_found("project"));
            }
            Some(ProjectScope {
                project: p,
                rights,
                grant: project_grant,
            })
        }
        (None, Some(_)) => return Err(ApiError::not_found("project")),
        _ => None,
    };
    Ok((
        OrgScope {
            org: found.org,
            rights: grant.effective(),
            every_project: grant.every_project(),
            grant: Some(grant),
        },
        project_scope,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use PolicyGap::{Needs2fa, NeedsOtherSignIn, NeedsSso};

    fn session(method: &str, second_factor: bool, oidc_org: Option<uuid::Uuid>) -> SessionRow {
        let now = OffsetDateTime::now_utc();
        SessionRow {
            id: uuid::Uuid::now_v7(),
            user_id: uuid::Uuid::now_v7(),
            method: method.to_owned(),
            second_factor,
            oidc_org_id: oidc_org,
            signed_in_at: now,
            last_used_at: now,
            expires_at: now,
            user_state: "active".into(),
            is_instance_admin: true,
        }
    }

    fn org(id: uuid::Uuid, require_sso: bool, require_2fa: bool) -> Area {
        Area::Org {
            id,
            require_sso,
            require_2fa,
        }
    }

    struct World {
        a: uuid::Uuid,
        b: uuid::Uuid,
    }

    const AREAS: usize = 10;

    impl World {
        fn new() -> World {
            World {
                a: uuid::Uuid::now_v7(),
                b: uuid::Uuid::now_v7(),
            }
        }

        fn areas(&self) -> [(&'static str, Area); AREAS] {
            [
                ("org A", org(self.a, false, false)),
                ("org A requiring SSO", org(self.a, true, false)),
                ("org B", org(self.b, false, false)),
                ("org B requiring SSO", org(self.b, true, false)),
                ("org B requiring 2FA", org(self.b, false, true)),
                ("org B requiring both", org(self.b, true, true)),
                ("account changes", Area::Account),
                (
                    "instance, no org chosen",
                    Area::Instance { admin_sso: None },
                ),
                (
                    "instance, org A chosen",
                    Area::Instance {
                        admin_sso: Some(self.a),
                    },
                ),
                (
                    "instance, org B chosen",
                    Area::Instance {
                        admin_sso: Some(self.b),
                    },
                ),
            ]
        }
    }

    fn check(
        s: &SessionRow,
        managed_by: Option<uuid::Uuid>,
        w: &World,
        expected: [Option<PolicyGap>; AREAS],
    ) {
        for ((name, area), want) in w.areas().into_iter().zip(expected) {
            assert_eq!(policy_gap(s, managed_by, area), want, "{name}");
        }
    }

    const OTHER: Option<PolicyGap> = Some(NeedsOtherSignIn);
    const SSO: Option<PolicyGap> = Some(NeedsSso);
    const TFA: Option<PolicyGap> = Some(Needs2fa);

    #[test]
    fn a_managed_sso_session_counts_like_a_password_and_administers_only_where_chosen() {
        let w = World::new();
        let sso = session("oidc", false, Some(w.a));
        check(
            &sso,
            Some(w.a),
            &w,
            [None, None, None, None, None, None, None, OTHER, None, OTHER],
        );
    }

    #[test]
    fn an_sso_session_that_doesnt_manage_the_account_opens_only_its_own_org() {
        let w = World::new();
        let sso = session("oidc", false, Some(w.a));
        check(
            &sso,
            None,
            &w,
            [
                None, None, OTHER, OTHER, OTHER, OTHER, OTHER, OTHER, OTHER, OTHER,
            ],
        );
        check(
            &sso,
            Some(w.b),
            &w,
            [
                None, None, OTHER, SSO, OTHER, SSO, OTHER, OTHER, OTHER, OTHER,
            ],
        );
    }

    #[test]
    fn an_sso_session_whose_org_is_gone_opens_nothing() {
        let w = World::new();
        let orphan = session("oidc", false, None);
        check(&orphan, None, &w, [OTHER; AREAS]);
        check(
            &orphan,
            Some(w.a),
            &w,
            [
                OTHER, SSO, OTHER, OTHER, OTHER, OTHER, OTHER, OTHER, OTHER, OTHER,
            ],
        );
    }

    #[test]
    fn a_session_without_sso_meets_each_orgs_own_policies() {
        let w = World::new();
        for method in ["password", "magic_link", "setup", "signup"] {
            let without_code = session(method, false, None);
            check(
                &without_code,
                None,
                &w,
                [None, None, None, None, TFA, TFA, None, None, None, None],
            );
            check(
                &without_code,
                Some(w.b),
                &w,
                [None, None, None, SSO, TFA, SSO, None, None, None, None],
            );
            let with_code = session(method, true, None);
            check(&with_code, None, &w, [None; AREAS]);
            check(
                &with_code,
                Some(w.a),
                &w,
                [None, SSO, None, None, None, None, None, None, None, None],
            );
        }
    }

    #[test]
    fn requiring_sso_closes_an_org_only_to_the_accounts_it_manages() {
        let w = World::new();
        let password = session("password", true, None);
        let required = org(w.b, true, false);
        assert_eq!(policy_gap(&password, None, required), None, "a guest");
        assert_eq!(policy_gap(&password, Some(w.a), required), None);
        assert_eq!(policy_gap(&password, Some(w.b), required), SSO);
        let own = session("oidc", false, Some(w.b));
        assert_eq!(policy_gap(&own, Some(w.b), required), None);
        let managed_elsewhere = session("oidc", false, Some(w.a));
        assert_eq!(policy_gap(&managed_elsewhere, Some(w.a), required), None);
    }

    #[test]
    fn the_second_factor_of_an_sso_session_is_the_providers() {
        let w = World::new();
        let strict = org(w.b, false, true);
        assert_eq!(
            policy_gap(&session("oidc", false, Some(w.a)), Some(w.a), strict),
            None
        );
        assert_eq!(
            policy_gap(&session("oidc", false, Some(w.b)), None, strict),
            None
        );
        assert_eq!(
            policy_gap(&session("password", false, None), Some(w.a), strict),
            TFA
        );
    }

    #[test]
    fn the_org_chosen_for_instance_admins_counts_only_while_it_manages_the_account() {
        let w = World::new();
        let chosen = Area::Instance {
            admin_sso: Some(w.a),
        };
        let from_a = session("oidc", false, Some(w.a));
        assert_eq!(policy_gap(&from_a, Some(w.a), chosen), None);
        assert_eq!(
            policy_gap(&from_a, None, chosen),
            OTHER,
            "the connection or the domain is gone, or the address moved"
        );
        assert_eq!(policy_gap(&from_a, Some(w.b), chosen), OTHER);
        assert_eq!(
            policy_gap(&session("oidc", false, Some(w.b)), Some(w.b), chosen),
            OTHER
        );
    }

    #[test]
    fn the_account_manager_is_looked_up_only_where_it_decides() {
        let w = World::new();
        let password = session("password", false, None);
        let sso = session("oidc", false, Some(w.a));
        assert!(!Area::Account.needs_manager(&password));
        assert!(
            !Area::Instance {
                admin_sso: Some(w.a)
            }
            .needs_manager(&password)
        );
        assert!(!org(w.b, false, true).needs_manager(&password));
        assert!(org(w.b, true, false).needs_manager(&password));
        for (name, area) in w.areas() {
            assert!(area.needs_manager(&sso), "{name}");
        }
    }
}
