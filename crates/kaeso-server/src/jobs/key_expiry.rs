use kaeso_core::id::{OrgId, ProjectId};
use kaeso_core::rights::ProjectRight;
use kaeso_store::pg::{keys, members, users};

use crate::state::AppState;

pub async fn run(state: AppState) -> Result<String, String> {
    let now = state.clock.now();
    let mut sent = 0;
    for days in [1, 7] {
        let expiring = keys::expiring(&state.pg, now, days)
            .await
            .map_err(|e| e.to_string())?;
        for key in expiring {
            if !keys::mark_warned(&state.pg, key.id, days)
                .await
                .map_err(|e| e.to_string())?
            {
                continue;
            }
            if days == 1 {
                let _ = keys::mark_warned(&state.pg, key.id, 7).await;
            }
            let recipients = match key.user_id {
                Some(user) => users::get(&state.pg, user)
                    .await
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .collect(),
                None => project_admins(&state, key.org_id, key.project_id).await?,
            };
            let expires = key.expires_at.date();
            let url = format!(
                "{}orgs/{}/projects/{}/keys",
                state.config.public_url,
                OrgId::from_uuid(key.org_id),
                ProjectId::from_uuid(key.project_id)
            );
            for user in recipients
                .into_iter()
                .filter(|u| u.state == "active" && u.notify_key_expiry)
            {
                if let Some(email) = &user.email {
                    let lang = crate::services::mail_lang::of_user(&state.pg, user.id, None).await;
                    state.mailer.send(kaeso_mail::templates::key_expiry_warning(
                        lang,
                        email,
                        &key.name,
                        &key.project_name,
                        expires,
                        &url,
                    ));
                    sent += 1;
                }
            }
        }
    }
    Ok(format!("sent {sent} key expiry warnings"))
}

async fn project_admins(
    state: &AppState,
    org: uuid::Uuid,
    project: uuid::Uuid,
) -> Result<Vec<users::User>, String> {
    let mut out = Vec::new();
    let mut after = None;
    loop {
        let page = members::list_org(&state.pg, org, after, 500, None, None)
            .await
            .map_err(|e| e.to_string())?;
        after = page.last().map(|m| m.id);
        for m in &page {
            let mut conn = state.pg.acquire().await.map_err(|e| e.to_string())?;
            let Some(found) =
                members::access_context(&mut conn, org, Some(m.user_id), Some(project))
                    .await
                    .map_err(|e| e.to_string())?
            else {
                continue;
            };
            let org_grant = found
                .org_grant
                .as_ref()
                .and_then(|g| crate::platform::grants::org_from_store(g).ok());
            let project_grant = found
                .project_grant
                .as_ref()
                .and_then(|g| crate::platform::grants::project_from_store(g).ok());
            let rights = kaeso_core::rights::effective_project_rights(
                org_grant.as_ref(),
                project_grant.as_ref(),
            );
            if rights.contains(ProjectRight::ProjectUpdate)
                && let Some(u) = users::get(&state.pg, m.user_id)
                    .await
                    .map_err(|e| e.to_string())?
            {
                out.push(u);
            }
        }
        if page.len() < 500 {
            break;
        }
    }
    Ok(out)
}
