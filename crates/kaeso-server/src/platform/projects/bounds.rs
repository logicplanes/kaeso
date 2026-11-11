use std::collections::BTreeSet;

use kaeso_store::pg::catalogue::{self, Subject};
use kaeso_store::pg::keys::KeyAuth;
use kaeso_store::pg::{members, tool_access};
use uuid::Uuid;

use crate::platform::access::{Caller, Ctx};
use crate::platform::error::{ApiError, ApiResult};
use crate::state::AppState;

pub(crate) fn widens(field: &str) -> ApiError {
    ApiError::PermissionDenied(format!(
        "A key can't widen `{field}` beyond its own; use a console session."
    ))
}

pub(crate) fn calling_key(ctx: &Ctx) -> Option<&KeyAuth> {
    match &ctx.caller {
        Caller::ProjectKey(key) => Some(key),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Lists {
    Models,
    Tools,
}

impl Lists {
    fn field(self) -> &'static str {
        match self {
            Lists::Models => "models",
            Lists::Tools => "tools",
        }
    }

    async fn of(
        self,
        conn: &mut sqlx::PgConnection,
        project: Uuid,
        subject: Subject,
    ) -> ApiResult<Option<BTreeSet<Uuid>>> {
        Ok(match self {
            Lists::Models => catalogue::access(conn, project, subject).await?,
            Lists::Tools => tool_access::access(conn, project, subject).await?,
        })
    }
}

async fn member_of(state: &AppState, key: &KeyAuth) -> ApiResult<Option<Subject>> {
    Ok(match (key.user_id, key.service_account_id) {
        (Some(user), _) => members::get_project(&state.pg, key.project_id, user)
            .await?
            .map(|m| Subject::Membership(m.id)),
        (None, Some(account)) => Some(Subject::ServiceAccount(account)),
        (None, None) => None,
    })
}

async fn reach(state: &AppState, key: &KeyAuth, lists: Lists) -> ApiResult<Option<BTreeSet<Uuid>>> {
    if matches!(lists, Lists::Tools) && !key.tools {
        return Ok(Some(BTreeSet::new()));
    }
    let member = member_of(state, key).await?;
    let mut conn = state.pg.acquire().await?;
    let own = lists
        .of(&mut conn, key.project_id, Subject::Key(key.id))
        .await?;
    let above = match member {
        Some(s) => lists.of(&mut conn, key.project_id, s).await?,
        None => None,
    };
    Ok(match (own, above) {
        (Some(a), Some(b)) => Some(a.intersection(&b).copied().collect()),
        (a, b) => a.or(b),
    })
}

fn within(list: Option<&BTreeSet<Uuid>>, bound: Option<&BTreeSet<Uuid>>) -> bool {
    match (list, bound) {
        (_, None) => true,
        (None, Some(_)) => false,
        (Some(l), Some(b)) => l.is_subset(b),
    }
}

// A key acts with its member's rights, so without this a key holding `limits.manage` or
// `tools.manage` could lift its own model or tool list, or leave another list looser than itself.
pub(crate) async fn check_list(
    state: &AppState,
    ctx: &Ctx,
    lists: Lists,
    before: Option<&BTreeSet<Uuid>>,
    after: Option<&BTreeSet<Uuid>>,
) -> ApiResult<()> {
    let Some(key) = calling_key(ctx) else {
        return Ok(());
    };
    if within(after, before) || within(after, reach(state, key, lists).await?.as_ref()) {
        return Ok(());
    }
    Err(widens(lists.field()))
}
