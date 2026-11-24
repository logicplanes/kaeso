pub mod data;
mod image;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use kaeso_core::id::{OrgId, ProjectId, SharedLinkId, UserId};
use kaeso_core::rights::{OrgRight, effective_project_rights};
use kaeso_store::pg::members;
use kaeso_store::pg::shared_links::{self as store, Policy, SharedLink};
use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};
use utoipa::ToSchema;
use uuid::Uuid;

use self::data::{KINDS, METRICS, Seen, SharedBreakdown, SharedData, SharedLimit, SharedRow};
use crate::auth::agents::AgentCaller;
use crate::auth::throttle::Throttle;
use crate::platform::access::{Ctx, agent_scope};
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::grants::{org_from_store, project_from_store};
use crate::platform::json::Body;
use crate::platform::pagination::List;
use crate::platform::params::Path;
use crate::state::AppState;

// Every open counts against the address, so the token can't be guessed by trying.
pub const OPENS_PER_IP: Throttle = Throttle {
    name: "share_ip",
    limit: 300,
    window: std::time::Duration::from_secs(15 * 60),
    on_outage: crate::auth::throttle::Outage::Allow,
};

const EXPIRIES: [i32; 4] = [1, 7, 30, 90];
const LONGEST_PERIOD: i32 = 400;
const GONE: &str = "This shared link doesn't exist, has expired or was revoked.";
const MEMBERS_ONLY: &str = "This link is for the organization's members; sign in to see it.";
const PERSON_ONLY: &str = "Shared links are made by a person; sign in, or use an agent.";

time::serde::format_description!(iso_date, Date, "[year]-[month]-[day]");

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(
    description = "A read-only link to an org's usage, a member's stats, a project's budgets or a project's summary."
)]
pub struct SharedLinkView {
    #[doc = "Id."]
    pub id: SharedLinkId,
    #[doc = "`analytics`, `member`, `budget` or `project_card`."]
    pub kind: String,
    #[doc = "The title the page shows."]
    pub title: String,
    #[doc = "The project it shows; `null` for the whole organization."]
    pub project: Option<ProjectId>,
    #[doc = "The member whose stats it shows, for `member`."]
    pub member: Option<UserId>,
    #[doc = "Whether it shows the last `days` days whenever it is opened (`true`), or\n\
        the numbers of `from` to `to` as they were when it was made."]
    pub live: bool,
    #[doc = "The days a live link shows, today included."]
    pub days: Option<i32>,
    #[doc = "The first day of a snapshot (UTC)."]
    #[schema(value_type = Option<String>, format = Date)]
    #[serde(with = "iso_date::option")]
    pub from: Option<Date>,
    #[doc = "The last day of a snapshot (UTC)."]
    #[schema(value_type = Option<String>, format = Date)]
    #[serde(with = "iso_date::option")]
    pub to: Option<Date>,
    #[doc = "The metric its chart shows: `requests`, `tokens`, `cost`, `errors`, `tools`\n\
        or `cache`."]
    pub metric: String,
    #[doc = "Whether it shows costs."]
    pub show_costs: bool,
    #[doc = "Whether members show as *Member 1*, *Member 2* and so on."]
    pub hide_members: bool,
    #[doc = "Whether keys show as *Key 1*, *Key 2* and so on."]
    pub hide_keys: bool,
    #[doc = "Whether projects show as *Project 1*, *Project 2* and so on, and the\n\
        project's own name is left out."]
    pub hide_projects: bool,
    #[doc = "Who can open it: `anyone` with the link, or `members` of the organization\n\
        signed in to Kaeso."]
    pub audience: String,
    #[doc = "Who made it."]
    pub created_by: UserId,
    #[doc = "Created."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[doc = "When it stops working; `null` for never."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub expires_at: Option<OffsetDateTime>,
    #[doc = "How often it was opened."]
    pub views: i64,
    #[doc = "When it was last opened."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_opened_at: Option<OffsetDateTime>,
}

