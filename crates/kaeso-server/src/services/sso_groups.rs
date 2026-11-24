//! An organization's role and project memberships from its identity provider's groups: at each
//! sign-in through its provider, the rules that match the ID token's groups set the person's
//! organization role and add their project memberships, and in `sync` mode take away what a
//! rule gave once it no longer matches. Only what a rule gave is ever taken away, and a sign-in
//! whose groups can't be read changes nothing.

use std::collections::BTreeMap;

use kaeso_core::id::{OrgId, ProjectId, UserId};
use kaeso_store::pg::members::{self, Grant};
use kaeso_store::pg::sso_groups::{self as store, Rule};
use kaeso_store::pg::{key_lifecycle, limits, roles};
use serde_json::{Value, json};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::auth::entra;
use crate::auth::oidc::{Claimed, Identity};
use crate::platform::audit::{Action, system_event};
use crate::platform::error::ApiResult;
use crate::platform::grants::RoleRef;
use crate::state::AppState;

pub const ADD: &str = "add";
pub const SYNC: &str = "sync";

/// What applying the rules changed that caches hold, to drop once the transaction committed.
#[derive(Debug, Default)]
pub struct Applied {
    keys: Vec<Uuid>,
    projects: Vec<Uuid>,
    org: Option<(Uuid, Uuid)>,
}

impl Applied {
    pub async fn forget(&self, state: &AppState) {
        crate::services::keys::forget(state, &self.keys).await;
        for project in &self.projects {
            crate::services::catalogue::forget(state, *project).await;
            crate::services::limits::forget(state, *project).await;
        }
        if let Some((org, user)) = self.org {
            crate::services::keys::forget_user_in_org(state, org, user).await;
        }
    }
}

fn grant(preset: &str, role_id: Option<Uuid>) -> Grant {
    Grant {
        preset: preset.to_owned(),
        role_id,
        ..Default::default()
    }
}

fn same(a: &Grant, preset: &str, role_id: Option<Uuid>) -> bool {
    a.preset == preset
        && a.role_id == role_id
        && a.rights_added.is_empty()
        && a.rights_removed.is_empty()
}

fn role_of(g: &Grant) -> Value {
    json!({ "role": RoleRef::of_org(g) })
}

fn project_role_of(g: &Grant) -> Value {
    json!({ "role": RoleRef::of_project(g) })
}

/// The organization's group settings and the groups of one sign-in.
#[derive(Debug)]
pub struct SignInGroups {
    settings: store::Settings,
    groups: Option<Vec<String>>,
}

/// Reads the groups of a sign-in through the organization's provider, before its transaction:
/// for a person in more groups than fit into an Entra ID token, this asks Microsoft Graph.
pub async fn read(
    state: &AppState,
    org: Uuid,
    issuer: &str,
    identity: &Identity,
) -> ApiResult<Option<SignInGroups>> {
    let Some(settings) = store::settings(&state.pg, org).await? else {
        return Ok(None);
    };
    let groups = match identity.groups(&settings.claim) {
        Claimed::Listed(groups) => Some(groups),
        Claimed::Missing => None,
        Claimed::TooMany => from_graph(state, issuer, identity).await,
    };
    Ok(Some(SignInGroups { settings, groups }))
}

async fn from_graph(state: &AppState, issuer: &str, identity: &Identity) -> Option<Vec<String>> {
    let Some(graph) = entra::graph_for(&state.accounts.entra, issuer) else {
        tracing::warn!(
            "an ID token pointed elsewhere for its groups; only Microsoft Entra ID's are read"
        );
        return None;
    };
    match entra::member_groups(
        state.config.auth.oidc_allow_private_networks,
        graph,
        &identity.access_token,
    )
    .await
    {
        Ok(groups) => Some(groups),
        Err(e) => {
            tracing::warn!(error = %e, "the groups of a sign-in could not be read from Microsoft Graph");
            None
        }
    }
}

