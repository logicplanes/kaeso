use std::net::IpAddr;
use std::sync::Arc;

use kaeso_core::keys::{KeyKind, kind_of};
use kaeso_core::rights::{OrgGrant, ProjectGrant, ProjectRights, effective_project_rights};
use kaeso_core::secret;
use kaeso_store::StoreError;
use kaeso_store::pg::keys::{self, KeyAuth, OrgKeyAuth};
use sqlx::Postgres;
use sqlx::pool::PoolConnection;
use time::OffsetDateTime;

use super::key_failures::{Gate, Throttled};
use crate::cache::{TieredCache, hex};
use crate::state::AppState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Invalid,
    Disabled,
    Archived,
    Expired,
    OrgDeleting,
    ProjectArchived,
    PrincipalInactive,
}

impl Refusal {
    pub fn message(self) -> &'static str {
        match self {
            Refusal::Invalid => "This API key is not valid; check that it is complete and current.",
            Refusal::Disabled => "This API key is disabled; a project admin can enable it again.",
            Refusal::Archived => "This API key was archived and no longer works.",
            Refusal::Expired => "This API key has expired.",
            Refusal::OrgDeleting => {
                "This organization is scheduled for deletion; its keys no longer work."
            }
            Refusal::ProjectArchived => "This key's project is archived; its keys no longer work.",
            Refusal::PrincipalInactive => {
                "The account or service account this key belongs to is no longer active."
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum KeyCaller {
    Project(Arc<KeyAuth>),
    Org(Arc<OrgKeyAuth>),
}

#[derive(Debug, thiserror::Error)]
pub enum LookupError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("too many failed API key lookups from this address")]
    Throttled(Throttled),
}

impl From<Throttled> for LookupError {
    fn from(t: Throttled) -> Self {
        LookupError::Throttled(t)
    }
}

pub async fn authenticate(
    state: &AppState,
    presented: &str,
    client: Option<IpAddr>,
) -> Result<Result<KeyCaller, Refusal>, LookupError> {
    let Some(kind) = kind_of(presented) else {
        return Ok(Err(Refusal::Invalid));
    };
    let hash = secret::hash(presented);
    let now = state.clock.now();
    match kind {
        KeyKind::Project => {
            let found = find(
                state,
                &state.caches.api_keys,
                "key",
                &hash,
                client,
                |k| &k.secret_hash,
                |mut conn, hash| async move { keys::auth_by_hash(&mut *conn, &hash).await },
            )
            .await?;
            let Some(key) = found else {
                return Ok(Err(Refusal::Invalid));
            };
            if let Err(r) = check_project_key(&key, now) {
                return Ok(Err(r));
            }
            touch(state, key.id, key.last_used_at, now, false).await;
            Ok(Ok(KeyCaller::Project(key)))
        }
        KeyKind::OrgPlatform => {
            let found = find(
                state,
                &state.caches.org_keys,
                "opk",
                &hash,
                client,
                |k| &k.secret_hash,
                |mut conn, hash| async move { keys::org_auth_by_hash(&mut *conn, &hash).await },
            )
            .await?;
            let Some(key) = found else {
                return Ok(Err(Refusal::Invalid));
            };
            if let Err(r) = check_org_key(&key, now) {
                return Ok(Err(r));
            }
            touch(state, key.id, key.last_used_at, now, true).await;
            Ok(Ok(KeyCaller::Org(key)))
        }
    }
}

async fn find<V, F, Fut>(
    state: &AppState,
    cache: &TieredCache<V>,
    name: &str,
    hash: &[u8],
    client: Option<IpAddr>,
    hash_of: fn(&V) -> &[u8],
    load: F,
) -> Result<Option<Arc<V>>, LookupError>
where
    V: Clone + serde::Serialize + serde::de::DeserializeOwned + Send + Sync + 'static,
    F: FnOnce(PoolConnection<Postgres>, Vec<u8>) -> Fut,
    Fut: Future<Output = Result<Option<V>, StoreError>>,
{
    let id = hex(hash);
    let matches = |v: &Arc<V>| secret::hashes_equal(hash_of(v), hash);
    if let Some(v) = cache.cached(&id).await.filter(matches) {
        return Ok(Some(v));
    }
    let unknown = format!("{name}:{id}");
    if state.caches.unknown_keys.contains_key(&unknown) {
        return Ok(None);
    }
    let gate = Gate::new(state, client);
    if let Some(g) = &gate {
        g.check().await?;
    }
    let pg = state.pg.clone();
    let owned = hash.to_vec();
    let waiting = gate.clone();
    let found = cache
        .get_or_load(&id, || async move {
            let conn = pg.acquire().await.map_err(StoreError::from)?;
            if let Some(g) = &waiting {
                g.check().await?;
            }
            Ok::<_, LookupError>(load(conn, owned).await?)
        })
        .await?
        .filter(matches);
    if found.is_none() {
        state.caches.unknown_keys.insert(unknown, ()).await;
        if let Some(g) = &gate {
            g.failed().await?;
        }
    }
    Ok(found)
}

pub fn check_project_key(k: &KeyAuth, now: OffsetDateTime) -> Result<(), Refusal> {
    if k.archived_at.is_some() {
        return Err(Refusal::Archived);
    }
    if k.disabled_at.is_some() {
        return Err(Refusal::Disabled);
    }
    if k.expires_at.is_some_and(|e| e <= now) {
        return Err(Refusal::Expired);
    }
    if k.org_state != "active" {
        return Err(Refusal::OrgDeleting);
    }
    if k.project_archived_at.is_some() {
        return Err(Refusal::ProjectArchived);
    }
    let principal_active = match (k.user_id, k.service_account_id) {
        (Some(_), None) => k.user_state.as_deref() == Some("active"),
        (None, Some(_)) => k.service_account_archived_at.is_none(),
        _ => false,
    };
    if !principal_active {
        return Err(Refusal::PrincipalInactive);
    }
    Ok(())
}

pub fn check_org_key(k: &OrgKeyAuth, now: OffsetDateTime) -> Result<(), Refusal> {
    if k.archived_at.is_some() {
        return Err(Refusal::Archived);
    }
    if k.disabled_at.is_some() {
        return Err(Refusal::Disabled);
    }
    if k.expires_at.is_some_and(|e| e <= now) {
        return Err(Refusal::Expired);
    }
    if k.org_state != "active" {
        return Err(Refusal::OrgDeleting);
    }
    Ok(())
}

pub fn project_rights(k: &KeyAuth) -> ProjectRights {
    use crate::platform::grants::{org_from_store, project_from_store};
    use kaeso_store::pg::members::Grant;
    let org = || -> Option<OrgGrant> {
        org_from_store(&Grant {
            preset: k.org_preset.clone()?,
            role_id: k.org_role_id,
            rights_added: k.org_rights_added.clone(),
            rights_removed: k.org_rights_removed.clone(),
            role_name: None,
            role_rights: k.org_role_rights.clone(),
            role_project_rights: k.org_role_project_rights.clone(),
        })
        .ok()
    };
    let project = || -> Option<ProjectGrant> {
        project_from_store(&Grant {
            preset: k.project_preset.clone()?,
            role_id: k.project_role_id,
            rights_added: k.project_rights_added.clone(),
            rights_removed: k.project_rights_removed.clone(),
            role_name: None,
            role_rights: k.project_role_rights.clone(),
            role_project_rights: None,
        })
        .ok()
    };
    match (k.user_id, k.service_account_id) {
        (Some(_), _) => match org() {
            Some(org) => effective_project_rights(Some(&org), project().as_ref()),
            None => ProjectRights::EMPTY,
        },
        (None, Some(_)) => project().map_or(ProjectRights::EMPTY, |g| g.effective()),
        (None, None) => ProjectRights::EMPTY,
    }
}

async fn touch(
    state: &AppState,
    id: uuid::Uuid,
    last: Option<OffsetDateTime>,
    now: OffsetDateTime,
    org_key: bool,
) {
    if last.is_some_and(|l| now - l < time::Duration::minutes(1)) {
        return;
    }
    if !state
        .caches
        .touched
        .entry(id)
        .or_insert(())
        .await
        .is_fresh()
    {
        return;
    }
    let pg = state.pg.clone();
    tokio::spawn(async move {
        let result = if org_key {
            keys::touch_org(&pg, id, now).await
        } else {
            keys::touch(&pg, id, now).await
        };
        if let Err(e) = result {
            tracing::warn!(error = %e, "cannot record the key's last use");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;
    use uuid::Uuid;

    fn key() -> KeyAuth {
        KeyAuth {
            id: Uuid::nil(),
            secret_hash: vec![1],
            org_id: Uuid::nil(),
            org_state: "active".into(),
            project_id: Uuid::nil(),
            project_archived_at: None,
            user_id: Some(Uuid::nil()),
            user_state: Some("active".into()),
            service_account_id: None,
            service_account_archived_at: None,
            inference: true,
            platform: false,
            expires_at: Some(datetime!(2026-11-01 0:00 UTC)),
            disabled_at: None,
            archived_at: None,
            last_used_at: None,
            org_preset: Some("member".into()),
            org_role_id: None,
            org_role_rights: None,
            org_role_project_rights: None,
            org_rights_added: vec![],
            org_rights_removed: vec![],
            project_preset: Some("developer".into()),
            project_role_id: None,
            project_role_rights: None,
            project_rights_added: vec![],
            project_rights_removed: vec!["api.request".into()],
        }
    }

    #[test]
    fn every_state_is_checked() {
        let now = datetime!(2026-10-11 12:00 UTC);
        assert_eq!(check_project_key(&key(), now), Ok(()));
        assert_eq!(
            check_project_key(&key(), datetime!(2026-11-01 0:00 UTC)),
            Err(Refusal::Expired)
        );
        let mut k = key();
        k.disabled_at = Some(now);
        assert_eq!(check_project_key(&k, now), Err(Refusal::Disabled));
        let mut k = key();
        k.org_state = "scheduled_for_deletion".into();
        assert_eq!(check_project_key(&k, now), Err(Refusal::OrgDeleting));
        let mut k = key();
        k.user_state = Some("deactivated".into());
        assert_eq!(check_project_key(&k, now), Err(Refusal::PrincipalInactive));
        let mut k = key();
        k.project_archived_at = Some(now);
        assert_eq!(check_project_key(&k, now), Err(Refusal::ProjectArchived));
    }

    #[test]
    fn keys_act_with_their_principals_rights() {
        let k = key();
        let rights = project_rights(&k);
        assert!(!rights.contains(kaeso_core::rights::ProjectRight::ApiRequest));
        assert!(rights.contains(kaeso_core::rights::ProjectRight::KeysOwnCreate));
        let mut gone = key();
        gone.org_preset = None;
        assert!(project_rights(&gone).is_empty());
        let mut admin = key();
        admin.org_preset = Some("admin".into());
        admin.project_preset = None;
        assert_eq!(project_rights(&admin), ProjectRights::all());
        let mut custom = key();
        custom.project_preset = Some("custom".into());
        custom.project_role_id = Some(Uuid::nil());
        custom.project_role_rights = Some(vec!["project.view".into(), "api.request".into()]);
        custom.project_rights_removed = vec![];
        assert_eq!(
            project_rights(&custom).names(),
            ["project.view", "api.request"]
        );
        let mut org_role = key();
        org_role.org_preset = Some("custom".into());
        org_role.org_role_id = Some(Uuid::nil());
        org_role.org_role_rights = Some(vec!["org.view".into()]);
        org_role.org_role_project_rights = Some(vec!["project.view".into(), "usage.view".into()]);
        org_role.project_preset = None;
        assert_eq!(
            project_rights(&org_role).names(),
            ["project.view", "usage.view"]
        );
    }
}