impl SharedLinkView {
    fn of(l: &SharedLink) -> SharedLinkView {
        SharedLinkView {
            id: SharedLinkId::from_uuid(l.id),
            kind: l.kind.clone(),
            title: l.title.clone(),
            project: l.project_id.map(ProjectId::from_uuid),
            member: l.member_id.map(UserId::from_uuid),
            live: l.live,
            days: l.period_days,
            from: if l.live { None } else { l.from_day },
            to: if l.live { None } else { l.to_day },
            metric: l.metric.clone(),
            show_costs: l.show_costs,
            hide_members: l.hide_members,
            hide_keys: l.hide_keys,
            hide_projects: l.hide_projects,
            audience: l.audience.clone(),
            created_by: UserId::from_uuid(l.created_by),
            created_at: l.created_at,
            expires_at: l.expires_at,
            views: l.views,
            last_opened_at: l.last_opened_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "A shared link to make.")]
pub struct SharedLinkCreate {
    #[doc = "`analytics` (the org's usage, or a project's with `project`), `member` (one\n\
        member's usage across the org), `budget` (a project's limits and budgets) or\n\
        `project_card` (a project's summary)."]
    #[schema(example = "analytics")]
    pub kind: String,
    #[doc = "The project; needed for `budget` and `project_card`."]
    pub project: Option<ProjectId>,
    #[doc = "The member, for `member`: oneself, or anyone with `usage.org.view`."]
    pub member: Option<UserId>,
    #[doc = "The title the page shows, 1 to 100 characters; one that names what it\n\
        shows when absent."]
    #[schema(example = "October spend")]
    pub title: Option<String>,
    #[doc = "Live (default): the last `days` days whenever it is opened. `false`: a\n\
        snapshot of `from` to `to` as the numbers are now."]
    pub live: Option<bool>,
    #[doc = "The days a live link shows, today included, 1 to 400; default 30."]
    #[schema(example = 30)]
    pub days: Option<i32>,
    #[doc = "The first day of a snapshot (UTC)."]
    #[schema(value_type = Option<String>, format = Date)]
    #[serde(default, with = "iso_date::option")]
    pub from: Option<Date>,
    #[doc = "The last day of a snapshot (UTC), at most 400 days after `from`."]
    #[schema(value_type = Option<String>, format = Date)]
    #[serde(default, with = "iso_date::option")]
    pub to: Option<Date>,
    #[doc = "The metric its chart shows; default `requests`."]
    pub metric: Option<String>,
    #[doc = "Show costs; needs the right to see them. Off by default."]
    pub show_costs: Option<bool>,
    #[doc = "Show members as *Member 1*, *Member 2* and so on."]
    pub hide_members: Option<bool>,
    #[doc = "Show keys as *Key 1*, *Key 2* and so on."]
    pub hide_keys: Option<bool>,
    #[doc = "Show projects as *Project 1*, *Project 2* and so on."]
    pub hide_projects: Option<bool>,
    #[doc = "`members` (default): people signed in to an account in the organization.\n\
        `anyone` with the link, where the organization allows it."]
    pub audience: Option<String>,
    #[doc = "1, 7, 30 or 90 days; absent for a link that never expires, where the\n\
        organization allows it."]
    #[schema(example = 30)]
    pub expires_in_days: Option<i32>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A new shared link, with its address shown this once.")]
pub struct SharedLinkSecret {
    #[doc = "The link."]
    pub link: SharedLinkView,
    #[doc = "The address to share; it holds the link's token, which Kaeso keeps only\n\
        as a hash."]
    #[schema(
        example = "https://kaeso.example.com/share/9fQ2kX7mVb3pL8rT1wZ5yH4uJ6sD0gA2cE7nM1qR3tU"
    )]
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "Who an organization's shared links may reach, and for how long.")]
pub struct SharedLinksPolicy {
    #[doc = "`off`: no links; `members`: links only for people signed in to the\n\
        organization; `anyone`: links anyone with the address can open."]
    #[schema(example = "members")]
    pub allow: String,
    #[doc = "The longest a link may last: 1, 7, 30 or 90 days; `null` lets links never\n\
        expire."]
    #[schema(example = 90)]
    pub max_days: Option<i32>,
}

