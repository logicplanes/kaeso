//! `/Users`: the members SCIM manages, by their address at a verified domain.

use axum::http::StatusCode;
use axum::response::Response;
use kaeso_core::id::UserId;
use kaeso_store::pg::{scim as store, sso_identities, users};
use serde_json::{Value, json};

use super::members::{deprovision, provision};
use super::{
    Asked, ScimError, ScimResult, Scope, USER_SCHEMA, body_json, boolean, filter, operations, page,
    rfc3339, scim_json,
};
use crate::state::AppState;

fn view(scope: &Scope, u: &store::Listed) -> Value {
    let id = UserId::from_uuid(u.user_id).to_string();
    let email = u.email.clone().unwrap_or_default();
    json!({
        "schemas": [USER_SCHEMA],
        "id": id,
        "externalId": u.external_id,
        "userName": email,
        "name": { "formatted": u.name },
        "displayName": u.name,
        "active": u.member,
        "emails": [{ "value": email, "primary": true, "type": "work" }],
        "meta": {
            "resourceType": "User",
            "created": rfc3339(u.created_at),
            "location": scope.location("Users", &id),
        }
    })
}

/// The users SCIM may see: members and the accounts it deprovisioned, at the verified domains.
async fn visible(state: &AppState, scope: &Scope) -> ScimResult<Vec<store::Listed>> {
    Ok(store::users(&state.pg, scope.org)
        .await?
        .into_iter()
        .filter(|u| u.email.as_deref().is_some_and(|e| scope.manages(e)))
        .collect())
}

fn unknown(id: &str) -> ScimError {
    ScimError::not_found(format!("No user found with id '{id}'."))
}

async fn find(state: &AppState, scope: &Scope, id: &str) -> ScimResult<store::Listed> {
    let user: UserId = id.parse().map_err(|_| unknown(id))?;
    visible(state, scope)
        .await?
        .into_iter()
        .find(|u| u.user_id == user.uuid())
        .ok_or_else(|| unknown(id))
}

pub(super) async fn list(state: &AppState, scope: &Scope, asked: &Asked) -> ScimResult<Response> {
    let wanted = filter(asked.query.as_deref(), &["userName", "externalId"])?;
    let all = visible(state, scope)
        .await?
        .into_iter()
        .filter(|u| match &wanted {
            Some(("userName", v)) => u
                .email
                .as_deref()
                .is_some_and(|e| kaeso_core::email::same_address(e, v)),
            Some((_, v)) => u.external_id.as_deref() == Some(v.as_str()),
            None => true,
        })
        .map(|u| view(scope, &u))
        .collect();
    Ok(page(asked.query.as_deref(), all))
}

pub(super) async fn get(state: &AppState, scope: &Scope, id: &str) -> ScimResult<Response> {
    let u = find(state, scope, id).await?;
    Ok(scim_json(StatusCode::OK, &view(scope, &u)))
}

/// What a request says about a user; `None` where it says nothing.
#[derive(Default)]
struct Attributes {
    user_name: Option<String>,
    external_id: Option<String>,
    display_name: Option<String>,
    formatted: Option<String>,
    given: Option<String>,
    family: Option<String>,
    active: Option<bool>,
}

impl Attributes {
    fn set(&mut self, path: &str, value: &Value) -> ScimResult<()> {
        let text = || value.as_str().map(str::to_owned);
        match path.to_ascii_lowercase().as_str() {
            "username" => self.user_name = text(),
            "externalid" => self.external_id = text(),
            "displayname" => self.display_name = text(),
            "name.formatted" => self.formatted = text(),
            "name.givenname" => self.given = text(),
            "name.familyname" => self.family = text(),
            "active" => self.active = Some(boolean(value, "active")?),
            "name" => {
                if let Some(o) = value.as_object() {
                    for (k, v) in o {
                        self.set(&format!("name.{k}"), v)?;
                    }
                }
            }
            p if p.starts_with("emails") && self.user_name.is_none() => {
                if let Some(email) = primary_email(value) {
                    self.user_name = Some(email);
                }
            }
            // Attributes Kaeso keeps nothing of, such as a title or the enterprise extension.
            _ => {}
        }
        Ok(())
    }

    fn of(body: &Value) -> ScimResult<Attributes> {
        let mut a = Attributes::default();
        if let Some(o) = body.as_object() {
            for (k, v) in o {
                if k != "schemas" && k != "id" && k != "meta" {
                    a.set(k, v)?;
                }
            }
        }
        Ok(a)
    }

