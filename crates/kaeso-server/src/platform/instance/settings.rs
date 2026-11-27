use axum::Json;
use axum::extract::State;
use kaeso_core::accounts::{AccountCreation, DomainRule, MAX_DOMAINS, OrgCreation};
use kaeso_core::id::OrgId;
use kaeso_store::pg::instance::{self, AdminSso, InstanceSettings};
use kaeso_store::pg::shared_links::{self, InstanceSwitches};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::MailTransport;
use crate::platform::access::{Ctx, from_sso};
use crate::platform::audit::Action;
use crate::platform::error::{ApiError, ApiResult};
use crate::platform::json::Body;
use crate::services::{accounts, admin_sso};
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Sign-in methods, set in configuration.")]
pub struct SignInMethods {
    #[doc = "Email and password."]
    pub password: bool,
    #[doc = "Sign-in links sent by email."]
    pub magic_link: bool,
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
    #[doc = "The install's IANA time zone: the console shows every time in it, and\n\
        automatic updates follow it. The Platform API's times stay in UTC."]
    #[schema(example = "Europe/Berlin")]
    pub timezone: String,
    #[doc = "Sign-in methods (read-only)."]
    pub sign_in: SignInMethods,
    #[doc = "Whether search engines may index the console's sign-in and sign-up pages\n\
        (`search_engines`, read-only); off, the install asks every crawler to keep out."]
    pub search_engines: bool,
    #[doc = "The org whose single sign-on may administer the instance\n\
        (`PUT instance/settings/admin-sso`); `null`: none, so every instance\n\
        admin signs in with a password or a sign-in link for it."]
    pub admin_sso: Option<AdminSsoView>,
    #[doc = "Mail (read-only)."]
    pub mail: MailSettings,
    #[doc = "The exchange rates the console converts prices and spend with."]
    pub exchange_rates: ExchangeRatesSetting,
    #[doc = "Whether organizations may make shared links; off, every link stops working\n\
        and none can be made."]
    pub shared_links: bool,
    #[doc = "Whether the pages of shared links show a small *Made with Kaeso* mark."]
    pub share_mark: bool,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "The daily fetch of the ECB's euro reference rates.")]
pub struct ExchangeRatesSetting {
    #[doc = "Whether the install fetches the rates each working day. Off keeps the rates\n\
        it has; without any, only US dollars can be used."]
    pub fetch: bool,
    #[doc = "The day of the latest rates the install has."]
    #[schema(example = "2026-11-16")]
    pub day: Option<String>,
    #[doc = "When the install last fetched the rates."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub checked_at: Option<time::OffsetDateTime>,
    #[doc = "Why the last fetch failed; `null` when it worked. Kaeso tries again in 15\n\
        minutes."]
    pub error: Option<String>,
}

async fn rates_setting(
    state: &AppState,
    db: impl sqlx::PgExecutor<'_>,
) -> ApiResult<ExchangeRatesSetting> {
    let status = kaeso_store::pg::exchange_rates::status(db).await?;
    Ok(ExchangeRatesSetting {
        fetch: status.enabled,
        day: crate::services::exchange_rates::book(state)
            .latest_day()
            .map(|d| d.to_string()),
        checked_at: status.checked_at,
        error: status.error,
    })
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The org whose single sign-on may administer the instance.")]
pub struct AdminSsoView {
    #[doc = "The org. An instance admin's session from its SSO may use\n\
        `instance/*` while the org manages the admin's account."]
    pub org: OrgId,
    #[doc = "Its name."]
    #[schema(example = "Acme")]
    pub name: String,
    #[doc = "Whether the org has an SSO connection. Without one it manages no\n\
        account, so the setting has no effect."]
    pub connected: bool,
}

fn view(
    state: &AppState,
    s: &InstanceSettings,
    admin_sso: Option<&AdminSso>,
    rates: &ExchangeRatesSetting,
    shares: InstanceSwitches,
) -> SettingsView {
    let c = &state.config;
    let policy = accounts::policy(s);
    SettingsView {
        admin_sso: admin_sso.map(|a| AdminSsoView {
            org: OrgId::from_uuid(a.org_id),
            name: a.org_name.clone(),
            connected: a.connected,
        }),
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
        timezone: s.timezone.clone(),
        sign_in: SignInMethods {
            password: c.auth.password,
            magic_link: c.auth.magic_link,
        },
        search_engines: c.search_engines,
        mail: MailSettings {
            transport: match c.mail.transport {
                MailTransport::Smtp => "smtp",
                MailTransport::Log => "log",
            },
            host: c.smtp.host.clone(),
            port: c.smtp.port,
            from: c.smtp.from.clone(),
        },
        exchange_rates: rates.clone(),
        shared_links: shares.shared_links,
        share_mark: shares.share_mark,
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
    let admin_sso = instance::admin_sso(&state.pg).await?;
    let rates = rates_setting(&state, &state.pg).await?;
    Ok(Json(view(
        &state,
        &instance::get(&state.pg).await?,
        admin_sso.as_ref(),
        &rates,
        shared_links::instance(&state.pg).await?,
    )))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(description = "The org whose single sign-on may administer the instance.")]
pub struct AdminSsoUpdate {
    #[doc = "An org with an SSO connection, or `null` for none."]
    pub org: Option<OrgId>,
}

#[utoipa::path(
    put,
    path = "/instance/settings/admin-sso",
    summary = "Chooses the org whose single sign-on may administer the instance, or\n\
    none.",
    description = "An instance admin's session from that org's SSO may then use `instance/*` \
    while the org manages the admin's account. Changing it needs a recent sign-in that didn't \
    come from SSO, so no org's identity provider can grant its admins the instance; a session \
    from SSO gets `other_sign_in_required`. The org must have an SSO connection; if it later \
    loses it, the setting has no effect. Audited as `instance.admin_sso_update`.",
    tag = "instance",
    request_body = AdminSsoUpdate,
    responses((status = 200, body = SettingsView))
)]
pub async fn set_admin_sso(
    State(state): State<AppState>,
    ctx: Ctx,
    Body(req): Body<AdminSsoUpdate>,
) -> ApiResult<Json<SettingsView>> {
    if from_sso(ctx.session()?) {
        return Err(ApiError::OtherSignInRequired);
    }
    ctx.require_recent_sign_in()?;
    let mut tx = state.pg.begin().await?;
    let changed = admin_sso::change(&mut tx, req.org).await?;
    ctx.audit(None, Action::InstanceAdminSsoUpdate)
        .before(admin_sso::audited(changed.before.as_ref()))
        .after(admin_sso::audited(changed.after.as_ref()))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    let rates = rates_setting(&state, &state.pg).await?;
    Ok(Json(view(
        &state,
        &changed.settings,
        changed.after.as_ref(),
        &rates,
        shared_links::instance(&state.pg).await?,
    )))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
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
    #[doc = "The install's IANA time zone, such as `Europe/Berlin` or `UTC`."]
    #[schema(example = "Europe/Berlin")]
    pub timezone: Option<String>,
    #[doc = "Whether the install fetches the ECB's exchange rates each working day."]
    pub exchange_rates: Option<bool>,
    #[doc = "Whether organizations may make shared links."]
    pub shared_links: Option<bool>,
    #[doc = "Whether shared links' pages show *Made with Kaeso*."]
    pub share_mark: Option<bool>,
}