impl SharedLinksPolicy {
    pub(crate) fn of(p: &Policy) -> SharedLinksPolicy {
        SharedLinksPolicy {
            allow: p.allow.clone(),
            max_days: p.max_days,
        }
    }
}

fn check_policy(p: &SharedLinksPolicy) -> ApiResult<Policy> {
    if !["off", "members", "anyone"].contains(&p.allow.as_str()) {
        return Err(ApiError::invalid(
            "allow",
            format!("`{}` is not one of off, members, anyone.", p.allow),
        ));
    }
    if p.max_days.is_some_and(|d| !EXPIRIES.contains(&d)) {
        return Err(ApiError::invalid(
            "max_days",
            format!(
                "`{}` is not one of 1, 7, 30, 90.",
                p.max_days.unwrap_or_default()
            ),
        ));
    }
    Ok(Policy {
        allow: p.allow.clone(),
        max_days: p.max_days,
    })
}

/// The rights `user` holds in `org`, and in `project` when given; `None` once they left or
/// their account is no longer active.
pub(crate) async fn seen_by(
    state: &AppState,
    user: Uuid,
    org: Uuid,
    project: Option<Uuid>,
) -> ApiResult<Option<Seen>> {
    let active = kaeso_store::pg::users::get(&state.pg, user)
        .await?
        .is_some_and(|u| u.state == "active");
    if !active {
        return Ok(None);
    }
    let mut conn = state.pg.acquire().await?;
    let Some(found) = members::access_context(&mut conn, org, Some(user), project).await? else {
        return Ok(None);
    };
    let Some(grant) = found.org_grant.as_ref().map(org_from_store).transpose()? else {
        return Ok(None);
    };
    let project_rights = match found.project {
        Some(p) if p.archived_at.is_none() => {
            let own = found
                .project_grant
                .as_ref()
                .map(project_from_store)
                .transpose()?;
            Some(effective_project_rights(Some(&grant), own.as_ref()))
        }
        _ => None,
    };
    Ok(Some(Seen {
        org: grant.effective(),
        project: project_rights,
    }))
}

fn default_title(kind: &str, scope: &str, hide: bool) -> String {
    match (kind, hide) {
        ("analytics", false) => format!("{scope} usage"),
        ("member", false) => format!("{scope}'s usage"),
        ("budget", false) => format!("{scope} budgets"),
        ("project_card", false) => format!("{scope} summary"),
        ("member", true) => "A member's usage".into(),
        ("budget", true) => "Budgets".into(),
        ("project_card", true) => "Project summary".into(),
        _ => "Usage".into(),
    }
}

async fn within_grant(
    state: &AppState,
    ctx: &Ctx,
    agent: &AgentCaller,
    seen: Seen,
    project: Option<Uuid>,
) -> ApiResult<Seen> {
    let org = ctx.org()?;
    let granted = match project {
        Some(p) => {
            match agent_scope(state, agent, org.id(), Some(ProjectId::from_uuid(p)), true).await {
                Ok((_, scope)) => scope.map(|s| s.rights),
                Err(ApiError::NotFound(_)) => None,
                Err(e) => return Err(e),
            }
        }
        None => None,
    };
    Ok(Seen {
        org: seen.org.intersect(org.rights),
        project: seen.project.zip(granted).map(|(p, g)| p.intersect(g)),
    })
}

fn person(ctx: &Ctx) -> ApiResult<Uuid> {
    ctx.acting_user()
        .map(|u| u.uuid())
        .ok_or_else(|| ApiError::PermissionDenied(PERSON_ONLY.into()))
}

async fn open_policy(state: &AppState, org: Uuid) -> ApiResult<Option<Policy>> {
    if !store::instance(&state.pg).await?.shared_links {
        return Ok(None);
    }
    let policy = store::policy(&state.pg, org).await?;
    Ok((policy.allow != "off").then_some(policy))
}