/// Applies the organization's rules to a member who signed in through its provider.
pub async fn apply(
    tx: &mut sqlx::PgConnection,
    org: Uuid,
    user: Uuid,
    read: Option<&SignInGroups>,
    default: Option<Grant>,
    now: OffsetDateTime,
) -> ApiResult<Applied> {
    let mut applied = Applied::default();
    let Some(SignInGroups { settings, groups }) = read else {
        return Ok(applied);
    };
    let Some(groups) = groups else {
        // Groups that can't be read change nothing: a claim broken at the provider, or a list
        // too long for the token that Graph wouldn't give, must not take anyone's access away.
        store::claim_missing(&mut *tx, org, now).await?;
        return Ok(applied);
    };
    store::set_last_groups(&mut *tx, org, user, groups).await?;
    let rules = store::rules(&mut *tx, org).await?;
    if rules.is_empty() {
        return Ok(applied);
    }
    let matched: Vec<&Rule> = rules
        .iter()
        .filter(|r| groups.contains(&r.group_value))
        .collect();
    let sync = settings.mode == SYNC;
    let org_id = OrgId::from_uuid(org);
    let target = ("user", UserId::from_uuid(user).to_string());

    if let Some(member) = members::get_org(&mut *tx, org, user).await?
        && member.preset != "owner"
    {
        let before = member.grant();
        let from_rule = store::org_from_rule(&mut *tx, org, user).await?;
        let wanted = matched.iter().find_map(|r| {
            r.org_preset
                .as_deref()
                .map(|p| (p, r.org_role_id, &r.group_value))
        });
        let change = match wanted {
            Some((preset, role_id, value)) if !same(&before, preset, role_id) || !from_rule => {
                Some((grant(preset, role_id), true, Some(value.clone())))
            }
            None if sync && from_rule => default.clone().map(|d| (d, false, None)),
            _ => None,
        };
        if let Some((after, by_rule, value)) = change {
            store::set_org(&mut *tx, org, user, &after, by_rule).await?;
            if !same(&before, &after.preset, after.role_id) {
                system_event(
                    tx,
                    Some(org_id),
                    Action::MemberRightsUpdate,
                    Some(target.clone()),
                    Some(role_of(&before)),
                    Some(json!({ "grant": role_of(&after), "via": "sso_groups", "group": value })),
                    now,
                )
                .await?;
                applied.org = Some((org, user));
            }
        }
    }

    let open = store::active_projects(&mut *tx, org).await?;
    let mut wanted: BTreeMap<Uuid, (&str, Option<Uuid>, &str)> = BTreeMap::new();
    for r in &matched {
        for p in &r.projects {
            if open.contains(&p.project_id) {
                wanted.entry(p.project_id).or_insert((
                    p.preset.as_str(),
                    p.role_id,
                    r.group_value.as_str(),
                ));
            }
        }
    }
    let current = store::project_memberships(&mut *tx, org, user).await?;
    // A rule keeps its projects' roles as JSON that no foreign key clears, so a custom role
    // deleted since still stands there; it gives nothing, and still goes before later rules.
    let roles = if wanted.values().any(|(_, role_id, _)| role_id.is_some()) {
        roles::list(&mut *tx, org).await?
    } else {
        Vec::new()
    };
    for (project, (preset, role_id, value)) in &wanted {
        if role_id.is_some_and(|id| !roles.iter().any(|r| r.id == id)) {
            continue;
        }
        let had = current.iter().find(|m| m.project_id == *project);
        let after = grant(preset, *role_id);
        let action = match had {
            None => Action::ProjectMemberAdd,
            Some(m) if m.from_sso_group && (m.preset != *preset || m.role_id != *role_id) => {
                Action::ProjectMemberRightsUpdate
            }
            Some(_) => continue,
        };
        store::put_project(&mut *tx, *project, user, preset, *role_id).await?;
        system_event(
            tx,
            Some(org_id),
            action,
            Some(target.clone()),
            had.map(|m| project_role_of(&grant(&m.preset, m.role_id))),
            Some(json!({
                "project": ProjectId::from_uuid(*project).to_string(),
                "grant": project_role_of(&after),
                "via": "sso_groups",
                "group": value,
            })),
            now,
        )
        .await?;
        applied.projects.push(*project);
        applied.org = Some((org, user));
    }
    if sync {
        for m in current
            .iter()
            .filter(|m| m.from_sso_group && !wanted.contains_key(&m.project_id))
        {
            members::delete_project(&mut *tx, m.project_id, user).await?;
            limits::delete_of_user(&mut *tx, org, Some(m.project_id), user).await?;
            let keys =
                key_lifecycle::archive_user_keys_in_project(&mut *tx, m.project_id, user, now)
                    .await?;
            system_event(
                tx,
                Some(org_id),
                Action::ProjectMemberRemove,
                Some(target.clone()),
                None,
                Some(json!({
                    "project": ProjectId::from_uuid(m.project_id).to_string(),
                    "archived_keys": keys.len(),
                    "via": "sso_groups",
                })),
                now,
            )
            .await?;
            applied.keys.extend(keys);
            applied.projects.push(m.project_id);
        }
    }
    Ok(applied)
}