    fn name(&self) -> Option<String> {
        let joined = [self.given.as_deref(), self.family.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        self.display_name
            .clone()
            .or_else(|| self.formatted.clone())
            .or_else(|| (!joined.is_empty()).then_some(joined))
            .and_then(|n| crate::platform::validate::display_name(&n))
    }
}

fn primary_email(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Array(list) => list
            .iter()
            .find(|e| e.get("primary").and_then(Value::as_bool) == Some(true))
            .or_else(|| list.first())
            .and_then(|e| e.get("value"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        Value::Object(_) => value
            .get("value")
            .and_then(Value::as_str)
            .map(str::to_owned),
        _ => None,
    }
}

fn address(scope: &Scope, raw: Option<&str>) -> ScimResult<String> {
    let email = raw
        .and_then(|r| kaeso_core::email::normalize(r).ok())
        .ok_or_else(|| ScimError::bad("invalidValue", "`userName` must be an email address."))?;
    if !scope.manages(&email) {
        return Err(ScimError::bad(
            "invalidValue",
            format!(
                "{email} isn't at one of the organization's verified domains; SCIM manages only those accounts."
            ),
        ));
    }
    Ok(email)
}

pub(super) async fn create(state: &AppState, scope: &Scope, asked: &Asked) -> ScimResult<Response> {
    let a = Attributes::of(&body_json(asked)?)?;
    let email = address(scope, a.user_name.as_deref())?;
    let active = a.active.unwrap_or(true);
    let taken = || {
        ScimError::conflict(
            Some("uniqueness"),
            "A user with this userName already exists.",
        )
    };
    if let Some(external) = &a.external_id
        && store::user_by_external(&state.pg, scope.org, external)
            .await?
            .is_some()
    {
        return Err(taken());
    }
    let (user, created) = match users::get_by_email(&state.pg, &email).await? {
        Some(u) => {
            let listed = visible(state, scope).await?;
            if listed.iter().any(|l| l.user_id == u.id && l.member) {
                return Err(taken());
            }
            (u.id, false)
        }
        None => {
            let id = UserId::new().uuid();
            let name = a
                .name()
                .unwrap_or_else(|| email.split('@').next().unwrap_or(&email).to_owned());
            let mut tx = state.pg.begin().await?;
            let inserted = users::insert(
                &mut *tx,
                &users::NewUser {
                    id,
                    email: &email,
                    name: &name,
                    password_hash: None,
                    email_verified_at: None,
                    is_instance_admin: false,
                },
            )
            .await?;
            if !inserted {
                return Err(taken());
            }
            sso_identities::add_pending(&mut *tx, scope.org, id).await?;
            tx.commit().await?;
            (id, true)
        }
    };
    if active {
        provision(state, scope.org, user, a.external_id.as_deref(), created).await?;
    } else {
        store::put_user(
            &state.pg,
            scope.org,
            user,
            a.external_id.as_deref(),
            false,
            state.clock.now(),
        )
        .await?;
    }
    let u = find(state, scope, &UserId::from_uuid(user).to_string()).await?;
    Ok(scim_json(StatusCode::CREATED, &view(scope, &u)))
}

/// Applies what a PUT or PATCH says to a user SCIM sees.
async fn apply(
    state: &AppState,
    scope: &Scope,
    u: &store::Listed,
    a: &Attributes,
) -> ScimResult<Response> {
    if let (Some(asked), Some(email)) = (a.user_name.as_deref(), u.email.as_deref())
        && !kaeso_core::email::same_address(asked, email)
    {
        return Err(ScimError::bad(
            "mutability",
            "Kaeso doesn't change an account's address through SCIM; the user signs in with it.",
        ));
    }
    if let Some(external) = &a.external_id
        && store::user_by_external(&state.pg, scope.org, external)
            .await?
            .is_some_and(|other| other != u.user_id)
    {
        return Err(ScimError::conflict(
            Some("uniqueness"),
            "Another user has this externalId.",
        ));
    }
    if let Some(name) = a.name().filter(|n| *n != u.name) {
        users::set_name(&state.pg, u.user_id, &name).await?;
    }
    let external = a.external_id.as_deref().or(u.external_id.as_deref());
    match a.active {
        Some(true) if !u.member => provision(state, scope.org, u.user_id, external, false).await?,
        Some(false) if u.member => deprovision(state, scope.org, u.user_id, external).await?,
        _ => {
            store::put_user(
                &state.pg,
                scope.org,
                u.user_id,
                external,
                u.member,
                state.clock.now(),
            )
            .await?;
        }
    }
    let after = find(state, scope, &UserId::from_uuid(u.user_id).to_string()).await?;
    Ok(scim_json(StatusCode::OK, &view(scope, &after)))
}

pub(super) async fn replace(
    state: &AppState,
    scope: &Scope,
    id: &str,
    asked: &Asked,
) -> ScimResult<Response> {
    let u = find(state, scope, id).await?;
    let mut a = Attributes::of(&body_json(asked)?)?;
    a.active = Some(a.active.unwrap_or(true));
    apply(state, scope, &u, &a).await
}

pub(super) async fn patch(
    state: &AppState,
    scope: &Scope,
    id: &str,
    asked: &Asked,
) -> ScimResult<Response> {
    let u = find(state, scope, id).await?;
    let mut a = Attributes::default();
    for (op, path, value) in operations(asked)? {
        if op == "remove" {
            continue;
        }
        match path {
            Some(p) => a.set(&p, &value)?,
            None => {
                if let Some(o) = value.as_object() {
                    for (k, v) in o {
                        a.set(k, v)?;
                    }
                }
            }
        }
    }
    apply(state, scope, &u, &a).await
}

pub(super) async fn delete(state: &AppState, scope: &Scope, id: &str) -> ScimResult<Response> {
    let u = find(state, scope, id).await?;
    if u.member {
        deprovision(state, scope.org, u.user_id, u.external_id.as_deref()).await?;
    }
    store::delete_user(&state.pg, scope.org, u.user_id).await?;
    let mut resp = Response::new(axum::body::Body::empty());
    *resp.status_mut() = StatusCode::NO_CONTENT;
    Ok(resp)
}