#[utoipa::path(
    post,
    path = "/orgs/{org}/shared-links",
    summary = "Makes a read-only link to usage, a member's stats, a project's budgets or a project's summary.",
    description = "Anyone with the address, or the organization's members signed in to Kaeso,\n\
    can open the link without other rights. It shows only what its maker can see, and stops\n\
    working when they no longer can, leave the organization, it expires or is revoked. The\n\
    address is returned this once. Request logs are never shared.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = SharedLinkCreate,
    responses((status = 201, body = SharedLinkSecret))
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<SharedLinkCreate>,
) -> ApiResult<(StatusCode, Json<SharedLinkSecret>)> {
    let maker = person(&ctx)?;
    let org = ctx.org()?.org.id;
    let Some(policy) = open_policy(&state, org).await? else {
        return Err(ApiError::PermissionDenied(
            "This organization doesn't allow shared links; ask one of its admins.".into(),
        ));
    };
    if !KINDS.contains(&req.kind.as_str()) {
        return Err(ApiError::invalid(
            "kind",
            format!(
                "`{}` is not one of analytics, member, budget, project_card.",
                req.kind
            ),
        ));
    }
    let needs_project = matches!(req.kind.as_str(), "budget" | "project_card");
    if needs_project && req.project.is_none() {
        return Err(ApiError::invalid(
            "project",
            "This kind of link needs a `project`.",
        ));
    }
    let member = match (req.kind.as_str(), req.member) {
        ("member", Some(m)) => Some(m.uuid()),
        ("member", None) => {
            return Err(ApiError::invalid(
                "member",
                "A member's stats need a `member`.",
            ));
        }
        _ => None,
    };
    let project = if req.kind == "member" {
        None
    } else {
        req.project.map(|p| p.uuid())
    };
    let metric = req.metric.clone().unwrap_or_else(|| "requests".into());
    if !METRICS.contains(&metric.as_str()) {
        return Err(ApiError::invalid(
            "metric",
            format!("`{metric}` is not one of requests, tokens, cost, errors, tools, cache."),
        ));
    }
    let audience = req.audience.clone().unwrap_or_else(|| "members".into());
    match (audience.as_str(), policy.allow.as_str()) {
        ("members", _) | ("anyone", "anyone") => {}
        ("anyone", _) => {
            return Err(ApiError::PermissionDenied(
                "This organization allows links only for its members.".into(),
            ));
        }
        (other, _) => {
            return Err(ApiError::invalid(
                "audience",
                format!("`{other}` is not one of anyone, members."),
            ));
        }
    }
    let expires_at = match (req.expires_in_days, policy.max_days) {
        (Some(d), _) if !EXPIRIES.contains(&d) => {
            return Err(ApiError::invalid(
                "expires_in_days",
                format!("`{d}` is not one of 1, 7, 30, 90."),
            ));
        }
        (None, Some(max)) => {
            return Err(ApiError::invalid(
                "expires_in_days",
                format!("The organization allows at most {max} days."),
            ));
        }
        (Some(d), Some(max)) if d > max => {
            return Err(ApiError::invalid(
                "expires_in_days",
                format!("The organization allows at most {max} days."),
            ));
        }
        (Some(d), _) => Some(ctx.now + time::Duration::days(i64::from(d))),
        (None, None) => None,
    };
    let live = req.live.unwrap_or(true);
    let today = ctx.now.date();
    let (period_days, from_day, to_day) = if live {
        let days = req.days.unwrap_or(30);
        if !(1..=LONGEST_PERIOD).contains(&days) {
            return Err(ApiError::invalid("days", "`days` must be 1 to 400."));
        }
        (Some(days), None, None)
    } else {
        match (req.from, req.to) {
            (Some(from), Some(to))
                if from <= to && to <= today && (to - from).whole_days() < 400 =>
            {
                (None, Some(from), Some(to))
            }
            _ => {
                return Err(ApiError::invalid(
                    "from",
                    "A snapshot needs `from` and `to`, at most 400 days apart and not after today.",
                ));
            }
        }
    };
    let title = match req.title.as_deref().map(str::trim) {
        Some(t) if (1..=100).contains(&t.chars().count()) => Some(t.to_owned()),
        Some(_) => {
            return Err(ApiError::invalid(
                "title",
                "`title` must be 1 to 100 characters.",
            ));
        }
        None => None,
    };
    let mut link = SharedLink {
        id: Uuid::now_v7(),
        org_id: org,
        project_id: project,
        kind: req.kind.clone(),
        member_id: member,
        title: String::new(),
        created_by: maker,
        created_at: ctx.now,
        expires_at,
        revoked_at: None,
        live,
        period_days,
        from_day,
        to_day,
        snapshot: None,
        metric,
        show_costs: req.show_costs.unwrap_or(false),
        hide_members: req.hide_members.unwrap_or(false),
        hide_keys: req.hide_keys.unwrap_or(false),
        hide_projects: req.hide_projects.unwrap_or(false),
        audience,
        views: 0,
        last_opened_at: None,
    };
    let seen = match (seen_by(&state, maker, org, project).await?, ctx.agent()) {
        (Some(seen), Some(agent)) => Some(within_grant(&state, &ctx, agent, seen, project).await?),
        (seen, _) => seen,
    };
    let can = seen.as_ref().and_then(|s| s.allows(&link));
    let Some((seen, costs)) = seen.zip(can) else {
        return Err(ApiError::PermissionDenied(
            "You can share only what you can see; ask an admin for the right to see it.".into(),
        ));
    };
    if link.show_costs && !costs {
        return Err(ApiError::invalid(
            "show_costs",
            "Showing costs needs the right to see them.",
        ));
    }
    if link.kind == "member" && member != Some(maker) && project.is_none() {
        let known = members::access_context(&mut *state.pg.acquire().await?, org, member, None)
            .await?
            .and_then(|f| f.org_grant)
            .is_some();
        if !known {
            return Err(ApiError::not_found("member"));
        }
    }
    let names = seen.names(&link);
    // A link opens with its maker's rights, so what an agent's grant hides is hidden here for good.
    if ctx.agent().is_some() {
        link.hide_members |= !names.members;
        link.hide_keys |= !names.keys;
    }
    let scope = scope_name(&state, &link, names).await?;
    link.title = title.unwrap_or_else(|| {
        default_title(
            &link.kind,
            scope.as_deref().unwrap_or_default(),
            scope.is_none(),
        )
    });
    if !live {
        let data = data::compute(
            &state,
            &link,
            names,
            link.show_costs,
            (from_day.unwrap_or(today), to_day.unwrap_or(today)),
        )
        .await?;
        link.snapshot = Some(serde_json::to_value(&data).map_err(ApiError::internal)?);
    }
    let token = kaeso_crypto::random_token();
    let hash = kaeso_core::secret::hash(&token);
    let view = SharedLinkView::of(&link);
    let mut tx = state.pg.begin().await?;
    store::insert(
        &mut *tx,
        &store::NewLink {
            link: &link,
            token_hash: &hash,
        },
    )
    .await?;
    let mut audit = ctx
        .audit(Some(OrgId::from_uuid(org)), Action::SharedLinkCreate)
        .target("shared_link", view.id)
        .after(&view);
    if let Some(p) = project {
        audit = audit.project(ProjectId::from_uuid(p));
    }
    audit.write(&mut tx).await?;
    tx.commit().await?;
    let url = format!(
        "{}/share/{token}",
        state.config.public_url.as_str().trim_end_matches('/')
    );
    Ok((
        StatusCode::CREATED,
        Json(SharedLinkSecret { link: view, url }),
    ))
}

