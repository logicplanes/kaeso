use axum::Json;
use axum::extract::State;
use kaeso_core::accounts::{AccountCreation, DomainRule, MAX_DOMAINS, OrgCreation};
use kaeso_store::pg::instance::{self, InstanceSettings};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::MailTransport;
use crate::platform::access::Ctx;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::services::accounts;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Sign-in methods, set in configuration.")]
pub struct SignInMethods {
    #[doc = "Email and password."]
    pub password: bool,
    #[doc = "Magic links."]
    pub magic_link: bool,
    #[doc = "SSO."]
    pub oidc: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Mail settings, set in configuration. The password is never shown.")]
pub struct MailSettings {
    #[doc = "`smtp` or `log`."]
    pub transport: &'static str,
    #[doc = "SMTP host."]
    pub host: Option<String>,
    #[doc = "SMTP port."]
    pub port: u16,
    #[doc = "Sender."]
    pub from: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The instance settings.")]
pub struct SettingsView {
    #[doc = "Who may create an account without an invitation.\n\
        Invitations work under every setting."]
    pub account_creation: AccountCreation,
    #[doc = "The email domains `allowed_domains` settings allow: `example.com`\n\
        exactly, `*.example.com` its subdomains; internationalised domains in\n\
        Unicode."]
    #[schema(example = json!(["example.com", "*.example.com"]))]
    pub allowed_domains: Vec<String>,
    #[doc = "Who may create orgs; instance admins always may."]
    pub org_creation: OrgCreation,
    #[doc = "Log metadata retention of new orgs, in days."]
    pub metadata_retention_days: i32,
    #[doc = "Log content retention of new orgs, in days."]
    pub content_retention_days: i32,
    #[doc = "The longest metadata retention an org may choose."]
    pub max_metadata_retention_days: i32,
    #[doc = "The longest content retention an org may choose."]
    pub max_content_retention_days: i32,
    #[doc = "Whether orgs can be deleted."]
    pub org_deletion: bool,
    #[doc = "Days an org scheduled for deletion stays read-only and restorable, 1 to 90."]
    #[schema(example = 7, minimum = 1, maximum = 90)]
    pub org_deletion_grace_days: i32,
    #[doc = "Sign-in methods (read-only)."]
    pub sign_in: SignInMethods,
    #[doc = "Mail (read-only)."]
    pub mail: MailSettings,
}

fn view(state: &AppState, s: &InstanceSettings) -> SettingsView {
    let c = &state.config;
    let policy = accounts::policy(s);
    SettingsView {
        account_creation: policy.account_creation,
        allowed_domains: policy
            .allowed_domains
            .iter()
            .map(DomainRule::display)
            .collect(),
        org_creation: policy.org_creation,
        metadata_retention_days: s.metadata_retention_days,
        content_retention_days: s.content_retention_days,
        max_metadata_retention_days: s.max_metadata_retention_days,
        max_content_retention_days: s.max_content_retention_days,
        org_deletion: s.org_deletion,
        org_deletion_grace_days: s.org_deletion_grace_days,
        sign_in: SignInMethods {
            password: c.auth.password,
            magic_link: c.auth.magic_link,
            oidc: c.auth.oidc,
        },
        mail: MailSettings {
            transport: match c.mail.transport {
                MailTransport::Smtp => "smtp",
                MailTransport::Log => "log",
            },
            host: c.smtp.host.clone(),
            port: c.smtp.port,
            from: c.smtp.from.clone(),
        },
    }
}

#[utoipa::path(
    get,
    path = "/instance/settings",
    summary = "The instance settings.",
    tag = "instance",
    responses((status = 200, body = SettingsView))
)]
pub async fn get(State(state): State<AppState>) -> ApiResult<Json<SettingsView>> {
    Ok(Json(view(&state, &instance::get(&state.pg).await?)))
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "Changes to the instance settings.")]
pub struct SettingsUpdate {
    #[doc = "Who may create an account without an invitation."]
    pub account_creation: Option<AccountCreation>,
    #[doc = "The allowed email domains, replacing the list: `example.com`, or\n\
        `*.example.com` for its subdomains; at most 100."]
    #[schema(example = json!(["example.com", "*.example.com"]))]
    pub allowed_domains: Option<Vec<String>>,
    #[doc = "Who may create orgs."]
    pub org_creation: Option<OrgCreation>,
    #[doc = "Default metadata retention, in days."]
    pub metadata_retention_days: Option<i32>,
    #[doc = "Default content retention, in days."]
    pub content_retention_days: Option<i32>,
    #[doc = "Maximum metadata retention, in days."]
    pub max_metadata_retention_days: Option<i32>,
    #[doc = "Maximum content retention, in days."]
    pub max_content_retention_days: Option<i32>,
    #[doc = "Whether orgs can be deleted. Turning it off leaves orgs already\n\
        scheduled on their dates."]
    pub org_deletion: Option<bool>,
    #[doc = "Days a new deletion waits, 1 to 90. Dates already set don't move."]
    pub org_deletion_grace_days: Option<i32>,
}

