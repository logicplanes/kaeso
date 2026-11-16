//! `/Groups`: the identity provider's groups, kept as it pushes them. They grant nothing yet.

use axum::http::StatusCode;
use axum::response::Response;
use kaeso_core::id::UserId;
use kaeso_store::pg::scim::{self as store, Group};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    Asked, GROUP_SCHEMA, ScimError, ScimResult, Scope, body_json, filter, operations, page,
    rfc3339, scim_json,
};
use crate::state::AppState;

fn view(scope: &Scope, g: &Group) -> Value {
    let id = g.id.to_string();
    json!({
        "schemas": [GROUP_SCHEMA],
        "id": id,
        "externalId": g.external_id,
        "displayName": g.display_name,
        "members": g.members.iter().map(|m| json!({ "value": UserId::from_uuid(*m).to_string() })).collect::<Vec<_>>(),
        "meta": {
            "resourceType": "Group",
            "created": rfc3339(g.created_at),
            "lastModified": rfc3339(g.updated_at),
            "location": scope.location("Groups", &id),
        }
    })
}

fn unknown(id: &str) -> ScimError {
    ScimError::not_found(format!("No group found with id '{id}'."))
}

async fn find(state: &AppState, scope: &Scope, id: &str) -> ScimResult<Group> {
    let parsed: Uuid = id.parse().map_err(|_| unknown(id))?;
    store::group(&state.pg, scope.org, parsed)
        .await?
        .ok_or_else(|| unknown(id))
}

/// The users a `members` value names, by their Kaeso ids; other values are left out.
fn members_of(value: &Value) -> Vec<Uuid> {
    let one = |m: &Value| {
        m.get("value")
            .and_then(Value::as_str)
            .and_then(|v| v.parse::<UserId>().ok())
            .map(|u| u.uuid())
    };
    match value {
        Value::Array(list) => list.iter().filter_map(one).collect(),
        Value::Object(_) => one(value).into_iter().collect(),
        _ => Vec::new(),
    }
}

fn add(members: &mut Vec<Uuid>, more: Vec<Uuid>) {
    for m in more {
        if !members.contains(&m) {
            members.push(m);
        }
    }
}

fn display_name(v: &Value) -> ScimResult<String> {
    v.as_str()
        .map(str::trim)
        .filter(|n| !n.is_empty() && n.chars().count() <= 200)
        .map(str::to_owned)
        .ok_or_else(|| {
            ScimError::bad(
                "invalidValue",
                "`displayName` must be a name of 1 to 200 characters.",
            )
        })
}

async fn save(
    state: &AppState,
    scope: &Scope,
    g: &Group,
    status: StatusCode,
) -> ScimResult<Response> {
    if !store::put_group(&state.pg, g).await? {
        return Err(ScimError::conflict(
            Some("uniqueness"),
            "Another group has this displayName.",
        ));
    }
    Ok(scim_json(status, &view(scope, g)))
}

pub(super) async fn list(state: &AppState, scope: &Scope, asked: &Asked) -> ScimResult<Response> {
    let wanted = filter(asked.query.as_deref(), &["displayName", "externalId"])?;
    let all = store::groups(&state.pg, scope.org)
        .await?
        .into_iter()
        .filter(|g| match &wanted {
            Some(("displayName", v)) => g.display_name.eq_ignore_ascii_case(v),
            Some((_, v)) => g.external_id.as_deref() == Some(v.as_str()),
            None => true,
        })
        .map(|g| view(scope, &g))
        .collect();
    Ok(page(asked.query.as_deref(), all))
}

pub(super) async fn get(state: &AppState, scope: &Scope, id: &str) -> ScimResult<Response> {
    let g = find(state, scope, id).await?;
    Ok(scim_json(StatusCode::OK, &view(scope, &g)))
}

pub(super) async fn create(state: &AppState, scope: &Scope, asked: &Asked) -> ScimResult<Response> {
    let body = body_json(asked)?;
    let now = state.clock.now();
    let g = Group {
        id: Uuid::now_v7(),
        org_id: scope.org,
        display_name: display_name(body.get("displayName").unwrap_or(&Value::Null))?,
        external_id: body
            .get("externalId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        members: members_of(body.get("members").unwrap_or(&Value::Null)),
        created_at: now,
        updated_at: now,
    };
    save(state, scope, &g, StatusCode::CREATED).await
}

pub(super) async fn replace(
    state: &AppState,
    scope: &Scope,
    id: &str,
    asked: &Asked,
) -> ScimResult<Response> {
    let mut g = find(state, scope, id).await?;
    let body = body_json(asked)?;
    g.display_name = display_name(body.get("displayName").unwrap_or(&Value::Null))?;
    g.external_id = body
        .get("externalId")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or(g.external_id);
    g.members = members_of(body.get("members").unwrap_or(&Value::Null));
    g.updated_at = state.clock.now();
    save(state, scope, &g, StatusCode::OK).await
}

/// `members[value eq "usr_…"]`, as Okta and Entra ID name one member to remove.
fn member_in_path(path: &str) -> Option<Uuid> {
    let inner = path.strip_prefix("members[")?.strip_suffix(']')?;
    let (_, value) = inner.split_once(" eq ")?;
    value
        .trim()
        .trim_matches('"')
        .parse::<UserId>()
        .ok()
        .map(|u| u.uuid())
}

pub(super) async fn patch(
    state: &AppState,
    scope: &Scope,
    id: &str,
    asked: &Asked,
) -> ScimResult<Response> {
    let mut g = find(state, scope, id).await?;
    for (op, path, value) in operations(asked)? {
        let lower = path.as_deref().map(str::to_ascii_lowercase);
        match (op.as_str(), lower.as_deref()) {
            ("add", Some("members")) => add(&mut g.members, members_of(&value)),
            ("replace", Some("members")) => g.members = members_of(&value),
            ("remove", Some("members")) => {
                let gone = members_of(&value);
                if gone.is_empty() {
                    g.members.clear();
                } else {
                    g.members.retain(|m| !gone.contains(m));
                }
            }
            ("remove", Some(p)) if p.starts_with("members[") => {
                if let Some(gone) = path.as_deref().and_then(member_in_path) {
                    g.members.retain(|m| *m != gone);
                }
            }
            (_, Some("displayname")) => g.display_name = display_name(&value)?,
            (_, Some("externalid")) => g.external_id = value.as_str().map(str::to_owned),
            ("add" | "replace", None) => {
                if let Some(name) = value.get("displayName") {
                    g.display_name = display_name(name)?;
                }
                if let Some(external) = value.get("externalId") {
                    g.external_id = external.as_str().map(str::to_owned);
                }
                if let Some(members) = value.get("members") {
                    if op == "add" {
                        add(&mut g.members, members_of(members));
                    } else {
                        g.members = members_of(members);
                    }
                }
            }
            _ => {}
        }
    }
    g.updated_at = state.clock.now();
    save(state, scope, &g, StatusCode::OK).await
}

pub(super) async fn delete(state: &AppState, scope: &Scope, id: &str) -> ScimResult<Response> {
    let g = find(state, scope, id).await?;
    store::delete_group(&state.pg, scope.org, g.id).await?;
    let mut resp = Response::new(axum::body::Body::empty());
    *resp.status_mut() = StatusCode::NO_CONTENT;
    Ok(resp)
}