// The project or member a link is about, as its page names it; `None` when the link hides it.
async fn scope_name(
    state: &AppState,
    link: &SharedLink,
    names: data::Names,
) -> ApiResult<Option<String>> {
    match (link.kind.as_str(), link.project_id, link.member_id) {
        ("member", _, Some(m)) if names.members => Ok(store::user_names(&state.pg, &[m])
            .await?
            .into_iter()
            .next()
            .map(|(_, n)| n)),
        ("member", ..) => Ok(None),
        (_, Some(p), _) if names.projects => Ok(store::project_names(&state.pg, link.org_id)
            .await?
            .into_iter()
            .find(|(id, _)| *id == p)
            .map(|(_, n)| n)),
        (_, Some(_), _) => Ok(None),
        _ => Ok(kaeso_store::pg::orgs::get(&state.pg, link.org_id)
            .await?
            .map(|o| o.name)),
    }
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/shared-links",
    summary = "The organization's shared links that still work, newest first.",
    description = "Every link with `usage.share.manage`, else one's own. The addresses\n\
    aren't listed; they were shown once.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = List<SharedLinkView>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
) -> ApiResult<Json<List<SharedLinkView>>> {
    let scope = ctx.org()?;
    let mine = if scope.rights.contains(OrgRight::UsageShareManage) {
        None
    } else {
        Some(person(&ctx)?)
    };
    let links = store::list(&state.pg, scope.org.id, mine, ctx.now).await?;
    Ok(Json(List::complete(
        links.iter().map(SharedLinkView::of).collect(),
    )))
}