#[utoipa::path(
    patch,
    path = "/instance/settings",
    summary = "Changes who may create accounts and orgs and the allowed domains (with a\n\
    recent sign-in), org deletion, and the retention defaults and maximums.\n\
    Content is never kept longer than metadata.",
    description = "Changing `timezone` while automatic updates are on also needs `instance.updates.apply` and a recent sign-in.",
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
    let rates_before = rates_setting(&state, &mut *tx).await?;
    let shares_before = shared_links::instance(&mut *tx).await?;
    let shares = InstanceSwitches {
        shared_links: req.shared_links.unwrap_or(shares_before.shared_links),
        share_mark: req.share_mark.unwrap_or(shares_before.share_mark),
    };
    shared_links::set_instance(&mut *tx, shares).await?;
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
    if let Some(zone) = &req.timezone {
        after.timezone = crate::platform::validate::validate_time_zone("timezone", zone)?;
    }
    if after.timezone != before.timezone {
        let updates = kaeso_store::pg::updates::settings_for_update(&mut tx).await?;
        if updates.mode == kaeso_update::schedule::Mode::Automatic.as_str() {
            ctx.require_instance_right(
                &state,
                kaeso_core::rights::InstanceRight::InstanceUpdatesApply,
            )
            .await?;
            ctx.require_recent_sign_in()?;
            kaeso_store::pg::updates::reset_window(&mut *tx, ctx.now).await?;
        }
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
    // Organizations above a lowered maximum come down to it, with the deletions a shorter
    // retention starts.
    let capped = if after.max_metadata_retention_days < before.max_metadata_retention_days
        || after.max_content_retention_days < before.max_content_retention_days
    {
        kaeso_store::pg::orgs::cap_retentions(
            &mut *tx,
            after.max_metadata_retention_days,
            after.max_content_retention_days,
        )
        .await?
    } else {
        Vec::new()
    };
    let by = crate::platform::keys::creator_of(&ctx);
    for c in &capped {
        let shrunk = crate::platform::logs::Shrunk {
            org: c.id,
            metadata: (c.metadata_before, c.metadata_days),
            content: (c.content_before, c.content_days),
        };
        crate::platform::logs::shrink(&mut tx, &shrunk, ctx.now, by).await?;
    }
    if let Some(on) = req.exchange_rates {
        kaeso_store::pg::exchange_rates::set_enabled(&mut *tx, on).await?;
    }
    let rates_after = rates_setting(&state, &mut *tx).await?;
    let admin_sso = instance::admin_sso(&mut *tx).await?;
    ctx.audit(None, Action::InstanceSettingsUpdate)
        .before(view(
            &state,
            &before,
            admin_sso.as_ref(),
            &rates_before,
            shares_before,
        ))
        .after(view(
            &state,
            &after,
            admin_sso.as_ref(),
            &rates_after,
            shares,
        ))
        .write(&mut tx)
        .await?;
    tx.commit().await?;
    for c in &capped {
        crate::platform::logs::forget_org(&state, c.id).await?;
    }
    if after.timezone != before.timezone {
        state.caches.time_zone.invalidate_all();
    }
    Ok(Json(view(
        &state,
        &after,
        admin_sso.as_ref(),
        &rates_after,
        shares,
    )))
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
#[serde(deny_unknown_fields)]
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
        Some(t) => crate::platform::validate::validate_email("to", &t)?,
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
