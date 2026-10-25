use sqlx::PgExecutor;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::StoreError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Client {
    pub id: Uuid,
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub software_id: Option<String>,
    pub software_version: Option<String>,
    pub created_at: OffsetDateTime,
}

pub async fn insert_client(db: impl PgExecutor<'_>, c: &Client) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO agent_clients (id, name, redirect_uris, software_id, software_version, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6)",
        c.id,
        c.name,
        &c.redirect_uris,
        c.software_id,
        c.software_version,
        c.created_at,
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get_client(db: impl PgExecutor<'_>, id: Uuid) -> Result<Option<Client>, StoreError> {
    Ok(sqlx::query_as!(
        Client,
        "SELECT id, name, redirect_uris, software_id, software_version, created_at \
         FROM agent_clients WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await?)
}

#[derive(Clone, Debug)]
pub struct NewRequest<'a> {
    pub id: Uuid,
    pub token_hash: &'a [u8],
    pub client_id: Uuid,
    pub redirect_uri: &'a str,
    pub state: Option<&'a str>,
    pub code_challenge: &'a str,
    pub now: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

pub async fn insert_request(db: impl PgExecutor<'_>, r: &NewRequest<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO agent_requests (id, token_hash, client_id, redirect_uri, state, code_challenge, created_at, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        r.id,
        r.token_hash,
        r.client_id,
        r.redirect_uri,
        r.state,
        r.code_challenge,
        r.now,
        r.expires_at,
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Request {
    pub id: Uuid,
    pub client_id: Uuid,
    pub client_name: String,
    pub software_id: Option<String>,
    pub redirect_uri: String,
    pub state: Option<String>,
    pub code_challenge: String,
    pub expires_at: OffsetDateTime,
}

pub async fn pending_request(
    db: impl PgExecutor<'_>,
    token_hash: &[u8],
    now: OffsetDateTime,
) -> Result<Option<Request>, StoreError> {
    Ok(sqlx::query_as!(
        Request,
        "SELECT r.id, r.client_id, c.name AS client_name, c.software_id, r.redirect_uri, r.state, \
         r.code_challenge, r.expires_at FROM agent_requests r JOIN agent_clients c ON c.id = r.client_id \
         WHERE r.token_hash = $1 AND r.used_at IS NULL AND r.expires_at > $2",
        token_hash,
        now
    )
    .fetch_optional(db)
    .await?)
}

pub async fn use_request(
    db: impl PgExecutor<'_>,
    id: Uuid,
    now: OffsetDateTime,
) -> Result<bool, StoreError> {
    let done = sqlx::query!(
        "UPDATE agent_requests SET used_at = $2 WHERE id = $1 AND used_at IS NULL AND expires_at > $2",
        id,
        now
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Agent {
    pub id: Uuid,
    pub user_id: Uuid,
    pub client_id: Option<Uuid>,
    pub name: String,
    pub harness: String,
    pub model: Option<String>,
    pub preset: String,
    pub customized: bool,
    pub instance_rights: Vec<String>,
    pub account_rights: Vec<String>,
    pub future_orgs: bool,
    pub future_org_rights: Vec<String>,
    pub future_project_rights: Vec<String>,
    pub sign_in_method: String,
    pub second_factor: bool,
    pub oidc_org_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub last_used_at: Option<OffsetDateTime>,
    pub expires_at: Option<OffsetDateTime>,
    pub revoked_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentOrg {
    pub org_id: Uuid,
    pub included: bool,
    pub rights: Vec<String>,
    pub all_projects: bool,
    pub project_rights: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentProject {
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub rights: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Grant {
    pub preset: String,
    pub customized: bool,
    pub instance_rights: Vec<String>,
    pub account_rights: Vec<String>,
    pub future_orgs: bool,
    pub future_org_rights: Vec<String>,
    pub future_project_rights: Vec<String>,
    pub orgs: Vec<AgentOrg>,
    pub projects: Vec<AgentProject>,
}

pub async fn insert_agent(
    db: &mut sqlx::PgConnection,
    a: &Agent,
    grant: &Grant,
) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO agents (id, user_id, client_id, name, harness, model, preset, customized, \
         instance_rights, account_rights, future_orgs, future_org_rights, future_project_rights, \
         sign_in_method, second_factor, oidc_org_id, created_at, last_used_at, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)",
        a.id,
        a.user_id,
        a.client_id,
        a.name,
        a.harness,
        a.model,
        grant.preset,
        grant.customized,
        &grant.instance_rights,
        &grant.account_rights,
        grant.future_orgs,
        &grant.future_org_rights,
        &grant.future_project_rights,
        a.sign_in_method,
        a.second_factor,
        a.oidc_org_id,
        a.created_at,
        a.last_used_at,
        a.expires_at,
    )
    .execute(&mut *db)
    .await?;
    insert_scopes(db, a.id, grant).await
}

async fn insert_scopes(
    db: &mut sqlx::PgConnection,
    agent: Uuid,
    grant: &Grant,
) -> Result<(), StoreError> {
    for o in &grant.orgs {
        sqlx::query!(
            "INSERT INTO agent_orgs (agent_id, org_id, included, rights, all_projects, project_rights) \
             VALUES ($1, $2, $3, $4, $5, $6)",
            agent,
            o.org_id,
            o.included,
            &o.rights,
            o.all_projects,
            &o.project_rights,
        )
        .execute(&mut *db)
        .await?;
    }
    for p in &grant.projects {
        sqlx::query!(
            "INSERT INTO agent_projects (agent_id, org_id, project_id, rights) VALUES ($1, $2, $3, $4)",
            agent,
            p.org_id,
            p.project_id,
            &p.rights,
        )
        .execute(&mut *db)
        .await?;
    }
    Ok(())
}

pub async fn replace_grant(
    db: &mut sqlx::PgConnection,
    agent: Uuid,
    grant: &Grant,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE agents SET preset = $2, customized = $3, instance_rights = $4, account_rights = $5, \
         future_orgs = $6, future_org_rights = $7, future_project_rights = $8 WHERE id = $1",
        agent,
        grant.preset,
        grant.customized,
        &grant.instance_rights,
        &grant.account_rights,
        grant.future_orgs,
        &grant.future_org_rights,
        &grant.future_project_rights,
    )
    .execute(&mut *db)
    .await?;
    sqlx::query!("DELETE FROM agent_orgs WHERE agent_id = $1", agent)
        .execute(&mut *db)
        .await?;
    insert_scopes(db, agent, grant).await
}

pub async fn get(
    db: impl PgExecutor<'_>,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<Agent>, StoreError> {
    Ok(sqlx::query_as!(
        Agent,
        "SELECT id, user_id, client_id, name, harness, model, preset, customized, instance_rights, \
         account_rights, future_orgs, future_org_rights, future_project_rights, sign_in_method, \
         second_factor, oidc_org_id, created_at, last_used_at, expires_at, revoked_at \
         FROM agents WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL",
        id,
        user_id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn lock(
    db: &mut sqlx::PgConnection,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<Agent>, StoreError> {
    Ok(sqlx::query_as!(
        Agent,
        "SELECT id, user_id, client_id, name, harness, model, preset, customized, instance_rights, \
         account_rights, future_orgs, future_org_rights, future_project_rights, sign_in_method, \
         second_factor, oidc_org_id, created_at, last_used_at, expires_at, revoked_at \
         FROM agents WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL FOR UPDATE",
        id,
        user_id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn list(db: impl PgExecutor<'_>, user_id: Uuid) -> Result<Vec<Agent>, StoreError> {
    Ok(sqlx::query_as!(
        Agent,
        "SELECT id, user_id, client_id, name, harness, model, preset, customized, instance_rights, \
         account_rights, future_orgs, future_org_rights, future_project_rights, sign_in_method, \
         second_factor, oidc_org_id, created_at, last_used_at, expires_at, revoked_at \
         FROM agents WHERE user_id = $1 AND revoked_at IS NULL ORDER BY id DESC",
        user_id
    )
    .fetch_all(db)
    .await?)
}

pub async fn orgs(db: impl PgExecutor<'_>, agent: Uuid) -> Result<Vec<AgentOrg>, StoreError> {
    Ok(sqlx::query_as!(
        AgentOrg,
        "SELECT org_id, included, rights, all_projects, project_rights FROM agent_orgs \
         WHERE agent_id = $1 ORDER BY org_id",
        agent
    )
    .fetch_all(db)
    .await?)
}

pub async fn projects(
    db: impl PgExecutor<'_>,
    agent: Uuid,
) -> Result<Vec<AgentProject>, StoreError> {
    Ok(sqlx::query_as!(
        AgentProject,
        "SELECT org_id, project_id, rights FROM agent_projects WHERE agent_id = $1 ORDER BY project_id",
        agent
    )
    .fetch_all(db)
    .await?)
}

pub async fn grant_of(db: &mut sqlx::PgConnection, a: &Agent) -> Result<Grant, StoreError> {
    Ok(Grant {
        preset: a.preset.clone(),
        customized: a.customized,
        instance_rights: a.instance_rights.clone(),
        account_rights: a.account_rights.clone(),
        future_orgs: a.future_orgs,
        future_org_rights: a.future_org_rights.clone(),
        future_project_rights: a.future_project_rights.clone(),
        orgs: orgs(&mut *db, a.id).await?,
        projects: projects(&mut *db, a.id).await?,
    })
}

pub struct Details<'a> {
    pub name: &'a str,
    pub harness: &'a str,
    pub expires_at: Option<OffsetDateTime>,
}

pub async fn update(db: impl PgExecutor<'_>, id: Uuid, d: &Details<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE agents SET name = $2, harness = $3, expires_at = $4 WHERE id = $1",
        id,
        d.name,
        d.harness,
        d.expires_at,
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_model(
    db: impl PgExecutor<'_>,
    id: Uuid,
    model: Option<&str>,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE agents SET model = $2 WHERE id = $1 AND model IS DISTINCT FROM $2",
        id,
        model
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn revoke(
    db: &mut sqlx::PgConnection,
    id: Uuid,
    now: OffsetDateTime,
) -> Result<bool, StoreError> {
    let done = sqlx::query!(
        "UPDATE agents SET revoked_at = $2 WHERE id = $1 AND revoked_at IS NULL",
        id,
        now
    )
    .execute(&mut *db)
    .await?;
    sqlx::query!(
        "UPDATE agent_tokens SET revoked_at = $2 WHERE agent_id = $1 AND revoked_at IS NULL",
        id,
        now
    )
    .execute(&mut *db)
    .await?;
    Ok(done.rows_affected() == 1)
}

pub async fn touch(
    db: impl PgExecutor<'_>,
    id: Uuid,
    now: OffsetDateTime,
    every: time::Duration,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE agents SET last_used_at = $2 WHERE id = $1 AND (last_used_at IS NULL OR last_used_at < $3)",
        id,
        now,
        now - every
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn revoke_unused(
    db: impl PgExecutor<'_>,
    unused_since: OffsetDateTime,
    now: OffsetDateTime,
) -> Result<u64, StoreError> {
    let done = sqlx::query!(
        "UPDATE agents SET revoked_at = $2 WHERE revoked_at IS NULL \
         AND ((expires_at IS NOT NULL AND expires_at <= $2) OR COALESCE(last_used_at, created_at) < $1)",
        unused_since,
        now
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Code,
    Access,
    Refresh,
}

impl TokenKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TokenKind::Code => "code",
            TokenKind::Access => "access",
            TokenKind::Refresh => "refresh",
        }
    }
}

#[derive(Clone, Debug)]
pub struct NewToken<'a> {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub kind: TokenKind,
    pub token_hash: &'a [u8],
    pub family: Uuid,
    pub redirect_uri: Option<&'a str>,
    pub code_challenge: Option<&'a str>,
    pub now: OffsetDateTime,
    pub expires_at: Option<OffsetDateTime>,
}

pub async fn insert_token(db: impl PgExecutor<'_>, t: &NewToken<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO agent_tokens (id, agent_id, kind, token_hash, family, redirect_uri, code_challenge, created_at, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        t.id,
        t.agent_id,
        t.kind.as_str(),
        t.token_hash,
        t.family,
        t.redirect_uri,
        t.code_challenge,
        t.now,
        t.expires_at,
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Debug)]
pub struct UsedToken {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub family: Uuid,
    pub redirect_uri: Option<String>,
    pub code_challenge: Option<String>,
    pub client_id: Option<Uuid>,
    pub fresh: bool,
}

// A token is spent by the UPDATE that marks it, so two requests racing with the same code or
// refresh token can't both win; the loser sees `fresh = false` and the family is revoked.
pub async fn spend(
    db: &mut sqlx::PgConnection,
    kind: TokenKind,
    token_hash: &[u8],
    now: OffsetDateTime,
) -> Result<Option<UsedToken>, StoreError> {
    let found = sqlx::query!(
        "SELECT t.id, t.agent_id, t.family, t.redirect_uri, t.code_challenge, a.client_id \
         FROM agent_tokens t JOIN agents a ON a.id = t.agent_id \
         WHERE t.token_hash = $1 AND t.kind = $2 AND t.revoked_at IS NULL AND a.revoked_at IS NULL \
         AND (t.expires_at IS NULL OR t.expires_at > $3)",
        token_hash,
        kind.as_str(),
        now
    )
    .fetch_optional(&mut *db)
    .await?;
    let Some(found) = found else {
        return Ok(None);
    };
    let spent = sqlx::query!(
        "UPDATE agent_tokens SET used_at = $2 WHERE id = $1 AND used_at IS NULL",
        found.id,
        now
    )
    .execute(&mut *db)
    .await?;
    Ok(Some(UsedToken {
        id: found.id,
        agent_id: found.agent_id,
        family: found.family,
        redirect_uri: found.redirect_uri,
        code_challenge: found.code_challenge,
        client_id: found.client_id,
        fresh: spent.rows_affected() == 1,
    }))
}

pub async fn revoke_family(
    db: impl PgExecutor<'_>,
    family: Uuid,
    now: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE agent_tokens SET revoked_at = $2 WHERE family = $1 AND revoked_at IS NULL",
        family,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn revoke_token(
    db: impl PgExecutor<'_>,
    token_hash: &[u8],
    now: OffsetDateTime,
) -> Result<Option<Uuid>, StoreError> {
    Ok(sqlx::query_scalar!(
        "UPDATE agent_tokens SET revoked_at = $2 WHERE token_hash = $1 AND kind <> 'code' \
         AND revoked_at IS NULL RETURNING family",
        token_hash,
        now
    )
    .fetch_optional(db)
    .await?)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentAuth {
    pub agent: Agent,
    pub token_expires_at: Option<OffsetDateTime>,
    pub user_state: String,
    pub is_instance_admin: bool,
}

pub async fn authenticate(
    db: impl PgExecutor<'_>,
    token_hash: &[u8],
    now: OffsetDateTime,
) -> Result<Option<AgentAuth>, StoreError> {
    let row = sqlx::query!(
        "SELECT a.id, a.user_id, a.client_id, a.name, a.harness, a.model, a.preset, a.customized, \
         a.instance_rights, a.account_rights, a.future_orgs, a.future_org_rights, a.future_project_rights, \
         a.sign_in_method, a.second_factor, a.oidc_org_id, a.created_at, a.last_used_at, a.expires_at, \
         a.revoked_at, t.expires_at AS token_expires_at, u.state AS user_state, u.is_instance_admin \
         FROM agent_tokens t JOIN agents a ON a.id = t.agent_id JOIN users u ON u.id = a.user_id \
         WHERE t.token_hash = $1 AND t.kind = 'access' AND t.revoked_at IS NULL AND a.revoked_at IS NULL \
         AND (t.expires_at IS NULL OR t.expires_at > $2)",
        token_hash,
        now
    )
    .fetch_optional(db)
    .await?;
    Ok(row.map(|r| AgentAuth {
        agent: Agent {
            id: r.id,
            user_id: r.user_id,
            client_id: r.client_id,
            name: r.name,
            harness: r.harness,
            model: r.model,
            preset: r.preset,
            customized: r.customized,
            instance_rights: r.instance_rights,
            account_rights: r.account_rights,
            future_orgs: r.future_orgs,
            future_org_rights: r.future_org_rights,
            future_project_rights: r.future_project_rights,
            sign_in_method: r.sign_in_method,
            second_factor: r.second_factor,
            oidc_org_id: r.oidc_org_id,
            created_at: r.created_at,
            last_used_at: r.last_used_at,
            expires_at: r.expires_at,
            revoked_at: r.revoked_at,
        },
        token_expires_at: r.token_expires_at,
        user_state: r.user_state,
        is_instance_admin: r.is_instance_admin,
    }))
}

pub async fn agent_for_refresh(
    db: impl PgExecutor<'_>,
    id: Uuid,
) -> Result<Option<AgentAuth>, StoreError> {
    let row = sqlx::query!(
        "SELECT a.id, a.user_id, a.client_id, a.name, a.harness, a.model, a.preset, a.customized, \
         a.instance_rights, a.account_rights, a.future_orgs, a.future_org_rights, a.future_project_rights, \
         a.sign_in_method, a.second_factor, a.oidc_org_id, a.created_at, a.last_used_at, a.expires_at, \
         a.revoked_at, u.state AS user_state, u.is_instance_admin \
         FROM agents a JOIN users u ON u.id = a.user_id WHERE a.id = $1 AND a.revoked_at IS NULL",
        id
    )
    .fetch_optional(db)
    .await?;
    Ok(row.map(|r| AgentAuth {
        agent: Agent {
            id: r.id,
            user_id: r.user_id,
            client_id: r.client_id,
            name: r.name,
            harness: r.harness,
            model: r.model,
            preset: r.preset,
            customized: r.customized,
            instance_rights: r.instance_rights,
            account_rights: r.account_rights,
            future_orgs: r.future_orgs,
            future_org_rights: r.future_org_rights,
            future_project_rights: r.future_project_rights,
            sign_in_method: r.sign_in_method,
            second_factor: r.second_factor,
            oidc_org_id: r.oidc_org_id,
            created_at: r.created_at,
            last_used_at: r.last_used_at,
            expires_at: r.expires_at,
            revoked_at: r.revoked_at,
        },
        token_expires_at: None,
        user_state: r.user_state,
        is_instance_admin: r.is_instance_admin,
    }))
}

#[derive(Clone, Debug)]
pub struct NewActivity<'a> {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub tool: &'a str,
    pub org_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub target: Option<&'a str>,
    pub status: i16,
    pub result: &'a str,
    pub ip: Option<std::net::IpAddr>,
    pub at: OffsetDateTime,
}

pub async fn insert_activity(
    db: impl PgExecutor<'_>,
    a: &NewActivity<'_>,
) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO agent_activity (id, agent_id, tool, org_id, project_id, target, status, result, ip, at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        a.id,
        a.agent_id,
        a.tool,
        a.org_id,
        a.project_id,
        a.target,
        a.status,
        a.result,
        a.ip.map(ipnet::IpNet::from),
        a.at,
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Activity {
    pub id: Uuid,
    pub tool: String,
    pub org_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub target: Option<String>,
    pub status: i16,
    pub result: String,
    pub ip: Option<ipnet::IpNet>,
    pub at: OffsetDateTime,
}

pub async fn activity(
    db: impl PgExecutor<'_>,
    agent: Uuid,
    before: Option<Uuid>,
    limit: i64,
) -> Result<Vec<Activity>, StoreError> {
    Ok(sqlx::query_as!(
        Activity,
        "SELECT id, tool, org_id, project_id, target, status, result, ip, at FROM agent_activity \
         WHERE agent_id = $1 AND ($2::uuid IS NULL OR id < $2) ORDER BY id DESC LIMIT $3",
        agent,
        before,
        limit
    )
    .fetch_all(db)
    .await?)
}

#[derive(Clone, Debug)]
pub struct OrgAgent {
    pub id: Uuid,
    pub name: String,
    pub harness: String,
    pub model: Option<String>,
    pub user_id: Uuid,
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub user_avatar: Option<String>,
    pub created_at: OffsetDateTime,
    pub last_used_at: Option<OffsetDateTime>,
    pub expires_at: Option<OffsetDateTime>,
}

pub async fn in_org(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    now: OffsetDateTime,
) -> Result<Vec<OrgAgent>, StoreError> {
    Ok(sqlx::query_as!(
        OrgAgent,
        "SELECT a.id, a.name, a.harness, a.model, a.user_id, u.name AS user_name, u.email AS user_email, \
         u.avatar AS user_avatar, a.created_at, a.last_used_at, a.expires_at \
         FROM agents a JOIN org_memberships m ON m.user_id = a.user_id AND m.org_id = $1 \
         JOIN users u ON u.id = a.user_id \
         LEFT JOIN agent_orgs ao ON ao.agent_id = a.id AND ao.org_id = $1 \
         WHERE a.revoked_at IS NULL AND (a.expires_at IS NULL OR a.expires_at > $2) \
         AND (ao.included OR (ao.agent_id IS NULL AND a.future_orgs)) ORDER BY a.id DESC",
        org_id,
        now
    )
    .fetch_all(db)
    .await?)
}

pub async fn exclude_org(
    db: &mut sqlx::PgConnection,
    agent: Uuid,
    org_id: Uuid,
) -> Result<(), StoreError> {
    sqlx::query!(
        "INSERT INTO agent_orgs (agent_id, org_id, included) VALUES ($1, $2, false) \
         ON CONFLICT (agent_id, org_id) DO UPDATE SET included = false, rights = '{}', project_rights = '{}'",
        agent,
        org_id
    )
    .execute(&mut *db)
    .await?;
    sqlx::query!(
        "DELETE FROM agent_projects WHERE agent_id = $1 AND org_id = $2",
        agent,
        org_id
    )
    .execute(&mut *db)
    .await?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy<'a> {
    pub allowed: bool,
    pub max: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredPolicy {
    pub allowed: bool,
    pub max: String,
}

pub async fn instance_policy(db: impl PgExecutor<'_>) -> Result<StoredPolicy, StoreError> {
    let row = sqlx::query!("SELECT agents_allowed, agents_max FROM instance_settings LIMIT 1")
        .fetch_optional(db)
        .await?;
    Ok(row.map_or_else(
        || StoredPolicy {
            allowed: true,
            max: "configure".to_owned(),
        },
        |r| StoredPolicy {
            allowed: r.agents_allowed,
            max: r.agents_max,
        },
    ))
}

pub async fn set_instance_policy(db: impl PgExecutor<'_>, p: Policy<'_>) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE instance_settings SET agents_allowed = $1, agents_max = $2",
        p.allowed,
        p.max
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn org_policy(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
) -> Result<Option<StoredPolicy>, StoreError> {
    Ok(sqlx::query_as!(
        StoredPolicy,
        "SELECT agents_allowed AS allowed, agents_max AS max FROM organizations WHERE id = $1",
        org_id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn org_policies(
    db: impl PgExecutor<'_>,
    org_ids: &[Uuid],
) -> Result<Vec<(Uuid, StoredPolicy)>, StoreError> {
    let rows = sqlx::query!(
        "SELECT id, agents_allowed, agents_max FROM organizations WHERE id = ANY($1)",
        org_ids
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            (
                r.id,
                StoredPolicy {
                    allowed: r.agents_allowed,
                    max: r.agents_max,
                },
            )
        })
        .collect())
}

pub async fn set_org_policy(
    db: impl PgExecutor<'_>,
    org_id: Uuid,
    p: Policy<'_>,
) -> Result<(), StoreError> {
    sqlx::query!(
        "UPDATE organizations SET agents_allowed = $2, agents_max = $3 WHERE id = $1",
        org_id,
        p.allowed,
        p.max
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Swept {
    pub requests: u64,
    pub tokens: u64,
    pub clients: u64,
    pub activity: u64,
}

pub async fn sweep(
    db: &mut sqlx::PgConnection,
    now: OffsetDateTime,
    activity_before: OffsetDateTime,
    clients_before: OffsetDateTime,
) -> Result<Swept, StoreError> {
    let requests = sqlx::query!("DELETE FROM agent_requests WHERE expires_at < $1", now)
        .execute(&mut *db)
        .await?
        .rows_affected();
    let tokens = sqlx::query!(
        "DELETE FROM agent_tokens WHERE (expires_at IS NOT NULL AND expires_at < $1) \
         OR (revoked_at IS NOT NULL AND revoked_at < $2)",
        now,
        now - time::Duration::days(1)
    )
    .execute(&mut *db)
    .await?
    .rows_affected();
    let clients = sqlx::query!(
        "DELETE FROM agent_clients c WHERE c.created_at < $1 \
         AND NOT EXISTS (SELECT 1 FROM agents a WHERE a.client_id = c.id) \
         AND NOT EXISTS (SELECT 1 FROM agent_requests r WHERE r.client_id = c.id)",
        clients_before
    )
    .execute(&mut *db)
    .await?
    .rows_affected();
    let activity = sqlx::query!("DELETE FROM agent_activity WHERE at < $1", activity_before)
        .execute(&mut *db)
        .await?
        .rows_affected();
    Ok(Swept {
        requests,
        tokens,
        clients,
        activity,
    })
}

#[derive(Clone, Debug)]
pub struct OrgProject {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub is_default: bool,
    pub avatar: Option<String>,
}

pub async fn active_projects(
    db: impl PgExecutor<'_>,
    org_ids: &[Uuid],
) -> Result<Vec<OrgProject>, StoreError> {
    Ok(sqlx::query_as!(
        OrgProject,
        "SELECT id, org_id, name, is_default, avatar FROM projects \
         WHERE org_id = ANY($1) AND archived_at IS NULL ORDER BY org_id, is_default DESC, name, id",
        org_ids
    )
    .fetch_all(db)
    .await?)
}