#[utoipa::path(
    delete,
    path = "/orgs/{org}/shared-links/{link}",
    summary = "Revokes a shared link; it stops working at once.",
    description = "One's own link, or any with `usage.share.manage`.",
    tag = "orgs",
    params(("org" = OrgId, Path), ("link" = SharedLinkId, Path)),
    responses((status = 204))
)]
pub async fn revoke(
    State(state): State<AppState>,
    ctx: Ctx,
    Path((_org, id)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    let scope = ctx.org()?;
    let id: SharedLinkId = id.parse().map_err(|_| ApiError::not_found("shared link"))?;
    let link = store::get(&state.pg, scope.org.id, id.uuid())
        .await?
        .filter(|l| l.revoked_at.is_none())
        .ok_or_else(|| ApiError::not_found("shared link"))?;
    let actor = ctx.acting_user().map(|u| u.uuid());
    if !scope.rights.contains(OrgRight::UsageShareManage) && actor != Some(link.created_by) {
        return Err(crate::platform::access::denied(
            kaeso_core::rights::Right::Org(OrgRight::UsageShareManage),
            "organization",
        ));
    }
    let mut tx = state.pg.begin().await?;
    store::revoke(&mut *tx, link.id, actor, ctx.now).await?;
    let mut audit = ctx
        .audit(Some(scope.id()), Action::SharedLinkRevoke)
        .target("shared_link", id)
        .before(SharedLinkView::of(&link));
    if let Some(p) = link.project_id {
        audit = audit.project(ProjectId::from_uuid(p));
    }
    audit.write(&mut tx).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/orgs/{org}/security/shared-links",
    summary = "Sets who the organization's shared links may reach and how long they may last.",
    description = "Links made before keep their audience and expiry, but none reaches\n\
    further than the policy now allows: with `off` they stop working, with `members` they\n\
    need a member's sign-in.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    request_body = SharedLinksPolicy,
    responses((status = 200, body = SharedLinksPolicy))
)]
pub async fn set_policy(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(_org): Path<String>,
    Body(req): Body<SharedLinksPolicy>,
) -> ApiResult<Json<SharedLinksPolicy>> {
    let next = check_policy(&req)?;
    let scope = ctx.org()?;
    let mut tx = state.pg.begin().await?;
    let before = store::policy(&mut *tx, scope.org.id).await?;
    store::set_policy(&mut *tx, scope.org.id, &next).await?;
    ctx.audit(Some(scope.id()), Action::SecuritySharedLinks)
        .target("org", scope.id())
        .before(SharedLinksPolicy::of(&before))
        .after(SharedLinksPolicy::of(&next))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(SharedLinksPolicy::of(&next)))
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "What a shared link shows.")]
pub struct SharedView {
    #[doc = "`analytics`, `member`, `budget` or `project_card`."]
    pub kind: String,
    #[doc = "The title its maker gave it."]
    pub title: String,
    #[doc = "The organization's name."]
    pub organization: String,
    #[doc = "The project's name; `null` for the whole organization or when the link\n\
        hides project names."]
    pub project: Option<String>,
    #[doc = "The member's name, for `member`; `null` when the link hides member names."]
    pub member: Option<String>,
    #[doc = "Whether the numbers are of the last days up to today, worked out as the\n\
        page is opened; `false` for numbers kept from when the link was made."]
    pub live: bool,
    #[doc = "The first day shown (UTC)."]
    #[schema(value_type = String, format = Date)]
    #[serde(with = "iso_date")]
    pub from: Date,
    #[doc = "The last day shown (UTC)."]
    #[schema(value_type = String, format = Date)]
    #[serde(with = "iso_date")]
    pub to: Date,
    #[doc = "The metric its chart shows."]
    pub metric: String,
    #[doc = "Whether costs show; without them every `cost` is `null` and spend budgets\n\
        are left out."]
    pub costs: bool,
    #[doc = "The organization's currency, which every `converted_cost` is in; a snapshot\n\
        keeps the one it was made with."]
    #[schema(example = "EUR")]
    pub currency: String,
    #[doc = "When the link was made."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[doc = "When the link stops working; `null` for never."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub expires_at: Option<OffsetDateTime>,
    #[doc = "Whether the page shows a small *Made with Kaeso* mark."]
    pub made_with_kaeso: bool,
    #[doc = "Usage by day in the period, days without usage left out."]
    pub days: Vec<SharedRow>,
    #[doc = "Usage by day in the period of the same length before it, for comparison."]
    pub previous_days: Vec<SharedRow>,
    #[doc = "The period's usage split by model, key, member, route, project or client,\n\
        as the kind of link shows it."]
    pub breakdowns: Vec<SharedBreakdown>,
    #[doc = "The project's limits and budgets, for `budget`."]
    pub limits: Vec<SharedLimit>,
}

