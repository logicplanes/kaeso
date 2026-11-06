use std::collections::HashMap;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use kaeso_core::id::UserId;
use kaeso_core::notifications::{Delivery, Kind};
use kaeso_core::rights::{
    InstanceRights, OrgRights, ProjectRights, Right, effective_project_rights,
};
use kaeso_crypto::MacKey;
use kaeso_store::pg::{members, notifications};
use uuid::Uuid;

use crate::platform::error::ApiResult;
use crate::platform::grants::{org_from_store, project_from_store};
use crate::state::AppState;

const UNSUBSCRIBE: &str = "notifications-unsubscribe";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Held {
    pub instance: InstanceRights,
    pub org: OrgRights,
    pub project: ProjectRights,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    AboutYou,
    Holds(Right),
}

impl Held {
    pub fn holds(&self, right: Right) -> bool {
        match right {
            Right::Org(r) => self.org.contains(r),
            Right::Project(r) => self.project.contains(r),
            Right::Instance(r) => self.instance.contains(r),
            Right::Account(_) => true,
        }
    }

    pub fn reason(&self, kind: Kind) -> Option<Reason> {
        if kind.rights().is_empty() {
            return Some(Reason::AboutYou);
        }
        kind.rights()
            .iter()
            .copied()
            .find(|r| self.holds(*r))
            .map(Reason::Holds)
    }
}

pub async fn held(state: &AppState, user: Uuid, instance_admin: bool) -> ApiResult<Held> {
    let orgs = members::orgs_of_user(&state.pg, user).await?;
    let projects = members::projects_of_user(&state.pg, user).await?;
    let mut held = Held {
        instance: if instance_admin {
            InstanceRights::all()
        } else {
            InstanceRights::EMPTY
        },
        ..Held::default()
    };
    let mut grants = HashMap::new();
    for o in &orgs {
        let grant = org_from_store(&o.grant())?;
        held.org = held.org.union(grant.effective());
        held.project = held
            .project
            .union(effective_project_rights(Some(&grant), None));
        grants.insert(o.org_id, grant);
    }
    for p in &projects {
        let own = project_from_store(&p.grant())?;
        held.project = held
            .project
            .union(effective_project_rights(grants.get(&p.org_id), Some(&own)));
    }
    Ok(held)
}

pub fn deliveries(settings: &[notifications::Setting]) -> HashMap<Kind, Delivery> {
    Kind::ALL
        .iter()
        .map(|k| {
            let chosen = settings
                .iter()
                .find(|s| s.kind == k.as_str())
                .and_then(|s| s.delivery.parse().ok());
            let delivery = if k.always() {
                Delivery::Now
            } else {
                chosen.unwrap_or(k.default_delivery())
            };
            (*k, delivery)
        })
        .collect()
}

pub fn unsubscribe_token(state: &AppState, user: UserId, kind: Kind) -> String {
    let id = user.uuid();
    let tag =
        MacKey::derive(&state.keyring, UNSUBSCRIBE).tag(&[id.as_bytes(), kind.as_str().as_bytes()]);
    format!("{user}.{kind}.{}", URL_SAFE_NO_PAD.encode(tag))
}

pub fn verify_unsubscribe(state: &AppState, token: &str) -> Option<(UserId, Kind)> {
    let mut parts = token.splitn(3, '.');
    let user: UserId = parts.next()?.parse().ok()?;
    let kind: Kind = parts.next()?.parse().ok()?;
    let tag = URL_SAFE_NO_PAD.decode(parts.next()?).ok()?;
    let valid = MacKey::derive(&state.keyring, UNSUBSCRIBE)
        .verify(&[user.uuid().as_bytes(), kind.as_str().as_bytes()], &tag);
    (valid && !kind.always()).then_some((user, kind))
}