#[utoipa::path(
    patch,
    path = "/instance/settings",
    summary = "Changes who may create accounts and orgs and the allowed domains (with a\n\
    recent sign-in), org deletion, and the retention defaults and maximums.\n\
    Content is never kept longer than metadata.",
    tag = "instance",
    request_body = SettingsUpdate,
    responses((status = 200, body = SettingsView))
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<SettingsUpdate>,
) -> ApiResult<Json<SettingsView>> {
    if req.account_creation.is_some() || req.allowed_domains.is_some() || req.org_creation.is_some()
    {
        ctx.require_recent_sign_in()?;
    }
    let mut tx = state.pg.begin().await?;
    let before = instance::get_for_update(&mut tx).await?;
    let mut after = before.clone();
    if let Some(a) = req.account_creation {
        after.account_creation = a.as_str().to_owned();
    }
    if let Some(o) = req.org_creation {
        after.org_creation = o.as_str().to_owned();
    }
    if let Some(domains) = &req.allowed_domains {
        after.allowed_domains = allowed_domains(domains)?;
    }
    let policy = accounts::policy(&after);
    if policy.allowed_domains.is_empty()
        && (policy.account_creation == AccountCreation::AllowedDomains
            || policy.org_creation == OrgCreation::AllowedDomains)
    {
        return Err(ApiError::invalid(
            "allowed_domains",
            "Add at least one allowed domain to use it.",
        ));
    }
    for (value, slot) in [
        (
            req.metadata_retention_days,
            &mut after.metadata_retention_days,
        ),
        (
            req.content_retention_days,
            &mut after.content_retention_days,
        ),
        (
            req.max_metadata_retention_days,
            &mut after.max_metadata_retention_days,
        ),
        (
            req.max_content_retention_days,
            &mut after.max_content_retention_days,
        ),
    ] {
        if let Some(v) = value {
            if !(1..=36_500).contains(&v) {
                return Err(ApiError::bad_request("Retention must be 1 to 36,500 days."));
            }
            *slot = v;
        }
    }
    if let Some(d) = req.org_deletion {
        after.org_deletion = d;
    }
    if let Some(days) = req.org_deletion_grace_days {
        if !(1..=90).contains(&days) {
            return Err(ApiError::invalid(
                "org_deletion_grace_days",
                "The grace period must be 1 to 90 days.",
            ));
        }
        after.org_deletion_grace_days = days;
    }
    if after.content_retention_days > after.metadata_retention_days {
        return Err(ApiError::invalid(
            "content_retention_days",
            "Content cannot be kept longer than metadata.",
        ));
    }
    if after.metadata_retention_days > after.max_metadata_retention_days
        || after.content_retention_days > after.max_content_retention_days
    {
        return Err(ApiError::bad_request(
            "A default retention cannot exceed its maximum.",
        ));
    }
    instance::update(&mut *tx, &after).await?;
    ctx.audit(None, "instance.settings_update")
        .before(serde_json::to_value(view(&state, &before)).unwrap_or_default())
        .after(serde_json::to_value(view(&state, &after)).unwrap_or_default())
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(view(&state, &after)))
}

fn allowed_domains(domains: &[String]) -> ApiResult<Vec<String>> {
    let rules = kaeso_core::accounts::parse_domains(domains).map_err(|(i, e)| {
        ApiError::invalid(
            &format!("allowed_domains[{i}]"),
            format!(
                "`{}` isn't a domain; use example.com or *.example.com.",
                e.0
            ),
        )
    })?;
    if rules.len() > MAX_DOMAINS {
        return Err(ApiError::invalid(
            "allowed_domains",
            format!("Allow at most {MAX_DOMAINS} domains."),
        ));
    }
    Ok(rules.iter().map(DomainRule::stored).collect())
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(description = "Where to send a test email.")]
pub struct TestEmail {
    #[doc = "The recipient; default: the caller."]
    pub to: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The outcome of a test email.")]
pub struct TestEmailResult {
    #[doc = "`sent`."]
    pub status: &'static str,
    #[doc = "The recipient."]
    pub to: String,
}

#[utoipa::path(
    post,
    path = "/instance/settings/test-email",
    summary = "Sends a test email now and reports whether the SMTP server took it.",
    tag = "instance",
    request_body = TestEmail,
    responses((status = 200, body = TestEmailResult))
)]
pub async fn test_email(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<TestEmail>,
) -> ApiResult<Json<TestEmailResult>> {
    let to = match req.to {
        Some(t) => crate::platform::auth::setup::validate_email("to", &t)?,
        None => kaeso_store::pg::users::get(&state.pg, ctx.user_id()?.uuid())
            .await?
            .and_then(|u| u.email)
            .ok_or_else(|| ApiError::invalid("to", "Name a recipient."))?,
    };
    let lang = crate::services::mail_lang::of_address(
        &state.pg,
        &to,
        None,
        ctx.accept_language.as_deref(),
    )
    .await;
    state
        .mailer
        .send_now(kaeso_mail::templates::test(lang, &to))
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "test email not sent");
            ApiError::Unavailable(
                "The email was not sent; check the mail settings and the gateway log.".into(),
            )
        })?;
    Ok(Json(TestEmailResult { status: "sent", to }))
}