struct Opened {
    link: SharedLink,
    names: data::Names,
    costs: bool,
    members_only: bool,
}

// Every check a link passes each time it is opened: it exists and works, the install and the
// organization still allow it, the viewer may see it, and its maker still sees what it shows.
async fn opened(state: &AppState, ctx: &Ctx, token: &str) -> ApiResult<Opened> {
    crate::platform::auth::throttle_ip(state, ctx, OPENS_PER_IP).await?;
    let gone = || ApiError::NotFound(GONE.into());
    let link = store::by_token(&state.pg, &kaeso_core::secret::hash(token))
        .await?
        .filter(|l| l.revoked_at.is_none() && l.expires_at.is_none_or(|at| at > ctx.now))
        .ok_or_else(gone)?;
    let policy = open_policy(state, link.org_id).await?.ok_or_else(gone)?;
    let seen = seen_by(state, link.created_by, link.org_id, link.project_id)
        .await?
        .ok_or_else(gone)?;
    let costs = seen.allows(&link).ok_or_else(gone)?;
    let members_only = link.audience == "members" || policy.allow != "anyone";
    Ok(Opened {
        names: seen.names(&link),
        costs: costs && link.show_costs,
        members_only,
        link,
    })
}

async fn viewer_is_member(state: &AppState, ctx: &Ctx, org: Uuid) -> ApiResult<bool> {
    let Some(user) = ctx.acting_user() else {
        return Ok(false);
    };
    if ctx.agent().is_some() {
        return Ok(false);
    }
    Ok(seen_by(state, user.uuid(), org, None).await?.is_some())
}

fn robots<T: IntoResponse>(body: T) -> Response {
    let mut resp = body.into_response();
    resp.headers_mut().insert(
        header::HeaderName::from_static("x-robots-tag"),
        HeaderValue::from_static("noindex, nofollow"),
    );
    resp
}

