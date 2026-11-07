use std::collections::{BTreeSet, HashMap};

use kaeso_core::id::{OrgId, ProjectId};
use kaeso_core::notifications::Kind;
use kaeso_core::rights::ProjectRight;
use kaeso_mail::Locale;
use kaeso_store::StoreError;
use kaeso_store::pg::{keys, members, notifications, users};
use uuid::Uuid;

use crate::jobs::JobError;
use crate::state::AppState;

pub async fn run(state: AppState) -> Result<String, JobError> {
    let now = state.clock.now();
    let zone = crate::time_zone::or_utc(&kaeso_store::pg::instance::time_zone(&state.pg).await?);
    let zone_name = zone.iana_name().unwrap_or("UTC");
    let mut sent = 0;
    for days in [1, 7] {
        let expiring = keys::expiring(&state.pg, now, days).await?;
        if expiring.is_empty() {
            continue;
        }
        let ids: Vec<Uuid> = expiring.iter().map(|k| k.id).collect();
        let claimed: BTreeSet<Uuid> = keys::claim_warnings(&state.pg, &ids, days)
            .await?
            .into_iter()
            .collect();
        let claimed_ids: Vec<Uuid> = claimed.iter().copied().collect();
        if days == 1 {
            keys::claim_warnings(&state.pg, &claimed_ids, 7).await?;
        }
        let expiring: Vec<keys::Expiring> = expiring
            .into_iter()
            .filter(|k| claimed.contains(&k.id))
            .collect();
        let shared: BTreeSet<(Uuid, Uuid)> = expiring
            .iter()
            .filter(|k| k.user_id.is_none())
            .map(|k| (k.org_id, k.project_id))
            .collect();
        let mut admins: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for (org, project) in shared {
            admins.insert(project, project_admins(&state, org, project).await?);
        }
        let mut wanted: BTreeSet<Uuid> = expiring.iter().filter_map(|k| k.user_id).collect();
        wanted.extend(admins.values().flatten().copied());
        let wanted: Vec<Uuid> = wanted.into_iter().collect();
        let people: HashMap<Uuid, users::Recipient> = users::recipients(&state.pg, &wanted)
            .await?
            .into_iter()
            .map(|r| (r.id, r))
            .collect();
        let off: BTreeSet<Uuid> =
            notifications::turned_off(&state.pg, &wanted, Kind::KeyExpiry.as_str())
                .await?
                .into_iter()
                .collect();
        let muted: BTreeSet<(Uuid, Uuid)> = notifications::mutes_of(&state.pg, &wanted)
            .await?
            .into_iter()
            .collect();
        let mut organizations = std::collections::BTreeMap::new();
        for key in &expiring {
            if let std::collections::btree_map::Entry::Vacant(slot) =
                organizations.entry(key.org_id)
            {
                slot.insert(
                    crate::services::notifications::triggers::org_name(&state, key.org_id).await?,
                );
            }
        }
        for key in &expiring {
            let organization = organizations
                .get(&key.org_id)
                .map_or_else(String::new, Clone::clone);
            let to: &[Uuid] = match &key.user_id {
                Some(user) => std::slice::from_ref(user),
                None => admins.get(&key.project_id).map_or(&[], Vec::as_slice),
            };
            let url = format!(
                "{}orgs/{}/projects/{}/keys",
                state.config.public_url,
                OrgId::from_uuid(key.org_id),
                ProjectId::from_uuid(key.project_id)
            );
            for person in to.iter().filter_map(|id| people.get(id)) {
                if person.state != "active"
                    || off.contains(&person.id)
                    || muted.contains(&(person.id, key.org_id))
                {
                    continue;
                }
                let Some(email) = &person.email else {
                    continue;
                };
                let lang = Locale::choose(person.locale.as_deref(), None);
                state.mailer.send(kaeso_mail::templates::key_expiry_warning(
                    lang,
                    email,
                    &key.name,
                    kaeso_mail::notices::Place {
                        organization: &organization,
                        project: Some(&key.project_name),
                    },
                    crate::time_zone::local(key.expires_at, &zone),
                    zone_name,
                    &url,
                ));
                sent += 1;
            }
        }
    }
    Ok(format!("sent {sent} key expiry warnings"))
}

async fn project_admins(
    state: &AppState,
    org: Uuid,
    project: Uuid,
) -> Result<Vec<Uuid>, StoreError> {
    let grants = members::project_grants(&state.pg, org, project).await?;
    Ok(grants
        .into_iter()
        .filter(|g| {
            let org_grant = crate::platform::grants::org_from_store(&g.org_grant).ok();
            let project_grant = g
                .project_grant
                .as_ref()
                .and_then(|p| crate::platform::grants::project_from_store(p).ok());
            kaeso_core::rights::effective_project_rights(org_grant.as_ref(), project_grant.as_ref())
                .contains(ProjectRight::ProjectUpdate)
        })
        .map(|g| g.user_id)
        .collect())
}
