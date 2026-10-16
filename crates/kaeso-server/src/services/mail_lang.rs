use kaeso_mail::Locale;
use kaeso_store::pg::users;
use sqlx::PgExecutor;
use uuid::Uuid;

pub async fn chosen(db: impl PgExecutor<'_>, user: Uuid) -> Option<String> {
    users::get_locale(db, user)
        .await
        .inspect_err(
            |e| tracing::warn!(error = %e, "cannot read a user's language; the email falls back"),
        )
        .ok()
        .flatten()
}

pub async fn of_user(db: impl PgExecutor<'_>, user: Uuid, accept_language: Option<&str>) -> Locale {
    Locale::choose(chosen(db, user).await.as_deref(), accept_language)
}

pub async fn of_address(
    db: &sqlx::PgPool,
    email: &str,
    fallback: Option<&str>,
    accept_language: Option<&str>,
) -> Locale {
    let own = match users::get_by_email(db, email).await {
        Ok(Some(user)) => chosen(db, user.id).await,
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(error = %e, "cannot look up an email's recipient; the email falls back");
            None
        }
    };
    Locale::choose(own.as_deref().or(fallback), accept_language)
}