#[utoipa::path(
    get,
    path = "/share/{token}",
    summary = "What a shared link shows.",
    description = "Opens a shared link by the token in its address: no sign-in for a link\n\
    for anyone, a session of one of the organization's members otherwise. Each open is\n\
    counted and written to the organization's audit log. Answers carry\n\
    `X-Robots-Tag: noindex`.",
    tag = "share",
    params(("token" = String, Path, description = "The token after `/share/` in the link's address.")),
    responses((status = 200, body = SharedView))
)]
pub async fn open(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(token): Path<String>,
) -> ApiResult<Response> {
    let o = opened(&state, &ctx, &token).await?;
    if o.members_only && !viewer_is_member(&state, &ctx, o.link.org_id).await? {
        return Err(ApiError::Authentication(MEMBERS_ONLY.into()));
    }
    let link = &o.link;
    let today = ctx.now.date();
    let window = data::period(link, today);
    let data = match (&link.snapshot, link.live) {
        (Some(kept), false) => serde_json::from_value::<SharedData>(kept.clone())
            .map_err(ApiError::internal)?
            .kept(),
        _ => data::compute(&state, link, o.names, o.costs, window).await?,
    };
    let data = if o.costs { data } else { data.without_costs() };
    let organization = kaeso_store::pg::orgs::get(&state.pg, link.org_id)
        .await?
        .map(|org| org.name)
        .unwrap_or_default();
    let project = match link.project_id {
        Some(p) if o.names.projects && link.kind != "member" => {
            store::project_names(&state.pg, link.org_id)
                .await?
                .into_iter()
                .find(|(id, _)| *id == p)
                .map(|(_, n)| n)
        }
        _ => None,
    };
    let member = match link.member_id {
        Some(m) if o.names.members => store::user_names(&state.pg, &[m])
            .await?
            .into_iter()
            .next()
            .map(|(_, n)| n),
        _ => None,
    };
    let mark = store::instance(&state.pg).await?.share_mark;
    let mut tx = state.pg.begin().await?;
    store::opened(&mut *tx, link.id, ctx.now).await?;
    let mut audit = ctx
        .audit(Some(OrgId::from_uuid(link.org_id)), Action::SharedLinkOpen)
        .target("shared_link", SharedLinkId::from_uuid(link.id));
    if let Some(p) = link.project_id {
        audit = audit.project(ProjectId::from_uuid(p));
    }
    audit.write(&mut tx).await?;
    tx.commit().await?;
    Ok(robots(Json(SharedView {
        kind: link.kind.clone(),
        title: link.title.clone(),
        organization,
        project,
        member,
        live: link.live,
        from: window.0,
        to: window.1,
        metric: link.metric.clone(),
        costs: data.costs,
        currency: data.currency,
        created_at: link.created_at,
        expires_at: link.expires_at,
        made_with_kaeso: mark,
        days: data.days,
        previous_days: data.previous_days,
        breakdowns: data.breakdowns,
        limits: data.limits,
    })))
}

#[utoipa::path(
    get,
    path = "/share/{token}/image.png",
    summary = "The preview image of a shared link, for chats and social posts.",
    description = "A 1200×630 PNG with the chart of the link's metric over its period; a\n\
    link for members only gets an image without numbers. Not counted as an open.",
    tag = "share",
    params(("token" = String, Path, description = "The token after `/share/` in the link's address.")),
    responses((status = 200, description = "The image", content_type = "image/png"))
)]
pub async fn preview(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(token): Path<String>,
) -> ApiResult<Response> {
    let o = opened(&state, &ctx, &token).await?;
    let link = &o.link;
    let values = if o.members_only || link.kind == "budget" {
        Vec::new()
    } else {
        let window = data::period(link, ctx.now.date());
        let data = match (&link.snapshot, link.live) {
            (Some(kept), false) => {
                serde_json::from_value::<SharedData>(kept.clone()).map_err(ApiError::internal)?
            }
            _ => data::compute(&state, link, o.names, o.costs, window).await?,
        };
        let data = if o.costs { data } else { data.without_costs() };
        image::series(&data.days, window, &link.metric)
    };
    let png = image::render(&values).map_err(ApiError::internal)?;
    Ok(robots((
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("image/png")),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=600"),
            ),
        ],
        png,
    )))
}
